# Services

Runnable SDKWork AIoT server processes.

| Service | Surface | Notes |
| --- | --- | --- |
| `sdkwork-aiot-cloud-gateway` | Edge device ingress | WebSocket/MQTT/UDP device transport (ADR 002) |
| `sdkwork-aiot-app-api` | Application app-api | Axum + `sdkwork-web-framework` |
| `sdkwork-aiot-admin-api` | Application backend-api | Axum + `sdkwork-web-framework` |
| `sdkwork-aiot-xiaozhi-simulator-ui` | Dev simulator | Local Xiaozhi compatibility UI |

Start the default split-service development stack with `pnpm dev`.
