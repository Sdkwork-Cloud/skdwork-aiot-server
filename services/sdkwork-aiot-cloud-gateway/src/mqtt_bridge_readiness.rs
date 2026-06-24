//! MQTT/UDP bridge runtime signals for gateway health and readiness probes.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub const ENV_MQTT_BRIDGE_ENABLE: &str = "SDKWORK_AIOT_GATEWAY_MQTT_BRIDGE_ENABLE";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MqttBridgeRuntimeSnapshot {
    pub bridge_enabled: bool,
    pub mqtt_loop_running: bool,
    pub mqtt_session_active: bool,
    pub udp_loop_running: bool,
    pub udp_socket_bound: bool,
}

#[derive(Debug)]
pub struct MqttBridgeRuntimeState {
    bridge_enabled: bool,
    mqtt_loop_running: AtomicBool,
    mqtt_session_active: AtomicBool,
    udp_loop_running: AtomicBool,
    udp_socket_bound: AtomicBool,
}

impl MqttBridgeRuntimeState {
    pub fn from_env() -> Arc<Self> {
        let bridge_enabled = std::env::var(ENV_MQTT_BRIDGE_ENABLE).as_deref() == Ok("1");
        Arc::new(Self::new(bridge_enabled))
    }

    pub fn new(bridge_enabled: bool) -> Self {
        Self {
            bridge_enabled,
            mqtt_loop_running: AtomicBool::new(false),
            mqtt_session_active: AtomicBool::new(false),
            udp_loop_running: AtomicBool::new(false),
            udp_socket_bound: AtomicBool::new(false),
        }
    }

    pub fn bridge_enabled(&self) -> bool {
        self.bridge_enabled
    }

    pub fn mqtt_loop_running(&self) -> &AtomicBool {
        &self.mqtt_loop_running
    }

    pub fn mqtt_session_active(&self) -> &AtomicBool {
        &self.mqtt_session_active
    }

    pub fn udp_loop_running(&self) -> &AtomicBool {
        &self.udp_loop_running
    }

    pub fn udp_socket_bound(&self) -> &AtomicBool {
        &self.udp_socket_bound
    }

    pub fn snapshot(&self) -> MqttBridgeRuntimeSnapshot {
        MqttBridgeRuntimeSnapshot {
            bridge_enabled: self.bridge_enabled,
            mqtt_loop_running: self.mqtt_loop_running.load(Ordering::Relaxed),
            mqtt_session_active: self.mqtt_session_active.load(Ordering::Relaxed),
            udp_loop_running: self.udp_loop_running.load(Ordering::Relaxed),
            udp_socket_bound: self.udp_socket_bound.load(Ordering::Relaxed),
        }
    }

    pub fn is_ready(&self) -> bool {
        if !self.bridge_enabled {
            return true;
        }
        self.mqtt_loop_running.load(Ordering::Relaxed)
            && self.mqtt_session_active.load(Ordering::Relaxed)
            && self.udp_loop_running.load(Ordering::Relaxed)
            && self.udp_socket_bound.load(Ordering::Relaxed)
    }
}

pub fn mqtt_bridge_health_status(snapshot: &MqttBridgeRuntimeSnapshot) -> &'static str {
    if !snapshot.bridge_enabled {
        "disabled"
    } else if snapshot.mqtt_loop_running
        && snapshot.mqtt_session_active
        && snapshot.udp_loop_running
        && snapshot.udp_socket_bound
    {
        "ok"
    } else {
        "degraded"
    }
}

pub fn mqtt_bridge_readiness_probe(
    state: Arc<MqttBridgeRuntimeState>,
) -> impl Fn() -> bool + Send + Sync + 'static {
    move || state.is_ready()
}

#[cfg(test)]
mod mqtt_bridge_readiness_tests {
    use super::*;

    #[test]
    fn bridge_readiness_passes_when_bridge_is_disabled() {
        let state = MqttBridgeRuntimeState::new(false);
        assert!(state.is_ready());
        assert_eq!(mqtt_bridge_health_status(&state.snapshot()), "disabled");
    }

    #[test]
    fn bridge_readiness_requires_mqtt_and_udp_loops_when_enabled() {
        let state = MqttBridgeRuntimeState::new(true);
        assert!(!state.is_ready());
        assert_eq!(mqtt_bridge_health_status(&state.snapshot()), "degraded");

        state.mqtt_loop_running.store(true, Ordering::Relaxed);
        assert!(!state.is_ready());

        state.mqtt_session_active.store(true, Ordering::Relaxed);
        assert!(!state.is_ready());

        state.udp_loop_running.store(true, Ordering::Relaxed);
        assert!(!state.is_ready());

        state.udp_socket_bound.store(true, Ordering::Relaxed);
        assert!(state.is_ready());
        assert_eq!(mqtt_bridge_health_status(&state.snapshot()), "ok");
    }
}
