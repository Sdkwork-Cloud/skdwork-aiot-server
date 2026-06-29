> Migrated from `docs/adr/001-iam-via-appbase-proxy.md` on 2026-06-24.
> Owner: SDKWork maintainers

## Status

Accepted — deployment topology, not in-process tables.

## Context

SDKWork AIoT forbids local IAM tables and foreign keys into `iam_*` schemas. Protected app/backend HTTP APIs use OpenAPI dual-token security (`AuthToken` + `AccessToken` schemes; wire headers `Authorization: Bearer <auth_token>` and `Access-Token: <access_token>`). Tenant, organization, user, and permission scope are resolved by `sdkwork-web-framework` and `sdkwork-iam-web-adapter` into `WebRequestContext`; OpenAPI contracts do not declare client-writable `X-Sdkwork-*` context parameters.

## Decision

1. Terminate user and service authentication at the **SDKWork AppBase / API gateway** layer using dual-token credentials.
2. Resolve `WebRequestContext` in-process through the SDKWork web framework and IAM adapter; do not require browsers or generated SDK clients to send `X-Sdkwork-*` projection headers.
3. Keep device-facing gateway auth separate: SQLite credentials or configured static device token, not AppBase session cookies.

## Consequences

- No `sdkwork-appbase` crate dependency is required inside this repository for production correctness.
- Operators must configure the upstream gateway to validate dual tokens and supply resolved IAM context to AIoT service binaries.
- Local development continues to use `SDKWORK_AIOT_TRUST_PROXY_HEADERS=1` in tests and trusted dev proxies.

## Verification

Architecture tests assert the workspace does not declare parallel IAM components or IAM-owned DDL.
