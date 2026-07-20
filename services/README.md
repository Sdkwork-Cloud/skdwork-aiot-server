# Services

Runnable SDKWork AIoT development support processes.

| Service | Surface | Notes |
| --- | --- | --- |
| `sdkwork-aiot-xiaozhi-simulator-ui` | Dev simulator | Local Xiaozhi compatibility UI |

Application HTTP is hosted only by `sdkwork-api-aiot-standalone-gateway`, which
consumes `sdkwork-api-aiot-assembly`. Start the standalone development topology
with `pnpm dev`.

Production runtime crates, including the responsibility-specific device edge
runtime, live under `crates/`.
