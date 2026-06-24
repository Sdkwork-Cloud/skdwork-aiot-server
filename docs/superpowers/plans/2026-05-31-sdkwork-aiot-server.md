# SDKWork AIoT Server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first standard, componentized Rust foundation for SDKWork AIoT Server: library-first runtime, protocol abstractions, IAM association context, component manifests, xiaozhi plugin manifest, and independently startable service shells.

**Architecture:** The implementation starts with reusable crates, not service binaries. The same `AiotRuntimeBuilder` must support embedded integration and standalone server assembly. IAM remains owned by `sdkwork-appbase`; AIoT only models resolved request context and association fields.

**Tech Stack:** Rust 2021, Cargo workspace, Tokio, Serde, thiserror, uuid, chrono, Axum for service shells only.

---

## File Structure

- Create `Cargo.toml`: workspace, dependency versions, crate members.
- Create `crates/sdkwork-aiot-contract`: common contracts, IDs, request context, component manifest, permission/path/event constants.
- Create `crates/sdkwork-iot-device-service`: DDD entities and command lifecycle.
- Create `crates/sdkwork-aiot-protocol`: protocol envelope, adapter traits, handshake, codec frame types.
- Create `crates/sdkwork-aiot-service-host`: componentized runtime builder, embedded/standalone assembly surface.
- Create `crates/sdkwork-aiot-storage`: storage traits and table catalog contract.
- Create `crates/sdkwork-aiot-storage-sqlx`: migration catalog text and SQL table constants.
- Create `crates/sdkwork-aiot-security`: device principal and auth-level model.
- Create `crates/sdkwork-aiot-observability`: safe trace/log field model and redaction helpers.
- Create `crates/sdkwork-aiot-adapter-xiaozhi`: xiaozhi plugin manifest and compatibility constants.
- Create `services/sdkwork-aiot-cloud-gateway`: standalone gateway shell using runtime builder.
- Create `services/sdkwork-aiot-admin-api`: backend API shell.
- Create `services/sdkwork-aiot-app-api`: app API shell.

## Task 1: Workspace And Red Tests

**Files:**
- Create: `Cargo.toml`
- Create: crate `Cargo.toml` and `src/lib.rs` placeholders
- Test: crate-level `tests/*_standard.rs`

- [x] **Step 1: Create workspace manifests and empty library crates.**
- [x] **Step 2: Write failing tests for contracts, protocol manifest, runtime composition, storage catalog, security principal, and xiaozhi manifest.**
- [x] **Step 3: Run `cargo test --workspace` and verify tests fail because required types/functions are missing.**

## Task 2: Contract Crate

**Files:**
- Modify: `crates/sdkwork-aiot-contract/src/lib.rs`
- Test: `crates/sdkwork-aiot-contract/tests/contract_standard.rs`

- [x] **Step 1: Implement value objects, `AiotRequestContext`, `AiotActorRef`, `AiotOwnershipRef`, `AiotComponentManifest`, permissions, routes, and events.**
- [x] **Step 2: Run contract crate tests and verify they pass.**

## Task 3: Protocol Crate

**Files:**
- Modify: `crates/sdkwork-aiot-protocol/src/lib.rs`
- Test: `crates/sdkwork-aiot-protocol/tests/protocol_standard.rs`

- [x] **Step 1: Implement protocol envelope, message classification, handshake context, adapter manifest, adapter/codec traits, and frame types.**
- [x] **Step 2: Run protocol crate tests and verify they pass.**

## Task 4: Runtime Crate

**Files:**
- Modify: `crates/sdkwork-aiot-service-host/src/lib.rs`
- Test: `crates/sdkwork-aiot-service-host/tests/runtime_standard.rs`

- [x] **Step 1: Implement `AiotRuntimeBuilder`, `AiotRuntime`, component registry, embedded bundle, standalone bundle, health status, and configuration checks.**
- [x] **Step 2: Run runtime crate tests and verify they pass.**

## Task 5: Domain, Storage, Security, Observability

**Files:**
- Modify: `crates/sdkwork-iot-device-service/src/lib.rs`
- Modify: `crates/sdkwork-aiot-storage/src/lib.rs`
- Modify: `crates/sdkwork-aiot-storage-sqlx/src/lib.rs`
- Modify: `crates/sdkwork-aiot-security/src/lib.rs`
- Modify: `crates/sdkwork-aiot-observability/src/lib.rs`
- Test: corresponding crate tests

- [x] **Step 1: Implement Product, Device, DeviceCommand lifecycle, table catalog, SQL migration catalog, device principal, and safe trace fields.**
- [x] **Step 2: Run focused tests and verify they pass.**

## Task 6: Xiaozhi Adapter Manifest

**Files:**
- Modify: `crates/sdkwork-aiot-adapter-xiaozhi/src/lib.rs`
- Test: `crates/sdkwork-aiot-adapter-xiaozhi/tests/xiaozhi_standard.rs`

- [x] **Step 1: Implement xiaozhi constants, manifest, base routes, supported protocol ids, headers, message type mapping, and OTA compatibility metadata.**
- [x] **Step 2: Run xiaozhi adapter tests and verify they pass.**

## Task 7: Service Shells

**Files:**
- Modify: `services/sdkwork-aiot-cloud-gateway/src/main.rs`
- Modify: `services/sdkwork-aiot-admin-api/src/main.rs`
- Modify: `services/sdkwork-aiot-app-api/src/main.rs`

- [x] **Step 1: Implement minimal standalone binaries that build the same runtime components.**
- [x] **Step 2: Run `cargo check --workspace` and verify service shells compile.**

## Task 8: Final Verification

- [x] **Step 1: Run `cargo fmt --all -- --check`.**
- [x] **Step 2: Run `cargo test --workspace`.**
- [x] **Step 3: Run `cargo check --workspace`.**
- [x] **Step 4: Report exact verification results and remaining gaps.**

## Additional Executed Standard Guardrails

- [x] Added `crates/sdkwork-aiot-architecture` to verify component discovery, SDKWork local specs, IAM non-ownership, service-shell reuse of runtime builder, SDK/OpenAPI artifacts, TypeScript SDK generated-boundary placeholders, and dependency direction.
- [x] Added `specs/README.md` and `specs/component.spec.json` for SDKWork component discovery.
- [x] Added app/backend OpenAPI source contracts, SDK generation manifests, SDK assembly manifests, and generated SDK placeholder package boundaries.
- [x] Added complete `iot_` table contract coverage and initial SQL migration coverage for all standard catalog tables.
- [x] Added `AiotIntegrationBundle`, `AiotConfig`, storage/protocol/http/gateway/health bundle contracts for fast embedded integration and standalone assembly.
- [x] Added `crates/sdkwork-aiot-transport` with pure Rust HTTP health/ready responses, WebSocket upgrade handshake, basic WebSocket frame decoding, and a standard transport server assembled from the shared runtime.
- [x] Updated `services/sdkwork-aiot-cloud-gateway` to start a standalone TCP HTTP gateway by default while still allowing `SDKWORK_AIOT_GATEWAY_NO_LISTEN=1` for startup checks.
- [x] Verified `sdkwork-aiot-cloud-gateway.exe` starts on a temporary port and returns `/healthz` with `ready=true`, then stops cleanly.
