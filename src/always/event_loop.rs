use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::always::log::{Event, Logger};
use crate::always::{AlwaysConfig, daemon, filter, notify, paste, vad};

pub fn run(cfg: AlwaysConfig) -> Result<()> {
    let _pid = daemon::PidGuard::install()?;
    let mut log = Logger::open(&cfg.log_path)?;
    print_banner(&cfg);
    log.write(Event::Start { cfg: &cfg });
    let mut last_process = Instant::now() - Duration::from_secs(10);

    // Start background warmup if performance layer is available
    #[cfg(feature = "commercial")]
    if let Some(ref perf) = cfg.performance {
        let perf_clone = Arc::clone(perf);
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(perf_clone.warmup_background());
        });
    }

    loop {
        process_one(&cfg, &mut log, &mut last_process)?;
    }
}

fn process_one(cfg: &AlwaysConfig, log: &mut Logger, last_process: &mut Instant) -> Result<()> {
    match vad::record_utterance(cfg).context("failed to record/transcribe utterance")? {
        vad::RecordResult::Speech { text, energy } => {
            handle_speech(cfg, log, &text, energy, last_process)?;
        }
        vad::RecordResult::Silence => log.write(Event::Silence),
        vad::RecordResult::Timeout => log.write(Event::Timeout),
        vad::RecordResult::DroppedLowEnergy { energy } => {
            log.write(Event::DroppedLowEnergy { energy });
        }
        vad::RecordResult::DroppedWhisperNoise { raw } => {
            eprintln!("noise {raw:?}");
            log.write(Event::DroppedWhisperNoise { raw: &raw });
        }
    }
    Ok(())
}

fn handle_speech(
    cfg: &AlwaysConfig,
    log: &mut Logger,
    text: &str,
    energy: f64,
    last_process: &mut Instant,
) -> Result<()> {
    let now = Instant::now();
    if in_cooldown(now, *last_process, cfg.cooldown_ms) {
        return Ok(());
    }
    *last_process = now;

    if !filter::should_accept(text, cfg) {
        eprintln!("filtered {text}");
        log.write(Event::Filtered { text, energy });
        notify::notify("vox filtered", text, false);
        return Ok(());
    }

    let transformed = apply_vocabulary(text, cfg);
    eprintln!("{transformed} (energy: {energy:.4})");
    log.write(Event::Pasting {
        text: &transformed,
        energy,
    });
    notify::notify("vox", &transformed, true);
    paste::paste(&transformed, cfg.auto_enter)?;
    Ok(())
}

fn in_cooldown(now: Instant, last_process: Instant, cooldown_ms: u32) -> bool {
    now.duration_since(last_process).as_millis() < cooldown_ms as u128
}

fn apply_vocabulary(text: &str, cfg: &AlwaysConfig) -> String {
    let mut result = text.to_string();
    
    // Apply base vocabulary
    if let Some(ref vocab) = cfg.vocab {
        result = vocab.apply(&result);
    }
    
    // Apply learned corrections
    if let Some(ref post_processor) = cfg.post_processor {
        result = post_processor.apply_learned_corrections(&result);
        result = post_processor.code_aware_pattern_match(&result);
    }
    
    result
}

fn print_banner(cfg: &AlwaysConfig) {
    eprintln!("Always-on mode enabled. Press Ctrl+C to stop.");
    eprintln!("Log: {}", cfg.log_path.display());
    eprintln!(
        "Settings -> energy_threshold: {} silence: {}s auto_enter: {} filter: {}",
        cfg.energy_threshold, cfg.silence_secs, cfg.auto_enter, cfg.filter_enabled
    );
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use super::{apply_vocabulary, in_cooldown};
    use crate::always::AlwaysConfig;
    use crate::always::text::Vocabulary;

    fn test_config(vocab: Option<Vocabulary>) -> AlwaysConfig {
        use crate::always::config::{VocabConfig, PostprocessConfig};
        #[cfg(feature = "commercial")]
        use crate::always::config::PerformanceConfig;

        AlwaysConfig {
            lang: "en".to_string(),
            timeout_secs: 30,
            silence_secs: 2.0,
            auto_enter: false,
            filter_enabled: true,
            energy_threshold: 0.05,
            onset_ms: 200,
            cooldown_ms: 1500,
            log_path: PathBuf::from("always.log"),
            vocab,
            context_vocab: None,
            post_processor: None,
            #[cfg(feature = "commercial")]
            performance: None,
            project_root: None,
            learning_enabled: false,
            groq_api_key: None,
            vocab_config: VocabConfig::default(),
            postprocess_config: PostprocessConfig::default(),
            #[cfg(feature = "commercial")]
            performance_config: PerformanceConfig::default(),
        }
    }

    #[test]
    fn vocabulary_is_applied_to_text_about_to_paste() {
        let cfg = test_config(Some(Vocabulary::default_patterns()));
        assert_eq!(
            apply_vocabulary("open src/main.rs", &cfg),
            "open `src/main.rs`"
        );
    }

    #[test]
    fn vocabulary_passthrough_when_not_configured() {
        let cfg = test_config(None);
        assert_eq!(
            apply_vocabulary("open src/main.rs", &cfg),
            "open src/main.rs"
        );
    }

    #[test]
    fn cooldown_uses_millisecond_window() {
        let now = Instant::now();
        assert!(in_cooldown(now, now - Duration::from_millis(1499), 1500));
        assert!(!in_cooldown(now, now - Duration::from_millis(1500), 1500));
    }
}
