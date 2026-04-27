# src/backend/

## Responsibility
TTS backend abstraction layer implementing the Strategy Pattern for pluggable text-to-speech engines. Provides platform-gated backend selection with unified interface for voice synthesis across macOS-native, Python-based, and pure Rust implementations.

## Design Patterns
- **Strategy Pattern**: `TtsBackend` trait defines common interface; each backend (say, qwen, qwen-native, piper, kokoro, voxtream) implements the trait independently
- **Factory Pattern**: `get_backend(name: &str)` function constructs boxed trait objects based on string identifier
- **Platform Abstraction**: Conditional compilation (`#[cfg(target_os = "macos")]`) gates platform-specific backends
- **Feature Flags**: Optional backends enabled via Cargo features (e.g., `kokoro` feature)

## Data & Control Flow
1. Client calls `get_backend(backend_name)` with backend identifier
2. Factory matches name against available implementations (respecting platform gates and feature flags)
3. Returns `Box<dyn TtsBackend>` trait object
4. Client invokes `speak(text, SpeakOptions)` on trait object
5. Backend-specific implementation:
   - **say**: Spawns macOS `say` command subprocess
   - **qwen**: Calls Python MLX-based qwen3-tts via Python bridge
   - **qwen-native**: Pure Rust inference using qwen3-tts-rs crate
   - **piper**: Uses piper-rs bindings for local neural TTS
   - **kokoro**: Optional pure Rust kokoro-tts implementation
   - **voxtream**: Streaming TTS backend (implementation details in voxtream.rs)

## Integration Points
- **Consumed by**: 
  - `src/main.rs`: CLI argument parsing and command dispatch
  - `src/chat/mod.rs`: Voice chat system uses `qwen` backend for responses
  - `src/daemon.rs`: Keeps models warm for fast inference
  - `src/mcp.rs`: MCP server exposes backends to AI assistants
- **Depends on**:
  - External crates: `qwen3-tts`, `piper-rs`, `kokoro-tts` (optional)
  - System commands: macOS `say` (platform-gated)
  - `crate::config`: Default backend configuration
  - `crate::db`: Voice clone metadata for reference audio (used by chat module)

## Key Abstractions
- `TtsBackend` trait: 4 methods (`name()`, `speak()`, `list_voices()`, `is_available()`)
- `SpeakOptions` struct: Unified options bag (voice, lang, rate, gender, style, ref_audio, ref_text, model, volume)
- `Result<Box<dyn TtsBackend>>`: Factory return type for error propagation
