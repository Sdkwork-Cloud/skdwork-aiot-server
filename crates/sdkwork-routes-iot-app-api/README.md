# sdkwork-routes-iot-app-api

Domain: iot
Capability: app-api-router
Package type: rust-crate
Status: standard

This README is the SDKWork module entrypoint for `sdkwork-routes-iot-app-api`. The machine-readable component contract is `../../specs/component.spec.json`; canonical standards are under `../../../sdkwork-specs/`.

## Public API

- `build_sdkwork_iot_app_api_router`
- `wrap_router_with_web_framework`
- `iot_public_path_prefixes`

## Required SDK Surface

- `@sdkwork/aiot-app-sdk` via generated app-api SDK family.

## Configuration

Router wiring consumes the shared `AiotApiServer` runtime surface and IAM web-framework resolver inputs declared in `specs/component.spec.json`. Route manifests are generated from OpenAPI under `apis/app-api/iot/`.

## SaaS/Private/Local Behavior

This router mounts `/app/v3/api/iot` handlers through `sdkwork-web-framework`. Deployment profile differences are handled by runtime topology and service binaries, not by duplicating route tables in this crate.

## Security

Protected app-api routes require IAM-resolved `WebRequestContext`. Do not bypass the web-framework layer or inject manual bearer headers in this crate.

## Extension Points

Add or adjust routes through OpenAPI authority updates, route manifest regeneration, and platform-service handlers. Do not hand-edit generated route include files.

## Verification

- `cargo test -p sdkwork-routes-iot-app-api`
- `pnpm api:check`

## Owner And Status

Owner and lifecycle status are tracked in `specs/component.spec.json`. Update that contract before changing public integration behavior.
