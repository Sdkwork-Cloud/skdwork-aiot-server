//! Xiaozhi Opus uplink buffering and session media profile (AIoT-owned).

use crate::opus_codec::{decode_xiaozhi_opus_packets_to_pcm16le, wrap_pcm16le_mono_wav};

pub const DEFAULT_XIAOZHI_SAMPLE_RATE: u32 = 24_000;
pub const DEFAULT_XIAOZHI_FRAME_DURATION_MS: u32 = 60;
pub const MAX_UPLINK_PACKETS: usize = 256;
pub const MAX_UPLINK_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XiaozhiSessionMediaProfile {
    pub sample_rate: u32,
    pub frame_duration_ms: u32,
}

impl Default for XiaozhiSessionMediaProfile {
    fn default() -> Self {
        Self {
            sample_rate: DEFAULT_XIAOZHI_SAMPLE_RATE,
            frame_duration_ms: DEFAULT_XIAOZHI_FRAME_DURATION_MS,
        }
    }
}

impl XiaozhiSessionMediaProfile {
    pub fn from_envelope_extensions(
        extensions: &std::collections::BTreeMap<String, String>,
    ) -> Self {
        let sample_rate = extensions
            .get("xiaozhi.audio.sample_rate")
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|rate| *rate > 0)
            .unwrap_or(DEFAULT_XIAOZHI_SAMPLE_RATE);
        let frame_duration_ms = extensions
            .get("xiaozhi.audio.frame_duration")
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|duration| *duration > 0)
            .unwrap_or(DEFAULT_XIAOZHI_FRAME_DURATION_MS);
        Self {
            sample_rate,
            frame_duration_ms,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct XiaozhiOpusUplinkBuffer {
    packets: Vec<Vec<u8>>,
    total_bytes: usize,
}

impl XiaozhiOpusUplinkBuffer {
    pub fn push_packet(&mut self, packet: Vec<u8>) -> Result<(), String> {
        if packet.is_empty() {
            return Ok(());
        }
        if self.packets.len() >= MAX_UPLINK_PACKETS {
            return Err(format!(
                "xiaozhi uplink buffer exceeded {MAX_UPLINK_PACKETS} packets"
            ));
        }
        let next_total = self.total_bytes.saturating_add(packet.len());
        if next_total > MAX_UPLINK_BYTES {
            return Err(format!(
                "xiaozhi uplink buffer exceeded {MAX_UPLINK_BYTES} bytes"
            ));
        }
        self.total_bytes = next_total;
        self.packets.push(packet);
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    pub fn clear(&mut self) {
        self.packets.clear();
        self.total_bytes = 0;
    }

    pub fn decode_to_wav(&self, profile: XiaozhiSessionMediaProfile) -> Result<Vec<u8>, String> {
        if self.packets.is_empty() {
            return Err("xiaozhi uplink buffer is empty".to_string());
        }
        let pcm = decode_xiaozhi_opus_packets_to_pcm16le(
            &self.packets,
            profile.sample_rate,
            profile.frame_duration_ms,
        )?;
        wrap_pcm16le_mono_wav(&pcm, profile.sample_rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opus_codec::encode_pcm16le_mono_to_opus_packets;

    #[test]
    fn buffer_rejects_oversized_uplink() {
        let mut buffer = XiaozhiOpusUplinkBuffer::default();
        let err = buffer
            .push_packet(vec![0u8; MAX_UPLINK_BYTES])
            .and_then(|()| buffer.push_packet(vec![1u8; 4]))
            .unwrap_err();
        assert!(err.contains("bytes"));
    }

    #[test]
    fn buffered_packets_decode_to_wav() {
        let frame_samples = 24_000 * 60 / 1000;
        let pcm = vec![0u8; frame_samples * 2];
        let packets = encode_pcm16le_mono_to_opus_packets(&pcm, 24_000, 60).unwrap();
        let mut buffer = XiaozhiOpusUplinkBuffer::default();
        for packet in packets {
            buffer.push_packet(packet).unwrap();
        }
        let wav = buffer
            .decode_to_wav(XiaozhiSessionMediaProfile::default())
            .unwrap();
        assert_eq!(&wav[0..4], b"RIFF");
    }
}
