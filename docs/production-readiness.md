# SDKWork AIoT Production Readiness

Current production readiness for the SDKWork AIoT server. Items marked **Done** have automated verification in the workspace test suite.

## Done

| Area | Status | Evidence |
| --- | --- | --- |
| Device/command/event/twin persistence | Done | `SqliteSqlxDeviceRepository` + admin/app-api wiring |
| Device session persistence (HTTP API) | Done | app-api/admin-api wire `with_device_session_repository` to SQL device repository |
| Device credential persistence + hash verification | Done | `SqliteSqlxCredentialRepository`; gateway opens credentials via `open_aiot_device_database_from_env()` |
| Tenant-scoped WS command delivery association | Done | `register_active_ws_session` stores credential-resolved tenant; command worker uses scoped association |
| Active credential uniqueness per tenant/device | Done | Migration `0004` + baseline DDL `uk_iot_device_credential_tenant_device_active` |
| Catalog admin-entity list pagination | Done | SQL `list_*_page`; dev/test memory fallback via bounded `paginate_vec` |
| Catalog standard seed + CRUD | Done | Idempotent `ensure_standard_catalog_seeded` on list/get |
| Catalog firmware artifact/rollout persistence | Done | `AiotFirmwareRepositoryHandle`, admin/app-api wiring |
| Command retrieve API (app + backend) | Done | `devices.commands.retrieve`; `pollCommandResult` uses SDK retrieve |
| Command idempotency replay on concurrent create | Done | Unique constraint + post-failure re-fetch in `create_command` |
| Structured trace logging | Done | `SDKWORK_AIOT_STRUCTURED_TRACE=1`, `sdkwork-aiot-observability` |
| OTLP HTTP trace export | Done | `SDKWORK_AIOT_OTLP_ENDPOINT` |
| Protocol ingest persistence | Done | Gateway opens shared `open_aiot_device_database_from_env()` |
| Transactional outbox publish worker | Done | `SqliteOutboxEventRepository`, gateway worker + `/readyz` lag probe; failure recording in DB transaction |
| Monotonic row ID allocation | Done | `iot_row_id_allocator` for devices, admin entities, commands, deliveries, credentials, sessions, events, twin properties, protocol ingest/dead-letter/outbox |
| Command idempotency (DB-only) | Done | `uk_iot_command_tenant_idempotency_key`; no process-local cache |
| Credential OpenAPI typed schema | Done | `AiotCredentialResponse` with optional `issuedSecret` on create |
| Production intelligence fail-closed | Done | `SDKWORK_AIOT_INTELLIGENCE_MODE=kernel` required in production; kernel/claw-router URLs validated |
| Gateway speak command kernel TTS | Done | `KernelSpeechPipeline::run_speak` + command delivery worker uses speech pipeline |
| Production proxy-header hardening | Done | Production requires `x-sdkwork-proxy-auth` matching `SDKWORK_AIOT_INTERNAL_TOKEN` when trusting proxy headers |
| List API `page_size > 200` rejection | Done | `validated_offset_list_params` → HTTP 400 / code `40003`; `http_api_standard` test |
| Release package checksum sync | Done | `pnpm release:package`, CI `release-smoke` |
| Release SBOM evidence | Done | `artifacts/release/sbom/*.sbom.json`, `pnpm sbom:check` |
| CDN publish gate | Done | `pnpm release:publish` |
| Production release runbook | Done | `docs/runbooks/production-release.md` |
| App/backend HTTP (Axum + web framework) | Done | `sdkwork-routes-iot-app-api`, `sdkwork-routes-iot-backend-api` |
| Gateway device ingress HTTP | Done | `sdkwork-aiot-transport` |
| CORS + security headers + rate limiting | Done | Success and error API responses apply security headers; auth rate limits |
| Production device auth fail-closed | Done | Gateway dev/prod token rules; credential repo required in production |
| Production durable DB gate (gateway + APIs) | Done | `device_database_config_is_durable_from_env()` in gateway and platform `assert_production_environment_safety()` |
| Internal route token auth | Done | `internal_route_authorized`; tests without `DEV_MODE` |
| MQTT/UDP multi-session bridge | Done | Per-device session map in gateway |
| Route manifest + OpenAPI alignment | Done | List APIs expose offset `page`/`page_size`; `PageInfo.mode` required |
| Postgres device persistence (cloud HA) | Done | `BlockingDevicePool` + dialect-aware repos; `row_decode` timestamp helpers; optional `postgres_device_database_round_trip` CRUD test |
| Drive Uploader (PC firmware upload) | Done | `@sdkwork/drive-app-sdk` |
| Store pagination + API governance CI | Done | `pnpm check:api-envelope`, `check:pagination`, `check:app-sdk-consumer-imports` |
| SDK manifest contract | Done | `sdk-manifest.json` per SDK family |
| Gateway WS command delivery | Done | Runs on shared device DB (SQLite file, Postgres, or dev memory); tenant-aware association from WS registration |
| Interactive console list pagination | Done | PC/H5 use server pagination with load-more |
| PC IoT fleet alerts | Done | Derived from live device health/offline state |
| Agents + Voice dialogue (PC/H5) | Done | `@sdkwork/agents-app-sdk` + `@sdkwork/voice-app-sdk` when configured |
| Backend OpenAPI SdkWork envelope alignment | Done | `SdkWorkApiResponse` / `ProblemDetail`; SDK regenerated |
| ProblemDetail numeric codes + traceId | Done | `SdkWorkProblemDetail` via `sdkwork-utils-rust`; `http_api_standard` tests |
| Firmware OTA scoped queries | Done | Tenant-scoped deployment lookup with `MAX_OTA_DEPLOYMENT_SCAN` limit |
| Twin property OOM guard | Done | `MAX_DEVICE_TWIN_PROPERTIES` SQL `LIMIT` on snapshot reads |
| WeChat mini-program voice (speak command) | Done | Fixed SdkWork list envelope + device picker mapping |
| Product requirements (PRD) | Done | `docs/product/prd/PRD.md` |

## Durable Persistence

Production deployments **must** set one of:

- `SDKWORK_AIOT_DEVICE_DB_PATH` (SQLite file), or
- `SDKWORK_AIOT_DEVICE_DATABASE_URL` + `SDKWORK_AIOT_DEVICE_DATABASE_ENGINE=postgres` (cloud Postgres HA)

When neither is configured, dev processes use shared in-memory SQLite (`file:sdkwork-aiot-device-db?mode=memory&cache=shared`). Setting `SDKWORK_AIOT_ENVIRONMENT=production` without durable persistence causes startup to fail fast on gateway and HTTP APIs.

## Required Sibling Services (commercial topology)

| Service | Purpose |
| --- | --- |
| `sdkwork-appbase` / IAM proxy | Dual-token auth termination (`SDKWORK_AIOT_TRUST_PROXY_HEADERS=1` only behind proxy; production requires `x-sdkwork-proxy-auth`) |
| `@sdkwork/drive-app-sdk` | Firmware and media uploads |
| `@sdkwork/agents-app-sdk` | Cloud agent dialogue (optional; device fallback remains) |
| `@sdkwork/voice-app-sdk` | Cloud STT/TTS (optional; browser/device fallbacks in dev) |
| `sdkwork-kernel` + `sdkwork-claw-router` | **Required** in production (`SDKWORK_AIOT_INTELLIGENCE_MODE=kernel`) |

## Deployment Architecture Notes

| Area | Resolution | Reference |
| --- | --- | --- |
| `sdkwork-appbase` IAM | Proxy-terminated auth with internal proxy token | `docs/adr/001-iam-via-appbase-proxy.md` |
| Axum/Tokio HTTP stack | Gateway minimal transport; APIs use `sdkwork-web-framework` | `docs/adr/002-http-transport-evolution.md` |
| Horizontal clustering | Sticky sessions + `SDKWORK_AIOT_DEVICE_EDGE_NODE_ID` | `docs/adr/003-device-edge-horizontal-scaling.md` |

## Planned (post-launch)

| Area | Target |
| --- | --- |
| Gateway session externalization (Redis) | Non-sticky load balancer support |
| Cursor pagination for high-volume event feeds | Large-tenant performance |
| Postgres CI integration test job (automated on every PR) | Cloud HA regression gate in CI |
| Billing / metering / tenant quotas | Commercial SaaS monetization |
| Multi-region active-active | Enterprise SLA |

## Commercial Deployment Gates

```powershell
pnpm check
pnpm verify
pnpm check:production-topology
pnpm check:deploy-manifest
pnpm release:preflight
pnpm release:validate
node ../sdkwork-specs/tools/check-pagination.mjs --workspace .
node ../sdkwork-specs/tools/check-api-response-envelope.mjs --workspace .
node ../sdkwork-specs/tools/check-app-sdk-consumer-imports.mjs --workspace .
cargo test -p sdkwork-iot-platform-service --test http_api_standard
cargo test -p sdkwork-aiot-device-edge-runtime --test device_edge_runtime_standard
```

Optional Postgres smoke (requires running database):

```powershell
$env:SDKWORK_AIOT_POSTGRES_TEST_URL='postgres://user:pass@localhost:5432/aiot_test'
cargo test -p sdkwork-aiot-storage-sqlx postgres_device_database_round_trip -- --ignored
```

## Production Environment Checklist

```powershell
$env:SDKWORK_AIOT_ENVIRONMENT='production'
$env:SDKWORK_AIOT_DEVICE_DB_PATH='D:\data\aiot-device.db'   # or Postgres env keys
$env:SDKWORK_AIOT_INTERNAL_TOKEN='<random-internal-token-at-least-32-chars>'
$env:SDKWORK_AIOT_CREDENTIAL_PEPPER='<random-pepper-at-least-32-chars>'
$env:SDKWORK_AIOT_CORS_ALLOWED_ORIGINS='https://console.example.com'
$env:SDKWORK_AIOT_INTELLIGENCE_MODE='kernel'
$env:SDKWORK_AIOT_INTELLIGENCE_KERNEL_HTTP_URL='https://kernel.example.com'
$env:SDKWORK_CLAW_ROUTER_APPLICATION_OPEN_HTTP_URL='https://claw-router.example.com'
$env:SDKWORK_CLAW_ROUTER_API_KEY='<random-claw-router-key-at-least-32-chars>'
$env:SDKWORK_AIOT_TRUST_PROXY_HEADERS='1'   # appbase must send x-sdkwork-proxy-auth
# Do NOT set SDKWORK_AIOT_DEV_MODE in production
```

For cloud Postgres HA, set `SDKWORK_AIOT_DEVICE_DATABASE_URL`, `SDKWORK_AIOT_DEVICE_DATABASE_ENGINE=postgres`, and related `SDKWORK_AIOT_DEVICE_DATABASE_*` keys (see `etc/topology/cloud.production.env`).
