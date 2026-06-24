use aes::cipher::{KeyIvInit, StreamCipher};
use aes::Aes128;
use ctr::Ctr128BE;
use sdkwork_aiot_protocol::{
    CodecKind, HandshakeContext, InboundFrame, MessageClass, MessageCodec, OutboundFrame,
    ProtocolAdapterManifest, ProtocolEnvelope, ProtocolError, ProtocolPluginScope, SessionPolicy,
    TransportBinding,
};
use sdkwork_aiot_security::DeviceAuthMode;

mod opus_codec;
mod opus_uplink;
mod provider_downlink;

pub use opus_codec::{
    decode_xiaozhi_opus_packet_to_pcm16le as decode_xiaozhi_opus_uplink_to_pcm16le,
    decode_xiaozhi_opus_packets_to_pcm16le, decode_xiaozhi_opus_uplink_to_wav,
    encode_pcm16le_mono_to_opus_packets, wrap_pcm16le_mono_wav,
};
pub use opus_uplink::{
    XiaozhiOpusUplinkBuffer, XiaozhiSessionMediaProfile, DEFAULT_XIAOZHI_FRAME_DURATION_MS,
    DEFAULT_XIAOZHI_SAMPLE_RATE, MAX_UPLINK_BYTES, MAX_UPLINK_PACKETS,
};
pub use provider_downlink::{
    encode_provider_audio_to_xiaozhi_opus, encode_provider_pcm_to_xiaozhi_opus_packets,
    ProviderTtsAudio,
};

pub const XIAOZHI_BASE_PATH: &str = "/iot/xiaozhi";
pub const XIAOZHI_WS_PATH: &str = "/iot/xiaozhi/ws";
pub const XIAOZHI_OTA_PATH: &str = "/iot/xiaozhi/ota";
pub const XIAOZHI_ACTIVATE_PATH: &str = "/iot/xiaozhi/activate";
pub const XIAOZHI_OTA_ACTIVATE_PATH: &str = "/iot/xiaozhi/ota/activate";
pub const XIAOZHI_MQTT_PATH: &str = "/iot/xiaozhi/mqtt";
pub const XIAOZHI_UDP_PATH: &str = "/iot/xiaozhi/udp";

pub const AUTHORIZATION_HEADER: &str = "Authorization";
pub const PROTOCOL_VERSION_HEADER: &str = "Protocol-Version";
pub const DEVICE_ID_HEADER: &str = "Device-Id";
pub const CLIENT_ID_HEADER: &str = "Client-Id";

pub const XIAOZHI_WEBSOCKET_PROTOCOL_ID: &str = "xiaozhi.websocket";
pub const XIAOZHI_MQTT_UDP_PROTOCOL_ID: &str = "xiaozhi.mqtt_udp";

const XIAOZHI_BINARY_TYPE_OPUS: u16 = 0;
const XIAOZHI_BINARY_TYPE_JSON: u16 = 1;
const XIAOZHI_UDP_PACKET_TYPE_AUDIO: u8 = 0x01;

pub fn xiaozhi_manifest() -> ProtocolAdapterManifest {
    ProtocolAdapterManifest::new("xiaozhi", env!("CARGO_PKG_VERSION"))
        .with_scope(ProtocolPluginScope::CompatibilityPlugin)
        .with_protocol(XIAOZHI_WEBSOCKET_PROTOCOL_ID)
        .with_protocol(XIAOZHI_MQTT_UDP_PROTOCOL_ID)
        .with_transport(TransportBinding::WebSocket)
        .with_transport(TransportBinding::Http)
        .with_transport(TransportBinding::Mqtt)
        .with_transport(TransportBinding::Udp)
        .with_codec(CodecKind::JsonText)
        .with_codec(CodecKind::JsonRpc)
        .with_codec(CodecKind::BinaryMedia)
        .with_session_policy(SessionPolicy::StatefulDeviceSession)
        .with_capability_bridge("mcp_jsonrpc")
        .with_security_mode(DeviceAuthMode::BearerToken.manifest_name())
        .with_security_mode(DeviceAuthMode::Hmac.manifest_name())
        .with_ota_profile("xiaozhi_ota")
        .with_provisioning_profile("xiaozhi_activation")
        .with_hardware_family("esp32")
        .with_hardware_family("esp32_s3")
        .with_runtime_profile("esp_idf")
        .with_runtime_profile("freertos")
        .with_firmware_profile("xiaozhi_ota")
}

pub fn xiaozhi_routes() -> Vec<&'static str> {
    vec![
        XIAOZHI_WS_PATH,
        XIAOZHI_OTA_PATH,
        XIAOZHI_ACTIVATE_PATH,
        XIAOZHI_OTA_ACTIVATE_PATH,
        XIAOZHI_MQTT_PATH,
        XIAOZHI_UDP_PATH,
    ]
}

pub fn xiaozhi_handshake_context<I, K, V>(headers: I) -> HandshakeContext
where
    I: IntoIterator<Item = (K, V)>,
    K: Into<String>,
    V: Into<String>,
{
    headers.into_iter().fold(
        HandshakeContext::new(TransportBinding::WebSocket).with_path(XIAOZHI_WS_PATH),
        |ctx, (name, value)| ctx.with_header(name, value),
    )
}

pub fn map_xiaozhi_message_class(message_type: &str) -> Option<MessageClass> {
    match message_type {
        "hello" => Some(MessageClass::Handshake),
        "audio" => Some(MessageClass::MediaFrame),
        "listen" | "stt" | "tts" | "llm" | "alert" | "custom" => Some(MessageClass::Event),
        "iot" => Some(MessageClass::PropertyReport),
        "mcp" | "abort" | "system" => Some(MessageClass::CommandRequest),
        "firmware_complete" | "ota_complete" | "firmware_report" => Some(MessageClass::OtaDeploy),
        "firmware" | "ota" => Some(MessageClass::OtaCheck),
        "goodbye" => Some(MessageClass::Disconnect),
        _ => None,
    }
}

pub fn resolve_xiaozhi_message_class(message_type: &str, json: &str) -> Option<MessageClass> {
    if matches!(message_type, "firmware" | "ota")
        && xiaozhi_firmware_payload_indicates_completion(json)
    {
        return Some(MessageClass::OtaDeploy);
    }

    map_xiaozhi_message_class(message_type)
}

fn xiaozhi_firmware_payload_indicates_completion(json: &str) -> bool {
    for field in ["state", "status", "result"] {
        if let Some(value) = json_scalar_field(json, field) {
            let normalized = value.to_ascii_lowercase();
            if matches!(
                normalized.as_str(),
                "success" | "completed" | "applied" | "ok" | "done"
            ) {
                return true;
            }
        }
    }

    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XiaozhiAudioParams {
    pub format: String,
    pub sample_rate: u32,
    pub channels: u8,
    pub frame_duration: u32,
}

impl XiaozhiAudioParams {
    pub fn opus(sample_rate: u32, channels: u8, frame_duration: u32) -> Self {
        Self {
            format: "opus".to_string(),
            sample_rate,
            channels,
            frame_duration,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XiaozhiServerHello {
    pub transport: String,
    pub session_id: String,
    pub audio_params: Option<XiaozhiAudioParams>,
    pub udp: Option<XiaozhiMqttUdpProfileConfig>,
}

impl XiaozhiServerHello {
    pub fn websocket(session_id: impl Into<String>) -> Self {
        Self {
            transport: "websocket".to_string(),
            session_id: session_id.into(),
            audio_params: None,
            udp: None,
        }
    }

    pub fn mqtt_udp(
        session_id: impl Into<String>,
        server: impl Into<String>,
        port: u16,
        key_hex: impl Into<String>,
        nonce_hex: impl Into<String>,
    ) -> Self {
        Self {
            transport: "udp".to_string(),
            session_id: session_id.into(),
            audio_params: None,
            udp: Some(XiaozhiMqttUdpProfileConfig {
                server: server.into(),
                port,
                key_hex: key_hex.into(),
                nonce_hex: nonce_hex.into(),
            }),
        }
    }

    pub fn with_audio_params(mut self, audio_params: XiaozhiAudioParams) -> Self {
        self.audio_params = Some(audio_params);
        self
    }
}

pub fn xiaozhi_server_hello_response(hello: XiaozhiServerHello) -> String {
    let mut out = format!(
        r#"{{"type":"hello","transport":"{}","session_id":"{}""#,
        json_escape(&hello.transport),
        json_escape(&hello.session_id)
    );

    if let Some(audio_params) = hello.audio_params {
        out.push_str(r#","audio_params":"#);
        out.push_str(&audio_params_json(&audio_params));
    }

    if let Some(udp) = hello.udp {
        out.push_str(r#","udp":{"server":""#);
        out.push_str(&json_escape(&udp.server));
        out.push_str(r#"","port":"#);
        out.push_str(&udp.port.to_string());
        out.push_str(r#","key":""#);
        out.push_str(&json_escape(&udp.key_hex));
        out.push_str(r#"","nonce":""#);
        out.push_str(&json_escape(&udp.nonce_hex));
        out.push_str(r#""}"#);
    }

    out.push('}');
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XiaozhiWebSocketOtaConfig {
    pub url: String,
    pub token: String,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XiaozhiMqttOtaConfig {
    pub endpoint: String,
    pub client_id: String,
    pub username: String,
    pub password: String,
    pub publish_topic: String,
    pub subscribe_topic: String,
    pub keepalive: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XiaozhiMqttUdpProfileConfig {
    pub server: String,
    pub port: u16,
    pub key_hex: String,
    pub nonce_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XiaozhiFirmwareOtaConfig {
    pub version: String,
    pub url: String,
    pub force: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XiaozhiActivationConfig {
    pub message: String,
    pub code: Option<String>,
    pub challenge: Option<String>,
    pub timeout_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XiaozhiServerTime {
    pub timestamp: i64,
    pub timezone_offset: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XiaozhiOtaMetadata {
    pub websocket: Option<XiaozhiWebSocketOtaConfig>,
    pub mqtt: Option<XiaozhiMqttOtaConfig>,
    pub udp: Option<XiaozhiMqttUdpProfileConfig>,
    pub firmware: Option<XiaozhiFirmwareOtaConfig>,
    pub activation: Option<XiaozhiActivationConfig>,
    pub server_time: Option<XiaozhiServerTime>,
}

impl XiaozhiOtaMetadata {
    pub fn new() -> Self {
        Self {
            websocket: None,
            mqtt: None,
            udp: None,
            firmware: None,
            activation: None,
            server_time: None,
        }
    }

    pub fn with_mqtt_udp(
        mut self,
        server: impl Into<String>,
        port: u16,
        key_hex: impl Into<String>,
        nonce_hex: impl Into<String>,
    ) -> Self {
        self.udp = Some(XiaozhiMqttUdpProfileConfig {
            server: server.into(),
            port,
            key_hex: key_hex.into(),
            nonce_hex: nonce_hex.into(),
        });
        self
    }

    pub fn with_websocket(
        mut self,
        url: impl Into<String>,
        token: impl Into<String>,
        version: u32,
    ) -> Self {
        self.websocket = Some(XiaozhiWebSocketOtaConfig {
            url: url.into(),
            token: token.into(),
            version,
        });
        self
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_mqtt(
        mut self,
        endpoint: impl Into<String>,
        client_id: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
        publish_topic: impl Into<String>,
        subscribe_topic: impl Into<String>,
        keepalive: u32,
    ) -> Self {
        self.mqtt = Some(XiaozhiMqttOtaConfig {
            endpoint: endpoint.into(),
            client_id: client_id.into(),
            username: username.into(),
            password: password.into(),
            publish_topic: publish_topic.into(),
            subscribe_topic: subscribe_topic.into(),
            keepalive,
        });
        self
    }

    pub fn with_firmware(
        mut self,
        version: impl Into<String>,
        url: impl Into<String>,
        force: bool,
    ) -> Self {
        self.firmware = Some(XiaozhiFirmwareOtaConfig {
            version: version.into(),
            url: url.into(),
            force: u8::from(force),
        });
        self
    }

    pub fn with_activation_code(
        mut self,
        message: impl Into<String>,
        code: impl Into<String>,
        timeout_ms: u32,
    ) -> Self {
        self.activation = Some(XiaozhiActivationConfig {
            message: message.into(),
            code: Some(code.into()),
            challenge: None,
            timeout_ms,
        });
        self
    }

    pub fn with_activation_challenge(
        mut self,
        message: impl Into<String>,
        challenge: impl Into<String>,
        timeout_ms: u32,
    ) -> Self {
        self.activation = Some(XiaozhiActivationConfig {
            message: message.into(),
            code: None,
            challenge: Some(challenge.into()),
            timeout_ms,
        });
        self
    }

    pub fn with_server_time(mut self, timestamp: i64, timezone_offset: i32) -> Self {
        self.server_time = Some(XiaozhiServerTime {
            timestamp,
            timezone_offset,
        });
        self
    }
}

impl Default for XiaozhiOtaMetadata {
    fn default() -> Self {
        Self::new()
    }
}

pub fn xiaozhi_ota_response(metadata: XiaozhiOtaMetadata) -> String {
    let mut fields = Vec::new();

    if let Some(websocket) = metadata.websocket {
        fields.push(format!(
            r#""websocket":{{"url":"{}","token":"{}","version":{}}}"#,
            json_escape(&websocket.url),
            json_escape(&websocket.token),
            websocket.version
        ));
    }

    if let Some(mqtt) = metadata.mqtt {
        fields.push(format!(
            r#""mqtt":{{"endpoint":"{}","client_id":"{}","username":"{}","password":"{}","publish_topic":"{}","subscribe_topic":"{}","keepalive":{}}}"#,
            json_escape(&mqtt.endpoint),
            json_escape(&mqtt.client_id),
            json_escape(&mqtt.username),
            json_escape(&mqtt.password),
            json_escape(&mqtt.publish_topic),
            json_escape(&mqtt.subscribe_topic),
            mqtt.keepalive
        ));
    }

    if let Some(udp) = metadata.udp {
        fields.push(format!(
            r#""udp":{{"server":"{}","port":{},"key":"{}","nonce":"{}"}}"#,
            json_escape(&udp.server),
            udp.port,
            json_escape(&udp.key_hex),
            json_escape(&udp.nonce_hex),
        ));
    }

    if let Some(firmware) = metadata.firmware {
        fields.push(format!(
            r#""firmware":{{"version":"{}","url":"{}","force":{}}}"#,
            json_escape(&firmware.version),
            json_escape(&firmware.url),
            firmware.force
        ));
    }

    if let Some(activation) = metadata.activation {
        let mut activation_fields = vec![format!(
            r#""message":"{}""#,
            json_escape(&activation.message)
        )];
        if let Some(code) = activation.code {
            activation_fields.push(format!(r#""code":"{}""#, json_escape(&code)));
        }
        if let Some(challenge) = activation.challenge {
            activation_fields.push(format!(r#""challenge":"{}""#, json_escape(&challenge)));
        }
        activation_fields.push(format!(r#""timeout_ms":{}"#, activation.timeout_ms));
        fields.push(format!(
            r#""activation":{{{}}}"#,
            activation_fields.join(",")
        ));
    }

    if let Some(server_time) = metadata.server_time {
        fields.push(format!(
            r#""server_time":{{"timestamp":{},"timezone_offset":{}}}"#,
            server_time.timestamp, server_time.timezone_offset
        ));
    }

    format!("{{{}}}", fields.join(","))
}

pub fn xiaozhi_activation_pending_response(message: &str) -> String {
    format!(
        r#"{{"activation":{{"status":"pending","message":"{}"}}}}"#,
        json_escape(message)
    )
}

pub fn xiaozhi_activation_accepted_response() -> String {
    r#"{"activation":{"status":"accepted"}}"#.to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XiaozhiWebSocketCodec {
    handshake_context: Option<HandshakeContext>,
}

impl XiaozhiWebSocketCodec {
    pub fn new() -> Self {
        Self {
            handshake_context: None,
        }
    }

    pub fn with_handshake_context(mut self, context: HandshakeContext) -> Self {
        self.handshake_context = Some(context);
        self
    }

    fn context_header(&self, name: &str) -> Option<&str> {
        self.handshake_context.as_ref().and_then(|context| {
            context.header(name).or_else(|| {
                context
                    .headers
                    .iter()
                    .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
                    .map(|(_, value)| value.as_str())
            })
        })
    }
}

impl Default for XiaozhiWebSocketCodec {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XiaozhiMqttCodec {
    device_id: Option<String>,
    client_id: Option<String>,
}

impl XiaozhiMqttCodec {
    pub fn new() -> Self {
        Self {
            device_id: None,
            client_id: None,
        }
    }

    pub fn with_device_and_client(
        mut self,
        device_id: impl Into<String>,
        client_id: impl Into<String>,
    ) -> Self {
        self.device_id = Some(device_id.into());
        self.client_id = Some(client_id.into());
        self
    }
}

impl Default for XiaozhiMqttCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageCodec for XiaozhiMqttCodec {
    fn decode(&self, frame: InboundFrame) -> Result<ProtocolEnvelope, ProtocolError> {
        if frame.binary {
            return Err(ProtocolError::new(
                "xiaozhi.mqtt.payload.unsupported_binary",
                "xiaozhi mqtt control channel requires UTF-8 JSON payload",
            ));
        }

        let text = std::str::from_utf8(&frame.payload).map_err(|_| {
            ProtocolError::new(
                "xiaozhi.payload.invalid_utf8",
                "xiaozhi text frames must be valid UTF-8 JSON",
            )
        })?;

        let message_type = json_string_field(text, "type").ok_or_else(|| {
            ProtocolError::new(
                "xiaozhi.message_type.missing",
                "xiaozhi JSON frame must contain a type field",
            )
        })?;
        let message_class =
            resolve_xiaozhi_message_class(&message_type, text).ok_or_else(|| {
                ProtocolError::new(
                    "xiaozhi.message_type.unsupported",
                    format!("unsupported xiaozhi message type: {message_type}"),
                )
            })?;

        let mut builder = ProtocolEnvelope::builder(XIAOZHI_MQTT_UDP_PROTOCOL_ID, message_class)
            .adapter("xiaozhi")
            .semantic_type(&message_type)
            .json_payload(text)
            .extension("xiaozhi.transport", "udp");

        if let Some(protocol_version) = json_scalar_field(text, "version") {
            builder = builder.protocol_version(protocol_version);
        }

        if let Some(device_id) = self.device_id.clone() {
            builder = builder.device(device_id);
        } else if let Some(device_id) = json_string_field(text, "device_id") {
            builder = builder.device(device_id);
        }

        if let Some(client_id) = self.client_id.clone() {
            builder = builder.client(client_id);
        } else if let Some(client_id) = json_string_field(text, "client_id") {
            builder = builder.client(client_id);
        }

        if let Some(session_id) = json_string_field(text, "session_id") {
            builder = builder.session(session_id);
        }

        if let Some(message_id) = json_string_field(text, "message_id") {
            builder = builder.message_id(message_id);
        }

        if let Some(correlation_id) = json_scalar_field(text, "correlation_id") {
            builder = builder.correlation_id(correlation_id);
        }

        builder = apply_features(builder, text);
        builder = apply_audio_params(builder, text);
        builder = apply_message_type_extensions(builder, text, &message_type);

        Ok(builder.build())
    }

    fn encode(&self, envelope: ProtocolEnvelope) -> Result<OutboundFrame, ProtocolError> {
        if envelope.protocol_id != XIAOZHI_MQTT_UDP_PROTOCOL_ID {
            return Err(ProtocolError::new(
                "xiaozhi.protocol.unsupported",
                "xiaozhi mqtt codec can only encode xiaozhi.mqtt_udp envelopes",
            ));
        }

        if envelope.payload_encoding == "binary" {
            return Err(ProtocolError::new(
                "xiaozhi.mqtt.payload.unsupported_binary",
                "xiaozhi mqtt control channel does not carry binary media payloads",
            ));
        }

        String::from_utf8(envelope.payload)
            .map(OutboundFrame::text)
            .map_err(|_| {
                ProtocolError::new(
                    "xiaozhi.payload.invalid_utf8",
                    "xiaozhi text frames must be valid UTF-8 JSON",
                )
            })
    }
}

type Aes128Ctr = Ctr128BE<Aes128>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XiaozhiUdpAudioPacket {
    pub flags: u8,
    pub ssrc: u32,
    pub timestamp: u32,
    pub sequence: u32,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XiaozhiUdpAudioCodec {
    key: [u8; 16],
    nonce_prefix: [u8; 16],
}

impl XiaozhiUdpAudioCodec {
    pub fn new(key_hex: &str, nonce_hex: &str) -> Result<Self, ProtocolError> {
        Ok(Self {
            key: decode_hex_16(key_hex, "xiaozhi.udp.key.invalid_hex")?,
            nonce_prefix: decode_hex_16(nonce_hex, "xiaozhi.udp.nonce.invalid_hex")?,
        })
    }

    pub fn encode_audio_packet(
        &self,
        timestamp: u32,
        sequence: u32,
        payload: impl AsRef<[u8]>,
    ) -> Result<Vec<u8>, ProtocolError> {
        let payload = payload.as_ref();
        let payload_len = u16::try_from(payload.len()).map_err(|_| {
            ProtocolError::new(
                "xiaozhi.udp.payload_too_large",
                "xiaozhi udp payload length exceeds u16",
            )
        })?;

        let mut nonce = self.nonce_prefix;
        nonce[2..4].copy_from_slice(&payload_len.to_be_bytes());
        nonce[8..12].copy_from_slice(&timestamp.to_be_bytes());
        nonce[12..16].copy_from_slice(&sequence.to_be_bytes());

        let mut encrypted_payload = payload.to_vec();
        let mut cipher = Aes128Ctr::new(&self.key.into(), &nonce.into());
        cipher.apply_keystream(&mut encrypted_payload);

        let mut packet = Vec::with_capacity(16 + encrypted_payload.len());
        packet.extend_from_slice(&nonce);
        packet.extend_from_slice(&encrypted_payload);
        Ok(packet)
    }

    pub fn decode_audio_packet(
        &self,
        packet: &[u8],
    ) -> Result<XiaozhiUdpAudioPacket, ProtocolError> {
        self.decode_audio_packet_with_min_sequence(packet, 0)
    }

    pub fn decode_audio_packet_with_min_sequence(
        &self,
        packet: &[u8],
        min_sequence: u32,
    ) -> Result<XiaozhiUdpAudioPacket, ProtocolError> {
        if packet.len() < 16 {
            return Err(ProtocolError::new(
                "xiaozhi.udp.packet.too_short",
                "xiaozhi udp packet must include a 16-byte header",
            ));
        }

        if packet[0] != XIAOZHI_UDP_PACKET_TYPE_AUDIO {
            return Err(ProtocolError::new(
                "xiaozhi.udp.packet.unsupported_type",
                format!("unsupported xiaozhi udp packet type: {}", packet[0]),
            ));
        }

        let flags = packet[1];
        let payload_len = u16::from_be_bytes([packet[2], packet[3]]) as usize;
        if packet.len() != 16 + payload_len {
            return Err(ProtocolError::new(
                "xiaozhi.udp.packet.payload_size_mismatch",
                "xiaozhi udp packet payload size does not match packet length",
            ));
        }

        let ssrc = u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]);
        let timestamp = u32::from_be_bytes([packet[8], packet[9], packet[10], packet[11]]);
        let sequence = u32::from_be_bytes([packet[12], packet[13], packet[14], packet[15]]);
        if sequence < min_sequence {
            return Err(ProtocolError::new(
                "xiaozhi.udp.sequence.stale",
                format!("stale xiaozhi udp sequence: {sequence} < {min_sequence}"),
            ));
        }

        let nonce = [
            packet[0], packet[1], packet[2], packet[3], packet[4], packet[5], packet[6], packet[7],
            packet[8], packet[9], packet[10], packet[11], packet[12], packet[13], packet[14],
            packet[15],
        ];
        let mut decrypted_payload = packet[16..].to_vec();
        let mut cipher = Aes128Ctr::new(&self.key.into(), &nonce.into());
        cipher.apply_keystream(&mut decrypted_payload);

        Ok(XiaozhiUdpAudioPacket {
            flags,
            ssrc,
            timestamp,
            sequence,
            payload: decrypted_payload,
        })
    }
}

impl MessageCodec for XiaozhiWebSocketCodec {
    fn decode(&self, frame: InboundFrame) -> Result<ProtocolEnvelope, ProtocolError> {
        if frame.binary {
            return self.decode_binary(frame.payload);
        }

        let text = std::str::from_utf8(&frame.payload).map_err(|_| {
            ProtocolError::new(
                "xiaozhi.payload.invalid_utf8",
                "xiaozhi text frames must be valid UTF-8 JSON",
            )
        })?;
        self.decode_text(text)
    }

    fn encode(&self, envelope: ProtocolEnvelope) -> Result<OutboundFrame, ProtocolError> {
        if envelope.protocol_id != XIAOZHI_WEBSOCKET_PROTOCOL_ID {
            return Err(ProtocolError::new(
                "xiaozhi.protocol.unsupported",
                "xiaozhi websocket codec can only encode xiaozhi.websocket envelopes",
            ));
        }

        if envelope.payload_encoding == "binary" {
            self.encode_binary(envelope)
        } else {
            String::from_utf8(envelope.payload)
                .map(OutboundFrame::text)
                .map_err(|_| {
                    ProtocolError::new(
                        "xiaozhi.payload.invalid_utf8",
                        "xiaozhi text frames must be valid UTF-8 JSON",
                    )
                })
        }
    }
}

impl XiaozhiWebSocketCodec {
    fn encode_binary(&self, envelope: ProtocolEnvelope) -> Result<OutboundFrame, ProtocolError> {
        let payload_len = envelope.payload.len();
        match self
            .context_header(PROTOCOL_VERSION_HEADER)
            .or(envelope.protocol_version.as_deref())
            .unwrap_or("1")
        {
            "2" => {
                let payload_size = u32::try_from(payload_len).map_err(|_| {
                    ProtocolError::new(
                        "xiaozhi.binary.payload_too_large",
                        "xiaozhi binary protocol v2 payload length exceeds u32",
                    )
                })?;
                let timestamp = envelope
                    .extensions
                    .get("xiaozhi.audio.timestamp_ms")
                    .and_then(|value| value.parse::<u32>().ok())
                    .unwrap_or(0);
                let mut frame = Vec::with_capacity(16 + payload_len);
                frame.extend_from_slice(&2u16.to_be_bytes());
                frame.extend_from_slice(&XIAOZHI_BINARY_TYPE_OPUS.to_be_bytes());
                frame.extend_from_slice(&0u32.to_be_bytes());
                frame.extend_from_slice(&timestamp.to_be_bytes());
                frame.extend_from_slice(&payload_size.to_be_bytes());
                frame.extend_from_slice(&envelope.payload);
                Ok(OutboundFrame::binary(frame))
            }
            "3" => {
                let payload_size = u16::try_from(payload_len).map_err(|_| {
                    ProtocolError::new(
                        "xiaozhi.binary.payload_too_large",
                        "xiaozhi binary protocol v3 payload length exceeds u16",
                    )
                })?;
                let mut frame = Vec::with_capacity(4 + payload_len);
                frame.push(XIAOZHI_BINARY_TYPE_OPUS as u8);
                frame.push(0);
                frame.extend_from_slice(&payload_size.to_be_bytes());
                frame.extend_from_slice(&envelope.payload);
                Ok(OutboundFrame::binary(frame))
            }
            _ => Ok(OutboundFrame::binary(envelope.payload)),
        }
    }

    fn decode_text(&self, text: &str) -> Result<ProtocolEnvelope, ProtocolError> {
        let message_type = json_string_field(text, "type").ok_or_else(|| {
            ProtocolError::new(
                "xiaozhi.message_type.missing",
                "xiaozhi JSON frame must contain a type field",
            )
        })?;
        let message_class =
            resolve_xiaozhi_message_class(&message_type, text).ok_or_else(|| {
                ProtocolError::new(
                    "xiaozhi.message_type.unsupported",
                    format!("unsupported xiaozhi message type: {message_type}"),
                )
            })?;

        let mut builder = apply_context(
            ProtocolEnvelope::builder(XIAOZHI_WEBSOCKET_PROTOCOL_ID, message_class),
            self,
        )
        .semantic_type(&message_type)
        .json_payload(text);

        if let Some(protocol_version) = self
            .context_header(PROTOCOL_VERSION_HEADER)
            .map(str::to_string)
            .or_else(|| json_scalar_field(text, "version"))
        {
            builder = builder.protocol_version(protocol_version);
        }

        if let Some(device_id) = json_string_field(text, "device_id") {
            builder = builder.device(device_id);
        }

        if let Some(client_id) = json_string_field(text, "client_id") {
            builder = builder.client(client_id);
        }

        if let Some(session_id) = json_string_field(text, "session_id") {
            builder = builder.session(session_id);
        }

        if let Some(message_id) = json_string_field(text, "message_id") {
            builder = builder.message_id(message_id);
        }

        let explicit_correlation_id = json_scalar_field(text, "correlation_id");
        if let Some(correlation_id) = explicit_correlation_id {
            builder = builder.correlation_id(correlation_id);
        }

        if let Some(idempotency_key) = json_string_field(text, "idempotency_key") {
            builder = builder.idempotency_key(idempotency_key);
        }

        if let Some(trace_id) = json_string_field(text, "trace_id") {
            builder = builder.trace_id(trace_id);
        }

        if let Some(transport) = json_string_field(text, "transport") {
            builder = builder.extension("xiaozhi.transport", transport);
        }

        builder = apply_features(builder, text);
        builder = apply_audio_params(builder, text);
        builder = apply_message_type_extensions(builder, text, &message_type);

        Ok(builder.build())
    }

    fn decode_binary(&self, payload: Vec<u8>) -> Result<ProtocolEnvelope, ProtocolError> {
        match self.context_header(PROTOCOL_VERSION_HEADER).unwrap_or("1") {
            "2" => self.decode_binary_protocol_v2(payload),
            "3" => self.decode_binary_protocol_v3(payload),
            version => Ok(self
                .binary_audio_builder(payload)
                .extension("xiaozhi.binary.protocol_version", version)
                .extension("xiaozhi.binary.message_type", "opus")
                .build()),
        }
    }

    fn decode_binary_protocol_v2(&self, frame: Vec<u8>) -> Result<ProtocolEnvelope, ProtocolError> {
        if frame.len() < 16 {
            return Err(ProtocolError::new(
                "xiaozhi.binary.short_header",
                "xiaozhi binary protocol v2 requires a 16-byte header",
            ));
        }

        let version = u16::from_be_bytes([frame[0], frame[1]]);
        let message_type = u16::from_be_bytes([frame[2], frame[3]]);
        let timestamp = u32::from_be_bytes([frame[8], frame[9], frame[10], frame[11]]);
        let payload_size = u32::from_be_bytes([frame[12], frame[13], frame[14], frame[15]]);
        let payload_size = usize::try_from(payload_size).map_err(|_| {
            ProtocolError::new(
                "xiaozhi.binary.payload_too_large",
                "xiaozhi binary protocol v2 payload size overflows usize",
            )
        })?;

        if frame.len() != 16 + payload_size {
            return Err(ProtocolError::new(
                "xiaozhi.binary.payload_size_mismatch",
                "xiaozhi binary protocol v2 payload size does not match frame length",
            ));
        }

        let inner_payload = frame[16..].to_vec();
        if message_type == XIAOZHI_BINARY_TYPE_JSON {
            let text = std::str::from_utf8(&inner_payload).map_err(|_| {
                ProtocolError::new(
                    "xiaozhi.payload.invalid_utf8",
                    "xiaozhi binary protocol v2 JSON payload must be UTF-8",
                )
            })?;
            return self.decode_text(text);
        }

        if message_type != XIAOZHI_BINARY_TYPE_OPUS {
            return Err(ProtocolError::new(
                "xiaozhi.binary.unsupported_message_type",
                format!("unsupported xiaozhi binary protocol v2 message type: {message_type}"),
            ));
        }

        Ok(self
            .binary_audio_builder(inner_payload)
            .extension("xiaozhi.binary.protocol_version", version.to_string())
            .extension("xiaozhi.binary.message_type", "opus")
            .extension("xiaozhi.audio.timestamp_ms", timestamp.to_string())
            .build())
    }

    fn decode_binary_protocol_v3(&self, frame: Vec<u8>) -> Result<ProtocolEnvelope, ProtocolError> {
        if frame.len() < 4 {
            return Err(ProtocolError::new(
                "xiaozhi.binary.short_header",
                "xiaozhi binary protocol v3 requires a 4-byte header",
            ));
        }

        let message_type = frame[0] as u16;
        let payload_size = u16::from_be_bytes([frame[2], frame[3]]) as usize;
        if frame.len() != 4 + payload_size {
            return Err(ProtocolError::new(
                "xiaozhi.binary.payload_size_mismatch",
                "xiaozhi binary protocol v3 payload size does not match frame length",
            ));
        }

        let inner_payload = frame[4..].to_vec();
        if message_type == XIAOZHI_BINARY_TYPE_JSON {
            let text = std::str::from_utf8(&inner_payload).map_err(|_| {
                ProtocolError::new(
                    "xiaozhi.payload.invalid_utf8",
                    "xiaozhi binary protocol v3 JSON payload must be UTF-8",
                )
            })?;
            return self.decode_text(text);
        }

        if message_type != XIAOZHI_BINARY_TYPE_OPUS {
            return Err(ProtocolError::new(
                "xiaozhi.binary.unsupported_message_type",
                format!("unsupported xiaozhi binary protocol v3 message type: {message_type}"),
            ));
        }

        Ok(self
            .binary_audio_builder(inner_payload)
            .extension("xiaozhi.binary.protocol_version", "3")
            .extension("xiaozhi.binary.message_type", "opus")
            .build())
    }

    fn binary_audio_builder(
        &self,
        payload: Vec<u8>,
    ) -> sdkwork_aiot_protocol::ProtocolEnvelopeBuilder {
        apply_context(
            ProtocolEnvelope::builder(XIAOZHI_WEBSOCKET_PROTOCOL_ID, MessageClass::MediaFrame),
            self,
        )
        .semantic_type("audio")
        .binary_payload(payload)
    }
}

fn apply_context(
    mut builder: sdkwork_aiot_protocol::ProtocolEnvelopeBuilder,
    codec: &XiaozhiWebSocketCodec,
) -> sdkwork_aiot_protocol::ProtocolEnvelopeBuilder {
    builder = builder.adapter("xiaozhi");

    if let Some(protocol_version) = codec.context_header(PROTOCOL_VERSION_HEADER) {
        builder = builder.protocol_version(protocol_version);
    }

    if let Some(device_id) = codec.context_header(DEVICE_ID_HEADER) {
        builder = builder.device(device_id);
    }

    if let Some(client_id) = codec.context_header(CLIENT_ID_HEADER) {
        builder = builder.client(client_id);
    }

    builder
}

fn apply_features(
    mut builder: sdkwork_aiot_protocol::ProtocolEnvelopeBuilder,
    json: &str,
) -> sdkwork_aiot_protocol::ProtocolEnvelopeBuilder {
    if let Some(features) = json_object_field(json, "features") {
        for feature in ["mcp", "aec"] {
            if let Some(value) = json_scalar_field(features, feature) {
                builder = builder.extension(format!("xiaozhi.feature.{feature}"), value);
            }
        }
    }

    builder
}

fn apply_audio_params(
    mut builder: sdkwork_aiot_protocol::ProtocolEnvelopeBuilder,
    json: &str,
) -> sdkwork_aiot_protocol::ProtocolEnvelopeBuilder {
    if let Some(audio_params) = json_object_field(json, "audio_params") {
        for (field, extension) in [
            ("format", "xiaozhi.audio.format"),
            ("sample_rate", "xiaozhi.audio.sample_rate"),
            ("channels", "xiaozhi.audio.channels"),
            ("frame_duration", "xiaozhi.audio.frame_duration"),
        ] {
            if let Some(value) = json_scalar_field(audio_params, field) {
                builder = builder.extension(extension, value);
            }
        }
    }

    builder
}

fn apply_message_type_extensions(
    mut builder: sdkwork_aiot_protocol::ProtocolEnvelopeBuilder,
    json: &str,
    message_type: &str,
) -> sdkwork_aiot_protocol::ProtocolEnvelopeBuilder {
    match message_type {
        "listen" => {
            for (field, extension) in [
                ("state", "xiaozhi.listen.state"),
                ("mode", "xiaozhi.listen.mode"),
                ("text", "xiaozhi.listen.text"),
            ] {
                if let Some(value) = json_scalar_field(json, field) {
                    builder = builder.extension(extension, value);
                }
            }
        }
        "abort" => {
            if let Some(reason) = json_scalar_field(json, "reason") {
                builder = builder.extension("xiaozhi.abort.reason", reason);
            }
        }
        "system" => {
            if let Some(command) = json_scalar_field(json, "command") {
                builder = builder.extension("xiaozhi.system.command", command);
            }
        }
        "tts" => {
            for (field, extension) in [("state", "xiaozhi.tts.state"), ("text", "xiaozhi.tts.text")]
            {
                if let Some(value) = json_scalar_field(json, field) {
                    builder = builder.extension(extension, value);
                }
            }
        }
        "stt" => {
            if let Some(text) = json_scalar_field(json, "text") {
                builder = builder.extension("xiaozhi.stt.text", text);
            }
        }
        "llm" => {
            for (field, extension) in [
                ("emotion", "xiaozhi.llm.emotion"),
                ("text", "xiaozhi.llm.text"),
            ] {
                if let Some(value) = json_scalar_field(json, field) {
                    builder = builder.extension(extension, value);
                }
            }
        }
        "alert" => {
            for (field, extension) in [
                ("status", "xiaozhi.alert.status"),
                ("message", "xiaozhi.alert.message"),
                ("emotion", "xiaozhi.alert.emotion"),
            ] {
                if let Some(value) = json_scalar_field(json, field) {
                    builder = builder.extension(extension, value);
                }
            }
        }
        "custom" => {
            if json_field_value_range(json, "payload").is_some() {
                builder = builder.extension("xiaozhi.custom.payload", "present");
            }
        }
        "mcp" => {
            builder = apply_mcp_extensions(builder, json);
        }
        _ => {}
    }

    builder
}

fn apply_mcp_extensions(
    mut builder: sdkwork_aiot_protocol::ProtocolEnvelopeBuilder,
    json: &str,
) -> sdkwork_aiot_protocol::ProtocolEnvelopeBuilder {
    let Some(payload) = json_object_field(json, "payload") else {
        return builder;
    };

    if let Some(jsonrpc) = json_string_field(payload, "jsonrpc") {
        builder = builder.extension("xiaozhi.mcp.jsonrpc", jsonrpc);
    }

    let method = json_string_field(payload, "method");
    if let Some(method) = method {
        builder = builder.extension("xiaozhi.mcp.method", method.clone());
    }

    let id = json_scalar_field(payload, "id");
    if let Some(id) = &id {
        builder = builder
            .correlation_id(id.clone())
            .extension("xiaozhi.mcp.id", id.clone());
        if let Some((start, end)) = json_field_value_range(payload, "id") {
            builder = builder.extension("xiaozhi.mcp.id_json", payload[start..end].trim());
        }
    }

    let has_method = json_field_value_range(payload, "method").is_some();
    let kind = if json_field_value_range(payload, "error").is_some() {
        "error"
    } else if json_field_value_range(payload, "result").is_some() {
        "response"
    } else if has_method && id.is_some() {
        "request"
    } else if has_method {
        "notification"
    } else {
        "payload"
    };

    builder.extension("xiaozhi.mcp.kind", kind)
}

fn audio_params_json(audio_params: &XiaozhiAudioParams) -> String {
    format!(
        r#"{{"format":"{}","sample_rate":{},"channels":{},"frame_duration":{}}}"#,
        json_escape(&audio_params.format),
        audio_params.sample_rate,
        audio_params.channels,
        audio_params.frame_duration
    )
}

fn decode_hex_16(input: &str, code: &str) -> Result<[u8; 16], ProtocolError> {
    let input = input.trim();
    if input.len() != 32 {
        return Err(ProtocolError::new(
            code,
            "xiaozhi udp crypto material must be 32 hex characters",
        ));
    }

    let mut out = [0u8; 16];
    for (index, chunk) in input.as_bytes().chunks(2).enumerate() {
        let hi = hex_nibble(chunk[0]).ok_or_else(|| {
            ProtocolError::new(code, "xiaozhi udp crypto material contains invalid hex")
        })?;
        let lo = hex_nibble(chunk[1]).ok_or_else(|| {
            ProtocolError::new(code, "xiaozhi udp crypto material contains invalid hex")
        })?;
        out[index] = (hi << 4) | lo;
    }

    Ok(out)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
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
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            ch if ch < ' ' => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out
}

fn json_string_field(input: &str, field: &str) -> Option<String> {
    let (start, _) = json_field_value_range(input, field)?;
    parse_json_string_at(input, start)
}

fn json_scalar_field(input: &str, field: &str) -> Option<String> {
    let (start, end) = json_field_value_range(input, field)?;
    let first = input.as_bytes().get(start).copied()?;
    if first == b'"' {
        parse_json_string_at(input, start)
    } else {
        let value = input[start..end].trim();
        if value.is_empty() || value.starts_with('{') || value.starts_with('[') {
            None
        } else {
            Some(value.to_string())
        }
    }
}

fn json_object_field<'a>(input: &'a str, field: &str) -> Option<&'a str> {
    let (start, end) = json_field_value_range(input, field)?;
    let value = input[start..end].trim();
    if value.starts_with('{') && value.ends_with('}') {
        Some(value)
    } else {
        None
    }
}

fn json_field_value_range(input: &str, field: &str) -> Option<(usize, usize)> {
    let mut search_start = 0usize;
    loop {
        let field_start = find_json_string_token(input, field, search_start)?;
        let after_field = field_start + field.len() + 2;
        let mut cursor = skip_json_ws(input, after_field);
        if input.as_bytes().get(cursor).copied()? != b':' {
            search_start = after_field;
            continue;
        }
        cursor = skip_json_ws(input, cursor + 1);
        let end = json_value_end(input, cursor)?;
        return Some((cursor, end));
    }
}

fn find_json_string_token(input: &str, token: &str, from: usize) -> Option<usize> {
    let needle = format!("\"{token}\"");
    let mut search_start = from;
    while let Some(offset) = input[search_start..].find(&needle) {
        let index = search_start + offset;
        if !is_escaped_quote(input, index) {
            return Some(index);
        }
        search_start = index + 1;
    }
    None
}

fn is_escaped_quote(input: &str, quote_index: usize) -> bool {
    let bytes = input.as_bytes();
    let mut cursor = quote_index;
    let mut slash_count = 0usize;
    while cursor > 0 {
        cursor -= 1;
        if bytes[cursor] == b'\\' {
            slash_count += 1;
        } else {
            break;
        }
    }
    slash_count % 2 == 1
}

fn skip_json_ws(input: &str, mut cursor: usize) -> usize {
    while input
        .as_bytes()
        .get(cursor)
        .is_some_and(u8::is_ascii_whitespace)
    {
        cursor += 1;
    }
    cursor
}

fn json_value_end(input: &str, start: usize) -> Option<usize> {
    match input.as_bytes().get(start).copied()? {
        b'"' => json_string_end(input, start).map(|end| end + 1),
        b'{' => json_composite_end(input, start, b'{', b'}').map(|end| end + 1),
        b'[' => json_composite_end(input, start, b'[', b']').map(|end| end + 1),
        _ => {
            let rest = &input[start..];
            let end = rest
                .find(|ch: char| ch == ',' || ch == '}' || ch == ']' || ch.is_whitespace())
                .map(|offset| start + offset)
                .unwrap_or(input.len());
            Some(end)
        }
    }
}

fn json_string_end(input: &str, start: usize) -> Option<usize> {
    if input.as_bytes().get(start).copied()? != b'"' {
        return None;
    }

    let mut escaped = false;
    for (offset, ch) in input[start + 1..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => return Some(start + 1 + offset),
            _ => {}
        }
    }

    None
}

fn json_composite_end(input: &str, start: usize, open: u8, close: u8) -> Option<usize> {
    if input.as_bytes().get(start).copied()? != open {
        return None;
    }

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (offset, byte) in input.as_bytes()[start..].iter().copied().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }

        match byte {
            b'"' => in_string = true,
            candidate if candidate == open => depth += 1,
            candidate if candidate == close => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(start + offset);
                }
            }
            _ => {}
        }
    }

    None
}

fn parse_json_string_at(input: &str, start: usize) -> Option<String> {
    let end = json_string_end(input, start)?;
    let mut out = String::new();
    let mut escaped = false;
    let mut unicode_escape = String::new();
    let mut in_unicode = false;

    for ch in input[start + 1..end].chars() {
        if in_unicode {
            unicode_escape.push(ch);
            if unicode_escape.len() == 4 {
                if let Ok(value) = u16::from_str_radix(&unicode_escape, 16) {
                    if let Some(decoded) = char::from_u32(value as u32) {
                        out.push(decoded);
                    }
                }
                unicode_escape.clear();
                in_unicode = false;
                escaped = false;
            }
            continue;
        }

        if escaped {
            match ch {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                '/' => out.push('/'),
                'b' => out.push('\u{08}'),
                'f' => out.push('\u{0c}'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                'u' => in_unicode = true,
                other => out.push(other),
            }
            if !in_unicode {
                escaped = false;
            }
            continue;
        }

        if ch == '\\' {
            escaped = true;
        } else {
            out.push(ch);
        }
    }

    Some(out)
}
