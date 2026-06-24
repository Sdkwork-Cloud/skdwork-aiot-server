//! Opus codec helpers for Xiaozhi device media (AIoT-owned, not claw-router).

use audiopus::coder::{Decoder, Encoder};
use audiopus::{Application, Channels, SampleRate};

const MAX_OPUS_PACKET_BYTES: usize = 4_000;

pub fn map_sample_rate(sample_rate: u32) -> Result<SampleRate, String> {
    match sample_rate {
        8_000 => Ok(SampleRate::Hz8000),
        12_000 => Ok(SampleRate::Hz12000),
        16_000 => Ok(SampleRate::Hz16000),
        24_000 => Ok(SampleRate::Hz24000),
        48_000 => Ok(SampleRate::Hz48000),
        _ => Err(format!("unsupported opus sample rate: {sample_rate}")),
    }
}

pub fn samples_per_frame(sample_rate: u32, frame_duration_ms: u32) -> Result<usize, String> {
    let samples = sample_rate as usize * frame_duration_ms as usize / 1000;
    if samples == 0 {
        return Err(format!(
            "invalid frame duration {frame_duration_ms}ms for sample rate {sample_rate}"
        ));
    }
    Ok(samples)
}

pub fn pcm16le_bytes_to_i16(pcm: &[u8]) -> Result<Vec<i16>, String> {
    if pcm.len() < 2 || !pcm.len().is_multiple_of(2) {
        return Err("pcm payload must contain 16-bit little-endian samples".to_string());
    }
    Ok(pcm
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect())
}

pub fn pcm16le_i16_to_bytes(samples: &[i16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

pub fn encode_pcm16le_mono_to_opus_packets(
    pcm: &[u8],
    sample_rate: u32,
    frame_duration_ms: u32,
) -> Result<Vec<Vec<u8>>, String> {
    let opus_rate = map_sample_rate(sample_rate)?;
    let frame_samples = samples_per_frame(sample_rate, frame_duration_ms)?;
    let samples = pcm16le_bytes_to_i16(pcm)?;
    if samples.is_empty() {
        return Err("pcm payload is empty".to_string());
    }

    let encoder = Encoder::new(opus_rate, Channels::Mono, Application::Voip)
        .map_err(|error| format!("opus encoder init failed: {error}"))?;

    let mut packets = Vec::new();
    for frame in samples.chunks(frame_samples) {
        let mut padded = vec![0i16; frame_samples];
        padded[..frame.len()].copy_from_slice(frame);
        let mut output = vec![0u8; MAX_OPUS_PACKET_BYTES];
        let encoded_len = encoder
            .encode(&padded, &mut output)
            .map_err(|error| format!("opus encode failed: {error}"))?;
        output.truncate(encoded_len);
        packets.push(output);
    }
    Ok(packets)
}

pub fn decode_xiaozhi_opus_packet_to_pcm16le(
    opus_packet: &[u8],
    sample_rate: u32,
    frame_duration_ms: u32,
) -> Result<Vec<u8>, String> {
    if opus_packet.is_empty() {
        return Err("opus packet is empty".to_string());
    }
    let opus_rate = map_sample_rate(sample_rate)?;
    let frame_samples = samples_per_frame(sample_rate, frame_duration_ms)?;
    let mut decoder = Decoder::new(opus_rate, Channels::Mono)
        .map_err(|error| format!("opus decoder init failed: {error}"))?;
    let mut pcm_out = vec![0i16; frame_samples];
    let decoded_samples = decoder
        .decode(Some(opus_packet), &mut pcm_out[..], false)
        .map_err(|error| format!("opus decode failed: {error}"))?;
    pcm_out.truncate(decoded_samples);
    Ok(pcm16le_i16_to_bytes(&pcm_out))
}

pub fn wrap_pcm16le_mono_wav(pcm: &[u8], sample_rate: u32) -> Result<Vec<u8>, String> {
    if pcm.len() < 2 || !pcm.len().is_multiple_of(2) {
        return Err("wav pcm payload must contain 16-bit little-endian samples".to_string());
    }
    let data_size = pcm.len() as u32;
    let riff_size = 36 + data_size;
    let byte_rate = sample_rate * 2;
    let block_align = 2u16;

    let mut wav = Vec::with_capacity(44 + pcm.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    wav.extend_from_slice(pcm);
    Ok(wav)
}

pub fn decode_xiaozhi_opus_uplink_to_wav(
    opus_packet: &[u8],
    sample_rate: u32,
    frame_duration_ms: u32,
) -> Result<Vec<u8>, String> {
    let pcm = decode_xiaozhi_opus_packet_to_pcm16le(opus_packet, sample_rate, frame_duration_ms)?;
    wrap_pcm16le_mono_wav(&pcm, sample_rate)
}

pub fn decode_xiaozhi_opus_packets_to_pcm16le(
    packets: &[Vec<u8>],
    sample_rate: u32,
    frame_duration_ms: u32,
) -> Result<Vec<u8>, String> {
    let mut pcm = Vec::new();
    for packet in packets {
        let frame_pcm =
            decode_xiaozhi_opus_packet_to_pcm16le(packet, sample_rate, frame_duration_ms)?;
        pcm.extend_from_slice(&frame_pcm);
    }
    if pcm.is_empty() {
        return Err("decoded pcm is empty".to_string());
    }
    Ok(pcm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_roundtrip_mono_24khz_60ms() {
        let sample_rate = 24_000;
        let frame_duration_ms = 60;
        let frame_samples = samples_per_frame(sample_rate, frame_duration_ms).unwrap();
        let pcm: Vec<u8> = (0..frame_samples)
            .flat_map(|index| ((index as i16).wrapping_mul(37)).to_le_bytes())
            .collect();

        let packets =
            encode_pcm16le_mono_to_opus_packets(&pcm, sample_rate, frame_duration_ms).unwrap();
        assert_eq!(packets.len(), 1);
        assert!(!packets[0].is_empty());

        let decoded =
            decode_xiaozhi_opus_packet_to_pcm16le(&packets[0], sample_rate, frame_duration_ms)
                .unwrap();
        assert_eq!(decoded.len(), pcm.len());
    }

    #[test]
    fn wav_wrapper_has_riff_header() {
        let pcm = vec![0u8; 480];
        let wav = wrap_pcm16le_mono_wav(&pcm, 24_000).unwrap();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(wav.len(), 44 + pcm.len());
    }
}
