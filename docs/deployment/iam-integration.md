# SDKWork AppBase IAM Integration

Production admin and app APIs assume callers present valid dual-token credentials and that `sdkwork-web-framework` resolves `WebRequestContext` before handlers run.

## Client Wire Credentials (OpenAPI / Generated SDK)

| Wire header | OpenAPI scheme | Purpose |
| --- | --- | --- |
| `Authorization: Bearer <auth_token>` | `AuthToken` | Principal/session identity |
| `Access-Token: <access_token>` | `AccessToken` | Tenant isolation and access context |

Generated `@sdkwork/aiot-app-sdk` and `@sdkwork/aiot-backend-sdk` clients inject these headers through `AuthTokenManager`; UI and service facades must not assemble them manually.

## Internal Context (Not OpenAPI Client Parameters)

Tenant, organization, user, and permission scope are carried in resolved `WebRequestContext`. OpenAPI authorities do not declare client-writable `X-Sdkwork-*` parameters.

Integration tests and trusted dev proxies may set association headers when `SDKWORK_AIOT_TRUST_PROXY_HEADERS=1` is enabled. Do not enable trust-proxy mode on internet-facing listeners without a validating reverse proxy.

## Local Development

Use bootstrap `Access-Token` from private runtime config (`SDKWORK_ACCESS_TOKEN` per `ENVIRONMENT_SPEC.md`) and authenticated session `Authorization` through the IAM runtime / TokenManager. See [ADR 001](../adr/001-iam-via-appbase-proxy.md).

## Gateway Exception

Device connections authenticate with `Device-Id` and `Authorization: Bearer <device secret>` (SQLite credential or configured static token). AppBase user tokens are not used on the Xiaozhi WebSocket path.
