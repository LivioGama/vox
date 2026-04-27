use std::path::PathBuf;

use anyhow::Result;

use crate::always::text::Vocabulary;
use crate::db::Preferences;
use crate::{config, db};

#[derive(Debug, Clone)]
pub struct AlwaysConfig {
    pub lang: String,
    pub timeout_secs: u32,
    pub silence_secs: f64,
    pub auto_enter: bool,
    pub filter_enabled: bool,
    pub energy_threshold: f64,
    pub onset_ms: u32,
    pub cooldown_ms: u32,
    pub log_path: PathBuf,
    pub vocab: Option<Vocabulary>,
}

impl AlwaysConfig {
    pub fn from_cli(
        lang: String,
        timeout_secs: u32,
        silence_secs: f64,
        auto_enter: bool,
        no_filter: bool,
    ) -> Result<Self> {
        let prefs = load_preferences()?;

        Ok(Self {
            lang,
            timeout_secs,
            silence_secs,
            auto_enter,
            filter_enabled: !no_filter,
            energy_threshold: prefs.stt_energy_threshold.unwrap_or(0.05),
            onset_ms: 200,
            cooldown_ms: prefs.stt_cooldown_ms.unwrap_or(1500),
            log_path: log_path_from_preferences(&prefs),
            vocab: Vocabulary::load(),
        })
    }

    pub fn for_hear(lang: String, timeout_secs: u32, silence_secs: f64) -> Result<Self> {
        let prefs = load_preferences()?;
        Ok(Self {
            lang,
            timeout_secs,
            silence_secs,
            auto_enter: false,
            filter_enabled: false,
            energy_threshold: prefs.hear_energy_threshold.unwrap_or(0.002),
            onset_ms: 200,
            cooldown_ms: 0,
            log_path: log_path_from_preferences(&prefs),
            vocab: None,
        })
    }
}

pub fn configured_log_path() -> Result<PathBuf> {
    load_preferences().map(|prefs| log_path_from_preferences(&prefs))
}

fn load_preferences() -> Result<Preferences> {
    db::open().and_then(|conn| db::get_preferences(&conn))
}

fn log_path_from_preferences(prefs: &Preferences) -> PathBuf {
    prefs
        .always_log_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(default_log_path)
}

fn default_log_path() -> PathBuf {
    config::config_dir().join("always.log")
}
