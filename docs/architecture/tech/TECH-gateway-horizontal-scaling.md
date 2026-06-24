> Migrated from `docs/adr/003-gateway-horizontal-scaling.md` on 2026-06-24.
> Owner: SDKWork maintainers

## Status

Accepted — sticky sessions with node identity; no shared WS store in this milestone.

## Context

WebSocket and MQTT/UDP bridge sessions are process-local. Multiple gateway replicas require deterministic routing for a given device session.

## Decision

1. Run multiple gateway instances behind a load balancer with **sticky sessions** (device id or connection cookie).
2. Set `SDKWORK_AIOT_GATEWAY_NODE_ID` per replica for structured metrics and trace correlation.
3. Use shared SQLite (`SDKWORK_AIOT_DEVICE_DB_PATH`) for credentials and protocol ingest; do not replicate in-memory bridge session state across nodes.

## Consequences

- Failover may drop active WebSocket sessions until devices reconnect.
- Bridge MQTT/UDP state remains per-process; enable bridge only on nodes that own UDP listeners or use dedicated bridge workers.

## Verification

Gateway exposes `/internal/bridge/health` and Prometheus-style metrics; structured trace lines include `nodeId` when `SDKWORK_AIOT_STRUCTURED_TRACE=1`.

