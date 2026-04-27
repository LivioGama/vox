//! Speech-to-text with DeepGram API.

use anyhow::{Context, Result};
use std::path::Path;

/// Transcribe audio file using DeepGram API.
pub fn transcribe(audio_path: &str, lang: Option<&str>, api_key: &str) -> Result<String> {
    if !Path::new(audio_path).exists() {
        anyhow::bail!("audio file not found: {audio_path}");
    }

    let client = reqwest::blocking::Client::builder()
        .build()
        .context("failed to create HTTP client")?;
    let language = lang.unwrap_or("en");

    let audio_data = std::fs::read(audio_path)
        .with_context(|| format!("failed to read audio file: {audio_path}"))?;

    let url = "https://api.deepgram.com/v1/listen";
    let response = client
        .post(url)
        .header("Authorization", format!("Token {}", api_key))
        .header("Content-Type", "audio/wav")
        .query(&[("model", "nova-2"), ("language", language), ("smart_format", "true")])
        .body(audio_data)
        .send()
        .context("failed to send DeepGram API request")?
        .error_for_status()
        .context("DeepGram API returned an error")?;

    let json: serde_json::Value = response
        .json()
        .context("failed to parse DeepGram API response")?;

    let text = json["results"]["channels"][0]["alternatives"][0]["transcript"]
        .as_str()
        .unwrap_or("");

    Ok(text.trim().to_string())
}
