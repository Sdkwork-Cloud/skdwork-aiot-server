# SDKWork AIoT Standards Alignment Roadmap

Status: active  
Owner: SDKWork maintainers  
Canonical source: [docs/adr/004-standards-alignment-roadmap.md](../../adr/004-standards-alignment-roadmap.md)

This TECH shard tracks the same phased alignment program as ADR 004. Read the ADR for the authoritative decision record, phase table, and verification commands.

## Current alignment summary

| Framework | Status |
| --- | --- |
| `sdkwork-specs` project dictionary + CI gates | Done |
| `sdkwork-web-framework` (app/backend `*-api`) | Done |
| `sdkwork-database` | Done |
| `sdkwork-utils` | Done |
| `sdkwork-drive` (PC firmware upload via Drive Uploader) | Done (Phase Y) |
| OTA Drive `MediaResource` URL resolution | Done (Phase Y+) |
| `sdkwork-discovery` | Deferred until RPC services exist |
| Pagination store alignment + CI governance (Phase Z) | Done |

## Intentional transport exception

Device gateway ingress (`sdkwork-aiot-cloud-gateway`) remains on the minimal transport stack documented in ADR 002. It is not an HTTP `*-api` surface and does not require `sdkwork-web-framework`.

## Verification

Run the commands listed in ADR 004, including:

- `pnpm check:drive-standard`
- `pnpm check:api-envelope`
- `pnpm check:pagination`
- `pnpm check:app-sdk-consumer-imports`
- `pnpm check`
- `cargo test -p sdkwork-aiot-architecture`
