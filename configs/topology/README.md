# AIoT topology profiles

Machine contract: `specs/topology.spec.json` (`schemaVersion: 2`, archetype `application-rest-edge-device`).

Platform standard: `../../sdkwork-specs/APP_RUNTIME_TOPOLOGY_ADOPTION.md`

## Active profiles

| Profile id | Command |
| --- | --- |
| `self-hosted.split-services.development` | `pnpm dev` |
| `cloud-hosted.split-services.development` | `pnpm dev:server:sqlite:split-services:cloud` |
| `self-hosted.split-services.production` | on-prem release wiring |
| `cloud-hosted.split-services.production` | SaaS release wiring |

Loader: `scripts/lib/aiot-topology.mjs` → `@sdkwork/app-topology`.

Edge ingress (`edge.device-ingress`) is never proxied by `sdkwork-api-cloud-gateway`.
