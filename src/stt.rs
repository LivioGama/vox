//! Speech-to-text with Groq Whisper API.

use anyhow::{Context, Result};
use reqwest::blocking::multipart;
use std::path::Path;

use crate::glossary;

/// Transcribe audio file using Groq Whisper API.
pub fn transcribe(audio_path: &str, _lang: Option<&str>, api_key: &str, _model: &str) -> Result<String> {
    if !Path::new(audio_path).exists() {
        anyhow::bail!("audio file not found: {audio_path}");
    }

    let audio_data = std::fs::read(audio_path)
        .with_context(|| format!("failed to read audio file: {audio_path}"))?;

    let client = reqwest::blocking::Client::new();

    let mut form = multipart::Form::new()
        .part("file", multipart::Part::bytes(audio_data).file_name("audio.wav").mime_str("audio/wav")?)
        .text("model", "whisper-large-v3")
        .text("response_format", "text")
        .text("language", "en");

    if let Some(prompt) = glossary::whisper_bias_prompt() {
        form = form.text("prompt", prompt.clone());
    }

    let response = client
        .post("https://api.groq.com/openai/v1/audio/transcriptions")
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .context("failed to send Groq Whisper API request")?
        .error_for_status()
        .context("Groq Whisper API returned an error")?;

    let text = response.text().context("failed to read Groq Whisper response")?;

    Ok(text.trim().to_string())
}
