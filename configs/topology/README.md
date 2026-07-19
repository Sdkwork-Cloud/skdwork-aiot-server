# AIoT topology profiles

Machine contract: `specs/topology.spec.json` (`schemaVersion: 4`, archetype `application-rest-edge-device`).

Platform standard: `../../sdkwork-specs/APP_RUNTIME_TOPOLOGY_ADOPTION.md`

## Active profiles

| Profile id | Command |
| --- | --- |
| `standalone.development` | `pnpm dev` |
| `cloud.development` | `pnpm dev:server:sqlite:cloud` |
| `standalone.production` | on-prem release wiring |
| `cloud.production` | SaaS release wiring |

Loader: `scripts/lib/aiot-topology.mjs` → `@sdkwork/app-topology`.

Edge ingress (`edge.device-ingress`) is never proxied by `sdkwork-api-cloud-gateway`.
