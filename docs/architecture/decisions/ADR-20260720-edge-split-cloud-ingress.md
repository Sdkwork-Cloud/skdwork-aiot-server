# ADR-20260720-edge-split-cloud-ingress

Status: accepted
Requirement: production topology and device ingress alignment
Owner: sdkwork-aiot-platform
Date: 2026-07-20
Specs: APP_RUNTIME_TOPOLOGY_SPEC.md, APPLICATION_GATEWAY_SPEC.md, ARCHITECTURE_DECISION_SPEC.md

## Context

AIoT has two distinct cloud connectivity planes. Application HTTP APIs are
consumed through the SDKWork platform gateway, while WebSocket, MQTT, UDP, OTA,
and device protocol traffic is owned by `sdkwork-aiot-cloud-gateway`. Historical
ADR 002 retains the specialized transport and ADR 003 defines horizontal
scaling for stateful device sessions.

Topology v5 requires this non-collapsed edge boundary and its governing
decision to be explicit. Cloud development must consume deployed endpoints and
must not start local API, database, platform gateway, or edge gateway services.

## Decision

Use the `edge-split` cloud ingress strategy.

- `sdkwork-api-cloud-gateway` owns platform and application API ingress.
- `sdkwork-aiot-cloud-gateway` owns device and edge protocol ingress.
- Cloud development uses `https://api-dev.sdkwork.com` for platform/application
  HTTP access and `https://edge-dev.aiot.sdkwork.com` for edge HTTP/WebSocket
  access.
- Standalone profiles continue to run the standalone gateway and edge gateway
  locally as declared by topology.

## Alternatives

- Collapse device traffic into the platform gateway: rejected because the
  platform gateway does not own WebSocket session affinity, MQTT/UDP bridging,
  OTA, or device protocol handling.
- Use a dedicated application HTTP gateway in addition to the edge gateway:
  rejected because application APIs already compose through the platform
  gateway and would create a redundant ingress layer.
- Start cloud services locally during cloud development: rejected by the
  remote-only cloud development contract.

## Consequences

Application API and device traffic can scale and fail independently, but
deployments must configure and observe both origins. Device reconnect and
sticky-session behavior remains governed by the historical gateway scaling
decision. The platform gateway remains free of device protocol ownership.

## Verification

- `node ../sdkwork-specs/tools/check-topology-deployment-profiles.mjs --workspace .. --repo sdkwork-aiot`
- `pnpm topology:validate`
- `pnpm gateway:validate:cloud`
- Gateway transport and horizontal scaling tests named by historical ADR 002
  and ADR 003 remain required.

## Supersedes / Superseded By

This record formalizes the topology consequence of historical ADR 002 and ADR
003 without superseding their transport and scaling decisions.
