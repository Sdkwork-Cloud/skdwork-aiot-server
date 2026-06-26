# Crates

Rust libraries for the SDKWork AIoT server workspace.

| Crate family | Responsibility |
| --- | --- |
| `sdkwork-aiot-contract` | Shared contracts and DTO boundaries |
| `sdkwork-iot-device-service` | Device domain services |
| `sdkwork-aiot-protocol` | Protocol plugin manifests and registry |
| `sdkwork-aiot-adapter-xiaozhi` | Xiaozhi compatibility plugin |
| `sdkwork-aiot-transport` | Device ingress transport (HTTP/WebSocket/MQTT/UDP) |
| `sdkwork-iot-platform-service` | Shared HTTP API handlers and route contracts |
| `sdkwork-routes-iot-app-api` | App-api Axum router with `sdkwork-web-framework` |
| `sdkwork-routes-iot-backend-api` | Backend-api Axum router with `sdkwork-web-framework` |
| `sdkwork-aiot-service-host` | In-process runtime composition |
| `sdkwork-aiot-storage*` | Persistence ports and SQLx repositories |
| `sdkwork-aiot-database-host` | sdkwork-database bootstrap and lifecycle host |
| `sdkwork-aiot-app-context` | Maps `WebRequestContext` into AIoT request context |
| `sdkwork-aiot-security` | Device and platform security helpers |
| `sdkwork-aiot-observability` | Logs, metrics, and trace fields |
| `sdkwork-aiot-architecture` | Architecture guard tests |

Runnable binaries live under `services/`.
