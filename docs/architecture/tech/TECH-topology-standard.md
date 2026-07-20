# SDKWork AIoT Topology

Archetype: `application-rest-edge-device` (`specs/topology.spec.json`, `schemaVersion: 5`).

Platform standard: `../sdkwork-specs/APP_RUNTIME_TOPOLOGY_SPEC.md`

## Development Profiles

`pnpm dev` delegates to `pnpm dev:standalone` and starts
`standalone.development`. `pnpm dev:cloud` starts local clients only and
uses the deployed URLs declared by `cloud.development`.

## Surfaces

| Surface id | Plane | Runtime owner |
| --- | --- | --- |
| `application.public-ingress` | application | `sdkwork-api-aiot-standalone-gateway` in standalone; platform-owned host in cloud |
| `application.app-http` | application | `sdkwork-api-aiot-assembly` app-api routes |
| `application.admin-http` | application | `sdkwork-api-aiot-assembly` backend-api routes |
| `edge.device-ingress` | edge | `sdkwork-aiot-device-edge-runtime` |
| `platform.api-gateway` | platform | Deployed platform API URL; process ownership is outside this repository |

The API assembly is host-neutral. Device edge ingress remains separate from
application HTTP API hosting and does not expose app-api, backend-api, or
open-api routes.

## Lifecycle

`pnpm exec sdkwork-app` loads `specs/topology.spec.json` and
`etc/topology/*.env` through `@sdkwork/app-topology`. There is no
application-local process orchestrator.

PC renderer client keys:
`apps/sdkwork-aiot-pc/packages/sdkwork-aiot-pc-core/src/sdk/topologyEnvKeys.ts`.

Validate with:

```bash
pnpm topology:validate
pnpm check:cloud-gateway-boundary
pnpm check:single-http-ingress
```
