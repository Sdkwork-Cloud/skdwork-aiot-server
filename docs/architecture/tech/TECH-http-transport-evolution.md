> Migrated from `docs/adr/002-http-transport-evolution.md` on 2026-06-24.
> Owner: SDKWork maintainers

## Status

Accepted — retain minimal transport; defer Axum migration.

## Context

Services use `sdkwork-aiot-transport` with thread-per-connection HTTP serving, full body reads, concurrent admin/app listeners, and WebSocket upgrade support tested in `transport_standard.rs`.

## Decision

1. Keep the current transport stack as the **supported production path** for AIoT services.
2. Do not introduce Axum/Tokio as a hard dependency until an explicit cross-service migration milestone is funded.
3. New HTTP behavior must extend `sdkwork-aiot-transport` and shared `sdkwork-iot-platform-service` handlers first.

## Consequences

- Lower operational complexity and predictable resource usage for device gateway workloads.
- Future Axum migration should be service-by-service behind the same route contracts and OpenAPI authorities.

## Verification

`cargo test -p sdkwork-aiot-transport` and HTTP API integration tests cover parsing, concurrency, and WebSocket framing.

