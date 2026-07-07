use clawrouter_open_sdk::{
    OpenAiAudioTranscriptionRequest, OpenAiSpeechCreateRequest, SdkworkAiClient,
};
use sdkwork_aiot_adapter_xiaozhi::decode_xiaozhi_opus_uplink_to_wav;
use std::collections::HashMap;
use std::sync::Arc;

use crate::claw_router::{decode_speech_payload, map_sdk_error};
use crate::config::IntelligenceConfig;
use crate::kernel_runtime::KernelRuntimeClient;
use crate::session_map::SessionMap;

const DEFAULT_XIAOZHI_FRAME_DURATION_MS: u32 = 60;

#[derive(Debug, Clone)]
pub struct SpeechTurnInput {
    pub xiaozhi_session_id: String,
    pub user_text: Option<String>,
    pub audio_bytes: Option<Vec<u8>>,
    pub asr_wav_bytes: Option<Vec<u8>>,
    pub uplink_sample_rate: Option<u32>,
    pub uplink_frame_duration_ms: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeechTurnOutput {
    pub stt_text: String,
    pub llm_emotion: String,
    pub llm_text: String,
    pub tts_audio: Vec<u8>,
    /// Provider format (`pcm`, `mp3`, …). Xiaozhi Opus is encoded in adapter-xiaozhi.
    pub tts_audio_format: String,
    pub tts_sample_rate: u32,
}

pub struct KernelSpeechPipeline {
    config: IntelligenceConfig,
    kernel: KernelRuntimeClient,
    claw: Arc<SdkworkAiClient>,
    session_map: SessionMap,
}

impl KernelSpeechPipeline {
    pub fn new(
        config: IntelligenceConfig,
        kernel: KernelRuntimeClient,
        claw_client: crate::claw_router::ClawRouterClient,
        session_map: SessionMap,
    ) -> Self {
        Self {
            config,
            kernel,
            claw: claw_client.sdk_client(),
            session_map,
        }
    }

    pub fn run_turn(&self, input: SpeechTurnInput) -> Result<SpeechTurnOutput, String> {
        let xiaozhi_session_id = input.xiaozhi_session_id.clone();
        let user_text = self.resolve_user_text(&input)?;
        let kernel_session_id = self.kernel.ensure_session(
            &self.session_map,
            &xiaozhi_session_id,
            &self.config.kernel_agent_id,
        )?;
        let assistant_text = self
            .kernel
            .send_user_message(&kernel_session_id, &user_text)?;
        self.finish_speech_output(user_text, assistant_text)
    }

    /// Synthesizes TTS for explicit speak/play commands without an LLM round trip.
    pub fn run_speak(&self, text: &str) -> Result<SpeechTurnOutput, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("speak text is required".to_string());
        }
        self.finish_speech_output(text.to_string(), text.to_string())
    }

    fn finish_speech_output(
        &self,
        stt_text: String,
        spoken_text: String,
    ) -> Result<SpeechTurnOutput, String> {
        let tts_audio = self.synthesize_speech(&spoken_text)?;
        Ok(SpeechTurnOutput {
            stt_text,
            llm_emotion: "neutral".to_string(),
            llm_text: spoken_text,
            tts_audio: tts_audio.bytes,
            tts_audio_format: tts_audio.format,
            tts_sample_rate: tts_audio.sample_rate,
        })
    }

    fn resolve_user_text(&self, input: &SpeechTurnInput) -> Result<String, String> {
        if let Some(text) = input
            .user_text
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Ok(text.to_string());
        }
        if let Some(wav_bytes) = input
            .asr_wav_bytes
            .as_deref()
            .filter(|bytes| !bytes.is_empty())
        {
            return self.transcribe_wav(wav_bytes);
        }
        if let Some(audio_bytes) = input
            .audio_bytes
            .as_deref()
            .filter(|bytes| !bytes.is_empty())
        {
            let sample_rate = input
                .uplink_sample_rate
                .filter(|rate| *rate > 0)
                .unwrap_or(self.config.tts_sample_rate);
            let frame_duration_ms = input
                .uplink_frame_duration_ms
                .filter(|duration| *duration > 0)
                .unwrap_or(DEFAULT_XIAOZHI_FRAME_DURATION_MS);
            return self.transcribe_audio(audio_bytes, sample_rate, frame_duration_ms);
        }
        Err("speech turn requires xiaozhi.listen.text or uplink audio".to_string())
    }

    fn transcribe_wav(&self, wav_bytes: &[u8]) -> Result<String, String> {
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, wav_bytes);
        self.transcribe_encoded_audio(encoded, "xiaozhi-uplink.wav")
    }

    fn transcribe_audio(
        &self,
        opus_packet: &[u8],
        sample_rate: u32,
        frame_duration_ms: u32,
    ) -> Result<String, String> {
        let wav = decode_xiaozhi_opus_uplink_to_wav(opus_packet, sample_rate, frame_duration_ms)?;
        self.transcribe_wav(&wav)
    }

    fn transcribe_encoded_audio(&self, encoded: String, filename: &str) -> Result<String, String> {
        let mut file_properties = HashMap::new();
        file_properties.insert("data".to_string(), serde_json::Value::String(encoded));
        file_properties.insert(
            "filename".to_string(),
            serde_json::Value::String(filename.to_string()),
        );
        let request = OpenAiAudioTranscriptionRequest {
            file: clawrouter_open_sdk::OpenAiFileReferenceInput {
                additional_properties: file_properties,
            },
            language: None,
            model: self.config.asr_model.clone(),
            prompt: None,
            response_format: Some("json".to_string()),
        };
        let claw = Arc::clone(&self.claw);
        let transcription = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async move { claw.audio().create_transcription(&request).await })
        })
        .map_err(map_sdk_error)?;
        let text = transcription.text.trim().to_string();
        if text.is_empty() {
            return Err("ASR returned empty transcription".to_string());
        }
        Ok(text)
    }

    fn synthesize_speech(
        &self,
        text: &str,
    ) -> Result<crate::claw_router::ProviderTtsAudio, String> {
        let request = OpenAiSpeechCreateRequest {
            input: text.to_string(),
            model: self.config.tts_model.clone(),
            voice: self.config.tts_voice.clone(),
            response_format: Some(self.config.tts_response_format.clone()),
            metadata: None,
            speed: None,
        };
        let claw = Arc::clone(&self.claw);
        let raw = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async move { claw.audio().create_speech(&request).await })
        })
        .map_err(map_sdk_error)?;
        Ok(crate::claw_router::ProviderTtsAudio {
            format: self.config.tts_response_format.clone(),
            sample_rate: self.config.tts_sample_rate,
            bytes: decode_speech_payload(&raw),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_speak_requires_non_empty_text() {
        let pipeline = KernelSpeechPipeline {
            config: IntelligenceConfig {
                mode: crate::config::IntelligenceMode::Kernel,
                kernel_http_url: "http://127.0.0.1:18280".to_string(),
                kernel_agent_id: "agent.xiaozhi".to_string(),
                claw_router_http_url: "http://127.0.0.1:1".to_string(),
                claw_router_api_key: None,
                asr_model: "openai/whisper-1".to_string(),
                tts_model: "openai/tts-1".to_string(),
                tts_voice: "alloy".to_string(),
                tts_response_format: "pcm".to_string(),
                tts_sample_rate: 24_000,
            },
            kernel: KernelRuntimeClient::new("http://127.0.0.1:18280".to_string()).unwrap(),
            claw: Arc::new(
                SdkworkAiClient::new_with_base_url("http://127.0.0.1:1").expect("client"),
            ),
            session_map: SessionMap::new(),
        };
        let err = pipeline.run_speak("   ").unwrap_err();
        assert!(err.contains("required"));
    }

    #[test]
    fn speech_turn_input_requires_text_or_audio() {
        let pipeline = KernelSpeechPipeline {
            config: IntelligenceConfig {
                mode: crate::config::IntelligenceMode::Kernel,
                kernel_http_url: "http://127.0.0.1:18280".to_string(),
                kernel_agent_id: "agent.xiaozhi".to_string(),
                claw_router_http_url: "http://127.0.0.1:1".to_string(),
                claw_router_api_key: None,
                asr_model: "openai/whisper-1".to_string(),
                tts_model: "openai/tts-1".to_string(),
                tts_voice: "alloy".to_string(),
                tts_response_format: "pcm".to_string(),
                tts_sample_rate: 24_000,
            },
            kernel: KernelRuntimeClient::new("http://127.0.0.1:18280".to_string()).unwrap(),
            claw: Arc::new(
                SdkworkAiClient::new_with_base_url("http://127.0.0.1:1").expect("client"),
            ),
            session_map: SessionMap::new(),
        };
        let err = pipeline
            .run_turn(SpeechTurnInput {
                xiaozhi_session_id: "s1".to_string(),
                user_text: None,
                audio_bytes: None,
                asr_wav_bytes: None,
                uplink_sample_rate: None,
                uplink_frame_duration_ms: None,
            })
            .unwrap_err();
        assert!(err.contains("requires"));
    }
}
