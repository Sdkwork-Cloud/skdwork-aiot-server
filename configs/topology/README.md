# AIoT topology profiles

Machine contract: `specs/topology.spec.json` (`schemaVersion: 2`, archetype `application-rest-edge-device`).

Platform standard: `../../sdkwork-specs/APP_RUNTIME_TOPOLOGY_ADOPTION.md`

## Active profiles

| Profile id | Command |
| --- | --- |
| `standalone.split-services.development` | `pnpm dev` |
| `cloud.split-services.development` | `pnpm dev:server:sqlite:split-services:cloud` |
| `standalone.split-services.production` | on-prem release wiring |
| `cloud.split-services.production` | SaaS release wiring |

Loader: `scripts/lib/aiot-topology.mjs` → `@sdkwork/app-topology`.

Edge ingress (`edge.device-ingress`) is never proxied by `sdkwork-api-cloud-gateway`.
