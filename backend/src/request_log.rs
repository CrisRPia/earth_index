use std::{
    fmt::{self, Write},
    future::Future,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};
use tonic::{Request, Response, Status};

const MAX_PAYLOAD_BYTES: usize = 8192;
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

// Stop formatting at the limit, rather than allocating a potentially 64 MiB
// request just to truncate it. Debug formatting escapes user-supplied newlines.
#[derive(Default)]
struct Preview {
    text: String,
    truncated: bool,
}

impl Write for Preview {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let remaining = MAX_PAYLOAD_BYTES - self.text.len();
        if text.len() <= remaining {
            self.text.push_str(text);
            return Ok(());
        }
        let mut end = remaining;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        self.text.push_str(&text[..end]);
        self.truncated = true;
        Err(fmt::Error)
    }
}

struct InFlight {
    id: u64,
    rpc: &'static str,
    start: Instant,
    finished: bool,
}

impl Drop for InFlight {
    fn drop(&mut self) {
        if !self.finished {
            tracing::warn!(
                request_id = self.id,
                rpc = self.rpc,
                elapsed_ms = self.start.elapsed().as_millis() as u64,
                "RPC handler dropped before completion"
            );
        }
    }
}

pub(crate) async fn run<T, R, F, Fut>(
    rpc: &'static str,
    request: Request<T>,
    handler: F,
) -> Result<Response<R>, Status>
where
    T: fmt::Debug,
    R: prost::Message,
    F: FnOnce(Request<T>) -> Fut,
    Fut: Future<Output = Result<Response<R>, Status>>,
{
    let mut flight = InFlight {
        id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        rpc,
        start: Instant::now(),
        finished: false,
    };
    let mut preview = Preview::default();
    let _ = write!(preview, "{:?}", request.get_ref());
    tracing::info!(request_id = flight.id, rpc,
        fly_request_id = ?request.metadata().get("fly-request-id").and_then(|v| v.to_str().ok()),
        accept_encoding = ?request.metadata().get("accept-encoding").and_then(|v| v.to_str().ok()),
        grpc_accept_encoding = ?request.metadata().get("grpc-accept-encoding").and_then(|v| v.to_str().ok()),
        grpc_timeout = ?request.metadata().get("grpc-timeout").and_then(|v| v.to_str().ok()),
        payload = %preview.text, payload_truncated = preview.truncated,
        "RPC received");
    let result = handler(request).await;
    let elapsed_ms = flight.start.elapsed().as_millis() as u64;
    match &result {
        Ok(response) => tracing::info!(
            request_id = flight.id,
            rpc,
            elapsed_ms,
            status = "Ok",
            response_protobuf_bytes = response.get_ref().encoded_len(),
            "RPC completed"
        ),
        Err(error) => tracing::warn!(request_id = flight.id, rpc, elapsed_ms,
            status = ?error.code(), error = %error.message(), "RPC failed"),
    }
    flight.finished = true;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_is_bounded_and_utf8_safe() {
        let mut preview = Preview::default();
        let _ = write!(preview, "{}", "é".repeat(MAX_PAYLOAD_BYTES));
        assert_eq!(preview.text.len(), MAX_PAYLOAD_BYTES);
        assert!(preview.truncated);
        let mut preview = Preview::default();
        preview.write_str("a").unwrap();
        let _ = write!(preview, "{}", "é".repeat(MAX_PAYLOAD_BYTES));
        assert_eq!(preview.text.len(), MAX_PAYLOAD_BYTES - 1);
        assert!(preview.truncated);
    }
}
