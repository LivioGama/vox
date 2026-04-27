use anyhow::{Context, Result};
use webrtc_vad::{Vad, VadMode};

use crate::always::AlwaysConfig;
use crate::always::audio::{self, FRAME_BYTES, FRAME_MS};

pub enum RecordResult {
    Speech { text: String, energy: f64 },
    Silence,
    DroppedLowEnergy { energy: f64 },
    DroppedWhisperNoise { raw: String },
    Timeout,
}

pub fn record_utterance(cfg: &AlwaysConfig) -> Result<RecordResult> {
    let silence_frames = ((cfg.silence_secs * 1000.0) / FRAME_MS as f64).ceil() as usize;
    let max_frames = (cfg.timeout_secs as usize * 1000) / FRAME_MS as usize;
    let min_speech_frames = (cfg.onset_ms / FRAME_MS).max(1) as usize;
    let mut vad =
        Vad::new_with_rate_and_mode(webrtc_vad::SampleRate::Rate16kHz, VadMode::VeryAggressive);
    let mut rec = audio::RecChild::spawn()?;
    let mut frame_buf = [0u8; FRAME_BYTES];
    let mut speech_samples = Vec::new();
    let mut consecutive_silence = 0usize;
    let mut consecutive_speech = 0usize;
    let mut in_speech = false;
    let mut total_frames = 0usize;

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
    let raw = match crate::stt::transcribe(&audio_str, Some(&cfg.lang)) {
        Ok(raw) => raw,
        Err(err) => {
            let _ = std::fs::remove_file(&audio_path);
            return Err(err).context("failed to transcribe utterance WAV");
        }
    };
    let _ = std::fs::remove_file(&audio_path);
    let text = strip_whisper_tokens(&raw);

    if text.is_empty() {
        return Ok(RecordResult::DroppedWhisperNoise { raw });
    }

    Ok(RecordResult::Speech {
        text,
        energy: speech_energy,
    })
}

pub fn strip_whisper_tokens(raw: &str) -> String {
    let mut text = raw.to_string();
    while let (Some(start), Some(end)) = (text.find('<'), text.find('>')) {
        if start < end {
            text.replace_range(start..=end, "");
        } else {
            break;
        }
    }
    text.trim().to_string()
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
    use super::{normalized_energy, strip_whisper_tokens};

    #[test]
    fn strips_whisper_tokens() {
        assert_eq!(strip_whisper_tokens("<|en|>hello<|end|>"), "hello");
    }

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
