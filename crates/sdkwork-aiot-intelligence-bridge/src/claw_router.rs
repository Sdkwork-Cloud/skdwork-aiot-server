use clawrouter_open_sdk::{SdkworkAiClient, SdkworkError};
use std::sync::Arc;

use crate::config::{IntelligenceConfig, CLAW_ROUTER_API_KEY_ENV};

#[derive(Clone)]
pub struct ClawRouterClient {
    client: Arc<SdkworkAiClient>,
}

impl ClawRouterClient {
    pub fn from_config(config: &IntelligenceConfig) -> Result<Self, String> {
        let client = SdkworkAiClient::new_with_base_url(&config.claw_router_http_url)
            .map_err(map_sdk_error)?;
        if let Some(api_key) = &config.claw_router_api_key {
            client.set_api_key(api_key.clone());
        } else if let Ok(api_key) = std::env::var(CLAW_ROUTER_API_KEY_ENV) {
            if !sdkwork_utils_rust::is_blank(Some(api_key.as_str())) {
                client.set_api_key(api_key);
            }
        }
        Ok(Self {
            client: Arc::new(client),
        })
    }

    pub fn sdk_client(&self) -> Arc<SdkworkAiClient> {
        Arc::clone(&self.client)
    }
}

pub fn map_sdk_error(error: SdkworkError) -> String {
    error.to_string()
}

/// Provider TTS bytes from claw-router (standard audio, not Xiaozhi Opus).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderTtsAudio {
    pub format: String,
    pub sample_rate: u32,
    pub bytes: Vec<u8>,
}

pub fn decode_speech_payload(raw: &str) -> Vec<u8> {
    if let Ok(bytes) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, raw) {
        if !bytes.is_empty() {
            return bytes;
        }
    }
    raw.as_bytes().to_vec()
}
