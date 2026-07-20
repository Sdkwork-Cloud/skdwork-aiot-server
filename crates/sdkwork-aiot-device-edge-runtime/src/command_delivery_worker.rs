use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sdkwork_aiot_storage::AiotCommandDeliveryRepository;
use sdkwork_aiot_storage_sqlx::open_aiot_device_database_from_env;
use sdkwork_aiot_transport::HttpRequest;

use crate::active_ws_sessions::{
    active_ws_sessions, evict_stale_active_ws_sessions, push_ws_command_replies,
};
use crate::{xiaozhi_speak_websocket_replies, XiaozhiSessionOptions};

const DEFAULT_COMMAND_DELIVERY_INTERVAL_MS: u64 = 500;
const DEFAULT_WS_SESSION_TTL_SECONDS: u64 = 300;
const ENV_COMMAND_DELIVERY_INTERVAL_MS: &str = "SDKWORK_AIOT_COMMAND_DELIVERY_INTERVAL_MS";
const ENV_WS_MEDIA_SESSION_TTL_SECONDS: &str = "SDKWORK_AIOT_WS_MEDIA_SESSION_TTL_SECONDS";

pub fn start_command_delivery_worker(running: Arc<AtomicBool>) {
    let interval_ms = std::env::var(ENV_COMMAND_DELIVERY_INTERVAL_MS)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_COMMAND_DELIVERY_INTERVAL_MS);
    let session_ttl = Duration::from_secs(
        std::env::var(ENV_WS_MEDIA_SESSION_TTL_SECONDS)
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_WS_SESSION_TTL_SECONDS)
            .max(30),
    );

    std::thread::spawn(move || {
        while running.load(Ordering::Relaxed) {
            if let Err(error) = run_command_delivery_once(session_ttl) {
                eprintln!("sdkwork-aiot-device-edge-runtime command_delivery_error={error}");
            }
            std::thread::sleep(Duration::from_millis(interval_ms));
        }
    });
}

fn run_command_delivery_once(session_ttl: Duration) -> Result<(), String> {
    let database = open_aiot_device_database_from_env().map_err(|error| format!("{error:?}"))?;
    let repository = database
        .device_repository()
        .map_err(|error| error.to_string())?;
    let session_options = XiaozhiSessionOptions::from_env();
    let speech_pipeline = session_options.speech_pipeline();
    let online_sessions = active_ws_sessions();
    if online_sessions.is_empty() {
        let _ = evict_stale_active_ws_sessions(session_ttl);
        crate::xiaozhi_ws_media_session::evict_stale_ws_media_sessions(session_ttl);
        return Ok(());
    }

    for (device_id, association) in online_sessions {
        let pending = repository
            .list_pending_for_device(&association, &device_id, 16)
            .map_err(|error| format!("{error:?}"))?;
        for delivery in pending {
            let Some(command) = repository
                .get_command_by_id(&association, &device_id, &delivery.command_id)
                .map_err(|error| format!("{error:?}"))?
            else {
                continue;
            };
            if command.capability_name != "audio.playback" || command.command_name != "speak" {
                continue;
            }
            let text = extract_speak_text(&command.request_payload_json);
            if text.is_empty() {
                continue;
            }
            let session_id = delivery
                .session_id
                .clone()
                .or(command.session_id.clone())
                .unwrap_or_else(|| format!("{device_id}-command"));
            let request = HttpRequest::new("GET", "/iot/xiaozhi/ws");
            let replies = xiaozhi_speak_websocket_replies(
                &request,
                &session_id,
                &text,
                speech_pipeline.as_deref(),
            )
            .map_err(|error| error.code)?;
            if !push_ws_command_replies(&device_id, replies) {
                continue;
            }
            repository
                .mark_delivered(&association, &delivery.command_id)
                .map_err(|error| format!("{error:?}"))?;
        }
    }

    let _ = evict_stale_active_ws_sessions(session_ttl);
    crate::xiaozhi_ws_media_session::evict_stale_ws_media_sessions(session_ttl);
    Ok(())
}

fn extract_speak_text(payload_json: &str) -> String {
    serde_json::from_str::<serde_json::Value>(payload_json)
        .ok()
        .and_then(|value| {
            value
                .get("text")
                .and_then(|text| text.as_str())
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_default()
}
