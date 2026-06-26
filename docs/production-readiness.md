# SDKWork AIoT Production Readiness

This document tracks production readiness for the SDKWork AIoT server after the audit/remediation loop. Items marked **Done** have automated verification in the workspace test suite.

## Done

| Area | Status | Evidence |
| --- | --- | --- |
| Device/command/event/twin persistence | Done | `SqliteSqlxDeviceRepository` + admin/app-api wiring |
| Device credential persistence + hash verification | Done | `SqliteSqlxCredentialRepository`, gateway `xiaozhi_device_token_valid` |
| Catalog product + firmware artifact/rollout persistence | Done | `SqlitePersistedEntityRepository`, `AiotCatalogRepositoryHandle`, `AiotFirmwareRepositoryHandle`, admin/app-api wiring |
| Catalog hardware/protocol/capability profile persistence | Done | `iot_admin_entity` entity kinds + seed fallbacks on GET |
| Structured trace logging | Done | `SDKWORK_AIOT_STRUCTURED_TRACE=1`, `sdkwork-aiot-observability`, gateway + HTTP API hooks |
| OTLP HTTP trace export | Done | `SDKWORK_AIOT_OTLP_ENDPOINT`, OTLP/HTTP JSON in `sdkwork-aiot-observability` |
| Protocol ingest persistence | Done | Gateway `protocol_ingest_from_env()` + `SqlxProtocolIngestUnitOfWork` |
| Transactional outbox publish worker | Done | `SqliteOutboxEventRepository`, `AiotOutboxDispatcher`, gateway worker + `/readyz` lag probe |
| Release package checksum sync | Done | `pnpm release:package`, `artifacts/release/release-packages.manifest.json`, CI `release-smoke` job |
| Release SBOM evidence | Done | `artifacts/release/sbom/*.sbom.json`, `pnpm sbom:check`, generated during `release:package` |
| CDN publish gate | Done | `pnpm release:publish` validates archives, checksums, and SBOM before operator upload |
| Production release runbook | Done | `docs/runbooks/production-release.md` |
| Rust fmt workspace boundary | Done | `cargo fmt -- --check` scopes to AIoT members only (not sibling path deps) |
| Deploy manifest topology alignment | Done | `pnpm check:deploy-manifest` validates `deployments/deploy.yaml` against production topology profiles |
| Documentation registry | Done | `docs/INDEX.yaml` registers canon, runbooks, and operational docs |
| Repository docs standard | Done | `pnpm check:docs-standard` (`DOCUMENTATION_SPEC.md`) |
| SBOM checksum parity | Done | `pnpm sbom:check --strict` matches app manifest checksums |
| Commercial readiness gate | Done | `pnpm release:preflight` runs deploy + release + optional publish checks |
| Docs index registry validation | Done | `pnpm check:docs-index` resolves `docs/INDEX.yaml` paths |
| App/backend HTTP (Axum + web framework) | Done | `sdkwork-routes-iot-app-api`, `sdkwork-routes-iot-backend-api`, `resolve_api_request_from_web_context` |
| Gateway device ingress HTTP | Done | `sdkwork-aiot-transport` minimal stack per ADR 002 |
| CORS + security headers + rate limiting | Done | `sdkwork-iot-platform-service` |
| Production device auth fail-closed | Done | Gateway dev/prod token rules |
| Internal route token auth | Done | `internal_route_authorized` |
| MQTT/UDP multi-session bridge | Done | Per-device session map in gateway; `/readyz` includes MQTT bridge probe when `SDKWORK_AIOT_GATEWAY_MQTT_BRIDGE_ENABLE=1` |
| Route manifest + OpenAPI alignment | Done | `sdks/_route-manifests/*`, architecture tests |
| Workspace verification | Done | `pnpm check`, `pnpm verify`, targeted `cargo test` crates |
| Production topology guardrails | Done | `pnpm check:production-topology`, production env profiles |
| Postgres device persistence (cloud HA) | Done | `BlockingDevicePool` + dialect-aware device/credential/outbox/admin-entity repos; `SDKWORK_AIOT_DEVICE_DATABASE_*` in cloud production topology |

## Shared SQLite Without Persistent Path

When `SDKWORK_AIOT_DEVICE_DB_PATH` is unset, admin/app-api services use the shared in-process URI `file:sdkwork-aiot-device-db?mode=memory&cache=shared` so device, credential, catalog product, and firmware artifact repositories observe the same schema. Production deployments should still set `SDKWORK_AIOT_DEVICE_DB_PATH` to a durable file path.

## Resolved By ADR / Deployment Guide

These items are closed for this repository scope with explicit architecture records:

| Area | Resolution | Reference |
| --- | --- | --- |
| `sdkwork-appbase` IAM | Proxy-terminated auth; no local IAM tables | `docs/adr/001-iam-via-appbase-proxy.md`, `docs/deployment/iam-integration.md` |
| Axum/Tokio HTTP stack | Gateway retains minimal transport; app/backend APIs use `sdkwork-web-framework` per ADR 004 | `docs/adr/002-http-transport-evolution.md`, `docs/adr/004-standards-alignment-roadmap.md` |
| Horizontal clustering | Sticky sessions + `SDKWORK_AIOT_GATEWAY_NODE_ID` | `docs/adr/003-gateway-horizontal-scaling.md` |

## Commercial Deployment Gates

Run before promoting a release candidate:

```powershell
pnpm check
pnpm verify
pnpm check:production-topology
pnpm check:deploy-manifest
pnpm release:preflight
pnpm release:validate
pnpm release:publish
```

After `pnpm release:build` and `pnpm release:package`, server install packages in `sdkwork.app.config.json` are enabled with SHA-256 checksums. Packaged archives land under `artifacts/release/linux/x64/server.tar.gz` and `artifacts/release/windows/x64/server.zip`, matching the CDN URL suffixes in the app manifest. CycloneDX SBOM evidence is written to `artifacts/release/sbom/`. Run `pnpm release:publish` to verify the release candidate, then upload archives and SBOM files to the CDN URLs before customer download.

## Production Environment Checklist

```powershell
$env:SDKWORK_AIOT_DEVICE_DB_PATH='D:\data\aiot-device.db'
$env:SDKWORK_AIOT_INTERNAL_TOKEN='<random-internal-token>'
# Outbox dispatch runs on edge gateway by default when the device DB path is set.
# Optional webhook sink (standalone http:// only):
# $env:SDKWORK_AIOT_OUTBOX_WEBHOOK_URL='http://127.0.0.1:8090/iot/outbox'
# Optional explicit dispatcher toggle (admin/app-api default off unless set to 1):
# $env:SDKWORK_AIOT_OUTBOX_DISPATCHER_ENABLED='1'
# $env:SDKWORK_AIOT_OUTBOX_LAG_READY_THRESHOLD='10000'
# Optional legacy static token fallback when credential rows are not used:
# $env:SDKWORK_AIOT_XIAOZHI_DEVICE_TOKEN='<legacy-device-token>'
$env:SDKWORK_AIOT_CORS_ALLOWED_ORIGINS='https://console.example.com'
# Optional JSON trace lines on stderr for log collectors:
# $env:SDKWORK_AIOT_STRUCTURED_TRACE='1'
# Optional OTLP/HTTP JSON export (OpenTelemetry collector / Jaeger OTLP receiver):
# $env:SDKWORK_AIOT_OTLP_ENDPOINT='http://127.0.0.1:4318/v1/traces'
# $env:SDKWORK_AIOT_OTLP_SERVICE_NAME='sdkwork-aiot-cloud-gateway'
# Optional gateway replica identity for metrics/traces:
# $env:SDKWORK_AIOT_GATEWAY_NODE_ID='gateway-a'
# Do NOT set SDKWORK_AIOT_DEV_MODE in production
```

Gateway device access in production:

1. Create device via admin API.
2. Create device credential via admin API; store returned `issuedSecret` securely on the device.
3. Device connects with `Device-Id` + `Authorization: Bearer <issuedSecret>`.

When `SDKWORK_AIOT_DEVICE_DB_PATH` is configured on gateway, admin-api, and app-api, credential verification uses the shared SQLite database. Admin-api also persists custom products and firmware artifacts/rollouts in the same database via migration `0002` (`iot_admin_entity`).

For cloud-hosted Postgres HA, set `SDKWORK_AIOT_DEVICE_DATABASE_URL`, `SDKWORK_AIOT_DEVICE_DATABASE_ENGINE=postgres`, and related `SDKWORK_AIOT_DEVICE_DATABASE_*` keys (see `configs/topology/cloud.split-services.production.env`). Device, credential, outbox, and admin-entity repositories share one `BlockingDevicePool` opened from those env keys.
