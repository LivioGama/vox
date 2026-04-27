# Repository Atlas: vox

## Project Responsibility
Cross-platform text-to-speech (TTS) CLI tool with six pluggable backends, voice cloning, MCP server integration for AI assistants, and optional daemon for low-latency inference. Targets terminal users, AI coding assistants (Claude Code/Desktop), and voice interface developers.

## System Architecture

### Core Philosophy
**Backend-agnostic TTS abstraction** — Strategy Pattern enables runtime selection of synthesis engines:
- Native system voices (macOS `say`)
- Pure Rust neural TTS (piper, qwen-native, kokoro)
- Python ML bridges (qwen via MLX-Audio)
- Zero-shot voice cloning (voxtream, qwen, qwen-native)

### Entry Points
- **Cargo.toml**: Dependency manifest, feature flags (`kokoro`, `metal`, `cuda`), release profile
- **src/main.rs**: CLI entry point with Clap command parsing
- **src/lib.rs**: Public library API surface
- **models.toml**: TTS model configuration (hub IDs, local paths, default settings)

### Key Design Patterns
1. **Strategy Pattern**: `TtsBackend` trait with 6 implementations
2. **Factory Pattern**: `backend::get_backend(name)` dynamic dispatch
3. **Repository Pattern**: SQLite-backed persistence for voice clones, stats, personas
4. **MCP Protocol**: JSON-RPC stdio server exposing 14 tools to AI assistants
5. **Daemon Pattern**: Optional background process for model warmup

## Directory Map (Aggregated)

| Directory | Responsibility | Detailed Map |
|-----------|----------------|--------------|
| **src/** | Core application logic, CLI orchestration, module coordination | [View Map](src/codemap.md) |
| **src/backend/** | TTS backend abstraction layer (Strategy Pattern for 6 synthesis engines) | [View Map](src/backend/codemap.md) |
| **src/chat/** | Voice conversation system with Claude API streaming (macOS-only) | [View Map](src/chat/codemap.md) |

## Top-Level Files

### Configuration
- **Cargo.toml**: Rust package manifest
  - Dependencies: clap (CLI), rusqlite (DB), tokio (async), rodio (audio), whisper-rs (STT), qwen3-tts (ML), piper-rs (TTS)
  - Features: `kokoro` (opt-in ONNX backend), `metal` (Apple acceleration), `cuda` (NVIDIA acceleration)
  - Build profile: LTO, single codegen-unit, strip symbols for release binary
- **models.toml**: TTS model registry (Hugging Face hub IDs, local paths, default voice/lang)

### Documentation
- **README.md**: User-facing documentation (install, usage, backend comparison)
- **CLAUDE.md**: AI assistant integration guide (MCP setup, slash commands)
- **AGENTS.md**: Agent-specific instructions (vox feedback in French after task completion)
- **CHANGELOG.md**: Release history
- **LICENSE**: Apache 2.0

### Infrastructure
- **install.sh**: One-line installer script (downloads pre-built binary for macOS/Linux)
- **.github/workflows/**: CI/CD pipeline (test, build, release-please)
- **release-please-config.json**: Automated release configuration

## Technology Stack

### Language & Runtime
- **Rust 2024 Edition**: Systems programming, memory safety, zero-cost abstractions
- **Tokio**: Async runtime for MCP server, network I/O, concurrent operations

### ML/AI Frameworks
- **Candle**: Pure Rust ML framework (qwen-native backend)
- **MLX-Audio**: Apple Silicon acceleration (qwen backend, macOS-only)
- **ONNX Runtime**: Optimized inference (piper, kokoro backends)
- **Whisper**: Speech-to-text (whisper-rs bindings with Metal acceleration)

### Audio
- **Rodio**: Cross-platform audio playback (WAV/MP3 decoding, volume control)
- **Hound**: WAV file encoding/decoding
- **SoX**: External `rec` command for microphone recording

### Storage
- **Rusqlite**: SQLite embedded database (voice clones, usage stats, personas)
- **Dirs**: Cross-platform user data directories

### UI/UX
- **Clap**: Command-line argument parsing with derive macros
- **Ratatui**: Terminal UI framework (setup wizard, interactive config)
- **Eframe/Egui**: Native GUI (system tray integration, planned)

## Data Flow — Standard TTS Operation

```
1. CLI Input
   ↓
2. main.rs (Clap parsing)
   ↓
3. input::get_text() — resolve text source (args/stdin/file/clipboard)
   ↓
4. backend::get_backend(backend_name) — factory constructs TTS engine
   ↓
5. backend.speak(text, opts) — synthesize audio
   ↓
6. rodio playback — stream audio to speakers
   ↓
7. db::log_usage() — record operation metadata
```

## Integration Ecosystem

### AI Assistants (via MCP)
- **Claude Code** (VS Code extension)
- **Claude Desktop** (standalone app)
- **Cursor**, **Windsurf**, **Zed**, **Codex**, etc. (any MCP-compatible tool)

**14 MCP Tools**:
- `vox_say` — Speak text aloud
- `vox_clone_add`, `vox_clone_list`, `vox_clone_remove` — Manage voice clones
- `vox_stats` — Usage statistics
- `vox_bench` — Auto-detect best backend for hardware
- `vox_chat` — Start voice conversation (macOS)
- `vox_hear` — Record + transcribe speech (macOS)
- Plus 6 more for personas, packs, daemon control

### Platform Integration
- **macOS**: Native `say` command, Accessibility API (clipboard pasting), Metal acceleration
- **Linux**: ALSA/PulseAudio audio, CUDA support
- **Windows**: DirectSound audio, CUDA support

## Feature Flags

| Flag | Effect | Target Use Case |
|------|--------|-----------------|
| `kokoro` | Enable kokoro-tts backend (ONNX, English-only) | Minimal latency on macOS |
| `metal` | Apple Metal GPU acceleration for qwen-native | M1/M2/M3 Mac performance |
| `cuda` | NVIDIA CUDA acceleration for qwen-native | Linux/Windows GPU workstations |

**Default build**: No optional features (portable CPU-only binary)

## Performance Characteristics

### Cold Start (First Invocation)
- **say**: 3s (system voice, no model loading)
- **piper**: <1s (fast ONNX loading)
- **qwen-native**: 11m33s (large Candle model load)
- **voxtream**: 68s (PyTorch model load + inference)

### Warm Inference (Subsequent Calls)
- **piper**: <1s
- **qwen**: ~2s (with daemon)
- **qwen-native**: ~3s (with daemon)
- **voxtream**: 19s GPU / 40s CPU

### Daemon Mode
Keeps models in memory (global `Arc<Mutex<Model>>`) to skip cold start:
- `vox daemon start` — Fork background process
- `vox daemon status` — Check running state
- `vox daemon stop` — Terminate gracefully

## Roadmap (from AGENTS.md)

### Phase 1: STT Foundation (vox_hear)
- Local Whisper-based speech-to-text
- VAD (Voice Activity Detection) for phrase boundary detection
- Exposed as MCP tool

### Phase 2: Streaming TTS
- Feed Claude API response stream sentence-by-sentence to TTS
- Reduce perceived latency by starting audio before full generation completes

### Phase 3: Full Conversational Loop (vox_converse)
- Continuous hear → Claude API stream → speak stream
- Push-to-talk / VAD / full-duplex modes
- Target latency: ~800ms to first syllable

### Phase 4: Personas
- `.voxpersona` files: voice clone + style + system prompt
- `vox persona create` command
- Multi-personality conversation support

## Build Artifacts

### Target Directory Layout
- `target/release/vox` — Optimized CLI binary (stripped, LTO)
- `target/debug/vox` — Development build with symbols

### Distribution
- **GitHub Releases**: Pre-built binaries for 5 platforms (macOS ARM, Linux x64, Linux x64+CUDA, Windows x64, Windows x64+CUDA)
- **Homebrew Tap**: `brew install rtk-ai/tap/vox` (macOS)
- **One-line installer**: `curl -fsSL https://raw.githubusercontent.com/rtk-ai/vox/main/install.sh | sh`

## Development Commands

```bash
# Build all features
cargo build --release --all-features

# Test suite
cargo test

# Run with logs
RUST_LOG=debug cargo run -- -b qwen "Hello world"

# Benchmarks
cargo test --release -- --test-threads=1 perf_test

# Generate documentation
cargo doc --no-deps --open
```

## Related Documentation
- [src/codemap.md](src/codemap.md) — Core module architecture
- [src/backend/codemap.md](src/backend/codemap.md) — TTS backend abstraction
- [src/chat/codemap.md](src/chat/codemap.md) — Voice conversation system
