use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rumqttc::{Client, Event, Incoming, MqttOptions, QoS};
use sdkwork_aiot_adapter_xiaozhi::{CLIENT_ID_HEADER, DEVICE_ID_HEADER};
use sdkwork_aiot_device_edge_runtime::XiaozhiMqttUdpSession;
use sdkwork_aiot_device_edge_runtime::{
    register_active_ws_session, touch_active_ws_session, unregister_active_ws_session,
    WebSocketSessionReply,
};

const HTTP_READ_TIMEOUT: Duration = Duration::from_secs(5);
const XIAOZHI_WEBSOCKET_READ_TIMEOUT: Duration = Duration::from_secs(125);
const DEFAULT_MQTT_KEEPALIVE_SECONDS: u64 = 30;
const DEFAULT_MQTT_QUEUE_CAPACITY: usize = 16;
const DEFAULT_MQTT_RECONNECT_BASE_MILLIS: u64 = 500;
const DEFAULT_MQTT_RECONNECT_MAX_MILLIS: u64 = 10_000;
const DEFAULT_SESSION_IDLE_TIMEOUT_SECONDS: u64 = 120;
const DEFAULT_UDP_READ_TIMEOUT_MILLIS: u64 = 2_000;
const DEFAULT_MQTT_PUBLISH_RETRY_ATTEMPTS: u32 = 2;
const DEFAULT_MQTT_PUBLISH_RETRY_DELAY_MILLIS: u64 = 100;
const DEFAULT_BRIDGE_STATS_LOG_INTERVAL_SECONDS: u64 = 30;
const DEFAULT_MQTT_MAX_OUTBOUND_PER_EVENT: usize = 8;
const MAX_WEBSOCKET_FRAME_BUFFER_BYTES: usize = 1_048_576;
const BRIDGE_STATS_PATH: &str = "/internal/bridge/stats";
const BRIDGE_METRICS_PATH: &str = "/internal/bridge/metrics";
const BRIDGE_HEALTH_PATH: &str = "/internal/bridge/health";
const BRIDGE_SESSIONS_PATH: &str = "/internal/bridge/sessions";
const BRIDGE_SESSIONS_PATH_PREFIX: &str = "/internal/bridge/sessions/";
const MCP_POLICY_STATS_PATH: &str = "/internal/xiaozhi/mcp-policy/stats";
const ACTIVATION_REGISTRY_STATS_PATH: &str = "/internal/xiaozhi/activation-registry/stats";
const ACTIVATION_REGISTRY_METRICS_PATH: &str = "/internal/xiaozhi/activation-registry/metrics";

fn main() {
    sdkwork_aiot_device_edge_runtime::assert_production_environment_safety();
    if sdkwork_aiot_device_edge_runtime::dev_mode_enabled() {
        eprintln!(
            "WARNING: SDKWORK_AIOT_DEV_MODE=1 bypasses device and internal route authentication; do not use in production"
        );
    }

    let running = Arc::new(AtomicBool::new(true));
    setup_shutdown_signal_handler(Arc::clone(&running));

    let outbox_lag = Arc::new(AtomicU64::new(0));
    let bridge_state = sdkwork_aiot_device_edge_runtime::MqttBridgeRuntimeState::from_env();
    let (server, session_options) =
        sdkwork_aiot_device_edge_runtime::standard_device_edge_server_and_session_options()
            .map(|(server, session_options)| {
                (
                    sdkwork_aiot_device_edge_runtime::attach_device_edge_readiness_probe(
                        server,
                        Arc::clone(&outbox_lag),
                        Some(Arc::clone(&bridge_state)),
                    ),
                    session_options,
                )
            })
            .expect("device edge transport server");

    println!(
        "sdkwork-aiot-device-edge-runtime mode={:?} components={} xiaozhi_websocket={}",
        server.runtime.mode(),
        server.runtime.component_names().len(),
        server.runtime.supports_protocol("xiaozhi.websocket")
    );

    if std::env::var("SDKWORK_AIOT_DEVICE_EDGE_NO_LISTEN").as_deref() == Ok("1") {
        return;
    }

    let server = Arc::new(server);
    let active_ws_connections = Arc::new(AtomicU64::new(0));
    sdkwork_aiot_device_edge_runtime::start_outbox_dispatcher_worker(
        Arc::clone(&running),
        Arc::clone(&outbox_lag),
    );
    sdkwork_aiot_device_edge_runtime::start_command_delivery_worker(Arc::clone(&running));
    let bridge_enabled = bridge_state.bridge_enabled();
    let bridge_stats = Arc::new(BridgeStats::new(current_unix_time_millis()));
    let bridge_control = if bridge_enabled {
        start_mqtt_udp_bridge(
            Arc::clone(&server),
            session_options.clone(),
            Arc::clone(&running),
            Arc::clone(&bridge_stats),
            Arc::clone(&bridge_state),
        )
    } else {
        Arc::new(BridgeControlHandle::disabled())
    };

    let bind_addr = std::env::var("SDKWORK_AIOT_EDGE_DEVICE_INGRESS_BIND")
        .unwrap_or_else(|_| "127.0.0.1:18080".to_string());
    let listener = TcpListener::bind(&bind_addr).expect("bind device edge listener");
    listener
        .set_nonblocking(true)
        .expect("set listener nonblocking");
    println!("sdkwork-aiot-device-edge-runtime listening on http://{bind_addr}");
    sdkwork_aiot_observability::emit_device_edge_lifecycle(
        "iot.device-edge.startup",
        &sdkwork_aiot_observability::TraceFields::new("device-edge-startup")
            .protocol("xiaozhi.websocket"),
    );
    sdkwork_aiot_observability::emit_runtime_metric(
        &sdkwork_aiot_observability::RuntimeMetricFields::new("sdkwork-aiot-device-edge-runtime")
            .connections(0),
    );

    while running.load(Ordering::Relaxed) {
        let stream = match listener.accept() {
            Ok((stream, _peer)) => stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
            Err(error) => {
                eprintln!("sdkwork-aiot-device-edge-runtime accept_error={error}");
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
        };

        let server = Arc::clone(&server);
        let session_options = session_options.clone();
        let context = ClientConnectionContext {
            bridge_stats: Arc::clone(&bridge_stats),
            bridge_state: Arc::clone(&bridge_state),
            bridge_control: Arc::clone(&bridge_control),
            active_ws_connections: Arc::clone(&active_ws_connections),
            outbox_lag: Arc::clone(&outbox_lag),
        };
        std::thread::spawn(move || {
            handle_client_connection(&server, &session_options, stream, context)
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MqttBridgeConfig {
    host: String,
    port: u16,
    client_id: String,
    subscribe_topic: String,
    publish_topic: String,
    keep_alive_seconds: u64,
    queue_capacity: usize,
    max_outbound_per_event: usize,
    reconnect_base_delay: Duration,
    reconnect_max_delay: Duration,
    publish_retry_attempts: u32,
    publish_retry_delay: Duration,
    stats_log_interval: Duration,
    publish_drop_cooldown: Duration,
}

impl MqttBridgeConfig {
    fn from_env() -> Self {
        Self {
            host: std::env::var("SDKWORK_AIOT_DEVICE_EDGE_MQTT_HOST")
                .unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: env_u16("SDKWORK_AIOT_DEVICE_EDGE_MQTT_PORT", 1883),
            client_id: std::env::var("SDKWORK_AIOT_DEVICE_EDGE_MQTT_CLIENT_ID")
                .unwrap_or_else(|_| "sdkwork-aiot-device-edge-runtime".to_string()),
            subscribe_topic: std::env::var("SDKWORK_AIOT_DEVICE_EDGE_MQTT_SUBSCRIBE_TOPIC")
                .unwrap_or_else(|_| "xiaozhi/up".to_string()),
            publish_topic: std::env::var("SDKWORK_AIOT_DEVICE_EDGE_MQTT_PUBLISH_TOPIC")
                .unwrap_or_else(|_| "xiaozhi/down".to_string()),
            keep_alive_seconds: env_u64(
                "SDKWORK_AIOT_DEVICE_EDGE_MQTT_KEEPALIVE_SECONDS",
                DEFAULT_MQTT_KEEPALIVE_SECONDS,
            )
            .max(1),
            queue_capacity: env_usize(
                "SDKWORK_AIOT_DEVICE_EDGE_MQTT_QUEUE_CAPACITY",
                DEFAULT_MQTT_QUEUE_CAPACITY,
            )
            .max(1),
            max_outbound_per_event: env_usize(
                "SDKWORK_AIOT_DEVICE_EDGE_MQTT_MAX_OUTBOUND_PER_EVENT",
                DEFAULT_MQTT_MAX_OUTBOUND_PER_EVENT,
            )
            .max(1),
            reconnect_base_delay: Duration::from_millis(env_u64(
                "SDKWORK_AIOT_DEVICE_EDGE_MQTT_RECONNECT_BASE_MILLIS",
                DEFAULT_MQTT_RECONNECT_BASE_MILLIS,
            )),
            reconnect_max_delay: Duration::from_millis(env_u64(
                "SDKWORK_AIOT_DEVICE_EDGE_MQTT_RECONNECT_MAX_MILLIS",
                DEFAULT_MQTT_RECONNECT_MAX_MILLIS,
            )),
            publish_retry_attempts: env_u64(
                "SDKWORK_AIOT_DEVICE_EDGE_MQTT_PUBLISH_RETRY_ATTEMPTS",
                DEFAULT_MQTT_PUBLISH_RETRY_ATTEMPTS as u64,
            ) as u32,
            publish_retry_delay: Duration::from_millis(env_u64(
                "SDKWORK_AIOT_DEVICE_EDGE_MQTT_PUBLISH_RETRY_DELAY_MILLIS",
                DEFAULT_MQTT_PUBLISH_RETRY_DELAY_MILLIS,
            )),
            stats_log_interval: Duration::from_secs(
                env_u64(
                    "SDKWORK_AIOT_DEVICE_EDGE_BRIDGE_STATS_LOG_INTERVAL_SECONDS",
                    DEFAULT_BRIDGE_STATS_LOG_INTERVAL_SECONDS,
                )
                .max(1),
            ),
            publish_drop_cooldown: Duration::from_millis(env_u64(
                "SDKWORK_AIOT_DEVICE_EDGE_MQTT_PUBLISH_DROP_COOLDOWN_MILLIS",
                0,
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdpBridgeConfig {
    bind_addr: String,
    read_timeout: Duration,
    session_idle_timeout: Duration,
    stats_log_interval: Duration,
}

impl UdpBridgeConfig {
    fn from_env() -> Self {
        Self {
            bind_addr: std::env::var("SDKWORK_AIOT_DEVICE_EDGE_UDP_BIND")
                .unwrap_or_else(|_| "0.0.0.0:8888".to_string()),
            read_timeout: Duration::from_millis(env_u64(
                "SDKWORK_AIOT_DEVICE_EDGE_UDP_READ_TIMEOUT_MILLIS",
                DEFAULT_UDP_READ_TIMEOUT_MILLIS,
            )),
            session_idle_timeout: Duration::from_secs(
                env_u64(
                    "SDKWORK_AIOT_DEVICE_EDGE_SESSION_IDLE_TIMEOUT_SECONDS",
                    DEFAULT_SESSION_IDLE_TIMEOUT_SECONDS,
                )
                .max(1),
            ),
            stats_log_interval: Duration::from_secs(
                env_u64(
                    "SDKWORK_AIOT_DEVICE_EDGE_BRIDGE_STATS_LOG_INTERVAL_SECONDS",
                    DEFAULT_BRIDGE_STATS_LOG_INTERVAL_SECONDS,
                )
                .max(1),
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManagedMqttUdpSession {
    session: XiaozhiMqttUdpSession,
    last_activity_millis: i64,
    udp_peer: Option<SocketAddr>,
}

impl ManagedMqttUdpSession {
    fn new(session: XiaozhiMqttUdpSession) -> Self {
        Self {
            session,
            last_activity_millis: current_unix_time_millis(),
            udp_peer: None,
        }
    }

    fn touch_now(&mut self) {
        self.last_activity_millis = current_unix_time_millis();
    }

    fn set_udp_peer(&mut self, peer: SocketAddr) {
        self.udp_peer = Some(peer);
        self.touch_now();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionState {
    sessions: HashMap<String, ManagedMqttUdpSession>,
}

impl SessionState {
    fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    fn snapshot_session(&self, session_key: Option<&str>) -> Option<XiaozhiMqttUdpSession> {
        let key = session_key?;
        self.sessions
            .get(key)
            .map(|managed| managed.session.clone())
    }

    fn upsert_session(&mut self, session_key: String, session: XiaozhiMqttUdpSession) {
        let udp_peer = self
            .sessions
            .get(&session_key)
            .and_then(|managed| managed.udp_peer);
        let mut managed = ManagedMqttUdpSession::new(session);
        managed.udp_peer = udp_peer;
        self.sessions.insert(session_key, managed);
    }

    fn remove_session(&mut self, session_key: &str) -> bool {
        self.sessions.remove(session_key).is_some()
    }
}

struct MqttBridgePublisher {
    inner: Mutex<MqttBridgePublisherState>,
}

struct MqttBridgePublisherState {
    client: Option<Client>,
    publish_topic: String,
    retry_attempts: u32,
    retry_delay: Duration,
    publish_drop_cooldown: Duration,
    max_outbound_per_event: usize,
}

impl MqttBridgePublisher {
    fn new(config: &MqttBridgeConfig) -> Self {
        Self {
            inner: Mutex::new(MqttBridgePublisherState {
                client: None,
                publish_topic: config.publish_topic.clone(),
                retry_attempts: config.publish_retry_attempts,
                retry_delay: config.publish_retry_delay,
                publish_drop_cooldown: config.publish_drop_cooldown,
                max_outbound_per_event: config.max_outbound_per_event,
            }),
        }
    }

    fn set_client(&self, client: Option<Client>) {
        self.inner
            .lock()
            .expect("mqtt bridge publisher lock")
            .client = client;
    }

    fn publish_reply(
        &self,
        reply: sdkwork_aiot_device_edge_runtime::MqttSessionReply,
        stats: &BridgeStats,
    ) {
        let state = self.inner.lock().expect("mqtt bridge publisher lock");
        let Some(client) = state.client.as_ref() else {
            if !reply.outbound_json.is_empty() {
                eprintln!("sdkwork-aiot-device-edge-runtime mqtt_publish_skipped=no_active_client");
            }
            return;
        };
        publish_bridge_json_messages(
            client,
            &state.publish_topic,
            reply.outbound_json,
            state.retry_attempts,
            state.retry_delay,
            state.publish_drop_cooldown,
            state.max_outbound_per_event,
            stats,
        );
    }
}

fn apply_bridge_session_state_locked(
    session_state: &mut SessionState,
    session_key: Option<&str>,
    reply: &sdkwork_aiot_device_edge_runtime::MqttSessionReply,
    next_session: Option<sdkwork_aiot_device_edge_runtime::XiaozhiMqttUdpSession>,
) {
    if reply.close_audio_channel {
        if let Some(key) = session_key {
            session_state.remove_session(key);
        }
        return;
    }
    if let Some(session) = next_session {
        let upsert_key = session_key
            .map(str::to_string)
            .unwrap_or_else(|| session.session_id.clone());
        session_state.upsert_session(upsert_key, session);
    } else if let Some(key) = session_key {
        if let Some(managed) = session_state.sessions.get_mut(key) {
            managed.touch_now();
        }
    }
}

fn apply_bridge_session_state(
    session_state: &Mutex<SessionState>,
    session_key: Option<&str>,
    reply: &sdkwork_aiot_device_edge_runtime::MqttSessionReply,
    next_session: Option<sdkwork_aiot_device_edge_runtime::XiaozhiMqttUdpSession>,
) {
    let mut guard = session_state.lock().expect("mqtt session lock");
    apply_bridge_session_state_locked(&mut guard, session_key, reply, next_session);
}

fn bridge_udp_peer_for_session(
    session_state: &Mutex<SessionState>,
    session_key: Option<&str>,
) -> Option<SocketAddr> {
    let key = session_key?;
    session_state
        .lock()
        .expect("mqtt session lock")
        .sessions
        .get(key)
        .and_then(|managed| managed.udp_peer)
}

fn send_bridge_udp_packets(
    udp_socket: &UdpSocket,
    peer: Option<SocketAddr>,
    packets: &[Vec<u8>],
    stats: &BridgeStats,
) {
    let Some(peer) = peer else {
        if !packets.is_empty() {
            eprintln!("sdkwork-aiot-device-edge-runtime udp_send_skipped=no_session_peer");
        }
        return;
    };
    for packet in packets {
        if let Err(error) = udp_socket.send_to(packet, peer) {
            eprintln!("sdkwork-aiot-device-edge-runtime udp_send_error={error}");
        } else {
            stats.udp_packets_total.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn publish_bridge_json_messages(
    client: &Client,
    publish_topic: &str,
    messages: Vec<String>,
    retry_attempts: u32,
    retry_delay: Duration,
    publish_drop_cooldown: Duration,
    max_outbound_per_event: usize,
    stats: &BridgeStats,
) {
    let (bounded_outbound, dropped) = bounded_outbound_messages(messages, max_outbound_per_event);
    if dropped > 0 {
        stats
            .mqtt_publish_dropped
            .fetch_add(dropped, Ordering::Relaxed);
    }

    for outbound in bounded_outbound {
        stats.mqtt_publish_attempts.fetch_add(1, Ordering::Relaxed);
        if let Err(error) = publish_with_retry(
            client,
            publish_topic,
            outbound,
            retry_attempts,
            retry_delay,
            stats,
        ) {
            eprintln!("sdkwork-aiot-device-edge-runtime mqtt_publish_error={error}");
            if duration_millis_u64(publish_drop_cooldown) > 0 {
                std::thread::sleep(publish_drop_cooldown);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BridgeStatsSnapshot {
    mqtt_reconnects: u64,
    mqtt_events_total: u64,
    mqtt_events_errors: u64,
    mqtt_session_errors: u64,
    mqtt_publish_attempts: u64,
    mqtt_publish_failures: u64,
    mqtt_publish_retries: u64,
    mqtt_publish_dropped: u64,
    udp_packets_total: u64,
    udp_decode_failures: u64,
    session_idle_purges: u64,
    udp_uplink_replies: u64,
}

struct BridgeControlInner {
    session_state: Arc<Mutex<SessionState>>,
    mqtt_publisher: Arc<MqttBridgePublisher>,
}

struct BridgeControlHandle {
    inner: Option<BridgeControlInner>,
}

impl BridgeControlHandle {
    fn disabled() -> Self {
        Self { inner: None }
    }

    fn enabled(
        session_state: Arc<Mutex<SessionState>>,
        mqtt_publisher: Arc<MqttBridgePublisher>,
    ) -> Self {
        Self {
            inner: Some(BridgeControlInner {
                session_state,
                mqtt_publisher,
            }),
        }
    }

    fn list_sessions_json(&self) -> String {
        let Some(inner) = &self.inner else {
            return r#"{"bridge_enabled":false,"sessions":[]}"#.to_string();
        };
        let guard = inner.session_state.lock().expect("mqtt session lock");
        let mut sessions = Vec::with_capacity(guard.sessions.len());
        for (session_key, managed) in &guard.sessions {
            let udp_peer = managed
                .udp_peer
                .map(|peer| format!(r#""{peer}""#))
                .unwrap_or_else(|| "null".to_string());
            sessions.push(format!(
                r#"{{"session_key":"{session_key}","session_id":"{}","device_id":"{}","client_id":"{}","remote_sequence":{},"local_sequence":{},"udp_peer":{udp_peer}}}"#,
                json_escape(&managed.session.session_id),
                json_escape(&managed.session.device_id),
                json_escape(&managed.session.client_id),
                managed.session.remote_sequence,
                managed.session.local_sequence,
            ));
        }
        format!(
            r#"{{"bridge_enabled":true,"sessions":[{}]}}"#,
            sessions.join(",")
        )
    }

    fn disconnect_session(
        &self,
        session_id: &str,
        stats: &BridgeStats,
    ) -> BridgeSessionDisconnectOutcome {
        let Some(inner) = &self.inner else {
            return BridgeSessionDisconnectOutcome::BridgeDisabled;
        };
        let mut guard = inner.session_state.lock().expect("mqtt session lock");
        let session_key = guard.sessions.iter().find_map(|(key, managed)| {
            if managed.session.session_id == session_id || key == session_id {
                Some(key.clone())
            } else {
                None
            }
        });
        let Some(session_key) = session_key else {
            return BridgeSessionDisconnectOutcome::NotFound;
        };
        let session_id_for_teardown = guard
            .sessions
            .get(&session_key)
            .map(|managed| managed.session.session_id.clone())
            .unwrap_or_else(|| session_key.clone());
        let reply = sdkwork_aiot_device_edge_runtime::xiaozhi_mqtt_server_teardown_reply(
            &session_id_for_teardown,
        );
        inner.mqtt_publisher.publish_reply(reply.clone(), stats);
        apply_bridge_session_state_locked(&mut guard, Some(session_key.as_str()), &reply, None);
        BridgeSessionDisconnectOutcome::Disconnected
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BridgeSessionDisconnectOutcome {
    Disconnected,
    NotFound,
    BridgeDisabled,
}

fn bridge_session_id_from_path(path: &str) -> Option<&str> {
    path.strip_prefix(BRIDGE_SESSIONS_PATH_PREFIX)
        .filter(|session_id| !session_id.is_empty() && !session_id.contains('/'))
}

#[derive(Debug)]
struct BridgeStats {
    mqtt_reconnects: AtomicU64,
    mqtt_events_total: AtomicU64,
    mqtt_events_errors: AtomicU64,
    mqtt_session_errors: AtomicU64,
    mqtt_publish_attempts: AtomicU64,
    mqtt_publish_failures: AtomicU64,
    mqtt_publish_retries: AtomicU64,
    mqtt_publish_dropped: AtomicU64,
    udp_packets_total: AtomicU64,
    udp_decode_failures: AtomicU64,
    session_idle_purges: AtomicU64,
    udp_uplink_replies: AtomicU64,
    last_log_millis: AtomicU64,
}

impl BridgeStats {
    fn new(now_millis: i64) -> Self {
        Self {
            mqtt_reconnects: AtomicU64::new(0),
            mqtt_events_total: AtomicU64::new(0),
            mqtt_events_errors: AtomicU64::new(0),
            mqtt_session_errors: AtomicU64::new(0),
            mqtt_publish_attempts: AtomicU64::new(0),
            mqtt_publish_failures: AtomicU64::new(0),
            mqtt_publish_retries: AtomicU64::new(0),
            mqtt_publish_dropped: AtomicU64::new(0),
            udp_packets_total: AtomicU64::new(0),
            udp_decode_failures: AtomicU64::new(0),
            session_idle_purges: AtomicU64::new(0),
            udp_uplink_replies: AtomicU64::new(0),
            last_log_millis: AtomicU64::new(now_millis.max(0) as u64),
        }
    }

    fn maybe_log_snapshot(&self, interval: Duration) {
        let now = current_unix_time_millis();
        let now_u64 = now.max(0) as u64;
        let interval_millis = duration_millis_u64(interval).max(1);
        let last = self.last_log_millis.load(Ordering::Relaxed);
        if now_u64.saturating_sub(last) < interval_millis {
            return;
        }
        if self
            .last_log_millis
            .compare_exchange(last, now_u64, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            return;
        }

        let snapshot = self.snapshot();
        eprintln!(
            "sdkwork-aiot-device-edge-runtime bridge_stats reconnects={} mqtt_events={} mqtt_event_errors={} mqtt_session_errors={} publish_attempts={} publish_failures={} publish_retries={} publish_dropped={} udp_packets={} udp_decode_failures={} session_idle_purges={} udp_uplink_replies={}",
            snapshot.mqtt_reconnects,
            snapshot.mqtt_events_total,
            snapshot.mqtt_events_errors,
            snapshot.mqtt_session_errors,
            snapshot.mqtt_publish_attempts,
            snapshot.mqtt_publish_failures,
            snapshot.mqtt_publish_retries,
            snapshot.mqtt_publish_dropped,
            snapshot.udp_packets_total,
            snapshot.udp_decode_failures,
            snapshot.session_idle_purges,
            snapshot.udp_uplink_replies,
        );
    }

    fn snapshot(&self) -> BridgeStatsSnapshot {
        BridgeStatsSnapshot {
            mqtt_reconnects: self.mqtt_reconnects.load(Ordering::Relaxed),
            mqtt_events_total: self.mqtt_events_total.load(Ordering::Relaxed),
            mqtt_events_errors: self.mqtt_events_errors.load(Ordering::Relaxed),
            mqtt_session_errors: self.mqtt_session_errors.load(Ordering::Relaxed),
            mqtt_publish_attempts: self.mqtt_publish_attempts.load(Ordering::Relaxed),
            mqtt_publish_failures: self.mqtt_publish_failures.load(Ordering::Relaxed),
            mqtt_publish_retries: self.mqtt_publish_retries.load(Ordering::Relaxed),
            mqtt_publish_dropped: self.mqtt_publish_dropped.load(Ordering::Relaxed),
            udp_packets_total: self.udp_packets_total.load(Ordering::Relaxed),
            udp_decode_failures: self.udp_decode_failures.load(Ordering::Relaxed),
            session_idle_purges: self.session_idle_purges.load(Ordering::Relaxed),
            udp_uplink_replies: self.udp_uplink_replies.load(Ordering::Relaxed),
        }
    }
}

fn start_mqtt_udp_bridge(
    server: Arc<sdkwork_aiot_transport::TransportServer>,
    session_options: sdkwork_aiot_device_edge_runtime::XiaozhiSessionOptions,
    running: Arc<AtomicBool>,
    stats: Arc<BridgeStats>,
    state: Arc<sdkwork_aiot_device_edge_runtime::MqttBridgeRuntimeState>,
) -> Arc<BridgeControlHandle> {
    let mqtt_config = MqttBridgeConfig::from_env();
    let udp_config = UdpBridgeConfig::from_env();
    let session_state = Arc::new(Mutex::new(SessionState::new()));
    let mqtt_publisher = Arc::new(MqttBridgePublisher::new(&mqtt_config));
    let bridge_control = Arc::new(BridgeControlHandle::enabled(
        Arc::clone(&session_state),
        Arc::clone(&mqtt_publisher),
    ));
    let udp_socket = match UdpSocket::bind(&udp_config.bind_addr) {
        Ok(socket) => Arc::new(socket),
        Err(error) => {
            eprintln!("sdkwork-aiot-device-edge-runtime udp_bind_error={error}");
            return Arc::clone(&bridge_control);
        }
    };
    if let Err(error) = udp_socket.set_read_timeout(Some(udp_config.read_timeout)) {
        eprintln!("sdkwork-aiot-device-edge-runtime udp_timeout_error={error}");
        return Arc::clone(&bridge_control);
    }

    let udp_state = Arc::clone(&session_state);
    let udp_running = Arc::clone(&running);
    let udp_stats = Arc::clone(&stats);
    let udp_runtime = Arc::clone(&state);
    let udp_shared_socket = Arc::clone(&udp_socket);
    let udp_publisher = Arc::clone(&mqtt_publisher);
    std::thread::spawn(move || {
        run_udp_bridge_loop(
            udp_state,
            udp_running,
            udp_config,
            udp_stats,
            udp_runtime,
            udp_shared_socket,
            udp_publisher,
        )
    });

    let mqtt_state = Arc::clone(&session_state);
    let mqtt_running = Arc::clone(&running);
    let mqtt_stats = Arc::clone(&stats);
    let mqtt_runtime = Arc::clone(&state);
    let mqtt_udp_socket = Arc::clone(&udp_socket);
    let mqtt_publisher_for_loop = Arc::clone(&mqtt_publisher);
    std::thread::spawn(move || {
        run_mqtt_bridge_loop(
            server,
            session_options,
            mqtt_state,
            mqtt_running,
            mqtt_config,
            mqtt_stats,
            mqtt_runtime,
            mqtt_udp_socket,
            mqtt_publisher_for_loop,
        )
    });
    bridge_control
}

#[allow(clippy::too_many_arguments)]
fn run_mqtt_bridge_loop(
    server: Arc<sdkwork_aiot_transport::TransportServer>,
    session_options: sdkwork_aiot_device_edge_runtime::XiaozhiSessionOptions,
    session_state: Arc<Mutex<SessionState>>,
    running: Arc<AtomicBool>,
    config: MqttBridgeConfig,
    stats: Arc<BridgeStats>,
    state: Arc<sdkwork_aiot_device_edge_runtime::MqttBridgeRuntimeState>,
    udp_socket: Arc<UdpSocket>,
    mqtt_publisher: Arc<MqttBridgePublisher>,
) {
    state.mqtt_loop_running().store(true, Ordering::Relaxed);
    let mut reconnect_attempt = 0u32;
    while running.load(Ordering::Relaxed) {
        if reconnect_attempt > 0 {
            stats.mqtt_reconnects.fetch_add(1, Ordering::Relaxed);
        }
        let mut options =
            MqttOptions::new(config.client_id.clone(), config.host.clone(), config.port);
        options.set_keep_alive(Duration::from_secs(config.keep_alive_seconds));
        let (client, mut connection) = Client::new(options, config.queue_capacity);
        mqtt_publisher.set_client(Some(client.clone()));
        if let Err(error) = client.subscribe(config.subscribe_topic.clone(), QoS::AtMostOnce) {
            mqtt_publisher.set_client(None);
            eprintln!("sdkwork-aiot-device-edge-runtime mqtt_subscribe_error={error}");
            state.mqtt_session_active().store(false, Ordering::Relaxed);
            sleep_reconnect_delay(
                reconnect_attempt,
                config.reconnect_base_delay,
                config.reconnect_max_delay,
            );
            reconnect_attempt = reconnect_attempt.saturating_add(1);
            continue;
        }

        reconnect_attempt = 0;
        state.mqtt_session_active().store(true, Ordering::Relaxed);
        loop {
            let event = match connection.iter().next() {
                Some(Ok(event)) => {
                    stats.mqtt_events_total.fetch_add(1, Ordering::Relaxed);
                    event
                }
                Some(Err(error)) => {
                    stats.mqtt_events_errors.fetch_add(1, Ordering::Relaxed);
                    eprintln!("sdkwork-aiot-device-edge-runtime mqtt_event_error={error}");
                    break;
                }
                None => {
                    eprintln!("sdkwork-aiot-device-edge-runtime mqtt_event_stream_closed");
                    break;
                }
            };

            if let Event::Incoming(Incoming::Publish(publish)) = event {
                let inbound = String::from_utf8_lossy(&publish.payload).to_string();
                let session_key =
                    sdkwork_aiot_device_edge_runtime::xiaozhi_mqtt_session_lookup_key(&inbound);
                let session_snapshot = {
                    let guard = session_state.lock().expect("mqtt session lock");
                    guard.snapshot_session(session_key.as_deref())
                };

                let response =
                    sdkwork_aiot_device_edge_runtime::xiaozhi_mqtt_session_reply_with_options(
                        &server,
                        session_snapshot.as_ref(),
                        &inbound,
                        &session_options,
                    );
                let (reply, next_session) = match response {
                    Ok(response) => response,
                    Err(error) => {
                        stats.mqtt_session_errors.fetch_add(1, Ordering::Relaxed);
                        eprintln!(
                            "sdkwork-aiot-device-edge-runtime mqtt_session_error={}",
                            error.code
                        );
                        continue;
                    }
                };

                apply_bridge_session_state(
                    &session_state,
                    session_key.as_deref(),
                    &reply,
                    next_session,
                );
                let udp_peer = bridge_udp_peer_for_session(&session_state, session_key.as_deref());
                send_bridge_udp_packets(
                    udp_socket.as_ref(),
                    udp_peer,
                    &reply.outbound_udp_packets,
                    stats.as_ref(),
                );
                publish_bridge_json_messages(
                    &client,
                    &config.publish_topic,
                    reply.outbound_json,
                    config.publish_retry_attempts,
                    config.publish_retry_delay,
                    config.publish_drop_cooldown,
                    config.max_outbound_per_event,
                    stats.as_ref(),
                );
            }
            stats.maybe_log_snapshot(config.stats_log_interval);
        }
        mqtt_publisher.set_client(None);
        state.mqtt_session_active().store(false, Ordering::Relaxed);

        sleep_reconnect_delay(
            reconnect_attempt,
            config.reconnect_base_delay,
            config.reconnect_max_delay,
        );
        reconnect_attempt = reconnect_attempt.saturating_add(1);
    }
    state.mqtt_session_active().store(false, Ordering::Relaxed);
    state.mqtt_loop_running().store(false, Ordering::Relaxed);
}

fn run_udp_bridge_loop(
    session_state: Arc<Mutex<SessionState>>,
    running: Arc<AtomicBool>,
    config: UdpBridgeConfig,
    stats: Arc<BridgeStats>,
    state: Arc<sdkwork_aiot_device_edge_runtime::MqttBridgeRuntimeState>,
    udp_socket: Arc<UdpSocket>,
    mqtt_publisher: Arc<MqttBridgePublisher>,
) {
    state.udp_loop_running().store(true, Ordering::Relaxed);
    state.udp_socket_bound().store(true, Ordering::Relaxed);

    let mut buf = [0u8; 2048];
    while running.load(Ordering::Relaxed) {
        let purged = purge_idle_sessions(&session_state, config.session_idle_timeout);
        if purged > 0 {
            stats
                .session_idle_purges
                .fetch_add(purged, Ordering::Relaxed);
        }

        let (len, from) = match udp_socket.recv_from(&mut buf) {
            Ok(value) => value,
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(error) => {
                eprintln!("sdkwork-aiot-device-edge-runtime udp_recv_error={error}");
                continue;
            }
        };
        stats.udp_packets_total.fetch_add(1, Ordering::Relaxed);

        struct UdpUplinkReplyAction {
            session_key: String,
            reply: sdkwork_aiot_device_edge_runtime::MqttSessionReply,
            session: sdkwork_aiot_device_edge_runtime::XiaozhiMqttUdpSession,
            peer: SocketAddr,
        }

        let mut uplink_action = None;
        {
            let mut guard = session_state.lock().expect("udp session lock");
            if guard.sessions.is_empty() {
                continue;
            }

            for (session_key, managed) in guard.sessions.iter_mut() {
                let packet = match sdkwork_aiot_device_edge_runtime::xiaozhi_mqtt_udp_decode_audio(
                    &managed.session,
                    &buf[..len],
                ) {
                    Ok(packet) => packet,
                    Err(_) => continue,
                };
                managed.session.remote_sequence = packet.sequence;
                managed.set_udp_peer(from);
                let mut session = managed.session.clone();
                match sdkwork_aiot_device_edge_runtime::xiaozhi_mqtt_udp_uplink_speech_reply(
                    &mut session,
                    &packet.payload,
                ) {
                    Ok(reply) => {
                        managed.session = session.clone();
                        uplink_action = Some(UdpUplinkReplyAction {
                            session_key: session_key.clone(),
                            reply,
                            session,
                            peer: from,
                        });
                        break;
                    }
                    Err(error) => {
                        eprintln!(
                            "sdkwork-aiot-device-edge-runtime udp_uplink_reply_error={}",
                            error.code
                        );
                    }
                }
            }
        }

        if let Some(action) = uplink_action {
            apply_bridge_session_state(
                &session_state,
                Some(action.session_key.as_str()),
                &action.reply,
                Some(action.session),
            );
            send_bridge_udp_packets(
                udp_socket.as_ref(),
                Some(action.peer),
                &action.reply.outbound_udp_packets,
                stats.as_ref(),
            );
            mqtt_publisher.publish_reply(action.reply, stats.as_ref());
            stats.udp_uplink_replies.fetch_add(1, Ordering::Relaxed);
            continue;
        }

        stats.udp_decode_failures.fetch_add(1, Ordering::Relaxed);
        eprintln!(
            "sdkwork-aiot-device-edge-runtime udp_decode_error=transport.udp.decode_failed from={from}"
        );
        stats.maybe_log_snapshot(config.stats_log_interval);
    }
    state.udp_socket_bound().store(false, Ordering::Relaxed);
    state.udp_loop_running().store(false, Ordering::Relaxed);
}

fn setup_shutdown_signal_handler(running: Arc<AtomicBool>) {
    install_ctrlc_handler(running);
}

fn install_ctrlc_handler(running: Arc<AtomicBool>) {
    if let Err(error) = ctrlc::set_handler(move || {
        running.store(false, Ordering::SeqCst);
    }) {
        eprintln!("sdkwork-aiot-device-edge-runtime ctrlc_handler_error={error}");
    }
}

struct ClientConnectionContext {
    bridge_stats: Arc<BridgeStats>,
    bridge_state: Arc<sdkwork_aiot_device_edge_runtime::MqttBridgeRuntimeState>,
    bridge_control: Arc<BridgeControlHandle>,
    active_ws_connections: Arc<AtomicU64>,
    outbox_lag: Arc<AtomicU64>,
}

fn handle_client_connection(
    server: &sdkwork_aiot_transport::TransportServer,
    session_options: &sdkwork_aiot_device_edge_runtime::XiaozhiSessionOptions,
    mut stream: TcpStream,
    context: ClientConnectionContext,
) {
    let _ = stream.set_read_timeout(Some(HTTP_READ_TIMEOUT));

    let buffer = match sdkwork_aiot_transport::read_full_http_request(
        &mut stream,
        sdkwork_aiot_transport::DEFAULT_MAX_HTTP_BODY_BYTES,
    ) {
        Ok(buffer) => buffer,
        Err(error) => {
            eprintln!("sdkwork-aiot-device-edge-runtime read_error={}", error.code);
            let response = problem_response(&error.code);
            let _ = stream.write_all(response.as_bytes());
            return;
        }
    };
    if buffer.is_empty() {
        return;
    }

    let parsed_request = sdkwork_aiot_transport::parse_http_request_prefix(&buffer);
    if let Ok((request, header_len)) = parsed_request {
        if request.path.starts_with("/internal/")
            && !sdkwork_aiot_device_edge_runtime::internal_route_authorized(&request)
        {
            let response = unauthorized_response("iot.device-edge.internal.unauthorized");
            let _ = stream.write_all(response.as_bytes());
            return;
        }

        if request.method == "GET" && request.path == BRIDGE_HEALTH_PATH {
            let response = bridge_health_response(
                context.bridge_state.as_ref(),
                context.bridge_stats.as_ref(),
            );
            let response = format_response(&response);
            let _ = stream.write_all(response.as_bytes());
            return;
        }
        if request.method == "GET" && request.path == BRIDGE_STATS_PATH {
            let response = bridge_stats_response(context.bridge_stats.as_ref());
            let response = format_response(&response);
            let _ = stream.write_all(response.as_bytes());
            return;
        }
        if request.method == "GET" && request.path == BRIDGE_METRICS_PATH {
            let response = bridge_metrics_response(context.bridge_stats.as_ref());
            let response = format_response(&response);
            let _ = stream.write_all(response.as_bytes());
            return;
        }
        if request.method == "GET" && request.path == BRIDGE_SESSIONS_PATH {
            let response = bridge_sessions_response(context.bridge_control.as_ref());
            let response = format_response(&response);
            let _ = stream.write_all(response.as_bytes());
            return;
        }
        if request.method == "DELETE" {
            if let Some(session_id) = bridge_session_id_from_path(&request.path) {
                let response = bridge_session_disconnect_response(
                    context.bridge_control.as_ref(),
                    context.bridge_stats.as_ref(),
                    session_id,
                );
                let response = format_response(&response);
                let _ = stream.write_all(response.as_bytes());
                return;
            }
        }
        if request.method == "GET" && request.path == MCP_POLICY_STATS_PATH {
            let response = mcp_policy_stats_response(session_options);
            let response = format_response(&response);
            let _ = stream.write_all(response.as_bytes());
            return;
        }
        if request.method == "GET" && request.path == ACTIVATION_REGISTRY_STATS_PATH {
            let response = activation_registry_stats_response();
            let response = format_response(&response);
            let _ = stream.write_all(response.as_bytes());
            return;
        }
        if request.method == "GET" && request.path == ACTIVATION_REGISTRY_METRICS_PATH {
            let response = activation_registry_metrics_response();
            let response = format_response(&response);
            let _ = stream.write_all(response.as_bytes());
            return;
        }

        if is_websocket_upgrade(&request)
            && request.path == sdkwork_aiot_adapter_xiaozhi::XIAOZHI_WS_PATH
        {
            if !sdkwork_aiot_device_edge_runtime::xiaozhi_device_token_valid(
                &request,
                session_options,
            ) {
                let response = unauthorized_response("iot.xiaozhi.websocket.unauthorized");
                let _ = stream.write_all(response.as_bytes());
                return;
            }

            let projected_connections = context.active_ws_connections.load(Ordering::Relaxed) + 1;
            if sdkwork_aiot_device_edge_runtime::ws_backpressure_rejects_new_connection(
                &server.runtime,
                projected_connections,
                context.outbox_lag.load(Ordering::Relaxed),
            ) {
                let response = service_unavailable_response("iot.xiaozhi.websocket.backpressure");
                let _ = stream.write_all(response.as_bytes());
                return;
            }

            match sdkwork_aiot_transport::build_websocket_handshake_response(&request) {
                Ok(response) => {
                    let response = format_response(&response);
                    if stream.write_all(response.as_bytes()).is_ok() {
                        context
                            .active_ws_connections
                            .fetch_add(1, Ordering::Relaxed);
                        let _guard = ActiveWsConnectionGuard {
                            counter: Arc::clone(&context.active_ws_connections),
                        };
                        handle_xiaozhi_websocket_session(
                            server,
                            session_options,
                            &request,
                            buffer[header_len..].to_vec(),
                            stream,
                        );
                    }
                }
                Err(error) => {
                    let response = problem_response(&error.code);
                    let _ = stream.write_all(response.as_bytes());
                }
            }
            return;
        }
    }

    let response = match sdkwork_aiot_transport::handle_http_request_bytes(server, &buffer) {
        Ok(response) => response,
        Err(error) => problem_response(&error.code),
    };

    if let Err(error) = stream.write_all(response.as_bytes()) {
        eprintln!("sdkwork-aiot-device-edge-runtime write_error={error}");
    }
}

fn is_websocket_upgrade(request: &sdkwork_aiot_transport::HttpRequest) -> bool {
    request.method == "GET"
        && request
            .header("upgrade")
            .is_some_and(|value| value.eq_ignore_ascii_case("websocket"))
        && request
            .header("connection")
            .is_some_and(|value| value.to_ascii_lowercase().contains("upgrade"))
}

fn handle_xiaozhi_websocket_session(
    server: &sdkwork_aiot_transport::TransportServer,
    session_options: &sdkwork_aiot_device_edge_runtime::XiaozhiSessionOptions,
    request: &sdkwork_aiot_transport::HttpRequest,
    initial_frame_bytes: Vec<u8>,
    mut stream: TcpStream,
) {
    let _ = stream.set_read_timeout(Some(XIAOZHI_WEBSOCKET_READ_TIMEOUT));

    let device_id = request
        .header(DEVICE_ID_HEADER)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("")
        .to_string();
    let session_id = request
        .header(CLIENT_ID_HEADER)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| device_id.clone());
    let (command_outbound_tx, command_outbound_rx) =
        std::sync::mpsc::channel::<Vec<WebSocketSessionReply>>();
    if !device_id.is_empty() {
        let association = sdkwork_aiot_device_edge_runtime::resolve_device_storage_association(
            request,
            session_options,
        );
        register_active_ws_session(&device_id, &session_id, command_outbound_tx, association);
    }

    struct ActiveWsSessionGuard {
        device_id: String,
    }
    impl Drop for ActiveWsSessionGuard {
        fn drop(&mut self) {
            if !self.device_id.is_empty() {
                unregister_active_ws_session(&self.device_id);
            }
        }
    }
    let _ws_session_guard = ActiveWsSessionGuard {
        device_id: device_id.clone(),
    };

    let mut read_buffer = [0u8; 8192];
    let mut frame_buffer = initial_frame_bytes;
    loop {
        if flush_ws_command_outbound(&mut stream, &command_outbound_rx) {
            return;
        }
        if !device_id.is_empty() {
            touch_active_ws_session(&device_id);
        }

        if !frame_buffer.is_empty()
            && process_xiaozhi_frame_buffer(
                server,
                session_options,
                request,
                &mut stream,
                &mut frame_buffer,
            )
        {
            return;
        }

        let read = match stream.read(&mut read_buffer) {
            Ok(0) => return,
            Ok(read) => read,
            Err(error) => {
                eprintln!("sdkwork-aiot-device-edge-runtime websocket_read_error={error}");
                return;
            }
        };
        frame_buffer.extend_from_slice(&read_buffer[..read]);
        if frame_buffer.len() > MAX_WEBSOCKET_FRAME_BUFFER_BYTES {
            eprintln!("sdkwork-aiot-device-edge-runtime websocket_frame_buffer_limit_exceeded");
            return;
        }
    }
}

fn flush_ws_command_outbound(
    stream: &mut TcpStream,
    command_outbound_rx: &std::sync::mpsc::Receiver<Vec<WebSocketSessionReply>>,
) -> bool {
    while let Ok(replies) = command_outbound_rx.try_recv() {
        for reply in replies {
            let should_close = matches!(reply, WebSocketSessionReply::Close);
            let frame = websocket_reply_frame(reply);
            if let Err(error) = stream.write_all(&frame) {
                eprintln!("sdkwork-aiot-device-edge-runtime websocket_write_error={error}");
                return true;
            }
            if should_close {
                return true;
            }
        }
    }
    false
}

fn process_xiaozhi_frame_buffer(
    server: &sdkwork_aiot_transport::TransportServer,
    session_options: &sdkwork_aiot_device_edge_runtime::XiaozhiSessionOptions,
    request: &sdkwork_aiot_transport::HttpRequest,
    stream: &mut TcpStream,
    frame_buffer: &mut Vec<u8>,
) -> bool {
    loop {
        let (frame, used) =
            match sdkwork_aiot_transport::decode_websocket_frame_prefix(frame_buffer) {
                Ok(result) => result,
                Err(error) if error.code == "transport.websocket.incomplete_frame" => return false,
                Err(error) => {
                    eprintln!(
                        "sdkwork-aiot-device-edge-runtime websocket_decode_error={}",
                        error.code
                    );
                    return true;
                }
            };
        frame_buffer.drain(..used);

        let replies =
            match sdkwork_aiot_device_edge_runtime::xiaozhi_websocket_session_reply_with_options(
                server,
                request,
                frame,
                session_options,
            ) {
                Ok(replies) => replies,
                Err(error) => {
                    vec![WebSocketSessionReply::Text(format!(
                        r#"{{"type":"alert","status":"Bad Request","message":"{}","emotion":"sad"}}"#,
                        json_escape(&error.code)
                    ))]
                }
            };

        for reply in replies {
            let should_close = matches!(reply, WebSocketSessionReply::Close);
            let frame = websocket_reply_frame(reply);
            if let Err(error) = stream.write_all(&frame) {
                eprintln!("sdkwork-aiot-device-edge-runtime websocket_write_error={error}");
                return true;
            }
            if should_close {
                return true;
            }
        }
    }
}

fn websocket_reply_frame(reply: WebSocketSessionReply) -> Vec<u8> {
    let frame = match reply {
        WebSocketSessionReply::Text(text) => sdkwork_aiot_transport::WebSocketFrame::text(text),
        WebSocketSessionReply::Binary(payload) => sdkwork_aiot_transport::WebSocketFrame {
            opcode: sdkwork_aiot_transport::WebSocketOpcode::Binary,
            payload,
        },
        WebSocketSessionReply::Pong(payload) => sdkwork_aiot_transport::WebSocketFrame {
            opcode: sdkwork_aiot_transport::WebSocketOpcode::Pong,
            payload,
        },
        WebSocketSessionReply::Close => sdkwork_aiot_transport::WebSocketFrame {
            opcode: sdkwork_aiot_transport::WebSocketOpcode::Close,
            payload: Vec::new(),
        },
    };

    sdkwork_aiot_transport::encode_websocket_frame(&frame)
}

fn format_response(response: &sdkwork_aiot_transport::HttpResponse) -> String {
    let mut out = format!(
        "HTTP/1.1 {} {}\r\n",
        response.status.code(),
        response.status.reason()
    );
    let mut has_content_length = false;
    for (name, value) in response.headers() {
        if name == "content-length" {
            has_content_length = true;
        }
        out.push_str(name);
        out.push_str(": ");
        out.push_str(value);
        out.push_str("\r\n");
    }
    if !has_content_length {
        out.push_str("content-length: ");
        out.push_str(response.body.len().to_string().as_str());
        out.push_str("\r\n");
    }
    out.push_str("\r\n");
    out.push_str(&response.body);
    out
}

fn bridge_health_response(
    runtime_state: &sdkwork_aiot_device_edge_runtime::MqttBridgeRuntimeState,
    stats: &BridgeStats,
) -> sdkwork_aiot_transport::HttpResponse {
    let runtime_snapshot = runtime_state.snapshot();
    let stats_snapshot = stats.snapshot();
    let body = bridge_health_snapshot_json(&runtime_snapshot, &stats_snapshot);
    sdkwork_aiot_transport::HttpResponse::new(sdkwork_aiot_transport::HttpStatus::Ok)
        .with_header("content-type", "application/json")
        .with_body(body)
}

fn bridge_stats_response(stats: &BridgeStats) -> sdkwork_aiot_transport::HttpResponse {
    let snapshot = stats.snapshot();
    let body = bridge_stats_snapshot_json(&snapshot);
    sdkwork_aiot_transport::HttpResponse::new(sdkwork_aiot_transport::HttpStatus::Ok)
        .with_header("content-type", "application/json")
        .with_body(body)
}

fn bridge_metrics_response(stats: &BridgeStats) -> sdkwork_aiot_transport::HttpResponse {
    let snapshot = stats.snapshot();
    let activation_registry_snapshot =
        sdkwork_aiot_device_edge_runtime::xiaozhi_activation_registry_stats_snapshot();
    let mut body = bridge_stats_prometheus_text(&snapshot);
    body.push_str(&activation_registry_prometheus_text(
        &activation_registry_snapshot,
    ));
    sdkwork_aiot_transport::HttpResponse::new(sdkwork_aiot_transport::HttpStatus::Ok)
        .with_header("content-type", "text/plain; version=0.0.4")
        .with_body(body)
}

fn bridge_sessions_response(
    bridge_control: &BridgeControlHandle,
) -> sdkwork_aiot_transport::HttpResponse {
    sdkwork_aiot_transport::HttpResponse::new(sdkwork_aiot_transport::HttpStatus::Ok)
        .with_header("content-type", "application/json")
        .with_body(bridge_control.list_sessions_json())
}

fn bridge_session_disconnect_response(
    bridge_control: &BridgeControlHandle,
    stats: &BridgeStats,
    session_id: &str,
) -> sdkwork_aiot_transport::HttpResponse {
    match bridge_control.disconnect_session(session_id, stats) {
        BridgeSessionDisconnectOutcome::Disconnected => {
            sdkwork_aiot_transport::HttpResponse::new(sdkwork_aiot_transport::HttpStatus::NoContent)
        }
        BridgeSessionDisconnectOutcome::NotFound => problem_json_response(
            sdkwork_aiot_transport::HttpStatus::NotFound,
            "iot.device-edge.bridge.session.not_found",
            &format!(r#""sessionId":"{}""#, json_escape(session_id)),
        ),
        BridgeSessionDisconnectOutcome::BridgeDisabled => problem_json_response(
            sdkwork_aiot_transport::HttpStatus::ServiceUnavailable,
            "iot.device-edge.bridge.disabled",
            r#""detail":"MQTT bridge is not enabled""#,
        ),
    }
}

fn problem_json_response(
    status: sdkwork_aiot_transport::HttpStatus,
    code: &str,
    extensions: &str,
) -> sdkwork_aiot_transport::HttpResponse {
    let body = format!(
        r#"{{"type":"about:blank","title":"{}","status":{},"code":"{}",{}}}"#,
        code,
        status.code(),
        code,
        extensions
    );
    sdkwork_aiot_transport::HttpResponse::new(status)
        .with_header("content-type", "application/problem+json")
        .with_body(body)
}

fn mcp_policy_stats_response(
    session_options: &sdkwork_aiot_device_edge_runtime::XiaozhiSessionOptions,
) -> sdkwork_aiot_transport::HttpResponse {
    let policy = session_options.mcp_tool_policy();
    let body = if let Some(snapshot) = policy.stats_snapshot() {
        format!(
            r#"{{"policy":"rule_based","allow_by_rule_matches":{},"allow_no_rule_matches":{},"deny_by_rule_matches":{}}}"#,
            snapshot.allow_by_rule_matches,
            snapshot.allow_no_rule_matches,
            snapshot.deny_by_rule_matches
        )
    } else {
        r#"{"policy":"custom","stats_available":false}"#.to_string()
    };
    sdkwork_aiot_transport::HttpResponse::new(sdkwork_aiot_transport::HttpStatus::Ok)
        .with_header("content-type", "application/json")
        .with_body(body)
}

fn activation_registry_stats_response() -> sdkwork_aiot_transport::HttpResponse {
    let snapshot = sdkwork_aiot_device_edge_runtime::xiaozhi_activation_registry_stats_snapshot();
    let body = format!(
        r#"{{"backend":"{}","register_total":{},"consume_total":{},"consume_hits":{},"consume_misses":{},"pruned_entries":{}}}"#,
        snapshot.backend_kind,
        snapshot.register_total,
        snapshot.consume_total,
        snapshot.consume_hits,
        snapshot.consume_misses,
        snapshot.pruned_entries
    );
    sdkwork_aiot_transport::HttpResponse::new(sdkwork_aiot_transport::HttpStatus::Ok)
        .with_header("content-type", "application/json")
        .with_body(body)
}

fn activation_registry_metrics_response() -> sdkwork_aiot_transport::HttpResponse {
    let snapshot = sdkwork_aiot_device_edge_runtime::xiaozhi_activation_registry_stats_snapshot();
    let body = activation_registry_prometheus_text(&snapshot);
    sdkwork_aiot_transport::HttpResponse::new(sdkwork_aiot_transport::HttpStatus::Ok)
        .with_header("content-type", "text/plain; version=0.0.4")
        .with_body(body)
}

fn activation_registry_prometheus_text(
    snapshot: &sdkwork_aiot_device_edge_runtime::XiaozhiActivationRegistryStatsSnapshot,
) -> String {
    let backend = json_escape(&snapshot.backend_kind);
    format!(
        "sdkwork_aiot_xiaozhi_activation_registry_register_total {}\n\
sdkwork_aiot_xiaozhi_activation_registry_consume_total {}\n\
sdkwork_aiot_xiaozhi_activation_registry_consume_hits_total {}\n\
sdkwork_aiot_xiaozhi_activation_registry_consume_misses_total {}\n\
sdkwork_aiot_xiaozhi_activation_registry_pruned_entries_total {}\n\
sdkwork_aiot_xiaozhi_activation_registry_backend{{backend=\"{}\"}} 1\n",
        snapshot.register_total,
        snapshot.consume_total,
        snapshot.consume_hits,
        snapshot.consume_misses,
        snapshot.pruned_entries,
        backend
    )
}

fn bridge_health_snapshot_json(
    runtime: &sdkwork_aiot_device_edge_runtime::MqttBridgeRuntimeSnapshot,
    stats: &BridgeStatsSnapshot,
) -> String {
    let status = sdkwork_aiot_device_edge_runtime::mqtt_bridge_health_status(runtime);
    let stats_json = bridge_stats_snapshot_json(stats);
    format!(
        r#"{{"status":"{}","bridge_enabled":{},"mqtt":{{"loop_running":{},"session_active":{}}},"udp":{{"loop_running":{},"socket_bound":{}}},"stats":{}}}"#,
        status,
        runtime.bridge_enabled,
        runtime.mqtt_loop_running,
        runtime.mqtt_session_active,
        runtime.udp_loop_running,
        runtime.udp_socket_bound,
        stats_json
    )
}

fn bridge_stats_snapshot_json(snapshot: &BridgeStatsSnapshot) -> String {
    format!(
        r#"{{"mqtt_reconnects":{},"mqtt_events_total":{},"mqtt_events_errors":{},"mqtt_session_errors":{},"mqtt_publish_attempts":{},"mqtt_publish_failures":{},"mqtt_publish_retries":{},"mqtt_publish_dropped":{},"udp_packets_total":{},"udp_decode_failures":{},"session_idle_purges":{},"udp_uplink_replies":{}}}"#,
        snapshot.mqtt_reconnects,
        snapshot.mqtt_events_total,
        snapshot.mqtt_events_errors,
        snapshot.mqtt_session_errors,
        snapshot.mqtt_publish_attempts,
        snapshot.mqtt_publish_failures,
        snapshot.mqtt_publish_retries,
        snapshot.mqtt_publish_dropped,
        snapshot.udp_packets_total,
        snapshot.udp_decode_failures,
        snapshot.session_idle_purges,
        snapshot.udp_uplink_replies
    )
}

fn bridge_stats_prometheus_text(snapshot: &BridgeStatsSnapshot) -> String {
    let lines = [
        (
            "sdkwork_aiot_bridge_mqtt_reconnects_total",
            snapshot.mqtt_reconnects,
        ),
        (
            "sdkwork_aiot_bridge_mqtt_events_total",
            snapshot.mqtt_events_total,
        ),
        (
            "sdkwork_aiot_bridge_mqtt_event_errors_total",
            snapshot.mqtt_events_errors,
        ),
        (
            "sdkwork_aiot_bridge_mqtt_session_errors_total",
            snapshot.mqtt_session_errors,
        ),
        (
            "sdkwork_aiot_bridge_mqtt_publish_attempts_total",
            snapshot.mqtt_publish_attempts,
        ),
        (
            "sdkwork_aiot_bridge_mqtt_publish_failures_total",
            snapshot.mqtt_publish_failures,
        ),
        (
            "sdkwork_aiot_bridge_mqtt_publish_retries_total",
            snapshot.mqtt_publish_retries,
        ),
        (
            "sdkwork_aiot_bridge_mqtt_publish_dropped_total",
            snapshot.mqtt_publish_dropped,
        ),
        (
            "sdkwork_aiot_bridge_udp_packets_total",
            snapshot.udp_packets_total,
        ),
        (
            "sdkwork_aiot_bridge_udp_decode_failures_total",
            snapshot.udp_decode_failures,
        ),
        (
            "sdkwork_aiot_bridge_session_idle_purges_total",
            snapshot.session_idle_purges,
        ),
        (
            "sdkwork_aiot_bridge_udp_uplink_replies_total",
            snapshot.udp_uplink_replies,
        ),
    ];

    let mut out = String::new();
    for (metric, value) in lines {
        out.push_str(metric);
        out.push(' ');
        out.push_str(&value.to_string());
        out.push('\n');
    }
    out
}

fn problem_response(code: &str) -> String {
    let body = format!(
        r#"{{"type":"about:blank","title":"Bad Request","status":400,"code":"{}"}}"#,
        json_escape(code)
    );
    format!(
        "HTTP/1.1 400 Bad Request\r\ncontent-type: application/problem+json\r\ncontent-length: {}\r\n\r\n{}",
        body.len(),
        body
    )
}

fn unauthorized_response(code: &str) -> String {
    let body = format!(
        r#"{{"type":"about:blank","title":"Unauthorized","status":401,"code":"{}"}}"#,
        json_escape(code)
    );
    format!(
        "HTTP/1.1 401 Unauthorized\r\ncontent-type: application/problem+json\r\ncontent-length: {}\r\n\r\n{}",
        body.len(),
        body
    )
}

fn service_unavailable_response(code: &str) -> String {
    let body = format!(
        r#"{{"type":"about:blank","title":"Service Unavailable","status":503,"code":"{}"}}"#,
        json_escape(code)
    );
    format!(
        "HTTP/1.1 503 Service Unavailable\r\ncontent-type: application/problem+json\r\ncontent-length: {}\r\n\r\n{}",
        body.len(),
        body
    )
}

struct ActiveWsConnectionGuard {
    counter: Arc<AtomicU64>,
}

impl Drop for ActiveWsConnectionGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::Relaxed);
    }
}

fn json_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch < ' ' => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out
}

fn sleep_reconnect_delay(attempt: u32, base: Duration, max: Duration) {
    let delay = reconnect_delay(attempt, base, max);
    std::thread::sleep(delay);
}

fn publish_with_retry(
    client: &Client,
    topic: &str,
    payload: String,
    retry_attempts: u32,
    retry_delay: Duration,
    stats: &BridgeStats,
) -> Result<(), String> {
    let mut attempts = 0u32;
    loop {
        match client.publish(topic, QoS::AtMostOnce, false, payload.clone()) {
            Ok(()) => return Ok(()),
            Err(error) => {
                if attempts >= retry_attempts {
                    stats.mqtt_publish_failures.fetch_add(1, Ordering::Relaxed);
                    return Err(error.to_string());
                }
                attempts = attempts.saturating_add(1);
                stats.mqtt_publish_retries.fetch_add(1, Ordering::Relaxed);
                std::thread::sleep(retry_delay);
            }
        }
    }
}

fn bounded_outbound_messages(messages: Vec<String>, max_outbound: usize) -> (Vec<String>, u64) {
    let limit = max_outbound.max(1);
    let dropped = messages.len().saturating_sub(limit) as u64;
    let bounded = messages.into_iter().take(limit).collect::<Vec<_>>();
    (bounded, dropped)
}

fn reconnect_delay(attempt: u32, base: Duration, max: Duration) -> Duration {
    let base_millis = duration_millis_u64(base).max(1);
    let max_millis = duration_millis_u64(max).max(base_millis);
    let shift = attempt.min(20);
    let factor = 1u64 << shift;
    let expanded = base_millis.saturating_mul(factor);
    Duration::from_millis(expanded.min(max_millis))
}

fn purge_idle_sessions(
    session_state: &Arc<Mutex<SessionState>>,
    session_idle_timeout: Duration,
) -> u64 {
    let now = current_unix_time_millis();
    let idle_timeout_millis = duration_millis_i64(session_idle_timeout).max(1);
    let mut guard = session_state.lock().expect("udp session lock");
    let before = guard.sessions.len();
    guard.sessions.retain(|_, managed| {
        !should_purge_idle_session(now, managed.last_activity_millis, idle_timeout_millis)
    });
    u64::try_from(before.saturating_sub(guard.sessions.len())).unwrap_or(u64::MAX)
}

fn should_purge_idle_session(
    now_millis: i64,
    last_activity_millis: i64,
    idle_timeout_millis: i64,
) -> bool {
    now_millis.saturating_sub(last_activity_millis) >= idle_timeout_millis
}

fn current_unix_time_millis() -> i64 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

fn duration_millis_u64(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn duration_millis_i64(duration: Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

fn env_string(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_u64(name: &str, default: u64) -> u64 {
    env_string(name)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_usize(name: &str, default: usize) -> usize {
    env_string(name)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default)
}

fn env_u16(name: &str, default: u16) -> u16 {
    env_string(name)
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_delay_uses_exponential_backoff_and_cap() {
        let base = Duration::from_millis(500);
        let max = Duration::from_millis(10_000);

        assert_eq!(reconnect_delay(0, base, max), Duration::from_millis(500));
        assert_eq!(reconnect_delay(1, base, max), Duration::from_millis(1_000));
        assert_eq!(reconnect_delay(3, base, max), Duration::from_millis(4_000));
        assert_eq!(
            reconnect_delay(10, base, max),
            Duration::from_millis(10_000)
        );
    }

    #[test]
    fn reconnect_delay_never_returns_zero() {
        let delay = reconnect_delay(0, Duration::from_millis(0), Duration::from_millis(0));
        assert_eq!(delay, Duration::from_millis(1));
    }

    #[test]
    fn should_purge_idle_session_detects_timeout_threshold() {
        assert!(!should_purge_idle_session(10_000, 9_001, 1_000));
        assert!(should_purge_idle_session(10_000, 9_000, 1_000));
        assert!(should_purge_idle_session(10_000, 8_500, 1_000));
    }

    #[test]
    fn purge_idle_sessions_clears_timed_out_entries() {
        let session = XiaozhiMqttUdpSession {
            device_id: "dev-001".to_string(),
            client_id: "client-001".to_string(),
            session_id: "dev-001-client-001".to_string(),
            udp_server: "127.0.0.1".to_string(),
            udp_port: 8888,
            udp_key_hex: "00112233445566778899AABBCCDDEEFF".to_string(),
            udp_nonce_hex: "01000000A1A2A3A40000000000000000".to_string(),
            remote_sequence: 42,
            local_sequence: 0,
        };
        let state = Arc::new(Mutex::new(SessionState {
            sessions: HashMap::from([(
                session.session_id.clone(),
                ManagedMqttUdpSession {
                    session,
                    last_activity_millis: 0,
                    udp_peer: None,
                },
            )]),
        }));

        let purged = purge_idle_sessions(&state, Duration::from_millis(1));
        assert_eq!(purged, 1);
        let guard = state.lock().expect("state lock");
        assert!(guard.sessions.is_empty());
    }

    #[test]
    fn bridge_stats_snapshot_json_contains_all_fields() {
        let snapshot = BridgeStatsSnapshot {
            mqtt_reconnects: 1,
            mqtt_events_total: 2,
            mqtt_events_errors: 3,
            mqtt_session_errors: 4,
            mqtt_publish_attempts: 5,
            mqtt_publish_failures: 6,
            mqtt_publish_retries: 7,
            mqtt_publish_dropped: 8,
            udp_packets_total: 9,
            udp_decode_failures: 10,
            session_idle_purges: 11,
            udp_uplink_replies: 0,
        };

        let json = bridge_stats_snapshot_json(&snapshot);
        assert!(json.contains(r#""mqtt_reconnects":1"#));
        assert!(json.contains(r#""mqtt_events_total":2"#));
        assert!(json.contains(r#""mqtt_events_errors":3"#));
        assert!(json.contains(r#""mqtt_session_errors":4"#));
        assert!(json.contains(r#""mqtt_publish_attempts":5"#));
        assert!(json.contains(r#""mqtt_publish_failures":6"#));
        assert!(json.contains(r#""mqtt_publish_retries":7"#));
        assert!(json.contains(r#""mqtt_publish_dropped":8"#));
        assert!(json.contains(r#""udp_packets_total":9"#));
        assert!(json.contains(r#""udp_decode_failures":10"#));
        assert!(json.contains(r#""session_idle_purges":11"#));
    }

    #[test]
    fn bridge_stats_prometheus_text_contains_expected_metrics() {
        let snapshot = BridgeStatsSnapshot {
            mqtt_reconnects: 1,
            mqtt_events_total: 2,
            mqtt_events_errors: 3,
            mqtt_session_errors: 4,
            mqtt_publish_attempts: 5,
            mqtt_publish_failures: 6,
            mqtt_publish_retries: 7,
            mqtt_publish_dropped: 8,
            udp_packets_total: 9,
            udp_decode_failures: 10,
            session_idle_purges: 11,
            udp_uplink_replies: 0,
        };

        let text = bridge_stats_prometheus_text(&snapshot);
        assert!(text.contains("sdkwork_aiot_bridge_mqtt_reconnects_total 1"));
        assert!(text.contains("sdkwork_aiot_bridge_mqtt_events_total 2"));
        assert!(text.contains("sdkwork_aiot_bridge_mqtt_event_errors_total 3"));
        assert!(text.contains("sdkwork_aiot_bridge_mqtt_session_errors_total 4"));
        assert!(text.contains("sdkwork_aiot_bridge_mqtt_publish_attempts_total 5"));
        assert!(text.contains("sdkwork_aiot_bridge_mqtt_publish_failures_total 6"));
        assert!(text.contains("sdkwork_aiot_bridge_mqtt_publish_retries_total 7"));
        assert!(text.contains("sdkwork_aiot_bridge_mqtt_publish_dropped_total 8"));
        assert!(text.contains("sdkwork_aiot_bridge_udp_packets_total 9"));
        assert!(text.contains("sdkwork_aiot_bridge_udp_decode_failures_total 10"));
        assert!(text.contains("sdkwork_aiot_bridge_session_idle_purges_total 11"));
    }

    #[test]
    fn bridge_health_snapshot_json_reports_runtime_status_and_stats() {
        let runtime = sdkwork_aiot_device_edge_runtime::MqttBridgeRuntimeSnapshot {
            bridge_enabled: true,
            mqtt_loop_running: true,
            mqtt_session_active: true,
            udp_loop_running: true,
            udp_socket_bound: true,
        };
        let stats = BridgeStatsSnapshot {
            mqtt_reconnects: 1,
            mqtt_events_total: 2,
            mqtt_events_errors: 3,
            mqtt_session_errors: 4,
            mqtt_publish_attempts: 5,
            mqtt_publish_failures: 6,
            mqtt_publish_retries: 7,
            mqtt_publish_dropped: 8,
            udp_packets_total: 9,
            udp_decode_failures: 10,
            session_idle_purges: 11,
            udp_uplink_replies: 0,
        };

        let json = bridge_health_snapshot_json(&runtime, &stats);
        assert!(json.contains(r#""status":"ok""#));
        assert!(json.contains(r#""bridge_enabled":true"#));
        assert!(json.contains(r#""mqtt":{"loop_running":true,"session_active":true}"#));
        assert!(json.contains(r#""udp":{"loop_running":true,"socket_bound":true}"#));
        assert!(json.contains(r#""stats":{"mqtt_reconnects":1"#));
    }

    #[test]
    fn bridge_health_snapshot_json_reports_disabled_when_bridge_is_off() {
        let runtime = sdkwork_aiot_device_edge_runtime::MqttBridgeRuntimeSnapshot {
            bridge_enabled: false,
            mqtt_loop_running: false,
            mqtt_session_active: false,
            udp_loop_running: false,
            udp_socket_bound: false,
        };
        let stats = BridgeStatsSnapshot {
            mqtt_reconnects: 0,
            mqtt_events_total: 0,
            mqtt_events_errors: 0,
            mqtt_session_errors: 0,
            mqtt_publish_attempts: 0,
            mqtt_publish_failures: 0,
            mqtt_publish_retries: 0,
            mqtt_publish_dropped: 0,
            udp_packets_total: 0,
            udp_decode_failures: 0,
            session_idle_purges: 0,
            udp_uplink_replies: 0,
        };

        let json = bridge_health_snapshot_json(&runtime, &stats);
        assert!(json.contains(r#""status":"disabled""#));
        assert!(json.contains(r#""bridge_enabled":false"#));
    }

    #[test]
    fn bounded_outbound_messages_limits_payload_and_reports_dropped_count() {
        let messages = vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
        ];
        let (bounded, dropped) = bounded_outbound_messages(messages, 2);
        assert_eq!(bounded, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(dropped, 2);
    }

    #[test]
    fn bridge_session_id_from_path_extracts_session_identifier() {
        assert_eq!(
            bridge_session_id_from_path("/internal/bridge/sessions/dev-001-client-001"),
            Some("dev-001-client-001")
        );
        assert_eq!(
            bridge_session_id_from_path("/internal/bridge/sessions/"),
            None
        );
        assert_eq!(
            bridge_session_id_from_path("/internal/bridge/sessions/a/b"),
            None
        );
    }

    #[test]
    fn bridge_control_disconnect_publishes_goodbye_and_removes_session() {
        let config = MqttBridgeConfig::from_env();
        let session_state = Arc::new(Mutex::new(SessionState::new()));
        let mqtt_publisher = Arc::new(MqttBridgePublisher::new(&config));
        let stats = BridgeStats::new(current_unix_time_millis());
        let control =
            BridgeControlHandle::enabled(Arc::clone(&session_state), Arc::clone(&mqtt_publisher));
        let session = XiaozhiMqttUdpSession {
            device_id: "dev-001".to_string(),
            client_id: "client-001".to_string(),
            session_id: "dev-001-client-001".to_string(),
            udp_server: "127.0.0.1".to_string(),
            udp_port: 8888,
            udp_key_hex: "00112233445566778899AABBCCDDEEFF".to_string(),
            udp_nonce_hex: "01000000A1A2A3A40000000000000000".to_string(),
            remote_sequence: 0,
            local_sequence: 0,
        };
        session_state
            .lock()
            .expect("state lock")
            .upsert_session(session.session_id.clone(), session);

        let outcome = control.disconnect_session("dev-001-client-001", &stats);
        assert_eq!(outcome, BridgeSessionDisconnectOutcome::Disconnected);
        assert!(session_state
            .lock()
            .expect("state lock")
            .sessions
            .is_empty());
    }

    #[test]
    fn bridge_control_disconnect_returns_not_found_for_unknown_session() {
        let config = MqttBridgeConfig::from_env();
        let session_state = Arc::new(Mutex::new(SessionState::new()));
        let mqtt_publisher = Arc::new(MqttBridgePublisher::new(&config));
        let stats = BridgeStats::new(current_unix_time_millis());
        let control = BridgeControlHandle::enabled(session_state, mqtt_publisher);

        let outcome = control.disconnect_session("missing-session", &stats);
        assert_eq!(outcome, BridgeSessionDisconnectOutcome::NotFound);
    }

    #[test]
    fn bridge_control_list_sessions_reports_disabled_state() {
        let control = BridgeControlHandle::disabled();
        assert_eq!(
            control.list_sessions_json(),
            r#"{"bridge_enabled":false,"sessions":[]}"#
        );
    }

    #[test]
    fn apply_bridge_session_state_removes_session_when_channel_closes() {
        let session = XiaozhiMqttUdpSession {
            device_id: "dev-001".to_string(),
            client_id: "client-001".to_string(),
            session_id: "dev-001-client-001".to_string(),
            udp_server: "127.0.0.1".to_string(),
            udp_port: 8888,
            udp_key_hex: "00112233445566778899AABBCCDDEEFF".to_string(),
            udp_nonce_hex: "01000000A1A2A3A40000000000000000".to_string(),
            remote_sequence: 0,
            local_sequence: 0,
        };
        let session_key = session.session_id.clone();
        let state = Mutex::new(SessionState {
            sessions: HashMap::from([(
                session_key.clone(),
                ManagedMqttUdpSession {
                    session,
                    last_activity_millis: 0,
                    udp_peer: None,
                },
            )]),
        });
        let reply =
            sdkwork_aiot_device_edge_runtime::xiaozhi_mqtt_server_teardown_reply(&session_key);

        apply_bridge_session_state(&state, Some(&session_key), &reply, None);

        let guard = state.lock().expect("state lock");
        assert!(guard.sessions.is_empty());
    }
}
