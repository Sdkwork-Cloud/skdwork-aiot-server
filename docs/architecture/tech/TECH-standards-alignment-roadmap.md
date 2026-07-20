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
| `sdkwork-drive` (PC firmware upload + OTA `MediaResource`) | Done |
| Store pagination + governance CI gates | Done |
| Gateway WS command delivery (`audio.playback/speak`) | Done |
| `sdkwork-discovery` | Deferred until RPC services exist |

## Intentional transport exception

Device protocol ingress (`sdkwork-aiot-device-edge-runtime`) remains on the minimal transport stack documented in ADR 002. It is not an HTTP `*-api` surface and does not require `sdkwork-web-framework`.

## Verification

Run the commands listed in ADR 004, including:

- `pnpm check:drive-standard`
- `pnpm check:api-envelope`
- `pnpm check:pagination`
- `pnpm check:app-sdk-consumer-imports`
- `pnpm check`
- `cargo test -p sdkwork-aiot-architecture`
