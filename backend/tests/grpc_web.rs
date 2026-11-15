//! Binary browser transport and CORS. No base64 or custom production codec.
mod common;
use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode},
};
use earth_index_api::{
    AppState,
    proto::{self, value::Kind},
    router, value,
};
use http_body_util::BodyExt;
use prost::Message;
use std::collections::HashMap;
use tower::ServiceExt;

fn app() -> Router {
    router(AppState {
        database: common::database(),
        statement_timeout_ms: 10000,
        max_response_bytes: 16 << 20,
        planetary: earth_index_api::planetary::Sampler::new().unwrap(),
    })
}
struct Reply {
    status: u32,
    body: Vec<u8>,
    trailers: String,
}
async fn rpc(app: &Router, method: &str, body: Vec<u8>) -> Reply {
    let mut frame = vec![0];
    frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
    frame.extend_from_slice(&body);
    rpc_frame(app, method, frame, None).await
}
async fn rpc_frame(app: &Router, method: &str, frame: Vec<u8>, timeout: Option<&str>) -> Reply {
    let mut builder = Request::builder();
    if let Some(timeout) = timeout {
        builder = builder.header("grpc-timeout", timeout);
    }
    let request = builder
        .method(Method::POST)
        .uri(format!("/earth_index.v1.EarthIndex/{method}"))
        .header("content-type", "application/grpc-web+proto")
        .header("x-grpc-web", "1")
        .header("origin", "http://localhost:5173")
        .body(Body::from(frame))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["access-control-allow-origin"], "*");
    assert_eq!(
        response.headers()["content-type"],
        "application/grpc-web+proto"
    );
    let mut status = response
        .headers()
        .get("grpc-status")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let mut trailers = String::new();
    for (key, val) in response.headers() {
        if matches!(key.as_str(), "sqlstate" | "error-code") {
            trailers.push_str(&format!("{key}:{}\r\n", val.to_str().unwrap()));
        }
    }
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let mut offset = 0;
    let mut data = Vec::new();
    while offset < body.len() {
        assert!(body.len() - offset >= 5);
        let flag = body[offset];
        let len = u32::from_be_bytes(body[offset + 1..offset + 5].try_into().unwrap()) as usize;
        offset += 5;
        assert!(body.len() - offset >= len);
        let frame = &body[offset..offset + len];
        offset += len;
        if flag & 0x80 != 0 {
            let text = std::str::from_utf8(frame).unwrap();
            trailers.push_str(text);
            for line in text.lines() {
                if let Some(value) = line.strip_prefix("grpc-status:") {
                    status = value.trim().parse().unwrap();
                }
            }
        } else {
            assert_eq!(
                flag, 0,
                "Browser response must be binary and uncompressed without grpc-accept-encoding"
            );
            data.extend_from_slice(frame);
        }
    }
    Reply {
        status,
        body: data,
        trailers,
    }
}

#[tokio::test]
#[ignore = "requires prepared DuckDB snapshot"]
async fn binary_browser_query_and_error_metadata() {
    let app = app();
    let reply = rpc(&app, "Metadata", proto::MetadataRequest {}.encode_to_vec()).await;
    assert_eq!(reply.status, 0);
    let metadata = proto::MetadataResponse::decode(reply.body.as_slice()).unwrap();
    let id = metadata
        .datasources
        .iter()
        .max_by_key(|d| d.coverage.len())
        .unwrap()
        .id
        .clone();
    let blob: Vec<u8> = (0..2_000_000).map(|i| (i % 256) as u8).collect();
    let request = proto::QueryRequest {
        datasource_id: id.clone(),
        query: "SELECT :i,:bytes,pack_u16(ARRAY[1,256,NULL])".into(),
        params: HashMap::from([
            (
                "i".into(),
                value::wrap(Kind::IntegerValue(9007199254740993)),
            ),
            ("bytes".into(), value::wrap(Kind::BytesValue(blob.clone()))),
        ]),
    };
    let reply = rpc(&app, "Query", request.encode_to_vec()).await;
    assert_eq!(reply.status, 0);
    let result = proto::QueryResponse::decode(reply.body.as_slice()).unwrap();
    assert_eq!(
        result.rows[0].values[0],
        value::wrap(Kind::IntegerValue(9007199254740993))
    );
    assert_eq!(
        result.rows[0].values[1],
        value::wrap(Kind::BytesValue(blob))
    );
    assert_eq!(
        result.rows[0].values[2],
        value::wrap(Kind::BytesValue(vec![1, 0, 0, 1, 255, 255]))
    );
    let request = proto::QueryRequest {
        datasource_id: id,
        query: "SELECT missing_column".into(),
        params: HashMap::new(),
    };
    let reply = rpc(&app, "Query", request.encode_to_vec()).await;
    assert_eq!(reply.status, 3);
    assert!(reply.trailers.contains("error-code:SQL_ERROR"));
    let reply = rpc(&app, "Query", vec![255]).await;
    assert_ne!(reply.status, 0);
}

#[tokio::test]
#[ignore = "requires prepared DuckDB snapshot"]
async fn preflight_health_and_schema_downloads() {
    let app = app();
    let request = Request::builder()
        .method(Method::OPTIONS)
        .uri("/earth_index.v1.EarthIndex/Query")
        .header("origin", "http://localhost:5173")
        .header("access-control-request-method", "POST")
        .header(
            "access-control-request-headers",
            "content-type,x-grpc-web,grpc-timeout",
        )
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert!(response.status().is_success());
    assert_eq!(response.headers()["access-control-allow-origin"], "*");
    let reply = rpc(&app, "Health", proto::HealthRequest {}.encode_to_vec()).await;
    assert_eq!(reply.status, 0);
    assert!(
        proto::HealthResponse::decode(reply.body.as_slice())
            .unwrap()
            .ready
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/proto/descriptor.bin")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert!(
        prost_types::FileDescriptorSet::decode(body)
            .unwrap()
            .file
            .iter()
            .any(|f| f
                .service
                .iter()
                .any(|s| s.name.as_deref() == Some("EarthIndex")))
    );
    let response = app
        .clone()
        .oneshot(Request::builder().uri("/docs").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
}

#[tokio::test]
#[ignore = "requires prepared DuckDB snapshot"]
async fn browser_deadline_and_input_size_limit() {
    let app = app();
    // Warm the database connection so this checks SQL cancellation itself.
    assert_eq!(
        rpc(&app, "Health", proto::HealthRequest {}.encode_to_vec())
            .await
            .status,
        0
    );
    let request = proto::QueryRequest {
        query: "SELECT sum(sin(i::DOUBLE)) FROM range(10000000000) t(i)".into(),
        params: HashMap::new(),
        datasource_id: "earth-ed5bc9904a5f".into(),
    }
    .encode_to_vec();
    let mut frame = vec![0];
    frame.extend_from_slice(&(request.len() as u32).to_be_bytes());
    frame.extend_from_slice(&request);
    let start = std::time::Instant::now();
    let reply = rpc_frame(&app, "Query", frame, Some("30m")).await;
    assert_eq!(reply.status, 4);
    assert!(start.elapsed() < std::time::Duration::from_millis(500));
    let mut frame = vec![0];
    frame.extend_from_slice(&((earth_index_api::MAX_REQUEST_BYTES + 1) as u32).to_be_bytes());
    let reply = rpc_frame(&app, "Query", frame, None).await;
    assert_eq!(reply.status, 11);
}
