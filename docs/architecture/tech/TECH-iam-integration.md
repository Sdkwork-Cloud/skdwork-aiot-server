> Migrated from `docs/deployment/iam-integration.md` on 2026-06-24.
> Owner: SDKWork maintainers

Production admin and app APIs assume an upstream SDKWork AppBase-compatible proxy has already authenticated the caller.

## Required Forwarded Headers

| Header | Purpose |
| --- | --- |
| `Authorization` | Service or app bearer token validated upstream |
| `Access-Token` | End-user session token validated upstream |
| `X-Sdkwork-Tenant-Id` | Tenant association |
| `X-Sdkwork-Organization-Id` | Organization association |
| `X-Sdkwork-Permission-Scope` | Permission scope for route authorization |

## Local Development

Integration tests set `SDKWORK_AIOT_TRUST_PROXY_HEADERS=1` and send the headers above directly. Do not enable trust-proxy mode on internet-facing listeners without a validating reverse proxy.

## Gateway Exception

Device connections authenticate with `Device-Id` and `Authorization: Bearer <device secret>` (SQLite credential or configured fallback). AppBase user tokens are not used on the Xiaozhi WebSocket path.

See [ADR 001](../adr/001-iam-via-appbase-proxy.md).

