# NASA Space Apps 2026 submission

Frozen submission repositories, archived after this manifest was added:

| Component | Submission repository | Implementation revision |
| --- | --- | --- |
| Earth Index backend and data pipeline | [CrisRPia/earth-index-space-apps-2026](https://github.com/CrisRPia/earth-index-space-apps-2026) | `ebc1ef343af4d556b6e51ac74cc625523aa672a9` |
| EarthConnect frontend | [CrisRPia/earthconnect-space-apps-2026](https://github.com/CrisRPia/earthconnect-space-apps-2026) | `70dfdc4a45fa9b52aceb0f4c68f122df6d3e2834` |

The submission copies preserve the source history and add this manifest. The
original working repositories remain active and may continue to change.

- [Live application](https://earth-index-crisrpia.fly.dev/)
- [Direct frontend link](https://earth-index-crisrpia.fly.dev/demo/frontend/)
- [API client guide](https://github.com/CrisRPia/earth-index-space-apps-2026/blob/main/docs/CLIENT_IMPL_GUIDE.md)
- [Data sources and methodology](https://github.com/CrisRPia/earth-index-space-apps-2026/blob/main/docs/DATA_SOURCES.md)

The live service is a deployment, not an immutable submission artifact. Its
combined image at submission is
`registry.fly.io/earth-index-crisrpia:earthconnect-70dfdc4`
(`sha256:613b78aea6ff8c75abce98a7eeb2c52abc98a53ab6417cdba9a28a18bb5d9b36`).
EarthConnect's deployment includes packaging adjustments for `/demo/frontend/`;
the hosted root redirects there. The backend dataset and credentials are not
included in Git; preparation instructions are in the backend repository.
