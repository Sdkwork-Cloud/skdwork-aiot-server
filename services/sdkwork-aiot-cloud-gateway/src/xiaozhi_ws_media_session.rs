use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use sdkwork_aiot_adapter_xiaozhi::XiaozhiSessionMediaProfile;

#[derive(Debug, Clone)]
struct XiaozhiWsMediaSession {
    profile: XiaozhiSessionMediaProfile,
    uplink: sdkwork_aiot_adapter_xiaozhi::XiaozhiOpusUplinkBuffer,
    last_touched: Instant,
}

fn store() -> &'static Mutex<HashMap<String, XiaozhiWsMediaSession>> {
    static STORE: OnceLock<Mutex<HashMap<String, XiaozhiWsMediaSession>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn upsert_ws_media_profile(session_id: &str, profile: XiaozhiSessionMediaProfile) {
    if let Ok(mut guard) = store().lock() {
        let entry = guard
            .entry(session_id.to_string())
            .or_insert_with(|| XiaozhiWsMediaSession {
                profile,
                uplink: sdkwork_aiot_adapter_xiaozhi::XiaozhiOpusUplinkBuffer::default(),
                last_touched: Instant::now(),
            });
        entry.profile = profile;
        entry.last_touched = Instant::now();
    }
}

pub fn push_ws_uplink_packet(session_id: &str, packet: Vec<u8>) -> Result<(), String> {
    let mut guard = store()
        .lock()
        .map_err(|_| "xiaozhi ws media session store poisoned".to_string())?;
    let entry = guard
        .entry(session_id.to_string())
        .or_insert_with(|| XiaozhiWsMediaSession {
            profile: XiaozhiSessionMediaProfile::default(),
            uplink: sdkwork_aiot_adapter_xiaozhi::XiaozhiOpusUplinkBuffer::default(),
            last_touched: Instant::now(),
        });
    entry.last_touched = Instant::now();
    entry.uplink.push_packet(packet)
}

pub fn clear_ws_uplink_buffer(session_id: &str) {
    if let Ok(mut guard) = store().lock() {
        if let Some(entry) = guard.get_mut(session_id) {
            entry.uplink.clear();
        }
    }
}

pub fn take_ws_uplink_wav(session_id: &str) -> Result<Option<Vec<u8>>, String> {
    let mut guard = store()
        .lock()
        .map_err(|_| "xiaozhi ws media session store poisoned".to_string())?;
    let Some(entry) = guard.get_mut(session_id) else {
        return Ok(None);
    };
    if entry.uplink.is_empty() {
        return Ok(None);
    }
    let profile = entry.profile;
    let wav = entry.uplink.decode_to_wav(profile)?;
    entry.uplink.clear();
    Ok(Some(wav))
}

pub fn ws_media_profile(session_id: &str) -> XiaozhiSessionMediaProfile {
    store()
        .lock()
        .ok()
        .and_then(|guard| guard.get(session_id).map(|entry| entry.profile))
        .unwrap_or_default()
}

pub fn evict_stale_ws_media_sessions(ttl: Duration) -> usize {
    let Ok(mut guard) = store().lock() else {
        return 0;
    };
    let now = Instant::now();
    let before = guard.len();
    guard.retain(|_, entry| now.duration_since(entry.last_touched) <= ttl);
    before.saturating_sub(guard.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdkwork_aiot_adapter_xiaozhi::encode_pcm16le_mono_to_opus_packets;

    #[test]
    fn uplink_buffer_roundtrip_per_session() {
        let session_id = "test-session-uplink-roundtrip";
        clear_ws_uplink_buffer(session_id);
        upsert_ws_media_profile(session_id, XiaozhiSessionMediaProfile::default());
        assert!(take_ws_uplink_wav(session_id).unwrap().is_none());

        let frame_samples = 24_000 * 60 / 1000;
        let pcm = vec![0u8; frame_samples * 2];
        let packets = encode_pcm16le_mono_to_opus_packets(&pcm, 24_000, 60).unwrap();
        for packet in packets {
            push_ws_uplink_packet(session_id, packet).unwrap();
        }
        let wav = take_ws_uplink_wav(session_id)
            .expect("decode")
            .expect("wav bytes");
        assert_eq!(&wav[0..4], b"RIFF");
        assert!(take_ws_uplink_wav(session_id).unwrap().is_none());
        clear_ws_uplink_buffer(session_id);
    }

    #[test]
    fn uplink_decode_fails_for_unsupported_sample_rate() {
        let session_id = "test-session-uplink-bad-rate";
        clear_ws_uplink_buffer(session_id);
        upsert_ws_media_profile(
            session_id,
            XiaozhiSessionMediaProfile {
                sample_rate: 99_999,
                frame_duration_ms: 60,
            },
        );
        push_ws_uplink_packet(session_id, vec![0xF8, 0xFF, 0xFE]).unwrap();
        assert!(take_ws_uplink_wav(session_id).is_err());
        clear_ws_uplink_buffer(session_id);
    }
}
