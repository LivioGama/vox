use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Result;

use crate::always::context_vocab::ContextVocabulary;
use crate::always::performance::PerformanceLayer;
use crate::always::postprocess::PostProcessor;
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
    pub context_vocab: Option<Arc<Mutex<ContextVocabulary>>>,
    pub post_processor: Option<Arc<PostProcessor>>,
    pub performance: Option<Arc<PerformanceLayer>>,
    pub project_root: Option<PathBuf>,
    pub learning_enabled: bool,
    pub groq_api_key: Option<String>,
    // Commercial-grade feature configuration
    pub vocab_config: VocabConfig,
    pub postprocess_config: PostprocessConfig,
    pub performance_config: PerformanceConfig,
}

#[derive(Debug, Clone)]
pub struct VocabConfig {
    pub file_patterns: Vec<String>,
    pub common_words: Vec<String>,
    pub min_term_length: usize,
    pub max_term_length: usize,
}

impl Default for VocabConfig {
    fn default() -> Self {
        Self {
            file_patterns: default_vocab_patterns(),
            common_words: default_common_words(),
            min_term_length: 2,
            max_term_length: 50,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PostprocessConfig {
    pub groq_model: String,
    pub learning_history_limit: usize,
    pub grammar_correction_enabled: bool,
    pub cache_ttl_seconds: u64,
}

impl Default for PostprocessConfig {
    fn default() -> Self {
        Self {
            groq_model: "llama3-8b-8192".to_string(),
            learning_history_limit: 1000,
            grammar_correction_enabled: true,
            cache_ttl_seconds: 300,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PerformanceConfig {
    pub transcription_cache_size: usize,
    pub vocab_cache_size: usize,
    pub warmup_duration_secs: u64,
    pub cache_ttl_seconds: u64,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            transcription_cache_size: 1000,
            vocab_cache_size: 10000,
            warmup_duration_secs: 5,
            cache_ttl_seconds: 300,
        }
    }
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
        let vocab = Vocabulary::load();
        
        // Load commercial-grade feature configs
        let vocab_config = load_vocab_config();
        let postprocess_config = load_postprocess_config();
        #[cfg(feature = "commercial")]
        let performance_config = load_performance_config();

        // Detect project root (current directory or parent with .git)
        let project_root = detect_project_root();

        // Initialize context vocabulary
        let context_vocab = if let Some(ref root) = project_root {
            Some(Arc::new(Mutex::new(ContextVocabulary::new_with_config(
                Some(root.clone()),
                vocab_config.clone(),
            ))))
        } else {
            None
        };

        // Initialize performance layer
        #[cfg(feature = "commercial")]
        let performance = Some(Arc::new(PerformanceLayer::new_with_config(
            performance_config.clone(),
        )));

        // Initialize post processor
        let post_processor = if let (Some(vocab), Some(context_vocab)) = (&vocab, &context_vocab) {
            let groq_api_key = std::env::var("GROQ_API_KEY").ok();
            Some(Arc::new(PostProcessor::new_with_config(
                vocab.clone(),
                Arc::clone(context_vocab),
                postprocess_config.clone(),
                groq_api_key,
            )))
        } else {
            None
        };

        #[cfg(feature = "commercial")]
        let config = Self {
            lang,
            timeout_secs,
            silence_secs,
            auto_enter,
            filter_enabled: !no_filter,
            energy_threshold: prefs.stt_energy_threshold.unwrap_or(0.05),
            onset_ms: 200,
            cooldown_ms: prefs.stt_cooldown_ms.unwrap_or(1500),
            log_path: log_path_from_preferences(&prefs),
            vocab,
            context_vocab,
            post_processor,
            performance,
            project_root,
            learning_enabled: postprocess_config.learning_history_limit > 0,
            groq_api_key: std::env::var("GROQ_API_KEY").ok(),
            vocab_config,
            postprocess_config,
            performance_config,
        };

        #[cfg(not(feature = "commercial"))]
        let config = Self {
            lang,
            timeout_secs,
            silence_secs,
            auto_enter,
            filter_enabled: !no_filter,
            energy_threshold: prefs.stt_energy_threshold.unwrap_or(0.05),
            onset_ms: 200,
            cooldown_ms: prefs.stt_cooldown_ms.unwrap_or(1500),
            log_path: log_path_from_preferences(&prefs),
            vocab,
            context_vocab,
            post_processor,
            project_root,
            learning_enabled: postprocess_config.learning_history_limit > 0,
            groq_api_key: std::env::var("GROQ_API_KEY").ok(),
            vocab_config,
            postprocess_config,
        };

        Ok(config)
    }

    pub fn for_hear(lang: String, timeout_secs: u32, silence_secs: f64) -> Result<Self> {
        let prefs = load_preferences()?;
        let vocab = Vocabulary::load();
        
        let vocab_config = load_vocab_config();
        let postprocess_config = load_postprocess_config();
        #[cfg(feature = "commercial")]
        let performance_config = load_performance_config();

        let project_root = detect_project_root();
        let context_vocab = if let Some(ref root) = project_root {
            Some(Arc::new(Mutex::new(ContextVocabulary::new_with_config(
                Some(root.clone()),
                vocab_config.clone(),
            ))))
        } else {
            None
        };

        #[cfg(feature = "commercial")]
        let performance = Some(Arc::new(PerformanceLayer::new_with_config(
            performance_config.clone(),
        )));

        let post_processor = if let (Some(vocab), Some(context_vocab)) = (&vocab, &context_vocab) {
            let groq_api_key = std::env::var("GROQ_API_KEY").ok();
            Some(Arc::new(PostProcessor::new_with_config(
                vocab.clone(),
                Arc::clone(context_vocab),
                postprocess_config.clone(),
                groq_api_key,
            )))
        } else {
            None
        };

        #[cfg(feature = "commercial")]
        {
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
                vocab,
                context_vocab,
                post_processor,
                performance,
                project_root,
                learning_enabled: postprocess_config.learning_history_limit > 0,
                groq_api_key: std::env::var("GROQ_API_KEY").ok(),
                vocab_config,
                postprocess_config,
                performance_config,
            })
        }
        #[cfg(not(feature = "commercial"))]
        {
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
                vocab,
                context_vocab,
                post_processor,
                project_root,
                learning_enabled: postprocess_config.learning_history_limit > 0,
                groq_api_key: std::env::var("GROQ_API_KEY").ok(),
                vocab_config,
                postprocess_config,
            })
        }
    }
}

fn load_vocab_config() -> VocabConfig {
    VocabConfig {
        file_patterns: std::env::var("VOX_FILE_PATTERNS")
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(default_vocab_patterns),
        common_words: std::env::var("VOX_COMMON_WORDS")
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(default_common_words),
        min_term_length: std::env::var("VOX_MIN_TERM_LENGTH")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(2),
        max_term_length: std::env::var("VOX_MAX_TERM_LENGTH")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(50),
    }
}

fn load_postprocess_config() -> PostprocessConfig {
    PostprocessConfig {
        groq_model: std::env::var("VOX_GROQ_MODEL")
            .unwrap_or_else(|_| "llama3-8b-8192".to_string()),
        learning_history_limit: std::env::var("VOX_LEARNING_LIMIT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1000),
        grammar_correction_enabled: std::env::var("VOX_GRAMMAR_CORRECTION")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(true),
        cache_ttl_seconds: std::env::var("VOX_CACHE_TTL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(300),
    }
}

#[cfg(feature = "commercial")]
fn load_performance_config() -> PerformanceConfig {
    PerformanceConfig {
        transcription_cache_size: std::env::var("VOX_TRANSCRIPTION_CACHE_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1000),
        vocab_cache_size: std::env::var("VOX_VOCAB_CACHE_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10000),
        warmup_duration_secs: std::env::var("VOX_WARMUP_DURATION")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5),
        cache_ttl_seconds: std::env::var("VOX_PERFORMANCE_CACHE_TTL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(300),
    }
}

fn default_vocab_patterns() -> Vec<String> {
    vec![
        r"\b[A-Z][a-zA-Z0-9]*\b".to_string(), // CamelCase
        r"\b[a-z]+_[a-z_]+\b".to_string(), // snake_case
        r"\b[A-Z_]+\b".to_string(), // SCREAMING_SNAKE_CASE
        r"\b[a-z]+[A-Z][a-zA-Z0-9]*\b".to_string(), // camelCase
    ]
}

fn default_common_words() -> Vec<String> {
    vec![
        "the", "and", "for", "are", "but", "not", "you", "all", "can", "had",
        "her", "was", "one", "our", "out", "with", "have", "this", "that", "from",
        "they", "will", "would", "there", "their", "what", "about", "which", "when",
        "make", "like", "into", "year", "your", "just", "over", "also", "such",
        "because", "these", "first", "being", "through", "after", "where", "should",
        "some", "those",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn detect_project_root() -> Option<PathBuf> {
    let current = std::env::current_dir().ok()?;
    
    // Check if current directory has .git
    if current.join(".git").exists() {
        return Some(current);
    }
    
    // Check parents
    let mut path = current.clone();
    while path.pop() {
        if path.join(".git").exists() {
            return Some(path);
        }
    }
    
    None
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
