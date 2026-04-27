# Vox — DeepGram STT Refactor Worktree

This is a git worktree for refactoring Vox to use DeepGram API for speech-to-text (STT).

## Goal

Transform Vox from a multi-backend TTS/STT application into a lightweight voice activation daemon that uses only DeepGram API for transcription, removing all local ML models and TTS backends.

## What Changed

### Removed
- All TTS backends (say, piper, qwen-native, kokoro, voxtream, qwen)
- Local ML models (Whisper, voice cloning models)
- Voice cloning functionality
- TTS-related database tables (voice_clones, usage_log)
- TTS configuration options (backend, voice, rate, gender, style, model)
- Documentation files (READMEs in multiple languages, CHANGELOG, AGENTS.md, CLAUDE.md)
- CI/CD configurations (.github, release-please)
- Build scripts (restart.sh, start.sh, install.sh)

### Kept
- DeepGram API integration for STT
- Local VAD (webrtc-vad) for speech detection
- Always-on voice activation daemon
- Vocabulary transformation and context-aware corrections
- Groq-based post-processing
- Clipboard paste automation
- Configuration management (preferences, API keys)

## Architecture

```
Microphone → VAD (webrtc-vad) → Audio Recording → DeepGram API → Vocabulary Transformation → Groq Post-processing → Clipboard Paste
```

## Usage

### Set API Key
```bash
export DEEPGRAM_API_KEY="your-api-key"
# or
vox config set deepgram_api_key "your-api-key"
```

### Start Always-On Daemon
```bash
vox always start
```

### Run in Foreground (Debug)
```bash
vox always run
```

### Stop Daemon
```bash
vox always stop
```

### Show Status
```bash
vox always status
```

### Configuration
```bash
vox config show
vox config set stt_energy_threshold 0.05
vox config set hear_energy_threshold 0.002
vox config set stt_cooldown_ms 1500
```

## Database

Location: `~/.config/vox/vox.db`

Schema:
- `preferences` — User settings (API keys, thresholds, timeouts)
- Removed: `voice_clones`, `usage_log`

## Building

### For local development (faster builds)
```bash
cargo build --profile release-fast
```

### For release builds (maximum performance)
```bash
cargo build --release
```

The `release-fast` profile uses thin LTO and 16 codegen units for faster builds during development, trading ~1-3% runtime performance for much faster compilation times. Use `--release` for production builds.

- DeepGram API key required
- SoX (for audio recording): `brew install sox`
- VAD mode: `VOX_VAD_MODE=local` (default) or `deepgram` (planned)

## Status

✅ Compiles successfully  
⚠️ Minor warnings (unused imports, dead code) — non-blocking

## Next Steps

- Implement DeepGram streaming VAD mode
- Add real-time streaming transcription
- Improve vocabulary extraction
- Add comprehensive tests
