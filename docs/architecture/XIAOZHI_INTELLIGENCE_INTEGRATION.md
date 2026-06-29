# Xiaozhi Production Intelligence Integration

Last updated: 2026-06-24

## Goal

Replace simulator-grade Xiaozhi STT/LLM/TTS and MCP tool execution with production
capabilities owned by SDKWork platform services:

| Capability | Owner | Transport |
| --- | --- | --- |
| ASR / TTS (provider audio) | `sdkwork-claw-router` | `clawrouter-open-sdk` (`/v1/audio/*`) — PCM/MP3, not Opus |
| Opus downlink (Xiaozhi wire) | `sdkwork-aiot-adapter-xiaozhi` | Provider PCM → Opus frames for WebSocket/MQTT-UDP |
| Agent session / LLM turns | `sdkwork-kernel` | `sdkwork-agent-client` runtime HTTP |
| MCP tool catalog + invoke | `sdkwork-kernel` | Runtime HTTP `/sessions/{id}/tools*` |

Xiaozhi wire format stays in `sdkwork-aiot-adapter-xiaozhi` and `sdkwork-aiot-cloud-gateway`.

## Architecture

```text
Xiaozhi device
  → adapter-xiaozhi (codec / message class)
  → gateway (session + listen/mcp routing)
  → sdkwork-aiot-intelligence-bridge
       ├─ clawrouter_open_sdk  (ASR, TTS)
       └─ sdkwork-agent-client (kernel runtime sessions, chat, tools)
```

## Modes

| `SDKWORK_AIOT_INTELLIGENCE_MODE` | Behavior |
| --- | --- |
| `simulator` (default) | Existing simulator STT/LLM/TTS + file-based MCP tools |
| `kernel` | Production bridge; misconfiguration fails closed (no silent fallback) |

## Environment

| Key | Purpose |
| --- | --- |
| `SDKWORK_AIOT_INTELLIGENCE_MODE` | `simulator` or `kernel` |
| `SDKWORK_AIOT_INTELLIGENCE_KERNEL_HTTP_URL` | Kernel public ingress (`/internal/v3/api/intelligence/runtime`) |
| `SDKWORK_AIOT_INTELLIGENCE_KERNEL_AGENT_ID` | Kernel agent id for Xiaozhi sessions (default `agent.xiaozhi`) |
| `SDKWORK_CLAW_ROUTER_APPLICATION_OPEN_HTTP_URL` | Claw Router Open SDK base URL |
| `SDKWORK_CLAW_ROUTER_API_KEY` | Claw Router bearer token |
| `SDKWORK_AIOT_INTELLIGENCE_ASR_MODEL` | Optional ASR catalog key |
| `SDKWORK_AIOT_INTELLIGENCE_TTS_MODEL` | Optional TTS catalog key |
| `SDKWORK_AIOT_INTELLIGENCE_TTS_VOICE` | TTS voice (default `alloy`) |

Fallback for kernel URL: `SDKWORK_KERNEL_APPLICATION_PUBLIC_HTTP_URL` when co-deployed.

## Speech turn (listen detect)

1. Resolve user text from `xiaozhi.listen.text` or ASR (`/v1/audio/transcriptions`) when binary audio is present.
2. Ensure kernel runtime session mapped from Xiaozhi `session_id`.
3. `POST /sessions/{id}/messages` with user text → assistant reply.
4. `POST /v1/audio/speech` with assistant text → provider PCM (default) via Claw Router.
5. `sdkwork-aiot-adapter-xiaozhi` (`opus_codec` + `provider_downlink`) encodes PCM → Opus packets.
6. Gateway emits Xiaozhi JSON (`stt`, `llm`, `tts`) + one or more binary Opus frames.

Uplink ASR:

1. Device sends Opus uplink packet(s).
2. Adapter decodes Opus → PCM and wraps WAV locally.
3. Claw Router ASR receives WAV (not raw Opus).

Claw Router must not emit or consume Opus for Xiaozhi; Opus is a device protocol concern owned by AIoT.

## MCP (tools/list, tools/call)

- `tools/list` merges kernel session tool catalog with optional device-local registry entries.
- `tools/call` executes via `POST /sessions/{kernelSessionId}/tools/{name}/execute`.
- Policy hooks remain in gateway (`XiaozhiSimulatorMcpToolPolicy`). Production topology profiles set `SDKWORK_AIOT_XIAOZHI_MCP_POLICY_DENY_BY_DEFAULT=1`; kernel mode in production also enables deny-by-default automatically. Add explicit `allow|...` rules via `SDKWORK_AIOT_XIAOZHI_MCP_POLICY_RULES` for permitted tools.

## Crate boundary

`sdkwork-aiot-intelligence-bridge` owns all kernel/clawrouter HTTP client wiring.
Gateway only selects mode and formats Xiaozhi replies.

## Verification

```bash
cargo test -p sdkwork-aiot-intelligence-bridge
cargo test -p sdkwork-aiot-cloud-gateway
pnpm verify
```

Production smoke requires running kernel agent-server and claw-router with topology env set.
