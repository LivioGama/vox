use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lru::LruCache;
use tokio::task;

#[cfg(feature = "commercial")]
use super::config::PerformanceConfig;

#[derive(Debug, Clone)]
pub struct PerformanceLayer {
    transcription_cache: Arc<Mutex<LruCache<String, CachedTranscription>>>,
    vocab_cache: Arc<Mutex<LruCache<String, String>>>,
    model_warmed: Arc<Mutex<bool>>,
    config: PerformanceConfig,
}

#[derive(Debug, Clone)]
struct CachedTranscription {
    text: String,
    timestamp: Instant,
    energy: f64,
}

impl PerformanceLayer {
    pub fn new() -> Self {
        Self::new_with_config(PerformanceConfig::default())
    }

    pub fn new_with_config(config: PerformanceConfig) -> Self {
        Self {
            transcription_cache: Arc::new(Mutex::new(LruCache::new(config.transcription_cache_size))),
            vocab_cache: Arc::new(Mutex::new(LruCache::new(config.vocab_cache_size))),
            model_warmed: Arc::new(Mutex::new(false)),
            config,
        }
    }

    pub fn get_cached_transcription(&self, audio_hash: &str) -> Option<String> {
        if let Ok(cache) = self.transcription_cache.lock() {
            if let Some(cached) = cache.get(audio_hash) {
                // Return if cached within TTL from config
                if cached.timestamp.elapsed() < Duration::from_secs(self.config.cache_ttl_seconds) {
                    return Some(cached.text.clone());
                }
            }
        }
        None
    }

    pub fn cache_transcription(&self, audio_hash: String, text: String, energy: f64) {
        if let Ok(mut cache) = self.transcription_cache.lock() {
            cache.put(audio_hash, CachedTranscription {
                text,
                timestamp: Instant::now(),
                energy,
            });
        }
    }

    pub fn get_cached_vocab(&self, key: &str) -> Option<String> {
        if let Ok(cache) = self.vocab_cache.lock() {
            cache.get(key).cloned()
        } else {
            None
        }
    }

    pub fn cache_vocab(&self, key: String, value: String) {
        if let Ok(mut cache) = self.vocab_cache.lock() {
            cache.put(key, value);
        }
    }

    pub fn is_model_warmed(&self) -> bool {
        *self.model_warmed.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn set_model_warmed(&self, warmed: bool) {
        *self.model_warmed.lock().unwrap_or_else(|e| e.into_inner()) = warmed;
    }

    pub async fn warmup_background(&self) {
        let model_warmed = Arc::clone(&self.model_warmed);
        let duration = Duration::from_secs(self.config.warmup_duration_secs);

        task::spawn(async move {
            // Simulate model warmup
            tokio::time::sleep(duration).await;
            *model_warmed.lock().unwrap() = true;
        });
    }

    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.transcription_cache.lock() {
            cache.clear();
        }
        if let Ok(mut cache) = self.vocab_cache.lock() {
            cache.clear();
        }
    }
}

/// Streaming STT processor for real-time transcription
pub struct StreamingStt {
    buffer: Vec<i16>,
    is_speaking: bool,
    silence_threshold: f64,
    min_speech_frames: usize,
}

impl StreamingStt {
    pub fn new(silence_threshold: f64, min_speech_frames: usize) -> Self {
        Self {
            buffer: Vec::new(),
            is_speaking: false,
            silence_threshold,
            min_speech_frames,
        }
    }
    
    pub fn process_frame(&mut self, frame: &[i16]) -> Option<Vec<i16>> {
        let energy = self.calculate_energy(frame);
        
        if energy > self.silence_threshold {
            self.is_speaking = true;
            self.buffer.extend_from_slice(frame);
            None
        } else if self.is_speaking {
            // End of speech
            if self.buffer.len() > self.min_speech_frames {
                let result = self.buffer.clone();
                self.buffer.clear();
                self.is_speaking = false;
                Some(result)
            } else {
                // Too short, discard
                self.buffer.clear();
                self.is_speaking = false;
                None
            }
        } else {
            // Not speaking, ignore
            None
        }
    }
    
    fn calculate_energy(&self, samples: &[i16]) -> f64 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum_sq: i64 = samples
            .iter()
            .map(|sample| (*sample as i64) * (*sample as i64))
            .sum();
        (sum_sq as f64 / samples.len() as f64).sqrt() / 32768.0
    }
    
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.is_speaking = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caches_transcription() {
        let layer = PerformanceLayer::new();
        layer.cache_transcription("hash1".to_string(), "test text".to_string(), 0.5);

        assert_eq!(
            layer.get_cached_transcription("hash1"),
            Some("test text".to_string())
        );
    }

    #[test]
    fn caches_vocabulary() {
        let layer = PerformanceLayer::new();
        layer.cache_vocab("key1".to_string(), "value1".to_string());

        assert_eq!(
            layer.get_cached_vocab("key1"),
            Some("value1".to_string())
        );
    }

    #[test]
    fn streaming_stt_processes_frames() {
        let mut stt = StreamingStt::new(0.1, 10);

        // High energy frame
        let high_energy = vec![1000i16; 100];
        assert!(stt.process_frame(&high_energy).is_none());

        // Low energy frame (end of speech)
        let low_energy = vec![0i16; 100];
        assert!(stt.process_frame(&low_energy).is_some());
    }
}
