# SDKWork AIoT Server Design

Date: 2026-05-31
Status: Draft for user review
Language: Rust
Primary mode: library-first, service binaries as assembly layers

## 1. Objective

Build a general-purpose, extensible IoT server for SDKWork. The server must not be a xiaozhi clone. Xiaozhi is one compatibility plugin and one reference implementation among many device and protocol ecosystems.

The platform must support socket and WebSocket communication first, while leaving stable extension points for MQTT, UDP, CoAP/LwM2M, Matter, Zigbee2MQTT, LoRaWAN, Modbus, OPC UA, ESPHome, Tasmota, OpenBeken, Zephyr, RIOT, NuttX, FreeRTOS, and future custom hardware protocols.

Current implementation scope after this design is approved:

- Server-side Rust libraries and service assembly.
- App/backend OpenAPI contracts.
- Generated SDKs following SDKWork standards.
- Xiaozhi compatibility as the first plugin.
- Extensible protocol, capability, OTA, provisioning, device registry, command, telemetry, and twin model.

Out of current scope:

- Full frontend product UI.
- Reimplementing MQTT brokers, LoRaWAN network servers, Matter controllers, Zigbee coordinators, or industrial protocol stacks where stable direct integration or bridge/plugin integration is more appropriate.
- Storing raw audio/media frames by default.

## 2. Standards Baseline

The design follows SDKWork standards from:

- `../sdkwork-specs`
- `../sdkwork-claw-router`
- `../sdkwork-appbase`

Normative constraints:

- App API prefix: `/app/v3/api`
- Backend API prefix: `/backend/v3/api`
- Xiaozhi device compatibility prefix: `/iot/xiaozhi/`
- OpenAPI is the source of truth for HTTP APIs.
- SDKs must be generated from OpenAPI. No handwritten raw HTTP fallback.
- Protected app/backend APIs require `Authorization` and `Access-Token`.
- Device protocol authentication maps into a `DevicePrincipal`, not directly into app/backend user auth.
- IAM, tenant, organization, user, role, permission, policy, app context, and sharding context are provided by `sdkwork-appbase`.
- AIoT must not create a parallel IAM system, IAM crate, or IAM API module. AIoT stores only IAM association fields and receives already-resolved IAM context from the host/appbase integration layer.
- Errors use RFC 9457 `application/problem+json`.
- Events follow CloudEvents-like structure.
- Database contracts are designed before DDL/ORM.
- High-frequency query fields must be real columns, not only JSON extension fields.
- `int64` and `decimal` API fields serialize as strings.
- Timestamps serialize as ISO 8601 UTC.
- Multi-tenant access paths must include tenant and organization isolation.

## 2.1 IAM And Appbase Boundary

`sdkwork-appbase` is the canonical IAM owner. AIoT consumes IAM as a shared kernel and must not reimplement it.

Relevant appbase packages:

```text
apps/sdkwork-appbase/packages/common/iam/sdkwork-iam-contracts
apps/sdkwork-appbase/packages/common/iam/sdkwork-iam-sdk-ports
apps/sdkwork-appbase/packages/common/iam/sdkwork-iam-service
apps/sdkwork-appbase/packages/common/iam/sdkwork-iam-runtime
apps/sdkwork-appbase/packages/native-rust/iam/sdkwork-iam-core-rust
apps/sdkwork-appbase/packages/native-rust/iam/sdkwork-iam-http-rust
apps/sdkwork-appbase/packages/native-rust/iam/sdkwork-iam-storage-sqlx-rust
```

AIoT integration rules:

- AIoT does not own `iam_tenant`, `iam_organization`, `iam_user`, `iam_role`, `iam_permission`, `iam_policy`, `iam_credential`, or IAM session tables.
- AIoT tables carry IAM association fields: `tenant_id`, `organization_id`, `user_id`, `owner_type`, `owner_id`, `data_scope`, `created_by`, `updated_by`, `deleted_by`.
- AIoT app/backend API assembly validates dual-token auth through the SDKWork IAM runtime or compatible Rust IAM context before invoking AIoT use cases.
- AIoT authorization is expressed as stable permission strings such as `iot.devices.read`, `iot.devices.write`, `iot.commands.execute`, `iot.firmware.write`. Permission evaluation remains in appbase IAM or the host integration layer.
- AIoT device-protocol authentication remains separate because devices are not app users. A successful device auth creates `DevicePrincipal`, which is scoped to an IAM tenant and organization.
- AIoT may emit audit/security events that reference IAM actor ids, but IAM remains the system of record for users, tenants, organizations, roles, permissions, and policies.

Required AIoT association contract:

```text
AiotRequestContext
  Carries resolved tenant_id, organization_id, user_id, actor_type, actor_id,
  data_scope, permission_scope, trace_id, and deployment context.

AiotActorRef
  Normalizes references to IAM users, IAM services, AIoT devices, and system actors
  without creating IAM-owned records.

AiotOwnershipRef
  Represents tenant, organization, user, service, or device ownership through
  owner_type and owner_id.
```

AIoT domain services receive this resolved context from the application/service assembly layer. They must not import appbase concrete IAM packages directly and must not publish IAM management APIs.

## 3. Library-First Architecture

The AIoT server must be implemented as reusable Rust crates first. A deployable server process is only an assembly layer.

Target workspace:

```text
crates/sdkwork-aiot-contract
crates/sdkwork-iot-device-service
crates/sdkwork-aiot-protocol
crates/sdkwork-aiot-service-host
crates/sdkwork-aiot-storage
crates/sdkwork-aiot-storage-sqlx
crates/sdkwork-aiot-security
crates/sdkwork-aiot-observability
crates/sdkwork-aiot-adapter-xiaozhi
crates/sdkwork-aiot-adapter-mqtt
crates/sdkwork-aiot-adapter-industrial
services/sdkwork-aiot-cloud-gateway
services/sdkwork-aiot-admin-api
services/sdkwork-aiot-app-api
generated/
sdks/sdkwork-aiot-app-sdk
sdks/sdkwork-aiot-backend-sdk
```

Responsibilities:

| Crate or service | Responsibility |
| --- | --- |
| `sdkwork-aiot-contract` | Public DTOs, OpenAPI-aligned schemas, event contracts, shared enums. |
| `sdkwork-iot-device-service` | DDD aggregates, domain services, use case ports, invariants. |
| `sdkwork-aiot-protocol` | Transport-neutral protocol envelope, adapter traits, codecs, capability mapping. |
| `sdkwork-aiot-service-host` | Embeddable runtime: plugin registry, session manager, command router, event dispatcher. |
| `sdkwork-aiot-storage` | Repository and unit-of-work traits. |
| `sdkwork-aiot-storage-sqlx` | SQLx implementation for Postgres and development SQLite. |
| `sdkwork-aiot-security` | Device auth, credential validation, HMAC, mTLS/X.509 hooks. |
| `sdkwork-aiot-observability` | Tracing, metrics, structured logs, audit helpers, redaction. |
| `sdkwork-aiot-adapter-xiaozhi` | Xiaozhi WebSocket, MQTT+UDP, OTA, activation compatibility. |
| `sdkwork-aiot-adapter-mqtt` | MQTT adapter/bridge abstraction. |
| `sdkwork-aiot-adapter-industrial` | Future Modbus/OPC UA bridge abstractions. |
| `sdkwork-aiot-cloud-gateway` | Binary assembly for TCP/UDP/WebSocket/MQTT listeners. |
| `sdkwork-aiot-admin-api` | Backend API binary assembly. |
| `sdkwork-aiot-app-api` | App API binary assembly. |

Embedding shape:

```rust
let runtime = AiotRuntime::builder()
    .with_storage(storage)
    .with_security(security)
    .with_observability(observability)
    .register_adapter(xiaozhi_adapter)
    .register_adapter(mqtt_adapter)
    .build();
```

The runtime must be usable by another Rust application without starting SDKWork's default service binaries.

## 3.1 Composable Component Architecture

AIoT must be built like composable building blocks. Every major capability is a replaceable component with explicit inputs, outputs, dependencies, and ownership boundaries.

Supported integration modes:

| Mode | Description | Requirement |
| --- | --- | --- |
| Embedded library | Another Rust application embeds AIoT runtime crates and mounts selected routes/listeners. | No hardcoded process globals, ports, URLs, storage, IAM, or protocol adapters. |
| Standalone server | SDKWork starts `services/sdkwork-aiot-cloud-gateway`, `services/sdkwork-aiot-admin-api`, and `services/sdkwork-aiot-app-api` as independent services. | Binaries must assemble the same runtime components used by embedded mode. |
| Sidecar gateway | AIoT gateway runs next to another app and exposes device protocols while management APIs stay in the host app. | Device protocol routes, command routing, and storage must remain configurable. |
| Hybrid monolith | Host app embeds admin/app APIs but starts gateway as a separate socket-heavy process. | Contracts, storage, and event model must stay identical across process boundaries. |

Component rules:

- Contracts are stable and live below runtime and service assembly.
- Domain core depends on contracts and abstract repositories, not SQLx, Redis, Axum, MQTT, or xiaozhi.
- Protocol adapters depend on protocol contracts and runtime ports, not database implementations.
- Storage implementations depend on storage traits and schema contracts, not service binaries.
- Service binaries depend on components and configuration, but components must not depend on service binaries.
- All components are initialized through explicit builders or manifests.
- Components must support fake/test implementations for contract tests.
- Optional adapters should be gated by features or assembly configuration.

Canonical component contracts:

```text
AiotComponentManifest
  name, version, domain, capabilities, required_features, config_schema

AiotRuntimeBuilder
  storage, cache, event_bus, security, observability, protocol_adapters, hooks

AiotProtocolComponent
  protocol_id, transports, codec, mapper, session_policy, security_modes

AiotStorageComponent
  repositories, migrations, health_checks, schema_version

AiotHttpComponent
  app_routes, backend_routes, route_prefix, OpenAPI source

AiotGatewayComponent
  websocket_routes, tcp_listeners, udp_listeners, mqtt_bindings
```

Fast integration target:

```rust
let aiot = AiotRuntime::builder()
    .with_storage(storage)
    .with_cache(cache)
    .with_event_bus(event_bus)
    .with_request_context(context_provider)
    .register_protocol(xiaozhi)
    .build();

let app = existing_axum_app
    .merge(aiot.app_routes())
    .merge(aiot.backend_routes())
    .merge(aiot.device_protocol_routes());
```

The standalone server must use the same builder and components:

```rust
let aiot = build_aiot_from_config(config)?;
aiot.serve().await?;
```

This prevents two implementations: one embedded and one standalone. The binary is only a deployment wrapper.

### 3.2 Component Boundary Matrix

The implementation must keep boundaries testable. A component that crosses its boundary should fail architecture checks.

| Component | May depend on | Must not depend on |
| --- | --- | --- |
| `sdkwork-aiot-contract` | `serde`, schema helpers, stable value types | Axum, SQLx, Redis, protocol adapters, service binaries |
| `sdkwork-iot-device-service` | contracts, repository traits, domain policies | Axum, SQLx concrete pools, Redis clients, WebSocket sockets, xiaozhi |
| `sdkwork-aiot-protocol` | contracts, bytes/codec abstractions | SQL tables, repositories, app/backend API handlers |
| `sdkwork-aiot-service-host` | core ports, protocol adapters, storage/cache/event bus traits | concrete application bootstraps, hardcoded listeners, appbase internals |
| `sdkwork-aiot-storage` | contracts, repository traits | protocol sockets, Axum handlers, xiaozhi messages |
| `sdkwork-aiot-storage-sqlx` | storage traits, SQLx, migrations | WebSocket/MQTT listeners, command transport encoding |
| `sdkwork-aiot-security` | device credential contracts, crypto helpers | appbase IAM table ownership, app/backend route registration |
| `sdkwork-aiot-adapter-*` | protocol traits, runtime ports, codec helpers | SQLx pools, DB writes, app/backend API handlers |
| `services/sdkwork-aiot-*` | runtime builder, route/listener components, config | domain invariants, custom DB logic, protocol-specific business rules |

Boundary rule:

```text
contracts <- core <- runtime <- service assembly
          <- protocol <- adapters
          <- storage traits <- storage implementation
```

No reverse dependency is allowed. Service binaries are replaceable launchers.

### 3.3 Fast Integration Package

The project must expose a small integration surface so another application can add AIoT capabilities without copying internal code.

Required integration artifacts:

```text
AiotRuntimeBuilder
AiotRuntime
AiotComponentManifest
AiotDefaultBundle
AiotStorageBundle
AiotProtocolBundle
AiotHttpRouteBundle
AiotGatewayListenerBundle
AiotConfig
AiotHealthCheck
```

Integration flow for an embedded host:

```text
1. Host resolves appbase IAM context.
2. Host creates storage/cache/event-bus implementations.
3. Host selects protocol adapters.
4. Host builds AiotRuntime.
5. Host mounts app/backend routes if needed.
6. Host mounts device protocol routes or starts gateway listeners.
7. Host uses generated SDKs for management API calls.
```

Standalone server flow:

```text
1. Load AiotConfig.
2. Build the same AiotRuntime.
3. Mount app/backend APIs under SDKWork standard prefixes.
4. Mount device protocol endpoints such as /iot/xiaozhi/ws.
5. Start configured socket/WebSocket/UDP/MQTT listeners.
6. Publish health, metrics, and readiness endpoints.
```

The standalone server must not expose extra behavior that cannot be reproduced through embedded component assembly.

### 3.4 Component Acceptance Checklist

Before implementation is accepted:

- A host application can build AIoT runtime without starting SDKWork service binaries.
- The standalone server uses the same runtime builder as embedded mode.
- xiaozhi can be included or excluded as a plugin.
- Storage can be swapped through storage traits.
- Redis/cache can be disabled for local development and enabled for production.
- Appbase IAM remains external; AIoT receives resolved request context and stores association fields only.
- App/backend routes can be mounted into an existing Axum router.
- Device protocol routes can be mounted independently from app/backend management APIs.
- Protocol adapters cannot write database tables directly.
- Service binaries contain no domain invariants.
- Generated SDKs target the same API contracts used by embedded and standalone modes.
- Boundary tests or dependency guardrails verify the dependency direction.

## 4. DDD Bounded Contexts

AIoT-owned bounded contexts:

```text
Product Catalog
Hardware Profile
Protocol Profile
Device Registry
Protocol Gateway
Connection & Session Runtime
Device Twin
Capability Model
Command & Control
Telemetry & Event
Media Runtime
OTA & Provisioning
Edge Gateway
Operations & Observability
```

External shared kernel:

```text
IAM & Tenant Context: provided by sdkwork-appbase
```

Core aggregates:

| Aggregate | Purpose |
| --- | --- |
| `Product` | Tenant-owned product line, default profiles, policies. |
| `HardwareProfile` | Chip, board, connectivity, security, OTA, runtime capability abstraction. |
| `ProtocolProfile` | Allowed protocol adapters, versions, endpoint policy, message constraints. |
| `CapabilityModel` | Standard properties, commands, events, media, and custom capabilities. |
| `Device` | Registered device identity, lifecycle, bindings, profile assignments. |
| `DeviceCredential` | Auth material references and validation metadata. |
| `DeviceConnection` | Physical/logical connection record. |
| `DeviceSession` | Runtime session, negotiated protocol, lease, audio/control state. |
| `DeviceTwin` | Desired/reported state and versioned property state. |
| `DeviceCommand` | Command request, delivery, ack, result, timeout, idempotency. |
| `TelemetryEvent` | Device-originated measurement or event record. |
| `FirmwareArtifact` | Firmware package metadata, hash, signature, compatibility target. |
| `FirmwareRollout` | Rollout policy, targets, status, deployment records. |
| `ProvisioningChallenge` | Activation, claim, binding, HMAC challenge flow. |
| `EdgeGateway` | Parent gateway, child topology, offline sync boundaries. |

Value objects:

```text
TenantId
OrganizationId
UserId
OwnerType
OwnerId
DataScope
ProductId
DeviceId
DeviceKey
ClientId
SessionId
ConnectionId
ProtocolId
AdapterId
CapabilityName
FirmwareVersion
ChipFamily
RuntimeProfile
BoardProfile
TraceId
IdempotencyKey
```

Important aggregate boundaries:

- `Device` owns registry identity and lifecycle, not live connection mechanics.
- `DeviceSession` owns negotiated protocol/session state, not device master data.
- `CapabilityModel` owns semantic capabilities, not transport-specific encoding.
- `ProtocolAdapter` translates protocol details into core messages and must not bypass domain services.
- `DeviceCommand` is an aggregate with lifecycle transitions, not a raw outbound packet.
- Tenant, organization, user, owner, and permission semantics come from appbase IAM context. AIoT only references those identifiers and enforces them through ports.

## 5. Hardware And Runtime Abstraction

Hardware support is modeled through profiles, not hardcoded conditionals.

Key dimensions:

```text
HardwareClass:
  mcu / linux_sbc / edge_gateway / industrial_controller /
  camera_device / audio_device / cellular_module / bridge_adapter

ChipFamily:
  esp32_s3 / esp32_c3 / esp8266 / nrf52 / stm32 / rp2040 /
  bk72xx / bl602 / w800 / rtl8720 / bcm2712 / linux_arm64 / riscv_mcu

RuntimeProfile:
  esp_idf / arduino / freertos / zephyr / riot / nuttx / linux /
  docker / home_assistant / pico_sdk / micropython /
  esphome / tasmota / openbeken / xiaozhi_firmware

ConnectivityProfile:
  wifi / ble / ethernet / lte / nb_iot / thread / zigbee / zigbee_usb /
  lora / serial

SecurityProfile:
  bearer_token / hmac / mtls_x509 / secure_boot / flash_encryption /
  secure_element / tpm / hardware_attestation / device_secret

OtaProfile:
  esp_ota / mcuboot / hawkbit_ddi / mender_like / tasmota_ota /
  openbeken_ota / xiaozhi_ota / http_firmware / apt_container_image /
  custom_http
```

Hardware class is explicit because a chip name alone is not enough to decide
protocol behavior. An ESP32-S3 voice device and a Raspberry Pi Pico W are MCU
firmware endpoints. A Raspberry Pi 4/5/CM, Jetson, or industrial Linux box is a
Linux SBC or edge gateway that may proxy many downstream devices over Zigbee,
Matter, BLE, Modbus, serial, camera, or audio paths. The first hardware class is
the primary class; additional classes describe composite roles such as
`linux_sbc + edge_gateway`.

Reference profiles:

| Profile | Hardware class | Chip family | Runtime | Connectivity | Security | OTA |
| --- | --- | --- | --- | --- | --- | --- |
| `hw-esp32-s3` | `mcu`, `audio_device` | `esp32_s3` | `esp_idf`, `freertos`, `xiaozhi_firmware` | `wifi`, `ble` | `secure_boot`, `flash_encryption`, `device_secret` | `xiaozhi_ota`, `esp_ota` |
| `hw-raspberry-pi-5` | `linux_sbc`, `edge_gateway` | `bcm2712` | `linux`, `docker`, `home_assistant` | `ethernet`, `wifi`, `zigbee_usb` | `tpm` | `apt_container_image` |
| `hw-raspberry-pi-pico-w` | `mcu` | `rp2040` | `pico_sdk`, `micropython`, `zephyr` | `wifi` | `device_secret` | `http_firmware`, `mcuboot` |
| `hw-industrial-arm64-gateway` | `linux_sbc`, `industrial_controller`, `edge_gateway` | `linux_arm64` | `linux`, `docker` | `ethernet`, `lte`, `serial` | `tpm`, `mtls_x509` | `apt_container_image`, `mender_like` |

This allows the platform to support ESP32/xiaozhi today, Raspberry Pi Linux
gateways and Pico/RP2040 MCU firmware endpoints next, and other non-ESP chips
or Linux gateways later without changing the core protocol model.

## 6. Protocol Architecture

The protocol layer is organized as seven layers:

```text
Transport Layer
Session Layer
Envelope Layer
Semantic Layer
Capability Layer
Plugin Layer
Governance Layer
```

Layer definitions:

| Layer | Examples | Responsibility |
| --- | --- | --- |
| Transport | TCP, UDP, TLS, WebSocket, HTTP, MQTT, CoAP, Serial, BLE | Bytes, frames, sockets, listener lifecycle. |
| Session | handshake, auth, heartbeat, reconnect, lease | Connection identity, lifecycle, liveness, backpressure. |
| Envelope | JSON, JSON-RPC, Protobuf, CBOR, binary, MQTT packet | Decode/encode protocol messages. |
| Semantic | telemetry, command, property, event, media, OTA | Map raw messages into domain intent. |
| Capability | MCP tool, Matter cluster, LwM2M object, Modbus register | Map implementation capability into standard capability model. |
| Plugin | xiaozhi, mqtt, modbus, opcua, zigbee2mqtt | Isolated compatibility implementation. |
| Governance | version, security, quota, audit, observability | Cross-cutting runtime enforcement. |

### 6.1 Standard Protocol Envelope

Internal protocol messages use a transport-neutral envelope.

```text
message_id
protocol_id
protocol_version
adapter_id
tenant_id
organization_id
product_id
device_id
client_id
connection_id
session_id
direction
message_class
semantic_type
content_type
payload_encoding
payload
qos
sequence_no
timestamp
correlation_id
idempotency_key
trace_id
security_context
capability_context
extensions
```

Required message classes:

```text
handshake
auth
heartbeat
disconnect
provisioning
telemetry
event
property_report
property_set
twin_desired
twin_reported
command_request
command_ack
command_result
media_frame
ota_check
ota_deploy
gateway_topology
security_event
diagnostic
```

The core domain must not branch on xiaozhi-specific `type`, MQTT topic shape, Modbus function code, or OPC UA node id directly. Those are adapter concerns.

### 6.2 Adapter Contracts

Conceptual Rust traits:

```rust
pub trait ProtocolAdapter {
    fn manifest(&self) -> ProtocolAdapterManifest;
    fn match_handshake(&self, ctx: &HandshakeContext) -> MatchResult;
    async fn accept(&self, conn: TransportConnection, runtime: RuntimePorts) -> Result<()>;
}

pub trait MessageCodec {
    fn decode(&self, frame: InboundFrame) -> Result<ProtocolEnvelope>;
    fn encode(&self, envelope: ProtocolEnvelope) -> Result<OutboundFrame>;
}

pub trait CapabilityMapper {
    fn to_core(&self, envelope: ProtocolEnvelope) -> Result<CoreDeviceMessage>;
    fn from_core(&self, command: CoreDeviceCommand) -> Result<ProtocolEnvelope>;
}

pub trait DeviceAuthProvider {
    async fn authenticate(&self, ctx: DeviceAuthContext) -> Result<DevicePrincipal>;
}
```

Adapter manifest:

```text
plugin_id
plugin_version
protocol_ids
transport_bindings
supported_protocol_versions
capability_bridges
security_modes
ota_profiles
provisioning_profiles
config_schema
message_schema_refs
event_schema_refs
compatibility_level
```

### 6.3 Versioning And Negotiation

Protocol negotiation must support:

- Transport selection.
- Protocol id and version.
- Codec selection.
- Binary frame version.
- Audio/media parameters.
- Capability bridge selection.
- Heartbeat interval.
- Max frame size.
- Compression support.
- Security mode.
- OTA/provisioning compatibility.

Negotiation result becomes part of `DeviceSession`.

### 6.4 Flow Control And Backpressure

Runtime must enforce:

- Per-connection inbound frame size limit.
- Per-device outbound queue limit.
- Per-tenant concurrent session quota.
- Per-adapter command rate limit.
- Per-message decode timeout.
- Backpressure policy: wait, drop low-priority, reject command, or close session.
- Media frame special handling with bounded buffers and no default persistence.

### 6.5 Protocol Errors

Device protocol errors are not HTTP problem details, but they should map to a common internal error model:

```text
protocol.invalid_frame
protocol.unsupported_version
protocol.auth_failed
protocol.session_timeout
protocol.capability_denied
protocol.command_timeout
protocol.payload_too_large
protocol.rate_limited
protocol.replay_detected
protocol.adapter_unavailable
```

When exposed through app/backend APIs, these map to SDKWork problem details.

## 7. Xiaozhi Compatibility Plugin

Xiaozhi base URL:

```text
https://domain/iot/xiaozhi/
```

Required endpoints:

```text
/iot/xiaozhi/ws
/iot/xiaozhi/ota
/iot/xiaozhi/activate
```

Xiaozhi mapping:

| Xiaozhi wire concept | SDKWork AIoT concept |
| --- | --- |
| `Authorization` header | Device credential token input. |
| `Protocol-Version` header | Protocol version negotiation. |
| `Device-Id` header | Physical identifier, often MAC. |
| `Client-Id` header | Firmware/client UUID. |
| `type: hello` | Handshake message. |
| `transport: websocket` | WebSocket transport binding. |
| `transport: udp` | Hybrid MQTT control + UDP media transport. |
| `audio_params` | Media session negotiation. |
| Binary Opus frame | `media_frame`. |
| `type: listen` | Audio capture/listen state event. |
| `type: abort` | Session or command cancellation. |
| `type: mcp` | MCP JSON-RPC capability bridge. |
| `stt`, `tts`, `llm` | Assistant/media runtime events. |
| OTA `activation` | Provisioning challenge. |
| OTA `mqtt`, `websocket` | Protocol bootstrap config. |
| OTA `firmware` | Firmware update check result. |

Xiaozhi adapter requirements:

- Accept WebSocket handshake headers.
- Decode JSON text frames and binary audio frames.
- Support binary protocol v1 raw Opus, v2 packed, v3 packed.
- Support hello negotiation timeout.
- Map MCP JSON-RPC payload to `CapabilityBridge::McpJsonRpc`.
- Support OTA response shape expected by xiaozhi firmware.
- Avoid making xiaozhi message names part of the core domain enum.

## 8. Other Open Source Mappings

The platform should learn from existing projects but not become coupled to them.

| Project | Useful abstraction |
| --- | --- |
| Xiaozhi ESP32 | AI hardware bootstrapping, WebSocket/MQTT+UDP compatibility, OTA activation, MCP-style interaction. |
| RMQTT | Canonical MQTT broker/server integration, Rust-native MQTT auth/ACL/hook/session/offline queue extension point. |
| ESPHome | Component model, YAML-driven hardware capabilities, ESP/RP2040/LibreTiny/Zephyr support. |
| Tasmota | MQTT topic command model, templates/modules, non-ESP chip compatibility. |
| Zigbee2MQTT | Zigbee bridge and MQTT topic mapping. |
| WLED | Smart lighting firmware, MQTT/JSON control surface, device effects/state model. |
| Raspberry Pi Linux SBC | Edge gateway pattern for Linux, Docker, Home Assistant, USB radios, camera/audio workloads, and downstream protocol bridges. |
| Raspberry Pi Pico/RP2040 | MCU firmware pattern for Pico SDK, MicroPython, Zephyr, constrained Wi-Fi telemetry, command, and OTA profiles. |
| ESP-IDF | Espressif chip SDK, task/runtime, networking, OTA, peripheral capability baseline. |
| Arduino-ESP32 | Arduino-compatible ESP32 hardware runtime, board/package model. |
| MicroPython | Microcontroller runtime, firmware scripting, board and peripheral abstraction. |
| Zephyr | RTOS, device tree, driver model, portable embedded hardware abstraction. |
| ThingsBoard | Device profile, attributes, telemetry, RPC, OTA package, queue/rule concepts. |

Design decision:

- Hono-like protocol adapter ideas are adopted as a pattern, but Hono is not kept as an external submodule.
- EdgeX-like device resource and command concepts are adopted into `CapabilityModel`, without keeping EdgeX source as a submodule.
- Ditto/KubeEdge desired/reported twin concepts are adopted as model inspiration, without keeping their source as submodules.
- ThingsBoard telemetry/latest/attribute/OTA ideas are adopted.
- MQTT broker/server should directly integrate RMQTT. LoRaWAN server, Zigbee bridge, Matter stack, Modbus library, and OPC UA stack should be integrated or bridged, not rewritten in v1.
- `external/` is limited to high-star, high-signal smart-hardware references plus explicit platform anchors such as Xiaozhi and RMQTT.
- Raspberry Pi Linux gateway and Raspberry Pi Pico support are modeled in core
  hardware/protocol standards, but no Raspberry Pi-specific source tree is kept
  in `external/` yet. Promote a new submodule only when it becomes a primary
  implementation reference, not merely because the hardware class is supported.

## 9. Product And Functional Plan

### P0: Standard Core And Xiaozhi Compatibility

Deliver:

- Library-first runtime.
- Composable component manifests and runtime builder.
- Embedded library mode and standalone server mode use the same runtime.
- Protocol adapter registry.
- Standard protocol envelope.
- Device registry.
- Device credential model.
- WebSocket listener assembly.
- Xiaozhi WebSocket adapter.
- Xiaozhi OTA/provisioning compatibility.
- Session lease.
- Command lifecycle.
- Device twin latest state.
- Basic telemetry/event ingestion.
- Backend OpenAPI.
- App OpenAPI.
- Generated backend/app SDKs.
- SQLx storage implementation.
- Redis online lease port and optional implementation.
- Outbox event table and publisher port.
- Structured tracing and metrics baseline.

### P1: Production IoT Platform

Deliver:

- MQTT adapter or broker bridge.
- UDP media session support for xiaozhi MQTT+UDP.
- Product/hardware/protocol profile APIs.
- Capability model APIs.
- Firmware artifact and rollout APIs.
- Redis-backed online state.
- NATS/Kafka command/event routing.
- ClickHouse or Timescale telemetry sink option.
- Tenant quotas and rate limits.
- Security event and audit event APIs.
- Device gateway/child topology model.

### P2: Ecosystem Expansion

Deliver:

- CoAP/LwM2M bridge.
- Matter bridge.
- Zigbee2MQTT bridge.
- ChirpStack bridge.
- Modbus mapper.
- OPC UA mapper.
- ESPHome/Tasmota/OpenBeken compatibility profiles.
- Edge gateway offline sync.
- Plugin marketplace/governance APIs.

## 10. Management API Surface

Device protocol compatibility:

```text
/iot/xiaozhi/ws
/iot/xiaozhi/ota
/iot/xiaozhi/activate
```

Backend API:

```text
/backend/v3/api/iot/products
/backend/v3/api/iot/hardware_profiles
/backend/v3/api/iot/protocol_profiles
/backend/v3/api/iot/capability_models
/backend/v3/api/iot/devices
/backend/v3/api/iot/devices/{deviceId}
/backend/v3/api/iot/devices/{deviceId}/credentials
/backend/v3/api/iot/devices/{deviceId}/sessions
/backend/v3/api/iot/devices/{deviceId}/capabilities
/backend/v3/api/iot/devices/{deviceId}/commands
/backend/v3/api/iot/devices/{deviceId}/twin
/backend/v3/api/iot/firmware_artifacts
/backend/v3/api/iot/firmware_rollouts
/backend/v3/api/iot/events
/backend/v3/api/iot/protocol_adapters
```

App API:

```text
/app/v3/api/iot/devices
/app/v3/api/iot/devices/{deviceId}
/app/v3/api/iot/devices/{deviceId}/commands
/app/v3/api/iot/devices/{deviceId}/twin
/app/v3/api/iot/devices/{deviceId}/events
```

Operation id examples:

```text
products.list
products.create
hardwareProfiles.list
protocolProfiles.list
capabilityModels.retrieve
devices.list
devices.retrieve
devices.update
devices.credentials.create
devices.sessions.list
devices.capabilities.list
devices.commands.create
devices.twin.retrieve
firmwareArtifacts.create
firmwareRollouts.create
events.list
protocolAdapters.list
```

Generated SDK examples:

```ts
client.iot.devices.retrieve(deviceId)
client.iot.devices.commands.create(deviceId, body)
client.iot.devices.twin.retrieve(deviceId)
client.iot.firmwareRollouts.create(body)
```

IAM and permission contract:

| API area | Permission examples |
| --- | --- |
| Products and profiles | `iot.products.read`, `iot.products.write`, `iot.profiles.read`, `iot.profiles.write` |
| Devices | `iot.devices.read`, `iot.devices.write`, `iot.devices.bind`, `iot.devices.delete` |
| Sessions and online state | `iot.sessions.read`, `iot.sessions.disconnect` |
| Commands | `iot.commands.read`, `iot.commands.execute`, `iot.commands.cancel` |
| Twin and telemetry | `iot.twins.read`, `iot.twins.write`, `iot.telemetry.read` |
| Firmware | `iot.firmware.read`, `iot.firmware.write`, `iot.firmware.rollout` |
| Protocol adapters | `iot.protocolAdapters.read`, `iot.protocolAdapters.write` |

These permission codes are registered/evaluated through appbase IAM. AIoT does not create IAM role or policy tables.

## 11. Database Design

All IoT domain tables use the `iot_` prefix.

### 11.1 Common Fields

Tenant-owned core tables include:

```text
id
uuid
tenant_id
organization_id
data_scope
created_at
updated_at
version
status
```

Optional owner, lifecycle, and audit fields:

```text
user_id
owner_type
owner_id
created_by
updated_by
deleted_at
deleted_by
archived_at
retention_until
```

Field ownership:

| Field | Meaning | Owner |
| --- | --- | --- |
| `tenant_id` | IAM tenant id associated with the row. | appbase IAM |
| `organization_id` | IAM organization id or `0` for tenant-wide data. | appbase IAM |
| `user_id` | IAM user id for user-owned resources when applicable. | appbase IAM |
| `owner_type` | Owner kind such as `tenant`, `organization`, `user`, `service`, `device`. | AIoT contract, references IAM/device principals |
| `owner_id` | Owner identifier matching `owner_type`. | AIoT contract, references IAM/device principals |
| `data_scope` | Data visibility scope compatible with SDKWork IAM/appbase semantics. | appbase IAM semantics |
| `created_by`, `updated_by`, `deleted_by` | Actor id from IAM actor context or service/device actor mapping. | appbase IAM or AIoT actor port |

AIoT must not use database foreign keys to IAM tables as a hard cross-service dependency. References are logical contracts validated by IAM ports and application-level checks. Local/private monolith deployments may share a database, but the service boundary still treats IAM as the owner.

Serialization rules:

- API `id`, `tenantId`, `organizationId`, `version`, and other int64 values are strings.
- Time fields are ISO 8601 UTC.
- Status is an enum with documented values.
- JSON extension fields must have a schema where possible.

### 11.2 Core Registry Tables

```text
iot_product
iot_hardware_profile
iot_protocol_profile
iot_capability_model
iot_capability_definition
iot_device
iot_device_credential
iot_device_binding
iot_gateway_child_device
```

`iot_device` important fields:

```text
device_key
product_id
hardware_profile_id
protocol_profile_id
tenant_id
organization_id
owner_type
owner_id
display_name
device_id
client_id
serial_number
mac_address
chip_family
runtime_profile
firmware_version
auth_state
lifecycle_state
last_seen_at
metadata
```

Required constraints:

```text
uk_iot_device_uuid
uk_iot_device_tenant_device_key
uk_iot_device_tenant_product_device_id
uk_iot_device_tenant_client_id
```

Recommended indexes:

```text
idx_iot_device_tenant_product_status
idx_iot_device_tenant_last_seen
idx_iot_device_tenant_hardware_profile
idx_iot_device_tenant_protocol_profile
```

`iot_device_binding` supports relationships to IAM-owned actors without creating IAM tables:

```text
device_id
binding_type
target_type
target_id
tenant_id
organization_id
role
status
bound_at
bound_by
expires_at
metadata
```

Examples:

```text
target_type=user, target_id=<iam_user.id>
target_type=organization, target_id=<iam_organization.id>
target_type=tenant, target_id=<iam_tenant.id>
target_type=service, target_id=<service principal id>
```

`iot_device_credential` rules:

- Store hashed secrets, certificate fingerprints, or external key references.
- Do not store plaintext tokens, private keys, or raw PSKs.
- Support credential rotation and revocation.

### 11.3 Protocol And Runtime Tables

```text
iot_protocol_adapter
iot_protocol_route
iot_device_connection
iot_device_session
iot_device_online_lease
iot_command
iot_command_delivery
iot_command_result
iot_protocol_message_dead_letter
```

Runtime strategy:

- Redis is the source of truth for online lease in production.
- Database stores session facts, projections, and crash recovery traces.
- Load balancer stickiness may improve performance but correctness must not depend on it.
- Cross-node command routing should use NATS, Kafka, or equivalent routing later.

Command required fields:

```text
command_id
tenant_id
organization_id
device_id
session_id
capability_name
command_name
request_payload
status
idempotency_key
timeout_at
created_at
ack_at
result_at
trace_id
```

Indexes:

```text
idx_iot_command_tenant_device_status_created
idx_iot_command_tenant_status_timeout
uk_iot_command_tenant_idempotency_key
idx_iot_command_delivery_tenant_session_status
```

### 11.4 Twin, Telemetry, And Event Tables

```text
iot_device_twin
iot_device_twin_property
iot_telemetry_latest
iot_telemetry_event
iot_device_event
iot_security_event
```

Twin design:

- `iot_device_twin` stores device-level twin metadata.
- `iot_device_twin_property` stores key-level desired/reported values and versions.
- Desired and reported state use separate value/version/timestamp fields.
- Important query keys are real columns.

Telemetry strategy:

- `iot_telemetry_latest` stores the latest value per device/key for fast reads.
- `iot_telemetry_event` may store bounded P0/P1 events but should not be the long-term high-volume time-series store.
- ClickHouse or TimescaleDB should be introduced for high-volume historical telemetry.

Indexes:

```text
idx_iot_twin_property_tenant_device_property
idx_iot_twin_property_tenant_updated
idx_iot_telemetry_latest_tenant_device_key
idx_iot_telemetry_event_tenant_device_time
idx_iot_device_event_tenant_device_time
idx_iot_security_event_tenant_time
```

Media policy:

- Raw audio/video frames are not persisted by default.
- Store only session metadata, quality metrics, event summaries, and error details.
- Debug capture requires explicit tenant policy, short retention, and redaction review.

### 11.5 OTA And Provisioning Tables

```text
iot_firmware_artifact
iot_firmware_rollout
iot_firmware_rollout_target
iot_firmware_deployment
iot_provisioning_challenge
iot_activation_record
```

Firmware artifact required fields:

```text
artifact_key
version
file_name
storage_uri
size_bytes
sha256
signature
signature_algorithm
target_chip_family
target_runtime_profile
target_hardware_profile_id
metadata
```

Rollout rules:

- Rollouts are asynchronous.
- Rollouts target product, hardware profile, protocol profile, firmware range, or explicit devices.
- Deployments record per-device state.
- Forced update and staged rollout must be explicit.
- OTA response must include hash/signature validation data where supported.

### 11.6 Outbox, Inbox, Audit

```text
iot_outbox_event
iot_inbox_event
iot_audit_log
```

Outbox event fields:

```text
event_id
event_type
aggregate_type
aggregate_id
tenant_id
organization_id
payload
status
next_attempt_at
attempt_count
created_at
published_at
trace_id
```

Events use stable dotted names:

```text
iot.device.registered
iot.device.connected
iot.device.disconnected
iot.device.session.started
iot.device.session.ended
iot.command.created
iot.command.acknowledged
iot.command.completed
iot.command.failed
iot.twin.desired.updated
iot.twin.reported.updated
iot.telemetry.received
iot.firmware.rollout.created
iot.firmware.deployment.completed
iot.security.deviceAuthFailed
```

### 11.7 Database Anti-Patterns

Forbidden:

- Storing tenant isolation only inside JSON.
- Storing credentials or tokens in plaintext.
- Using one generic `iot_message` table as the only system of record for every domain action.
- Storing all telemetry forever in Postgres without retention and partition strategy.
- Allowing protocol adapters to write core tables directly.
- Treating online state as a durable DB flag without TTL lease.
- Using JSON extensions for high-frequency filters and sort keys.

## 12. Security Design

Device auth levels:

```text
L1 bearer token
L2 HMAC challenge
L3 mTLS/X.509
L4 secure element / TPM / hardware attestation
```

Security requirements:

- Device protocol auth maps to `DevicePrincipal`.
- Device principal includes tenant, product, device, credential id, auth level, and trust flags.
- App/backend protected APIs use SDKWork dual-token auth through appbase IAM runtime/context.
- Command dispatch checks device, tenant, organization, product, capability, object ownership, data scope, and operation permission through an IAM authorization port.
- OTA requires artifact hash, signature, target validation, and rollback policy.
- UDP media uses anti-replay sequence checks where applicable.
- Logs must redact tokens, credentials, raw payloads, and sensitive device data.
- Security events are emitted for auth failures, replay detection, suspicious reconnect patterns, credential rotation, and OTA violations.

Actor model:

```text
IamUserActor
IamServiceActor
DeviceActor
SystemActor
```

`DeviceActor` is created by AIoT device authentication. `IamUserActor` and `IamServiceActor` come from appbase IAM. Audit records normalize all of them through `IamActorPort` or an equivalent AIoT actor normalization port.

## 13. High Availability And High Concurrency

Target runtime properties:

- Horizontally scalable gateway nodes.
- Stateless HTTP API nodes.
- Redis-backed online session lease.
- Event stream for cross-node command routing and event publication.
- Postgres for core facts.
- Optional ClickHouse/Timescale for high-volume telemetry.
- Bounded per-session queues.
- Backpressure and timeout policy for device commands.
- Graceful shutdown draining active sessions where possible.
- Node crash recovery through lease expiration and command timeout.

Command routing:

```text
API command request
-> domain command aggregate
-> command delivery record
-> locate online session owner
-> local session actor or cross-node route
-> adapter encode
-> device ack/result
-> command result update
-> outbox event
```

Correctness must not depend on sticky sessions. Stickiness is an optimization.

## 14. Observability

Required logs and metrics:

- Connection accepted/closed count.
- Active sessions by adapter, tenant, product.
- Handshake latency and failures.
- Auth failures.
- Decode/encode failures.
- Command dispatch latency.
- Command timeout count.
- Telemetry ingest count.
- OTA check/deployment count.
- Backpressure drops/rejections.
- Frame size violations.
- Protocol dead-letter count.

Trace fields:

```text
trace_id
tenant_id
organization_id
product_id
device_id
session_id
connection_id
adapter_id
protocol_id
message_class
semantic_type
command_id
```

Sensitive payloads must not be logged.

## 15. Implementation Roadmap After Spec Approval

Implementation should be split into milestones:

1. Workspace and contract baseline.
2. Composable runtime builder and component manifests.
3. IAM association field and resolved context mapping.
4. Core DDD model and repository traits.
5. Protocol envelope, adapter traits, runtime builder.
6. SQLx storage contracts and initial migrations.
7. Xiaozhi WebSocket adapter.
8. Xiaozhi OTA/provisioning compatibility.
9. Backend/app OpenAPI contracts.
10. Generated SDKs.
11. Runtime gateway service assembly.
12. Verification tests and contract checks.

No implementation should begin until this spec is reviewed and accepted.

## 16. Self Review

### Accepted

- The design treats xiaozhi as a plugin, not the core architecture.
- The protocol abstraction is layered enough for WebSocket, socket, MQTT, UDP, CoAP/LwM2M, Matter, Modbus, OPC UA, and bridge-style integrations.
- The library-first constraint is explicit.
- The same runtime supports embedded library mode and standalone server mode.
- IAM ownership is correctly delegated to sdkwork-appbase; AIoT only stores association fields and receives resolved request context.
- SDKWork API, SDK, security, event, performance, observability, and database rules are reflected.
- DDD boundaries separate device registry, session runtime, command lifecycle, telemetry, twin, OTA, and protocols.
- Database design avoids a single universal message table.

### Risks

- The first implementation slice can still become too large if MQTT+UDP, telemetry history, OTA rollout, and xiaozhi WebSocket are attempted together.
- Plugin ABI stability must be decided later. In-process Rust traits are enough for first-party plugins; external dynamic plugins or WASM plugins need a separate design.
- MQTT should likely be broker-bridge first, not custom broker first.
- High-volume telemetry requires a storage decision beyond Postgres before production scale.
- MCP bridge must be permissioned carefully because tool invocation can become remote command execution.
- Component boundaries must be guarded by tests so service binaries do not accumulate domain logic over time.
- IAM association must avoid hard dependencies on appbase internal implementation details; use resolved context and stable association fields only.

### Required Follow-Up Decisions

1. Confirm P0 should implement only xiaozhi WebSocket first, with MQTT+UDP deferred to P1.
2. Confirm Postgres + Redis are the P0 persistence/runtime baseline.
3. Confirm plugin model starts with in-process Rust crates, not dynamic loading.
4. Confirm OpenAPI and generated TypeScript SDKs are required in the first implementation milestone.
5. Confirm whether app API should expose end-user device control in P0 or defer most user-facing APIs to backend/admin first.
6. Confirm AIoT permission codes should be registered into appbase IAM as `iot.*`.
7. Confirm embedded library mode should target Axum route mounting first, with other host integrations later.

## 17. Implementation Review On 2026-05-31

The first standard foundation slice has been implemented as code and tests.

Implemented:

- Rust workspace with component crates and standalone service shells.
- Library-first `AiotRuntimeBuilder`, `AiotRuntime`, `AiotIntegrationBundle`, `AiotConfig`, storage/protocol/http/gateway/health bundle contracts.
- SDKWork component discovery files in `specs/README.md` and `specs/component.spec.json`.
- Contract crate with AIoT domain record, API surface records, permission catalog, appbase IAM association context, ownership references, and component manifest.
- Protocol crate with transport-neutral envelope, message classes, handshake context, adapter/codec traits, protocol catalog, plugin scopes, and capability bridge abstractions.
- Core DDD slice with Product, Device, HardwareProfile, ProtocolProfile, CapabilityDefinition, and DeviceCommand lifecycle.
- Storage table catalog with complete `iot_` table contracts and no appbase IAM-owned tables.
- SQL migration catalog with initial DDL coverage for all standard `iot_` tables and no IAM hard foreign keys.
- Device security principal and auth-level model.
- Observability trace fields and sensitive header redaction.
- Xiaozhi adapter manifest, routes, headers, and compatibility message class mapping.
- Transport crate with pure Rust HTTP health/ready response handling, WebSocket upgrade handshake, basic frame decoding, and a `TransportServer` assembled from the shared runtime.
- Gateway binary starts the standard transport server by default and exposes `/healthz`, `/readyz`, and `/iot/xiaozhi/ws` upgrade handling without introducing service-owned domain logic.
- App/backend OpenAPI source contracts, SDK generation manifests, SDK assembly manifests, and reserved TypeScript SDK package boundaries.
- Architecture guard tests for IAM non-ownership, dependency direction, service-shell runtime reuse, SDK/OpenAPI artifacts, and generated SDK boundary.

Reviewed as aligned:

- Xiaozhi remains a protocol plugin, not the core domain.
- Embedded and standalone modes use the same runtime assembly entrypoints.
- AIoT does not create `sdkwork-aiot-iam`, IAM APIs, or IAM tables.
- Protocol and hardware abstractions cover xiaozhi plus MQTT, CoAP/LwM2M, Matter, Zigbee2MQTT, LoRaWAN, Modbus, OPC UA, ESPHome, Tasmota, and OpenBeken at catalog/adapter-contract level.
- SDKWork API prefixes, generated SDK source-of-truth, dual-token security declaration, RFC 9457 problem details, and dotted operationId rules are represented in OpenAPI contracts.

Remaining next-stage implementation work:

- Production-grade async HTTP/WebSocket server integration and route mounting.
- Concrete repository implementations over SQLx pools.
- Device authentication validators, HMAC/mTLS verification, credential rotation, and rate limiting.
- Actual xiaozhi WebSocket frame decode/encode and OTA response handling.
- Command routing, session actors, Redis/NATS/Kafka integration, and backpressure behavior.
- Generated TypeScript SDK materialization from OpenAPI instead of placeholder package boundaries.
- Contract validation tooling for OpenAPI and SDK generation in CI.
