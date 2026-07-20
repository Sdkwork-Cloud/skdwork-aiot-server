# sdkwork-aiot
repository-kind: application

## SDKWork Standards Alignment

Standards alignment status and migration phases are tracked in `docs/adr/004-standards-alignment-roadmap.md`.

Quick verification:

```bash
pnpm check
pnpm verify
pnpm deploy:validate
pnpm release:preflight
```

Production release: [docs/runbooks/production-release.md](docs/runbooks/production-release.md).
The application remains `DRAFT`; release phases require an explicit target or deployment profile,
and publish/deploy stay fail-closed without immutable artifact evidence and approvals.

Root directory dictionary (active capabilities):

| Directory | Purpose |
| --- | --- |
| `apis/` | HTTP API contract index and regeneration notes |
| `apps/` | PC, H5, mini program, and shared client surfaces |
| `crates/` | Rust libraries and HTTP API component |
| `services/` | Device protocol edge runtime and development simulator |
| `sdks/` | OpenAPI authorities, route manifests, generated SDK families |
| `etc/` | Source configuration and topology profile env templates |
| `deployments/` | Deployment profiles and release handoff (`deploy.yaml`) |
| `scripts/` | Contract, release, and verification tooling |
| `docs/` | ADRs, topology, production readiness |
| `specs/` | Component and topology contracts |
| `tests/` | Cross-package test index |
| `jobs/`, `tools/`, `plugins/`, `examples/` | Reserved capability placeholders |

Inactive standard directories are documented rather than omitted without explanation.

## Development

Default profile: `standalone.development` (`specs/topology.spec.json`).

```bash
pnpm dev
```

Cloud development profile:

```bash
pnpm dev:cloud
```

Run the Xiaozhi simulator UI in a second terminal when needed:

```bash
cargo run -p sdkwork-aiot-xiaozhi-simulator-ui
```

See `docs/topology-standard.md` and `specs/topology.spec.json`.

## Xiaozhi Gateway Simulator

The repository includes a cross-platform terminal UI simulator for local
Xiaozhi compatibility checks.

`pnpm dev` starts the canonical standalone API gateway, device edge runtime,
and client surfaces from `standalone.development`. The simulator remains an
explicit development process.

The simulator exercises the same compatibility surface used by ESP32 firmware:

- OTA metadata: `POST /iot/xiaozhi/ota`
- WebSocket session route: `/iot/xiaozhi/ws`
- Xiaozhi handshake headers or browser query parameters:
  `Protocol-Version`, `Device-Id`, `Client-Id`, `Authorization`
- Device-to-server messages: `hello`, `listen`, `abort`, `mcp`, binary Opus
  frames
- Server-to-device responses: server `hello`, `stt`, `llm`, `tts`, MCP
  `initialize`, MCP `tools/list`

Default simulator env overrides:

- `SDKWORK_AIOT_EDGE_DEVICE_INGRESS_HTTP_URL`
- `SDKWORK_AIOT_XIAOZHI_SIMULATOR_PROTOCOL_VERSION`
- `SDKWORK_AIOT_XIAOZHI_SIMULATOR_DEVICE_ID`
- `SDKWORK_AIOT_XIAOZHI_SIMULATOR_CLIENT_ID`
- `SDKWORK_AIOT_XIAOZHI_SIMULATOR_TOKEN`

## Xiaozhi Activation + MCP Config

The device edge runtime supports a restart-safe activation challenge registry and
optional simulator MCP tool catalog override.

Activation challenge registry:

- `SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_PATH`: optional file path used to
  persist OTA-issued activation challenges.
- `SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_KIND`: optional backend selector:
  `file` (default when path is set), `sqlite`, or `redis`.
- `SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_REDIS_URL`: Redis connection URL
  used when kind is `redis`, for example `redis://127.0.0.1:6379/0`.
- `SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_REDIS_PREFIX`: optional Redis key
  prefix (default `sdkwork:aiot:xiaozhi:activation`).
- `SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_LOCK_WAIT_MILLIS`: lock wait
  timeout for shared registry file (default `2000`).
- `SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_LOCK_POLL_MILLIS`: lock retry poll
  interval (default `20`).
- `SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_LOCK_STALE_MILLIS`: stale lock file
  eviction threshold (default `30000`).
- If no durable backend is configured, the registry is in-memory only (old behavior).

Optional integration test with real Redis:

- Set `SDKWORK_AIOT_DEVICE_EDGE_TEST_REDIS_URL=redis://127.0.0.1:6379/0` before
  running `cargo test -p sdkwork-aiot-device-edge-runtime -- --nocapture --test-threads=1`
  to enable the Redis-backed end-to-end activation registry test.

Simulator MCP tool catalog:

- `SDKWORK_AIOT_XIAOZHI_SIMULATOR_MCP_TOOLS_PATH`: optional JSON file path for
  overriding built-in simulator tools.
- `SDKWORK_AIOT_XIAOZHI_MCP_POLICY_RULES`: optional inline policy rules for MCP
  tool allow/deny decisions.
- `SDKWORK_AIOT_XIAOZHI_MCP_POLICY_DENY_BY_DEFAULT`: when set to `1/true/yes/on`,
  reject MCP tool calls that do not match an explicit allow rule (production
  topology profiles enable this by default).
- `SDKWORK_AIOT_XIAOZHI_MCP_POLICY_LOG_ALLOW`: when set to `1/true/yes/on`,
  emit allow decision logs for all MCP calls. By default, allow logs are
  emitted only when a concrete policy rule matched; deny logs are always
  emitted.
- Supported JSON shapes:
  - object root: `{ "tools": [ ... ] }`
  - array root: `[ ... ]`
- Tool entry fields: `name`, `description`, `inputSchema`, `userOnly`,
  optional `resultText`.
- If loading fails or the file is empty, the device edge runtime falls back to built-in tools.

Policy rule format:

- Rules are `;`-separated.
- Each rule is `allow|...` or `deny|...`.
- Supported predicates: `tool=<name>`, `transport=<websocket|mqtt>`,
  `device_prefix=<prefix>`, `client_prefix=<prefix>`.
- Numeric argument predicates are also supported:
  `arg_<field>_gt=<n>`, `arg_<field>_gte=<n>`, `arg_<field>_lt=<n>`,
  `arg_<field>_lte=<n>`, `arg_<field>_eq=<n>`, `arg_<field>_ne=<n>`.
- String argument predicates:
  `arg_<field>_str_eq=<text>`, `arg_<field>_str_ne=<text>`,
  `arg_<field>_str_prefix=<prefix>`.
- Boolean argument predicates:
  `arg_<field>_bool_eq=true|false`, `arg_<field>_bool_ne=true|false`.
- First matching rule wins. If no rule matches, tool call is allowed.
- Rule index in logs is zero-based.

Example:

```text
deny|tool=self.reboot|transport=websocket;allow|tool=self.reboot|transport=websocket|device_prefix=lab-
```

Numeric threshold example:

```text
deny|tool=self.audio_speaker.set_volume|transport=websocket|arg_volume_gt=80
```

For explicit assembly in custom bootstraps/tests, the device edge runtime exposes
`standard_device_edge_server_with_plugins_activation_registry_and_mcp_tools(...)`
to inject OTA provider, activation verifier, activation registry, and MCP tool
provider together.

When the bootstrap also needs to reuse the exact injected MCP provider across
long-running session loops, use
`standard_device_edge_server_and_session_options_with_plugins_activation_registry_and_mcp_tools(...)`
and pass the returned session options into the option-aware WS/MQTT helpers.

For long-running WebSocket/MQTT loops, device edge session handlers can also reuse a
preloaded provider via `XiaozhiSessionOptions`:

- `XiaozhiSessionOptions::from_env()`: load once from env/file fallback.
- `XiaozhiSessionOptions::from_mcp_tool_provider_and_invoker(...)`: inject both
  tool catalog and custom execution layer for plugin-style tool call handling.
- `XiaozhiSessionOptions::from_mcp_tool_provider_invoker_and_policy(...)`:
  additionally inject authorization policy hooks before tool execution.
- `xiaozhi_websocket_session_reply_with_options(...)`: websocket reply path with
  injected options.
- `xiaozhi_mqtt_session_reply_with_options(...)`: MQTT reply path with injected
  options.

Rule-based policy implementations also expose lightweight decision counters via
`RuleBasedXiaozhiSimulatorMcpToolPolicy::stats_snapshot()`:

- `allow_by_rule_matches`
- `allow_no_rule_matches`
- `deny_by_rule_matches`

Gateway process endpoint for runtime visibility:

- `GET /internal/xiaozhi/mcp-policy/stats`: returns current rule-based MCP
  policy counters when the active session policy supports stats.
  - `{"policy":"rule_based",...}` when default rule-based policy is active.
  - `{"policy":"custom","stats_available":false}` when a custom policy does
    not expose counters.

## MQTT + UDP Bridge (Optional)

The device edge runtime can run an optional MQTT+UDP compatibility bridge for
`xiaozhi.mqtt_udp` flows.

Enable it:

```powershell
$env:SDKWORK_AIOT_DEVICE_EDGE_MQTT_BRIDGE_ENABLE='1'
cargo run -p sdkwork-aiot-device-edge-runtime
```

Key runtime knobs:

- `SDKWORK_AIOT_DEVICE_EDGE_MQTT_HOST` / `SDKWORK_AIOT_DEVICE_EDGE_MQTT_PORT`
- `SDKWORK_AIOT_DEVICE_EDGE_MQTT_SUBSCRIBE_TOPIC` / `..._PUBLISH_TOPIC`
- `SDKWORK_AIOT_DEVICE_EDGE_MQTT_RECONNECT_BASE_MILLIS` / `..._MAX_MILLIS`
- `SDKWORK_AIOT_DEVICE_EDGE_MQTT_PUBLISH_RETRY_ATTEMPTS` / `..._RETRY_DELAY_MILLIS`
- `SDKWORK_AIOT_DEVICE_EDGE_MQTT_MAX_OUTBOUND_PER_EVENT`
- `SDKWORK_AIOT_DEVICE_EDGE_MQTT_PUBLISH_DROP_COOLDOWN_MILLIS`
- `SDKWORK_AIOT_DEVICE_EDGE_UDP_BIND`
- `SDKWORK_AIOT_DEVICE_EDGE_SESSION_IDLE_TIMEOUT_SECONDS`
- `SDKWORK_AIOT_DEVICE_EDGE_BRIDGE_STATS_LOG_INTERVAL_SECONDS`

Behavior:

- MQTT reconnect uses exponential backoff with a cap.
- Publish failures are retried with bounded attempts.
- Per-event outbound publish fan-out is bounded (excess payloads are dropped and counted).
- UDP session state is purged after idle timeout.
- Bridge health counters are periodically logged to stderr.
- Bridge runtime health can be pulled via `GET /internal/bridge/health`.
- Bridge counters can be pulled via `GET /internal/bridge/stats`.
- Prometheus-style metrics can be pulled via `GET /internal/bridge/metrics`.

## SDKWork Documentation Contract

Domain: device
Capability: aiot-runtime
Package type: rust-crate
Status: standard

### Public API

Public exports are declared in `specs/component.spec.json` under `contracts.publicExports`.

### Required SDK Surface

- `@sdkwork/aiot-app-sdk`
- `@sdkwork/aiot-backend-sdk`

### Configuration

Configuration keys and runtime entrypoints are declared in `specs/component.spec.json`.

### Deployment Profile Behavior

This module follows the canonical `standalone` and `cloud` deployment profile rules linked from
`specs/component.spec.json`.

### Security

Do not add secrets, live tokens, manual auth headers, or app-local credential handling to this module.

### Extension Points

Extension points are limited to declared public exports, runtime entrypoints, SDK clients, events, and config keys.

### Verification

- `cargo fmt -- --check`
- `cargo test --workspace`
- `cargo check --workspace`

### Owner And Status

Owner and lifecycle status are tracked in `specs/component.spec.json`.

## Documentation Canon

- [docs/README.md](docs/README.md)
- [docs/product/prd/PRD.md](docs/product/prd/PRD.md)
- [docs/architecture/tech/TECH_ARCHITECTURE.md](docs/architecture/tech/TECH_ARCHITECTURE.md)

## Application Roots

- [apps directory index](apps/README.md)
