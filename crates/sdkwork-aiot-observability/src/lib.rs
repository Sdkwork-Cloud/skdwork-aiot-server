use std::io::{self, Write};

mod otlp;

pub use otlp::{
    configured_otlp_endpoint, configured_otlp_service_name, format_otlp_runtime_metric_payload,
    format_otlp_trace_payload, otlp_export_enabled, parse_otlp_endpoint, OtlpEndpoint,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceFields {
    pub trace_id: String,
    pub tenant_id: Option<String>,
    pub organization_id: Option<String>,
    pub product_id: Option<String>,
    pub device_id: Option<String>,
    pub session_id: Option<String>,
    pub connection_id: Option<String>,
    pub adapter_id: Option<String>,
    pub protocol_id: Option<String>,
    pub message_class: Option<String>,
    pub semantic_type: Option<String>,
    pub command_id: Option<String>,
}

impl TraceFields {
    pub fn new(trace_id: impl Into<String>) -> Self {
        Self {
            trace_id: trace_id.into(),
            tenant_id: None,
            organization_id: None,
            product_id: None,
            device_id: None,
            session_id: None,
            connection_id: None,
            adapter_id: None,
            protocol_id: None,
            message_class: None,
            semantic_type: None,
            command_id: None,
        }
    }

    pub fn tenant(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }

    pub fn organization(mut self, organization_id: impl Into<String>) -> Self {
        self.organization_id = Some(organization_id.into());
        self
    }

    pub fn device(mut self, device_id: impl Into<String>) -> Self {
        self.device_id = Some(device_id.into());
        self
    }

    pub fn protocol(mut self, protocol_id: impl Into<String>) -> Self {
        self.protocol_id = Some(protocol_id.into());
        self
    }

    pub fn session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }
}

pub fn redact_header(name: &str, value: &str) -> String {
    if is_sensitive_header(name) {
        "<redacted>".to_string()
    } else {
        value.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMetricFields {
    pub component: String,
    pub tenant_id: Option<String>,
    pub protocol_id: Option<String>,
    pub node_id: Option<String>,
    pub node_connections: Option<u64>,
    pub tenant_sessions: Option<u64>,
    pub device_inflight: Option<u64>,
    pub outbox_lag: Option<u64>,
    pub backpressure_action: Option<String>,
}

impl RuntimeMetricFields {
    pub fn new(component: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            tenant_id: None,
            protocol_id: None,
            node_id: configured_node_id(),
            node_connections: None,
            tenant_sessions: None,
            device_inflight: None,
            outbox_lag: None,
            backpressure_action: None,
        }
    }

    pub fn tenant(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }

    pub fn protocol(mut self, protocol_id: impl Into<String>) -> Self {
        self.protocol_id = Some(protocol_id.into());
        self
    }

    pub fn node(mut self, node_id: impl Into<String>) -> Self {
        self.node_id = Some(node_id.into());
        self
    }

    pub fn connections(mut self, connections: u64) -> Self {
        self.node_connections = Some(connections);
        self
    }

    pub fn sessions(mut self, sessions: u64) -> Self {
        self.tenant_sessions = Some(sessions);
        self
    }

    pub fn device_inflight(mut self, inflight: u64) -> Self {
        self.device_inflight = Some(inflight);
        self
    }

    pub fn outbox_lag(mut self, lag: u64) -> Self {
        self.outbox_lag = Some(lag);
        self
    }

    pub fn backpressure(mut self, action: impl Into<String>) -> Self {
        self.backpressure_action = Some(action.into());
        self
    }

    pub fn contains_payload_fields(&self) -> bool {
        false
    }
}

pub fn structured_trace_enabled() -> bool {
    std::env::var("SDKWORK_AIOT_STRUCTURED_TRACE").as_deref() == Ok("1")
}

pub fn configured_node_id() -> Option<String> {
    std::env::var("SDKWORK_AIOT_DEVICE_EDGE_NODE_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn emit_trace_event(event: &str, fields: &TraceFields) {
    if structured_trace_enabled() {
        let _ = write_structured_line(&format_trace_event(event, fields));
    }
    otlp::export_trace_event(event, fields);
}

pub fn emit_runtime_metric(fields: &RuntimeMetricFields) {
    if structured_trace_enabled() {
        let _ = write_structured_line(&format_runtime_metric(fields));
    }
    otlp::export_runtime_metric(fields);
}

pub fn emit_api_request_trace(method: &str, path: &str, status: u16) {
    if structured_trace_enabled() {
        let line = format!(
            r#"{{"event":"api.request","method":"{}","path":"{}","status":{}}}"#,
            json_escape(method),
            json_escape(path),
            status
        );
        let _ = write_structured_line(&line);
    }
    otlp::export_api_request_trace(method, path, status);
}

pub fn emit_device_edge_lifecycle(event: &str, fields: &TraceFields) {
    emit_trace_event(event, fields);
}

pub fn format_trace_event(event: &str, fields: &TraceFields) -> String {
    let mut parts = vec![
        format!(r#""event":"{}""#, json_escape(event)),
        format!(r#""traceId":"{}""#, json_escape(&fields.trace_id)),
    ];
    append_optional_string(&mut parts, "tenantId", fields.tenant_id.as_deref());
    append_optional_string(
        &mut parts,
        "organizationId",
        fields.organization_id.as_deref(),
    );
    append_optional_string(&mut parts, "productId", fields.product_id.as_deref());
    append_optional_string(&mut parts, "deviceId", fields.device_id.as_deref());
    append_optional_string(&mut parts, "sessionId", fields.session_id.as_deref());
    append_optional_string(&mut parts, "connectionId", fields.connection_id.as_deref());
    append_optional_string(&mut parts, "adapterId", fields.adapter_id.as_deref());
    append_optional_string(&mut parts, "protocolId", fields.protocol_id.as_deref());
    append_optional_string(&mut parts, "messageClass", fields.message_class.as_deref());
    append_optional_string(&mut parts, "semanticType", fields.semantic_type.as_deref());
    append_optional_string(&mut parts, "commandId", fields.command_id.as_deref());
    format!("{{{}}}", parts.join(","))
}

pub fn format_runtime_metric(fields: &RuntimeMetricFields) -> String {
    let mut parts = vec![format!(
        r#""event":"runtime.metric","component":"{}""#,
        json_escape(&fields.component)
    )];
    append_optional_string(&mut parts, "tenantId", fields.tenant_id.as_deref());
    append_optional_string(&mut parts, "protocolId", fields.protocol_id.as_deref());
    append_optional_string(&mut parts, "nodeId", fields.node_id.as_deref());
    append_optional_u64(&mut parts, "nodeConnections", fields.node_connections);
    append_optional_u64(&mut parts, "tenantSessions", fields.tenant_sessions);
    append_optional_u64(&mut parts, "deviceInflight", fields.device_inflight);
    append_optional_u64(&mut parts, "outboxLag", fields.outbox_lag);
    append_optional_string(
        &mut parts,
        "backpressureAction",
        fields.backpressure_action.as_deref(),
    );
    format!("{{{}}}", parts.join(","))
}

fn append_optional_string(parts: &mut Vec<String>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        parts.push(format!(r#""{key}":"{}""#, json_escape(value)));
    }
}

fn append_optional_u64(parts: &mut Vec<String>, key: &str, value: Option<u64>) {
    if let Some(value) = value {
        parts.push(format!(r#""{key}":{value}"#));
    }
}

fn json_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn write_structured_line(line: &str) -> io::Result<()> {
    let mut stderr = io::stderr().lock();
    writeln!(stderr, "{line}")
}

fn is_sensitive_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "authorization" | "access-token" | "cookie" | "set-cookie" | "x-api-key"
    )
}
