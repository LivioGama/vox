//! Intelligent text transformation with pattern matching and context-aware rules.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Vocabulary {
    #[serde(default)]
    pub corrections: HashMap<String, String>,
    #[serde(default)]
    pub patterns: Vec<PatternRule>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PatternRule {
    #[serde(rename = "pattern")]
    pub regex: String,
    pub replacement: String,
    #[serde(default)]
    pub add_quotes: bool,
    #[serde(default)]
    pub quote_type: String, // "single", "double", "backtick"
}

impl Default for Vocabulary {
    fn default() -> Self {
        Vocabulary {
            corrections: HashMap::new(),
            patterns: get_default_patterns(),
        }
    }
}

/// Get default transformation patterns for common speech-to-text corrections.
pub fn get_default_patterns() -> Vec<PatternRule> {
    vec![
        // "dot" followed by word → ".word" (file extensions)
        PatternRule {
            regex: r"\bdot\s+(\w+)".to_string(),
            replacement: r".$1".to_string(),
            add_quotes: false,
            quote_type: "double".to_string(),
        },
        // File extensions in sentences should be quoted
        PatternRule {
            regex: r"\b(\w+\.[a-z]{2,4})\b".to_string(),
            replacement: r"`$1`".to_string(),
            add_quotes: true,
            quote_type: "backtick".to_string(),
        },
        // Commands like "git commit" should be in backticks
        PatternRule {
            regex: r"\b(git\s+\w+|npm\s+\w+|cargo\s+\w+|bun\s+\w+)".to_string(),
            replacement: r"`$1`".to_string(),
            add_quotes: true,
            quote_type: "backtick".to_string(),
        },
        // Filenames with path separators
        PatternRule {
            regex: r"[\w/\-\.]+/[\w/\-\.]+".to_string(),
            replacement: r"`$0`".to_string(),
            add_quotes: true,
            quote_type: "backtick".to_string(),
        },
    ]
}

/// Apply intelligent text transformations using vocabulary corrections and pattern rules.
pub fn apply_intelligent_transformation(text: &str, vocab: &Vocabulary) -> String {
    let mut result = text.to_string();
    
    // First apply simple string corrections (for backwards compatibility)
    let mut corrections_vec: Vec<_> = vocab.corrections.iter().collect();
    corrections_vec.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
    
    for (wrong, correct) in corrections_vec {
        result = result.replace(wrong, correct);
    }
    
    // Then apply pattern rules
    for rule in &vocab.patterns {
        if let Ok(re) = Regex::new(&rule.regex) {
            let replacement = if rule.add_quotes {
                let quote_char = match rule.quote_type.as_str() {
                    "single" => "'",
                    "double" => "\"",
                    "backtick" => "`",
                    _ => "`",
                };
                rule.replacement.replace("$0", &format!("{}$0{}", quote_char, quote_char))
            } else {
                rule.replacement.clone()
            };
            result = re.replace_all(&result, &replacement).to_string();
        }
    }
    
    result
}

/// Load vocabulary from ~/.iris/vocabulary.json, or return default patterns.
#[cfg(target_os = "macos")]
pub fn load_vocabulary() -> Vocabulary {
    let vocab_path = dirs::home_dir()
        .map(|h| h.join(".iris").join("vocabulary.json"));
    
    if let Some(path) = vocab_path {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(vocab) = serde_json::from_str::<Vocabulary>(&content) {
                return vocab;
            }
        }
    }
    
    // Fallback: default vocabulary with patterns
    Vocabulary::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dot_iris_transformation() {
        let vocab = Vocabulary::default();
        let input = "dot iris";
        let output = apply_intelligent_transformation(input, &vocab);
        assert_eq!(output, ".iris");
    }

    #[test]
    fn test_file_extension_transformation() {
        let vocab = Vocabulary::default();
        let input = "create file test dot rs";
        let output = apply_intelligent_transformation(input, &vocab);
        assert!(output.contains("test.rs"));
    }

    #[test]
    fn test_git_command_quoting() {
        let vocab = Vocabulary::default();
        let input = "run git commit";
        let output = apply_intelligent_transformation(input, &vocab);
        assert!(output.contains("`git commit`"));
    }

    #[test]
    fn test_simple_corrections() {
        let mut corrections = HashMap::new();
        corrections.insert("dont".to_string(), "don't".to_string());
        
        let vocab = Vocabulary {
            corrections,
            patterns: vec![],
        };

        let input = "I dont know";
        let output = apply_intelligent_transformation(input, &vocab);
        assert_eq!(output, "I don't know");
    }

    #[test]
    fn test_combined_transformations() {
        let mut corrections = HashMap::new();
        corrections.insert("wanna".to_string(), "want to".to_string());
        
        let vocab = Vocabulary {
            corrections,
            patterns: get_default_patterns(),
        };

        let input = "I wanna create file test dot rs";
        let output = apply_intelligent_transformation(input, &vocab);
        assert!(output.contains("want to"));
        assert!(output.contains("test.rs"));
    }
}
