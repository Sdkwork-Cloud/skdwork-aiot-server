# SDKWork AIoT Production Readiness

This document tracks production readiness for the SDKWork AIoT server. Items marked **Done** have automated verification in the workspace test suite.

## Done

| Area | Status | Evidence |
| --- | --- | --- |
| Device/command/event/twin persistence | Done | `SqliteSqlxDeviceRepository` + admin/app-api wiring |
| Device session persistence (HTTP API) | Done | app-api/admin-api wire `with_device_session_repository` to SQL device repository |
| Device credential persistence + hash verification | Done | `SqliteSqlxCredentialRepository`, gateway `xiaozhi_device_token_valid` |
| Catalog admin-entity list pagination | Done | SQL `list_*_page`; storage failures return `ProblemDetail` |
| Catalog standard seed + CRUD | Done | Idempotent `ensure_standard_catalog_seeded` on list/get |
| Catalog firmware artifact/rollout persistence | Done | `AiotFirmwareRepositoryHandle`, admin/app-api wiring |
| Structured trace logging | Done | `SDKWORK_AIOT_STRUCTURED_TRACE=1`, `sdkwork-aiot-observability` |
| OTLP HTTP trace export | Done | `SDKWORK_AIOT_OTLP_ENDPOINT` |
| Protocol ingest persistence | Done | Gateway opens shared `open_aiot_device_database_from_env()`; no silent in-memory fallback |
| Transactional outbox publish worker | Done | `SqliteOutboxEventRepository`, gateway worker + `/readyz` lag probe |
| Postgres protocol/outbox ID allocation | Done | Unified SQL `MAX(id)+1` allocation for SQLite and Postgres |
| Release package checksum sync | Done | `pnpm release:package`, CI `release-smoke` |
| Release SBOM evidence | Done | `artifacts/release/sbom/*.sbom.json`, `pnpm sbom:check` |
| CDN publish gate | Done | `pnpm release:publish` |
| Production release runbook | Done | `docs/runbooks/production-release.md` |
| App/backend HTTP (Axum + web framework) | Done | `sdkwork-routes-iot-app-api`, `sdkwork-routes-iot-backend-api` |
| Gateway device ingress HTTP | Done | `sdkwork-aiot-transport` |
| CORS + security headers + rate limiting | Done | `sdkwork-iot-platform-service` |
| Production device auth fail-closed | Done | Gateway dev/prod token rules |
| Internal route token auth | Done | `internal_route_authorized` |
| MQTT/UDP multi-session bridge | Done | Per-device session map in gateway |
| Route manifest + OpenAPI alignment | Done | `sdks/_route-manifests/*`, architecture tests |
| Postgres device persistence (cloud HA) | Done | `BlockingDevicePool` + dialect-aware repos |
| Drive Uploader (PC firmware upload) | Done | `@sdkwork/drive-app-sdk` |
| Store pagination + API governance CI | Done | `pnpm check:api-envelope`, `check:pagination`, `check:app-sdk-consumer-imports` |
| SDK manifest contract | Done | `sdk-manifest.json` per SDK family |
| Gateway WS command delivery | Done | Runs on shared device DB (SQLite file, Postgres, or dev memory); tenant-aware association lookup |
| Interactive console list pagination | Done | PC/H5/mini-program use server pagination; SDK `pageSize` param |
| PC IoT fleet alerts | Done | Derived from live device health/offline state (no demo seed data) |
| Mini-program SDK envelope + command polling | Done | `aiot-app-sdk-client.js` unwraps `{ items, pageInfo }`, polls command results |

## Shared SQLite Without Persistent Path

When `SDKWORK_AIOT_DEVICE_DB_PATH` is unset, services use the shared in-process URI `file:sdkwork-aiot-device-db?mode=memory&cache=shared`. Production deployments must set `SDKWORK_AIOT_DEVICE_DB_PATH` or cloud Postgres env keys.

## Resolved By ADR / Deployment Guide

| Area | Resolution | Reference |
| --- | --- | --- |
| `sdkwork-appbase` IAM | Proxy-terminated auth | `docs/adr/001-iam-via-appbase-proxy.md` |
| Axum/Tokio HTTP stack | Gateway minimal transport; APIs use `sdkwork-web-framework` | `docs/adr/002-http-transport-evolution.md` |
| Horizontal clustering | Sticky sessions + `SDKWORK_AIOT_GATEWAY_NODE_ID` | `docs/adr/003-gateway-horizontal-scaling.md` |

## Commercial Deployment Gates

```powershell
pnpm check
pnpm verify
pnpm check:production-topology
pnpm check:deploy-manifest
pnpm release:preflight
pnpm release:validate
pnpm release:publish
node ../sdkwork-specs/tools/check-pagination.mjs --workspace .
node ../sdkwork-specs/tools/check-api-response-envelope.mjs --workspace .
node ../sdkwork-specs/tools/check-app-sdk-consumer-imports.mjs --workspace .
```

## Production Environment Checklist

```powershell
$env:SDKWORK_AIOT_DEVICE_DB_PATH='D:\data\aiot-device.db'
$env:SDKWORK_AIOT_INTERNAL_TOKEN='<random-internal-token>'
$env:SDKWORK_AIOT_CREDENTIAL_PEPPER='<random-pepper-at-least-32-chars>'
$env:SDKWORK_AIOT_CORS_ALLOWED_ORIGINS='https://console.example.com'
# Do NOT set SDKWORK_AIOT_DEV_MODE in production
```

For cloud Postgres HA, set `SDKWORK_AIOT_DEVICE_DATABASE_URL`, `SDKWORK_AIOT_DEVICE_DATABASE_ENGINE=postgres`, and related `SDKWORK_AIOT_DEVICE_DATABASE_*` keys (see `configs/topology/cloud.split-services.production.env`).
