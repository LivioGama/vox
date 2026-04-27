use crate::always::AlwaysConfig;

pub fn quick_reject(text: &str) -> bool {
    let normalized = text
        .trim()
        .trim_matches(|ch: char| ch.is_ascii_punctuation())
        .to_lowercase();
    if normalized.split_whitespace().count() >= 2 {
        return false;
    }

    const SINGLE_FILLERS: &[&str] = &[
        "ok", "okay", "yes", "no", "yeah", "yep", "nope", "hmm", "uh", "um", "ah", "oh", "wow",
        "cool", "nice", "sure",
    ];
    SINGLE_FILLERS.iter().any(|filler| normalized == *filler)
}

pub fn should_accept(text: &str, cfg: &AlwaysConfig) -> bool {
    !cfg.filter_enabled || !quick_reject(text)
}

#[cfg(test)]
mod tests {
    use super::quick_reject;

    #[test]
    fn rejects_single_fillers_only() {
        assert!(quick_reject("uh"));
        assert!(quick_reject("okay."));
        assert!(!quick_reject("uh open settings"));
    }
}
