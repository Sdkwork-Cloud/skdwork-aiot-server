# SDKWork AIoT Server Component Specs

This component is the SDKWork AIoT server foundation. It is designed as a reusable Rust component first and service binaries second.

## Boundary

- Domain: `iot`
- Database prefix: `iot_`
- App API prefix: `/app/v3/api/iot`
- Backend API prefix: `/backend/v3/api/iot`
- Device compatibility prefix: `/iot/xiaozhi`
- IAM owner: external `sdkwork-appbase`

AIoT stores only IAM association fields such as `tenant_id`, `organization_id`, `user_id`, `owner_type`, `owner_id`, `created_by`, and `updated_by`. It must not create IAM tables, IAM APIs, or a parallel IAM crate.

## Integration Modes

- Embedded library: host applications build `AiotRuntime` and mount selected routes/listeners.
- Standalone server: `services/sdkwork-aiot-*` binaries assemble the same runtime builder and expose configured routes.
- Plugin protocol: xiaozhi and future protocols register as protocol adapters; core domain models stay protocol-neutral.

## Protocol Plugin Standard

Every device protocol implementation is a plugin. Xiaozhi is the first compatibility plugin, not a core special case.

`ProtocolAdapterManifest` is the canonical plugin contract. A plugin must declare:

- `scope`: standard adapter, compatibility plugin, or bridge adapter.
- `protocol_ids`: stable protocol identifiers such as `xiaozhi.websocket`, `mqtt.v5`, `coap.lwm2m`, or `modbus.bridge`.
- `transports`: WebSocket, TCP, UDP, MQTT, CoAP, serial, BLE, HTTP, or future bindings.
- `codecs`: JSON text, JSON-RPC, binary media, binary payload, protobuf, CBOR, topic payload, or register map.
- `session_policies`: stateful device sessions, stateless uplink, broker sessions, bridge sessions, or gateway-multiplexed sessions.
- `capability_bridges`: mapping rules from protocol payloads to SDKWork semantic capabilities.
- `security_modes`: device auth modes such as bearer token, HMAC, mTLS, X.509, broker credential, or bridge trust.
- `hardware_families`, `runtime_profiles`, and `firmware_profiles`: compatibility metadata for chips, SDKs, RTOS/runtime stacks, and firmware ecosystems.

Adapters may decode protocol frames into `ProtocolEnvelope` and encode `ProtocolEnvelope` back to transport frames. They must not own database writes, IAM user/session logic, or app/backend API handlers.

Runtime registration uses the manifest plus `AiotProtocolRoute` metadata. The runtime maps protocol-neutral `MessageClass` values into standard ingest actions and then into core domain ingest plans. Storage implementations consume repository/outbox/dead-letter ports, not plugin-specific payloads.

## Runtime Topology

- Machine contract: `topology.spec.json` (`schemaVersion: 2`, archetype `application-rest-edge-device`)
- Profile env: `../configs/topology/*.env`
- Human summary: `../docs/topology-standard.md`
- Dev entry: `pnpm dev`

## Canonical Specs

This component narrows the root SDKWork standards:

- `API_SPEC.md`
- `SDK_SPEC.md`
- `DOMAIN_SPEC.md`
- `DATABASE_SPEC.md`
- `COMPONENT_SPEC.md`
- `SECURITY_SPEC.md`
- `OBSERVABILITY_SPEC.md`
- `TEST_SPEC.md`
- `WEB_FRAMEWORK_SPEC.md`
- `WEB_BACKEND_SPEC.md`
- `DEPLOYMENT_SPEC.md`
- `GITHUB_WORKFLOW_SPEC.md`
- `DRIVE_SPEC.md`
- `APP_RUNTIME_TOPOLOGY_SPEC.md`

Standards alignment phases: `../docs/adr/004-standards-alignment-roadmap.md`

## Drive Integration

Client firmware and media uploads use `@sdkwork/drive-app-sdk` (`client.uploader.*`) from `@sdkwork/aiot-pc-core`. AIoT backend APIs accept Drive-backed `MediaResource` references (`source: drive`) and never expose duplicate upload endpoints. Verify with `pnpm check:drive-standard`.

## Project Structure

- Shared Rust libraries live under `crates/`, including contracts, protocol, runtime, storage, security, observability, transport, HTTP routers, architecture checks, and the `sdkwork-aiot-adapter-xiaozhi` compatibility plugin.
- Runnable services live under `services/`: `sdkwork-aiot-cloud-gateway`, `sdkwork-aiot-admin-api`, and `sdkwork-aiot-app-api`.
- Tests are colocated in each crate or service under `tests/`, usually with `*_standard.rs` names.
- Generated or packaged SDK artifacts live under `sdks/`; design and planning notes live under `docs/`.
- The `external/` tree contains reference projects and submodules and should not be edited for normal product changes.

## Build, Test, and Development Commands

- `pnpm dev`: topology-aware dev entry for the default `standalone.development` profile.
- `pnpm dev:server:sqlite:cloud`: cloud deployment profile dev workflow.
- `pnpm check`: workspace standard, database, API, SDK, topology, drive, api-envelope, pagination, app-sdk-consumer-imports, Rust fmt, and clippy gates.
- `pnpm verify`: `pnpm check` plus `cargo test --workspace`.
- `pnpm release:build` / `pnpm release:package` / `pnpm release:validate` / `pnpm release:publish` / `pnpm release:preflight`: server release binaries, CDN-aligned archives, SBOM evidence, and unified preflight gate.
- `pnpm test:topology-validate`: validate `specs/topology.spec.json`.
- `pnpm test:topology-baggage`: scan active paths for retired topology vocabulary.
- `cargo build --workspace`: compile all workspace crates and services.
- `cargo test -p sdkwork-aiot-cloud-gateway`: run tests for one package.
- Optional persistent device DB: `$env:SDKWORK_AIOT_DEVICE_DB_PATH='D:\\data\\aiot-device.db'`.

## Testing Guidelines

Use Rust integration tests in each package's `tests/` directory. Name test files by behavior or standard surface, for example `xiaozhi_standard.rs`, `gateway_standard.rs`, or `transport_standard.rs`. Add focused tests for protocol compatibility, gateway routing, adapter parsing, and error cases before changing behavior. Run the narrow package test first, then `cargo test --workspace` before opening a pull request.

## Security and Configuration

Do not commit real device tokens, broker credentials, certificates, or local bind secrets. Prefer environment variables for service configuration. When changing Xiaozhi access paths, verify both real-device headers and browser simulator query-parameter compatibility.
