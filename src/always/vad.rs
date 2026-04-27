use anyhow::{Context, Result};
use std::collections::VecDeque;
use webrtc_vad::{Vad, VadMode};

use crate::always::AlwaysConfig;
use crate::always::audio::{self, FRAME_BYTES, FRAME_MS};

pub enum RecordResult {
    Speech { text: String, energy: f64 },
    Silence,
    DroppedLowEnergy { energy: f64 },
    DroppedNoise { raw: String },
    Timeout,
}

pub fn record_utterance(cfg: &AlwaysConfig) -> Result<RecordResult> {
    match cfg.vad_mode {
        crate::always::config::VadMode::Local => record_with_local_vad(cfg),
        crate::always::config::VadMode::DeepGram => record_with_deepgram_vad(cfg),
    }
}

fn record_with_local_vad(cfg: &AlwaysConfig) -> Result<RecordResult> {
    let silence_frames = ((cfg.silence_secs * 1000.0) / FRAME_MS as f64).ceil() as usize;
    let max_frames = (cfg.timeout_secs as usize * 1000) / FRAME_MS as usize;
    let min_speech_frames = (cfg.onset_ms / FRAME_MS).max(1) as usize;
    // Pre-buffer: keep 500ms of audio before speech detection to catch first words
    let pre_buffer_frames = (500 / FRAME_MS as usize).max(1);
    let mut vad =
        Vad::new_with_rate_and_mode(webrtc_vad::SampleRate::Rate16kHz, VadMode::Aggressive);
    let mut rec = audio::RecChild::spawn()?;
    let mut frame_buf = [0u8; FRAME_BYTES];
    let mut speech_samples = Vec::new();
    let mut consecutive_silence = 0usize;
    let mut consecutive_speech = 0usize;
    let mut in_speech = false;
    let mut total_frames = 0usize;
    let mut pre_buffer: VecDeque<Vec<i16>> = VecDeque::with_capacity(pre_buffer_frames);

    loop {
        let read = rec.read_frame(&mut frame_buf)?;
        if read < FRAME_BYTES {
            break;
        }

        let samples: Vec<i16> = frame_buf
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        let is_speech = vad.is_voice_segment(&samples).unwrap_or(false);

        if is_speech {
            consecutive_speech += 1;
            consecutive_silence = 0;
            if consecutive_speech >= min_speech_frames {
                in_speech = true;
                // Prepend pre-buffer to capture audio before VAD triggered
                for buffered_samples in pre_buffer.drain(..) {
                    speech_samples.extend_from_slice(&buffered_samples);
                }
            }
            if in_speech {
                speech_samples.extend_from_slice(&samples);
            }
        } else if in_speech {
            consecutive_silence += 1;
            consecutive_speech = 0;
            speech_samples.extend_from_slice(&samples);
            if consecutive_silence >= silence_frames {
                break;
            }
        } else {
            consecutive_speech = 0;
            // Maintain pre-buffer: add new frame, drop oldest if full
            pre_buffer.push_back(samples.clone());
            if pre_buffer.len() > pre_buffer_frames {
                pre_buffer.pop_front();
            }
        }

        total_frames += 1;
        if total_frames >= max_frames {
            break;
        }
    }

    if speech_samples.is_empty() {
        return if total_frames >= max_frames {
            Ok(RecordResult::Timeout)
        } else {
            Ok(RecordResult::Silence)
        };
    }

    let speech_energy = normalized_energy(&speech_samples);
    if speech_energy < cfg.energy_threshold {
        return Ok(RecordResult::DroppedLowEnergy {
            energy: speech_energy,
        });
    }

    let audio_path = audio::temp_wav_path();
    audio::write_wav_i16_mono_16k(&audio_path, &speech_samples)?;
    let audio_str = audio_path.to_string_lossy().to_string();
    let raw = match crate::stt::transcribe(&audio_str, Some(&cfg.lang), &cfg.groq_stt_api_key, "whisper-large-v3") {
        Ok(raw) => raw,
        Err(err) => {
            let _ = std::fs::remove_file(&audio_path);
            return Err(err).context("failed to transcribe utterance WAV");
        }
    };
    let _ = std::fs::remove_file(&audio_path);

    if raw.is_empty() {
        return Ok(RecordResult::DroppedNoise { raw });
    }

    Ok(RecordResult::Speech {
        text: raw,
        energy: speech_energy,
    })
}

// TODO: Implement DeepGram streaming VAD mode
// For now, fall back to local VAD
fn record_with_deepgram_vad(cfg: &AlwaysConfig) -> Result<RecordResult> {
    eprintln!("DeepGram streaming VAD not yet implemented, falling back to local VAD");
    record_with_local_vad(cfg)
}

fn normalized_energy(samples: &[i16]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: i64 = samples
        .iter()
        .map(|sample| (*sample as i64) * (*sample as i64))
        .sum();
    (sum_sq as f64 / samples.len() as f64).sqrt() / 32768.0
}

#[cfg(test)]
mod tests {
    use super::normalized_energy;

    #[test]
    fn normalized_energy_handles_empty_input() {
        assert_eq!(normalized_energy(&[]), 0.0);
    }

    #[test]
    fn normalized_energy_scales_to_i16_range() {
        let energy = normalized_energy(&[16_384, -16_384]);
        assert!((energy - 0.5).abs() < 0.001);
    }
}
