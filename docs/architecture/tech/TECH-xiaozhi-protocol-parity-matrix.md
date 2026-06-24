> Migrated from `docs/xiaozhi-protocol-parity-matrix.md` on 2026-06-24.
> Owner: SDKWork maintainers

# Xiaozhi Protocol Parity Matrix

Last updated: 2026-06-17 (audited against `external/xiaozhi-esp32` @ `277cbd10`)

This matrix tracks parity between `external/xiaozhi-esp32` and this repository for the xiaozhi-compatible IoT server.

Initialize the reference tree before audits:

```bash
git submodule update --init external/xiaozhi-esp32
```

## WebSocket Control + Media

| External behavior | Local status | Notes |
| --- | --- | --- |
| WS headers (`Authorization`, `Protocol-Version`, `Device-Id`, `Client-Id`) | Implemented | Request parsing + handshake context mapping in adapter/gateway. |
| `hello` exchange (`transport:"websocket"`) | Implemented | Server hello generated with session and audio params. |
| Binary v1/v2/v3 Opus frame decode/encode | Implemented | Adapter codec supports v1 raw and v2/v3 headers; v2 JSON-in-binary passthrough supported. |
| JSON message family (`listen/abort/stt/tts/llm/alert/custom/mcp/goodbye`) | Implemented | Message class mapping + extension extraction + simulator replies. |
| `tts.state` includes `start`, `stop`, `sentence_start` | Implemented | Simulator path emits full TTS lifecycle: `start` → binary Opus downlink → `sentence_start` → `stop`. |
| `listen.state` includes `start`, `stop`, `detect` + `mode` | Implemented | Codec preserves `xiaozhi.listen.state/mode/text`. |
| `abort.reason` (`wake_word_detected`) | Implemented | Codec preserves `xiaozhi.abort.reason`. |
| WebSocket `goodbye` | N/A on device | ESP32 WS stack does not send `goodbye`; gateway accepts it for simulator/testing only. |
| Legacy `type:"iot"` | Deprecated on ESP32 main | Still mapped in adapter for older firmware; replaced by MCP on current main branch. |
| Ping/pong/close frame handling | Implemented | Gateway session loop handles control opcodes. |

## MCP JSON-RPC

| External behavior | Local status | Notes |
| --- | --- | --- |
| `type:"mcp"` wrapper | Implemented | Preserved with envelope extensions. |
| Request/response/notification/error classification | Implemented | `xiaozhi.mcp.kind` derived for routing; `notifications/*` are ignored (no automatic reply), aligned with external parser behavior. |
| JSON-RPC version guard (`jsonrpc == "2.0"`) | Implemented | Invalid or missing JSON-RPC version MCP request payloads are ignored (no reply), aligned with external parser semantics. |
| MCP request `params` shape guard | Implemented | Non-object `params` in generic MCP requests are ignored (no reply), aligned with external parser behavior; `tools/call` still returns explicit external-style errors for its documented preconditions. |
| MCP request numeric `id` on device | External constraint | ESP32 `McpServer::ParseMessage` accepts numeric IDs only; local server-side MCP parser accepts numeric and string IDs when acting as MCP server (more permissive). |
| MCP payload-only frame handling | Implemented | MCP frames with `jsonrpc`/`id` only (no `method`/`result`/`error`) are ignored without automatic `tools/list` follow-up, aligned with external parser behavior. |
| ID preservation (numeric + string) | Implemented | Correlation ID and JSON literal preserved (`xiaozhi.mcp.id_json`). |
| Initialize/tools list/tools call simulator path | Implemented (simulator grade) | Supports `initialize`, `tools/list`, `tools/call`, string/numeric IDs, and unknown-method errors. |
| Production intelligence bridge (`SDKWORK_AIOT_INTELLIGENCE_MODE=kernel`) | Implemented (integration grade) | `sdkwork-aiot-intelligence-bridge` routes speech via Claw Router ASR/TTS and agent turns via kernel runtime HTTP; MCP `tools/list` + `tools/call` use kernel session tool catalog/execute; Opus codec/uplink owned by `sdkwork-aiot-adapter-xiaozhi`. See [XIAOZHI_INTELLIGENCE_INTEGRATION.md](architecture/XIAOZHI_INTELLIGENCE_INTEGRATION.md). |
| `tools/list` cursor pagination and `withUserTools` | Implemented (simulator grade) | Cursor-based paging and user-only tool visibility toggling are available in simulator reply path. |
| Simulator MCP tool provider override | Implemented (simulator grade) | Optional file-driven catalog via `SDKWORK_AIOT_XIAOZHI_SIMULATOR_MCP_TOOLS_PATH`, with built-in fallback. |
| `tools/call` precondition + argument validation | Implemented (simulator grade) | Precondition errors align with external (`Missing params`, `Missing name`, `Invalid arguments`); required args return `Missing valid argument: <name>` for missing/type mismatch; integer range violations return external-style errors (`Value is below minimum allowed: <min>`, `Value exceeds maximum allowed: <max>`); integer inputs accept JSON numbers and truncate toward int semantics before range checks. |

## OTA + Activation HTTP

| External behavior | Local status | Notes |
| --- | --- | --- |
| `/iot/xiaozhi/ota` response with websocket profile | Implemented | Includes `websocket.url/token/version`. |
| `/iot/xiaozhi/ota/activate` alias for activation | Implemented | Added compatibility alias because external firmware computes activate URL by appending `/activate` to OTA URL (`.../ota/activate`). |
| OTA `mqtt` section | Implemented | Env-driven endpoint/client_id/credentials/topics/keepalive. |
| OTA `udp` section (`server/port/key/nonce`) | Implemented (server-side extension) | Env-driven; current ESP32 `ota.cc` does not parse top-level `udp` (UDP profile comes from MQTT hello), but local OTA response supports it for deployments that pre-provision UDP. |
| OTA `firmware`, `activation`, `server_time` sections | Implemented | Env-driven and tested. |
| `/iot/xiaozhi/activate` pending/accepted flow | Implemented | Issue/consume lifecycle with timeout + replay rejection; verifier remains pluggable. |
| Activation-Version 2 payload fields (`algorithm`, `serial_number`, `challenge`, `hmac`) | Implemented (opt-in strict mode) | `SDKWORK_AIOT_XIAOZHI_ACTIVATE_STRICT_V2=1` enforces v2 field presence, `algorithm=hmac-sha256`, and header/body serial alignment; non-v2 requests keep legacy compatibility. |
| Activation challenge persistence | Implemented (pluggable durable backends) | `SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_PATH` supports durable restart-safe registry. Default file-backed mode remains available; optional SQLite mode (`SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_KIND=sqlite`) provides transactional register/consume semantics. Optional Redis mode (`SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_KIND=redis` + `SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_REDIS_URL`) supports shared, distributed challenge consumption semantics. |
| Activation registry multi-process/node safety | Implemented (single-host + shared-db option) | File mode uses lock-file coordination (wait/poll/stale controls). SQLite mode uses transactional prune+consume on shared DB path, enabling stronger cross-process consistency under shared-storage deployments. |

## MQTT + UDP Hybrid

| External behavior | Local status | Notes |
| --- | --- | --- |
| MQTT hello (`transport:"udp"`) decode | Implemented | `XiaozhiMqttCodec` decodes + runtime pipeline. |
| MQTT server hello with UDP crypto profile | Implemented | `XiaozhiServerHello::mqtt_udp(...)`. |
| Device-initiated MQTT `goodbye` (no echo) | Implemented | Gateway closes UDP session without replying `goodbye`; ignores mismatched `session_id`, aligned with `mqtt_protocol.cc`. |
| Server-initiated MQTT `goodbye` | Implemented (simulator grade) | `xiaozhi_mqtt_server_teardown_reply` emits `goodbye` with `close_audio_channel`; bridge removes session on teardown. Operator control: `GET /internal/bridge/sessions`, `DELETE /internal/bridge/sessions/{session_id}`. |
| Bridge session control API | Implemented | Internal endpoints list active MQTT+UDP bridge sessions and disconnect by `session_id`, publishing server-initiated `goodbye` when bridge is enabled. |
| UDP uplink → speech reply path | Implemented | Simulator: immediate STT/LLM/TTS on UDP packet. Production (`kernel`): UDP packets buffer in `xiaozhi_ws_media_session`; `listen/detect` triggers ASR via buffered WAV. |
| UDP packet shape (`type/flags/len/ssrc/timestamp/sequence`) | Implemented | `XiaozhiUdpAudioCodec` encode/decode. |
| UDP AES-CTR payload encryption/decryption | Implemented | Key/nonce hex profile, deterministic tests; server outbound encoding via `XiaozhiMqttUdpSession::encode_outbound_audio`. |
| UDP server→device downlink (MQTT path) | Implemented (simulator grade) | `MqttSessionReply.outbound_udp_packets` + bridge shared UDP socket sends encrypted Opus to learned peer address. |
| Replay/stale sequence rejection | Implemented | `decode_audio_packet_with_min_sequence(...)`. |
| Main-process MQTT bridge loop | Implemented (optional) | `SDKWORK_AIOT_GATEWAY_MQTT_BRIDGE_ENABLE=1` starts MQTT+UDP worker threads. |
| MQTT reconnect and backoff policy | Implemented | Exponential reconnect backoff with env-configurable base/max window. |
| UDP session idle cleanup | Implemented | Idle sessions are purged by timeout to avoid stale audio decoding state. |
| Bridge observability counters | Implemented (lightweight) | Periodic stats logs + pull endpoints (`GET /internal/bridge/health`, `GET /internal/bridge/stats`, `GET /internal/bridge/metrics`) expose runtime state, reconnects, event errors, publish retries/failures, dropped outbound events, UDP decode failures, and idle purges. |
| MQTT publish retry policy | Implemented | Publish retries are bounded and configurable; terminal failure is logged and counted. |
| MQTT outbound fan-out guard | Implemented | Per-event outbound payload count is capped; overflow messages are dropped and counted for backpressure visibility. |
| Full broker/session orchestration hardening | Implemented (control plane) | Graceful shutdown, health/stats/metrics, bounded fan-out/retry, idle purge, and internal session disconnect API exist; distributed rate limits and trace hooks remain optional. |

## Plugin / Architecture

| External/Target behavior | Local status | Notes |
| --- | --- | --- |
| Compatibility plugin manifest includes WS+MQTT+UDP+HTTP | Implemented | `xiaozhi_manifest()`. |
| Runtime routes include WS/OTA/activate + MQTT/UDP | Implemented | `xiaozhi.websocket` and `xiaozhi.mqtt_udp` route registration. |
| Pluggable activation verifier | Implemented | `XiaozhiActivationVerifier` trait + assembly injection. |
| Pluggable OTA profile provider | Implemented | `XiaozhiOtaProfileProvider` trait + assembly injection. |
| Pluggable simulator MCP tool provider | Implemented (simulator grade) | `XiaozhiSimulatorMcpToolProvider` + env/file override + composite bootstrap entrypoints for server-only and server+session-options assembly. |
| Session-scoped MCP provider reuse for WS/MQTT loops | Implemented | `XiaozhiSessionOptions` allows one-time provider load/injection and is wired into gateway main loop + option-aware session reply APIs. |
| Pluggable MCP tool execution layer | Implemented (simulator grade) | `XiaozhiSimulatorMcpToolInvoker` decouples tool catalog from execution, supports invocation context (`transport/session/device/client`), and preserves external error envelope semantics. |
| Pluggable MCP tool authorization policy | Implemented (simulator grade) | `XiaozhiSimulatorMcpToolPolicy` adds pre-invocation allow/deny hooks; policy rejections are returned through the same external-style MCP error envelope. Default policy supports env rules (`SDKWORK_AIOT_XIAOZHI_MCP_POLICY_RULES`) with first-match predicates on tool/transport/device prefix/client prefix plus argument conditions for numeric (`arg_<field>_<op>=<number>`), string (`arg_<field>_str_eq/ne/prefix`), and boolean (`arg_<field>_bool_eq/ne`). Decision observability now includes structured policy logs (`mcp_policy_decision`) with match index and context; deny is always logged and allow can be fully logged via `SDKWORK_AIOT_XIAOZHI_MCP_POLICY_LOG_ALLOW`. Rule-based counters are exposed through `stats_snapshot()` for tests/diagnostics. |
| MCP policy runtime stats endpoint | Implemented | Gateway exposes `GET /internal/xiaozhi/mcp-policy/stats` for in-process rule-based policy counters (`allow_by_rule_matches`, `allow_no_rule_matches`, `deny_by_rule_matches`). If custom policy injection is used and stats are unsupported, endpoint returns `stats_available:false`. |
| Activation registry runtime stats endpoint | Implemented | Gateway exposes `GET /internal/xiaozhi/activation-registry/stats` for backend kind (`in_memory`/`file`/`sqlite`) and counters (`register_total`, `consume_total`, `consume_hits`, `consume_misses`, `pruned_entries`) to support operational monitoring of challenge lifecycle health. |
| Activation registry runtime metrics endpoint | Implemented | Gateway exposes `GET /internal/xiaozhi/activation-registry/metrics` (Prometheus text format) with activation lifecycle counters and backend label gauge (`sdkwork_aiot_xiaozhi_activation_registry_backend{backend=\"...\"} 1`) for direct scrape integration. The same activation metrics are also included in `GET /internal/bridge/metrics` for single-endpoint scrape compatibility. |
| Transport compatibility route closure injection | Implemented | Arc closure handlers in transport server. |
| Cross-platform simulator UI | Implemented | `sdkwork-aiot-xiaozhi-simulator-ui` provides terminal UI controls for connect/hello/listen/abort/MCP operations; legacy `/simulators/xiaozhi` browser page is retired. |

## App-API / Backend-API Contract Parity

| Target behavior | Local status | Notes |
| --- | --- | --- |
| Route-contract/OpenAPI operationId alignment | Implemented | Automated tests ensure backend/app OpenAPI `operationId` values are present in `standard_api_route_contracts()`. |
| Route-contract/OpenAPI permission alignment | Implemented | Automated tests ensure `x-sdkwork-required-permission` matches route contract permissions for backend/app APIs. |
| Tenant+organization read isolation (`devices`, `commands`, `events`, `twin`) | Implemented | End-to-end isolation tests validate same `deviceId` under different tenant/org returns only scoped data (no cross-scope leakage). |
| Backend device CRUD scope isolation | Implemented | Cross-organization tests verify create/update/delete and retrieve are scoped by `tenant_id + organization_id`; deleting one scope does not remove sibling scope records. |
| Backend device credentials lifecycle (`list/create/revoke`) | Implemented | `devices.credentials.list/create/delete` routes are mounted with tenant+organization scope checks; delete executes revoke semantics (`status=revoked`, `revokedAt`) and missing credential returns `api.device.credential.not_found`. |
| Backend control operations (`sessions.disconnect`, `commands.cancel`) | Implemented | `devices.sessions.disconnect` and `devices.commands.cancel` are mounted with explicit IAM permissions (`iot.sessions.disconnect`, `iot.commands.cancel`), update runtime state, and enforce scoped 404 behavior (`api.device.not_found`, `api.device.session.not_found`, `api.command.not_found`). |
| Command idempotency scope | Implemented (tenant+organization) | Idempotency dedup for command create is scoped by `(tenant_id, organization_id, idempotency_key)` in in-memory/sqlite repositories and migration DDL. |
| Backend mutation error semantics (`devices.create/update/delete`, `devices.credentials.create`) | Implemented | Contract tests enforce standard mutation error mapping: `400` for invalid JSON/invalid fields, `403` for permission mismatch, `404` for scoped resource not found, `409` for duplicate identity conflicts. |
| App/API & Backend/API problem+json error envelope parity | Implemented | Core failure paths on both app and backend surfaces enforce `application/problem+json` plus stable fields (`type`, `title`, `status`, `code`), with route-specific extensions such as `requiredPermission` and `deviceId` preserved. |
| OpenAPI Problem response declaration vs runtime error status behavior | Implemented | Contract tests validate each surface defines `components.responses.Problem` as `application/problem+json`, and that observed runtime 4xx/409 error statuses are covered by explicit response codes or `default: #/components/responses/Problem` on corresponding operations. |
| TypeScript SDK ProblemDetails parity (app/backend) | Implemented | Both TS SDKs export `ProblemDetails`, `isProblemDetails`, and `normalizeProblemDetails` aligned with OpenAPI `components.schemas.ProblemDetails` required fields (`type`, `title`, `status`) and optional extension fields (`detail`, `traceId`, `code`) while preserving additional properties; both also export stable problem-code catalogs and guards (`SDKWORK_AIOT_*_PROBLEM_CODES`, `isSdkworkAiot*ProblemCode`) covering core runtime API error codes. |

## Open Gaps (Next Iteration)

1. Distributed bridge rate limits and richer trace hooks (control plane exists; advanced backpressure optional).
2. Multi-node activation challenge coordination hardening for distributed, non-shared-file deployments (managed DB/Redis backend and operational guidance).
3. Production MCP auth hardening (device capability registry + deny-by-default policy presets for kernel mode).
4. End-to-end integration tests with real MQTT broker and live UDP sockets in CI profile.
5. CI profile with live kernel agent-server + claw-router for production intelligence smoke tests.

## Operator Notes: Activation Registry Backend & Metrics

Use these environment variables to select activation challenge persistence backend:

- In-memory (default): do not set `SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_PATH`.
- File-backed: set `SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_PATH=<path>` and keep kind unset (or set to `file`).
- SQLite-backed: set both `SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_PATH=<path>` and `SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_KIND=sqlite`.
- Redis-backed: set `SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_KIND=redis` and `SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_REDIS_URL=redis://host:6379/0` (optional prefix via `SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_REDIS_PREFIX`).

Prometheus scrape example:

```yaml
scrape_configs:
  - job_name: sdkwork-aiot-cloud-gateway
    static_configs:
      - targets: ['127.0.0.1:18080']
    metrics_path: /internal/bridge/metrics
```

Direct activation-only scrape path:

```yaml
scrape_configs:
  - job_name: sdkwork-aiot-activation-registry
    static_configs:
      - targets: ['127.0.0.1:18080']
    metrics_path: /internal/xiaozhi/activation-registry/metrics
```

## Operator Notes: MQTT+UDP Bridge Session Control

Enable the bridge worker threads:

```bash
SDKWORK_AIOT_GATEWAY_MQTT_BRIDGE_ENABLE=1 cargo run -p sdkwork-aiot-cloud-gateway
```

List active bridge sessions (internal route; requires `SDKWORK_AIOT_DEV_MODE=1` or `SDKWORK_AIOT_INTERNAL_TOKEN`):

```bash
curl http://127.0.0.1:18080/internal/bridge/sessions
```

Server-initiated teardown for a xiaozhi MQTT+UDP session (publishes `goodbye` and removes local bridge state):

```bash
curl -X DELETE http://127.0.0.1:18080/internal/bridge/sessions/{session_id}
```

Responses:

- `204 No Content` when the session was disconnected.
- `404` with `gateway.bridge.session.not_found` when the session is unknown.
- `503` with `gateway.bridge.disabled` when the bridge is not enabled.

