# sdkwork-routes-iot-backend-api

Domain: iot
Capability: backend-api-router
Package type: rust-crate
Status: standard

This README is the SDKWork module entrypoint for `sdkwork-routes-iot-backend-api`. The machine-readable component contract is `../../specs/component.spec.json`; canonical standards are under `../../../sdkwork-specs/`.

## Public API

- `build_sdkwork_iot_backend_api_router`
- `wrap_router_with_web_framework`
- `iot_public_path_prefixes`

## Required SDK Surface

- `@sdkwork/aiot-backend-sdk` via generated backend-api SDK family.

## Configuration

Router wiring consumes the shared `AiotApiServer` runtime surface and IAM web-framework resolver inputs declared in `specs/component.spec.json`. Route manifests are generated from OpenAPI under `apis/backend-api/iot/`.

## SaaS/Private/Local Behavior

This router mounts `/backend/v3/api/iot` handlers through `sdkwork-web-framework`. Deployment profile differences are handled by runtime topology and service binaries, not by duplicating route tables in this crate.

## Security

Protected backend-api routes require IAM-resolved `WebRequestContext`. Do not bypass the web-framework layer or inject manual bearer headers in this crate.

## Extension Points

Add or adjust routes through OpenAPI authority updates, route manifest regeneration, and platform-service handlers. Do not hand-edit generated route include files.

## Verification

- `cargo test -p sdkwork-routes-iot-backend-api`
- `pnpm api:check`

## Owner And Status

Owner and lifecycle status are tracked in `specs/component.spec.json`. Update that contract before changing public integration behavior.
