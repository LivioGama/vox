//! Speech-to-text with whisper.cpp via whisper-rs on macOS.

use std::borrow::Cow;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result};
use hound::WavReader;
use reqwest::blocking::Client;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

const MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin";
const MODEL_FILENAME: &str = "ggml-large-v3-turbo.bin";

static WHISPER_CTX: OnceLock<Mutex<WhisperContext>> = OnceLock::new();
static WHISPER_LOG_SILENCED: OnceLock<()> = OnceLock::new();

pub fn transcribe(audio_path: &str, lang: Option<&str>) -> Result<String> {
    if !Path::new(audio_path).exists() {
        anyhow::bail!("audio file not found: {audio_path}");
    }

    if let Ok(ctx) = whisper_ctx()
        && let Ok(text) = transcribe_whisper(ctx, audio_path, lang)
        && !text.trim().is_empty()
    {
        return Ok(text);
    }

    transcribe_fallback(audio_path, lang)
}

fn whisper_ctx() -> Result<&'static Mutex<WhisperContext>> {
    if let Some(ctx) = WHISPER_CTX.get() {
        return Ok(ctx);
    }

    let model_path = ensure_whisper_model()?;
    silence_whisper_logs();
    let ctx = WhisperContext::new_with_params(&model_path, WhisperContextParameters::default())
        .with_context(|| format!("failed to load whisper model: {}", model_path.display()))?;
    let _ = WHISPER_CTX.set(Mutex::new(ctx));
    WHISPER_CTX
        .get()
        .ok_or_else(|| anyhow::anyhow!("failed to cache whisper context"))
}

fn silence_whisper_logs() {
    WHISPER_LOG_SILENCED.get_or_init(|| {
        whisper_rs::install_logging_hooks();
    });
}

pub fn ensure_whisper_model() -> Result<PathBuf> {
    let home_dir = dirs::home_dir().context("failed to locate home directory")?;
    let model_dir = home_dir.join(".cache").join("vox").join("models");
    let model_path = model_dir.join(MODEL_FILENAME);
    if model_path.exists() {
        return Ok(model_path);
    }

    fs::create_dir_all(&model_dir)
        .with_context(|| format!("failed to create model dir: {}", model_dir.display()))?;

    let tmp_path = model_path.with_extension("download");
    let client = Client::new();
    let mut response = client
        .get(MODEL_URL)
        .send()
        .context("failed to start whisper model download")?
        .error_for_status()
        .context("whisper model download returned an error status")?;

    let total = response.content_length().unwrap_or(0);
    let mut file = File::create(&tmp_path)
        .with_context(|| format!("failed to create {}", tmp_path.display()))?;
    let mut downloaded = 0u64;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = response
            .read(&mut buf)
            .context("failed while downloading whisper model")?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .context("failed while writing whisper model")?;
        downloaded += n as u64;
        if total > 0 {
            eprint!("\rDownloading whisper model: {downloaded}/{total} bytes");
            let _ = std::io::stderr().flush();
        }
    }
    if total > 0 {
        eprintln!();
    }
    file.flush().context("failed to flush whisper model")?;
    fs::rename(&tmp_path, &model_path).with_context(|| {
        format!(
            "failed to move downloaded whisper model into place: {}",
            model_path.display()
        )
    })?;
    Ok(model_path)
}

fn transcribe_whisper(
    ctx: &'static Mutex<WhisperContext>,
    audio_path: &str,
    lang: Option<&str>,
) -> Result<String> {
    let samples = load_wav_mono_16k(audio_path)?;
    let mut state = ctx
        .lock()
        .map_err(|_| anyhow::anyhow!("whisper context lock poisoned"))?
        .create_state()
        .context("failed to create whisper state")?;
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    if let Some(lang) = lang {
        params.set_language(Some(lang));
    }
    state
        .full(params, &samples)
        .context("whisper transcription failed")?;

    let mut text = String::new();
    for segment in state.as_iter() {
        text.push_str(&segment_text(segment)?);
    }
    Ok(text.trim().to_string())
}

fn segment_text(segment: whisper_rs::WhisperSegment<'_>) -> Result<Cow<'_, str>> {
    segment
        .to_str_lossy()
        .map_err(|e| anyhow::anyhow!(e))
        .context("failed to read whisper segment text")
}

fn load_wav_mono_16k(audio_path: &str) -> Result<Vec<f32>> {
    let reader = WavReader::open(audio_path)
        .with_context(|| format!("failed to open wav file: {audio_path}"))?;
    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;
    let sample_rate = spec.sample_rate as usize;

    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .map(|s| s.context("failed to read float sample"))
            .collect::<Result<Vec<_>>>()?,
        hound::SampleFormat::Int => {
            let bits = spec.bits_per_sample.max(1) as u32;
            let max = ((1_i64 << (bits - 1)) - 1) as f32;
            reader
                .into_samples::<i32>()
                .map(|s| {
                    s.context("failed to read integer sample")
                        .map(|v| (v as f32 / max).clamp(-1.0, 1.0))
                })
                .collect::<Result<Vec<_>>>()?
        }
    };

    let mono = if channels == 1 {
        raw
    } else {
        raw.chunks(channels)
            .map(|chunk| chunk.iter().copied().sum::<f32>() / channels as f32)
            .collect()
    };

    Ok(resample_linear(&mono, sample_rate, 16_000))
}

fn resample_linear(samples: &[f32], src_rate: usize, dst_rate: usize) -> Vec<f32> {
    if samples.is_empty() || src_rate == dst_rate {
        return samples.to_vec();
    }
    let ratio = dst_rate as f64 / src_rate as f64;
    let out_len = ((samples.len() as f64) * ratio).round().max(1.0) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 / ratio;
        let left = pos.floor() as usize;
        let right = (left + 1).min(samples.len().saturating_sub(1));
        let frac = (pos - left as f64) as f32;
        let a = samples[left.min(samples.len().saturating_sub(1))];
        let b = samples[right];
        out.push(a * (1.0 - frac) + b * frac);
    }
    out
}

fn transcribe_fallback(audio_path: &str, lang: Option<&str>) -> Result<String> {
    let output = build_transcribe_command(audio_path, lang)
        .output()
        .context(
            "Failed to run mlx-whisper STT. Is mlx-whisper installed? (pip install mlx-whisper)",
        )?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("STT failed: {stderr}");
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).context("Failed to parse STT JSON output")?;
    Ok(parsed["text"].as_str().unwrap_or("").trim().to_string())
}

fn build_transcribe_command(audio_path: &str, lang: Option<&str>) -> Command {
    let lang_arg = lang.unwrap_or("en");
    let model = "mlx-community/whisper-large-v3-turbo";
    let script = format!(
        "import mlx_whisper, json; r = mlx_whisper.transcribe('{}', path_or_hf_repo='{}', language='{}'); print(json.dumps({{'text': r.get('text', '')}}))",
        audio_path.replace('\'', "\\'"),
        model,
        lang_arg,
    );
    let mut cmd = Command::new("python3");
    cmd.arg("-c").arg(script);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_path_has_expected_name() {
        assert!(MODEL_FILENAME.ends_with(".bin"));
    }

    #[test]
    fn resample_noop_when_rates_match() {
        let data = vec![0.0f32, 1.0, 0.5];
        assert_eq!(resample_linear(&data, 16_000, 16_000), data);
    }
}
