# SDKWork AIoT PRD

Status: active
Owner: SDKWork maintainers
Application: sdkwork-aiot
Updated: 2026-07-06
Specs: REQUIREMENTS_SPEC.md, DOCUMENTATION_SPEC.md

## 1. Background And Problem

Enterprises need a protocol-neutral IoT control plane that connects edge devices (starting with Xiaozhi-compatible firmware), exposes tenant-scoped app/backend APIs, and integrates with SDKWork IAM, Drive, Agents, and Voice without duplicating platform identity or upload flows.

## 2. Target Users

| Persona | Need |
| --- | --- |
| Platform operator | Provision products, firmware, rollouts, credentials, and monitor fleet health |
| Tenant admin | Manage devices, twins, commands, and OTA within IAM-scoped organizations |
| Device firmware engineer | Connect via WebSocket/MQTT/HTTP with stable protocol adapters |
| Console end user (PC/H5) | Operate devices, agents, and voice dialogue through composed SDKs |

## 3. Goals And Non-Goals

### Goals

- Durable device/command/event/twin persistence on SQLite (standalone) and PostgreSQL (cloud HA)
- SdkWork v3 API envelopes, store-level offset pagination, and generated composed SDKs
- Production fail-closed gateway auth, credential verification, and intelligence kernel configuration
- OTA via Drive-backed `MediaResource` references (no duplicate upload endpoints)
- Transactional outbox for command dispatch and integration events

### Non-Goals (current release)

- Multi-region active-active gateway session replication without sticky routing
- Billing, metering, and license enforcement (planned platform integration)
- WeChat mini-program cloud agents/voice (device `speak` only)
- In-app IAM or parallel identity store

## 4. Scope

| Surface | In scope |
| --- | --- |
| `sdkwork-api-aiot-assembly` app-api surface | Tenant-scoped device/command/event/twin APIs |
| `sdkwork-api-aiot-assembly` backend-api surface | Catalog, credentials, firmware, rollouts |
| `sdkwork-aiot-device-edge-runtime` | Xiaozhi ingress, OTA, protocol ingest, outbox worker |
| PC/H5 consoles | IoT fleet, device ops, agents, voice (via sibling SDKs) |

## 5. User Scenarios

1. Operator registers a product and firmware artifact (Drive upload + backend registration), creates a rollout, and devices receive OTA offers on reconnect.
2. Tenant user lists devices with server pagination, sends a command, and polls command status via `devices.commands.retrieve`.
3. Xiaozhi device connects with per-device credential; the device edge runtime persists telemetry through protocol ingest.
4. Console user runs an agent session (Agents SDK) with device command fallback when cloud agents are unavailable.

## 6. Success Metrics

| Metric | Target |
| --- | --- |
| API contract gates | `pnpm check:api-envelope`, `check:pagination`, `api:check` pass |
| Persistence | Durable DB required in production; Postgres code path + optional smoke test |
| Security | No `DEV_MODE` in production; internal routes require token; device auth uses credential store; proxy context requires `x-sdkwork-proxy-auth` in production |
| Intelligence | Production requires `SDKWORK_AIOT_INTELLIGENCE_MODE=kernel` with claw-router/kernel URLs; speak commands use kernel TTS |
| Pagination | List APIs reject `page_size > 200` with HTTP 400 / code `40003` |
| Release | `pnpm release:preflight` green with SBOM and checksum evidence |

## 7. Phases

| Phase | Focus | Status |
| --- | --- | --- |
| P0 | Core persistence, APIs, gateway, standards alignment (ADR 004 A–Z) | Done |
| P1 | Production hardening (Postgres, fail-closed kernel, bounded twin/ID allocation, command retrieve) | Done |
| P2 | HA runbooks, outbox webhook defaults, session externalization | Planned |
| P3 | Commercial metering, SLA dashboards, multi-region | Planned |

## 8. Linked Requirements

- `specs/README.md` — component boundary and verification commands
- `docs/production-readiness.md` — deployment gates
- `docs/runbooks/production-release.md` — release procedure
- `docs/adr/004-standards-alignment-roadmap.md` — standards migration history

## 9. Open Questions

- Gateway session map externalization (Redis/etcd) for non-sticky load balancers
- Cursor-mode pagination for high-volume event feeds (offset mode today)
