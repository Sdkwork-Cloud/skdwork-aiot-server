use sdkwork_utils_rust::is_blank;

pub const ENV_INTELLIGENCE_MODE: &str = "SDKWORK_AIOT_INTELLIGENCE_MODE";
pub const ENV_INTELLIGENCE_KERNEL_HTTP_URL: &str = "SDKWORK_AIOT_INTELLIGENCE_KERNEL_HTTP_URL";
pub const ENV_INTELLIGENCE_KERNEL_AGENT_ID: &str = "SDKWORK_AIOT_INTELLIGENCE_KERNEL_AGENT_ID";
pub const ENV_INTELLIGENCE_ASR_MODEL: &str = "SDKWORK_AIOT_INTELLIGENCE_ASR_MODEL";
pub const ENV_INTELLIGENCE_TTS_MODEL: &str = "SDKWORK_AIOT_INTELLIGENCE_TTS_MODEL";
pub const ENV_INTELLIGENCE_TTS_VOICE: &str = "SDKWORK_AIOT_INTELLIGENCE_TTS_VOICE";
pub const ENV_INTELLIGENCE_TTS_RESPONSE_FORMAT: &str =
    "SDKWORK_AIOT_INTELLIGENCE_TTS_RESPONSE_FORMAT";
pub const ENV_INTELLIGENCE_TTS_SAMPLE_RATE: &str = "SDKWORK_AIOT_INTELLIGENCE_TTS_SAMPLE_RATE";

pub const KERNEL_PUBLIC_HTTP_URL_FALLBACK_ENV: &str = "SDKWORK_KERNEL_APPLICATION_PUBLIC_HTTP_URL";
pub const CLAW_ROUTER_OPEN_HTTP_URL_ENV: &str = "SDKWORK_CLAW_ROUTER_APPLICATION_OPEN_HTTP_URL";
pub const CLAW_ROUTER_API_KEY_ENV: &str = "SDKWORK_CLAW_ROUTER_API_KEY";

pub const DEFAULT_KERNEL_AGENT_ID: &str = "agent.xiaozhi";
pub const DEFAULT_ASR_MODEL: &str = "openai/whisper-1";
pub const DEFAULT_TTS_MODEL: &str = "openai/tts-1";
pub const DEFAULT_TTS_VOICE: &str = "alloy";
/// Claw Router TTS response format (standard provider audio, not Xiaozhi Opus).
pub const DEFAULT_TTS_RESPONSE_FORMAT: &str = "pcm";
pub const DEFAULT_TTS_SAMPLE_RATE: u32 = 24_000;

/// Canonical kernel runtime mount prefix on application public ingress.
pub const INTERNAL_RUNTIME_MOUNT_PREFIX: &str = "/internal/v3/api/intelligence/runtime";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntelligenceMode {
    Simulator,
    Kernel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntelligenceConfig {
    pub mode: IntelligenceMode,
    pub kernel_http_url: String,
    pub kernel_agent_id: String,
    pub claw_router_http_url: String,
    pub claw_router_api_key: Option<String>,
    pub asr_model: String,
    pub tts_model: String,
    pub tts_voice: String,
    pub tts_response_format: String,
    pub tts_sample_rate: u32,
}

pub fn intelligence_mode_from_env() -> IntelligenceMode {
    match std::env::var(ENV_INTELLIGENCE_MODE)
        .ok()
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some("kernel") | Some("production") => IntelligenceMode::Kernel,
        _ => IntelligenceMode::Simulator,
    }
}

pub fn is_kernel_mode() -> bool {
    intelligence_mode_from_env() == IntelligenceMode::Kernel
}

impl IntelligenceConfig {
    pub fn from_env() -> Result<Self, String> {
        let mode = intelligence_mode_from_env();
        if mode != IntelligenceMode::Kernel {
            return Err(format!(
                "{ENV_INTELLIGENCE_MODE} must be 'kernel' to load production intelligence config"
            ));
        }

        let kernel_http_url = resolve_kernel_http_url()?;
        let claw_router_http_url = std::env::var(CLAW_ROUTER_OPEN_HTTP_URL_ENV).map_err(|_| {
            format!("{CLAW_ROUTER_OPEN_HTTP_URL_ENV} must be set for production intelligence")
        })?;
        if is_blank(Some(claw_router_http_url.as_str())) {
            return Err(format!(
                "{CLAW_ROUTER_OPEN_HTTP_URL_ENV} must not be blank for production intelligence"
            ));
        }

        let claw_router_api_key = std::env::var(CLAW_ROUTER_API_KEY_ENV)
            .ok()
            .filter(|value| !is_blank(Some(value.as_str())));

        let kernel_agent_id = std::env::var(ENV_INTELLIGENCE_KERNEL_AGENT_ID)
            .ok()
            .filter(|value| !is_blank(Some(value.as_str())))
            .unwrap_or_else(|| DEFAULT_KERNEL_AGENT_ID.to_string());

        let asr_model = std::env::var(ENV_INTELLIGENCE_ASR_MODEL)
            .ok()
            .filter(|value| !is_blank(Some(value.as_str())))
            .unwrap_or_else(|| DEFAULT_ASR_MODEL.to_string());

        let tts_model = std::env::var(ENV_INTELLIGENCE_TTS_MODEL)
            .ok()
            .filter(|value| !is_blank(Some(value.as_str())))
            .unwrap_or_else(|| DEFAULT_TTS_MODEL.to_string());

        let tts_voice = std::env::var(ENV_INTELLIGENCE_TTS_VOICE)
            .ok()
            .filter(|value| !is_blank(Some(value.as_str())))
            .unwrap_or_else(|| DEFAULT_TTS_VOICE.to_string());

        let tts_response_format = std::env::var(ENV_INTELLIGENCE_TTS_RESPONSE_FORMAT)
            .ok()
            .filter(|value| !is_blank(Some(value.as_str())))
            .unwrap_or_else(|| DEFAULT_TTS_RESPONSE_FORMAT.to_string());

        let tts_sample_rate = std::env::var(ENV_INTELLIGENCE_TTS_SAMPLE_RATE)
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|rate| *rate > 0)
            .unwrap_or(DEFAULT_TTS_SAMPLE_RATE);

        Ok(Self {
            mode,
            kernel_http_url,
            kernel_agent_id,
            claw_router_http_url,
            claw_router_api_key,
            asr_model,
            tts_model,
            tts_voice,
            tts_response_format,
            tts_sample_rate,
        })
    }
}

fn resolve_kernel_http_url() -> Result<String, String> {
    if let Ok(url) = std::env::var(ENV_INTELLIGENCE_KERNEL_HTTP_URL) {
        if !is_blank(Some(url.as_str())) {
            return Ok(url);
        }
    }
    std::env::var(KERNEL_PUBLIC_HTTP_URL_FALLBACK_ENV).map_err(|_| {
        format!(
            "{ENV_INTELLIGENCE_KERNEL_HTTP_URL} or {KERNEL_PUBLIC_HTTP_URL_FALLBACK_ENV} must be set for production intelligence"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_defaults_to_simulator() {
        std::env::remove_var(ENV_INTELLIGENCE_MODE);
        assert_eq!(intelligence_mode_from_env(), IntelligenceMode::Simulator);
    }

    #[test]
    fn mode_kernel_aliases() {
        std::env::set_var(ENV_INTELLIGENCE_MODE, "kernel");
        assert_eq!(intelligence_mode_from_env(), IntelligenceMode::Kernel);
        std::env::set_var(ENV_INTELLIGENCE_MODE, "production");
        assert_eq!(intelligence_mode_from_env(), IntelligenceMode::Kernel);
        std::env::remove_var(ENV_INTELLIGENCE_MODE);
    }
}
