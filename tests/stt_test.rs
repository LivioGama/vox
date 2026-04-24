#![cfg(target_os = "macos")]

use vox::stt;

#[test]
fn transcribe_missing_file_returns_error() {
    let result = stt::transcribe("/tmp/does-not-exist.wav", Some("en"));
    assert!(result.is_err());
}

#[test]
fn transcribe_accepts_language_argument() {
    let result = stt::transcribe("/tmp/does-not-exist.wav", Some("fr"));
    assert!(result.is_err());
}
