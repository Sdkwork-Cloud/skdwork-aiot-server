use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::WebSocketSessionReply;
use sdkwork_aiot_storage::AiotStorageAssociation;

#[derive(Debug)]
struct ActiveWsSession {
    outbound: std::sync::mpsc::Sender<Vec<WebSocketSessionReply>>,
    last_touched: Instant,
    association: AiotStorageAssociation,
}

fn store() -> &'static Mutex<HashMap<String, ActiveWsSession>> {
    static STORE: OnceLock<Mutex<HashMap<String, ActiveWsSession>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn register_active_ws_session(
    device_id: &str,
    _session_id: &str,
    outbound: std::sync::mpsc::Sender<Vec<WebSocketSessionReply>>,
    association: AiotStorageAssociation,
) {
    if let Ok(mut guard) = store().lock() {
        guard.insert(
            device_id.to_string(),
            ActiveWsSession {
                outbound,
                last_touched: Instant::now(),
                association,
            },
        );
    }
}

pub fn unregister_active_ws_session(device_id: &str) {
    if let Ok(mut guard) = store().lock() {
        guard.remove(device_id);
    }
}

pub fn push_ws_command_replies(device_id: &str, replies: Vec<WebSocketSessionReply>) -> bool {
    let Ok(guard) = store().lock() else {
        return false;
    };
    let Some(session) = guard.get(device_id) else {
        return false;
    };
    session.outbound.send(replies).is_ok()
}

pub fn touch_active_ws_session(device_id: &str) {
    if let Ok(mut guard) = store().lock() {
        if let Some(session) = guard.get_mut(device_id) {
            session.last_touched = Instant::now();
        }
    }
}

pub fn evict_stale_active_ws_sessions(ttl: Duration) -> usize {
    let Ok(mut guard) = store().lock() else {
        return 0;
    };
    let now = Instant::now();
    let before = guard.len();
    guard.retain(|_, session| now.duration_since(session.last_touched) <= ttl);
    before.saturating_sub(guard.len())
}

pub fn active_ws_sessions() -> Vec<(String, AiotStorageAssociation)> {
    store()
        .lock()
        .ok()
        .map(|guard| {
            guard
                .iter()
                .map(|(device_id, session)| (device_id.clone(), session.association.clone()))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
pub fn active_ws_device_ids() -> Vec<String> {
    active_ws_sessions()
        .into_iter()
        .map(|(device_id, _)| device_id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_push_roundtrip() {
        unregister_active_ws_session("dev-test");
        let (sender, receiver) = std::sync::mpsc::channel();
        register_active_ws_session(
            "dev-test",
            "session-test",
            sender,
            AiotStorageAssociation::default(),
        );
        assert!(push_ws_command_replies(
            "dev-test",
            vec![WebSocketSessionReply::Text(
                "{\"type\":\"tts\"}".to_string()
            )],
        ));
        let replies = receiver.recv().expect("outbound");
        assert_eq!(replies.len(), 1);
        assert_eq!(active_ws_device_ids(), vec!["dev-test"]);
        unregister_active_ws_session("dev-test");
    }
}
