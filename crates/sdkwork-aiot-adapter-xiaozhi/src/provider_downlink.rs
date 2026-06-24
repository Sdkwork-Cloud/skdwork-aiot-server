//! Provider TTS audio ↔ Xiaozhi Opus media (AIoT-owned, not claw-router).

use crate::opus_codec::encode_pcm16le_mono_to_opus_packets;

/// Provider-side audio container returned from intelligence backends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderTtsAudio {
    pub format: String,
    pub sample_rate: u32,
    pub bytes: Vec<u8>,
}

impl ProviderTtsAudio {
    pub fn pcm(sample_rate: u32, bytes: Vec<u8>) -> Self {
        Self {
            format: "pcm".to_string(),
            sample_rate,
            bytes,
        }
    }
}

/// Encode provider TTS audio into Xiaozhi Opus downlink packets.
pub fn encode_provider_pcm_to_xiaozhi_opus_packets(
    audio: &ProviderTtsAudio,
    frame_duration_ms: u32,
) -> Result<Vec<Vec<u8>>, String> {
    match audio.format.as_str() {
        "pcm" => encode_pcm16le_mono_to_opus_packets(
            &audio.bytes,
            audio.sample_rate,
            frame_duration_ms,
        ),
        "opus" => Err(
            "provider returned opus; xiaozhi opus downlink must be encoded in sdkwork-aiot-adapter-xiaozhi"
                .to_string(),
        ),
        other => Err(format!(
            "unsupported provider audio format for xiaozhi downlink: {other}"
        )),
    }
}

/// Encode provider TTS audio into the first Xiaozhi Opus downlink packet.
pub fn encode_provider_audio_to_xiaozhi_opus(
    audio: &ProviderTtsAudio,
    frame_duration_ms: u32,
) -> Result<Vec<u8>, String> {
    encode_provider_pcm_to_xiaozhi_opus_packets(audio, frame_duration_ms)?
        .into_iter()
        .next()
        .ok_or_else(|| "provider pcm produced no opus packets".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_provider_opus_format() {
        let audio = ProviderTtsAudio {
            format: "opus".to_string(),
            sample_rate: 24_000,
            bytes: vec![0xF8, 0xFF],
        };
        let err = encode_provider_audio_to_xiaozhi_opus(&audio, 60).unwrap_err();
        assert!(err.contains("adapter-xiaozhi"));
    }

    #[test]
    fn pcm_encodes_to_opus_packet() {
        let frame_samples = 24_000 * 60 / 1000;
        let audio = ProviderTtsAudio::pcm(24_000, vec![0u8; frame_samples * 2]);
        let packet = encode_provider_audio_to_xiaozhi_opus(&audio, 60).unwrap();
        assert!(!packet.is_empty());
    }
}
