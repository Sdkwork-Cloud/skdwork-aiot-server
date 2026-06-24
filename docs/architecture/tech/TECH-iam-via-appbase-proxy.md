> Migrated from `docs/adr/001-iam-via-appbase-proxy.md` on 2026-06-24.
> Owner: SDKWork maintainers

## Status

Accepted — deployment topology, not in-process tables.

## Context

SDKWork AIoT forbids local IAM tables and foreign keys into `iam_*` schemas. Admin and app HTTP APIs expect `Authorization`, `Access-Token`, and `X-Sdkwork-*` association headers that mirror SDKWork AppBase request context.

## Decision

1. Terminate user and service authentication at the **SDKWork AppBase / API gateway** layer.
2. Forward validated association headers to `sdkwork-aiot-admin-api` and `sdkwork-aiot-app-api`.
3. Keep device-facing gateway auth separate: SQLite credentials or configured legacy token, not AppBase session cookies.

## Consequences

- No `sdkwork-appbase` crate dependency is required inside this repository for production correctness.
- Operators must configure the upstream proxy to inject tenant, organization, and permission scope headers.
- Local development continues to use `SDKWORK_AIOT_TRUST_PROXY_HEADERS=1` in tests and trusted dev proxies.

## Verification

Architecture tests assert the workspace does not declare parallel IAM components or IAM-owned DDL.

