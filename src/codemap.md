# src/

## Responsibility
Core application logic and CLI entry point. Orchestrates TTS operations, voice cloning, database management, MCP server, daemon lifecycle, GUI/TUI interfaces, and AI assistant integrations.

## Design Patterns
- **Command Pattern**: Clap-based CLI with subcommands (`Clone`, `Chat`, `Daemon`, `Init`, `Serve`, etc.)
- **Facade Pattern**: Top-level modules expose simplified interfaces to complex subsystems
- **Repository Pattern**: `db.rs` abstracts SQLite operations for voice clones, usage stats, personas
- **Server Pattern**: `mcp.rs` implements Model Context Protocol stdio server
- **Daemon Pattern**: `daemon.rs` manages long-running background process for model warmup

## Module Responsibilities

### Entry Points
- **main.rs**: CLI argument parsing, command dispatch, error handling
- **lib.rs**: Public API surface for library consumers

### Core Features
- **audio.rs**: Audio playback abstraction (rodio-based WAV playback with volume control)
- **clone.rs**: Voice cloning management (add/remove/list reference audio for zero-shot TTS)
- **config.rs**: User preferences loading/saving (default backend, model, voice)
- **db.rs**: SQLite database operations (voice clones, usage statistics, personas)
- **input.rs**: Text input handling (stdin, file paths, clipboard integration)

### AI Integration
- **mcp.rs**: MCP server implementation exposing 14 tools over stdio for Claude Code/Desktop
- **init.rs**: AI assistant integration installer (MCP config, CLAUDE.md, slash commands)
- **chat.rs**: Voice conversation system with Claude API streaming (macOS-only, see `src/chat/codemap.md`)
- **stt.rs**: Speech-to-text using Whisper (macOS-only, VAD-based recording)

### Infrastructure
- **daemon.rs**: TTS daemon for model warmup (reduces cold-start latency from 15s → 2s)
- **pack.rs**: Fun sound pack manager (peon-ping compatible celebratory audio)
- **tui.rs**: Terminal UI for interactive setup (ratatui-based voice configuration)
- **gui.rs**: Native GUI for system tray integration (eframe/egui-based)

## Data & Control Flow

### Standard TTS Flow
1. `main.rs` parses CLI args via Clap
2. If no subcommand: `text` positional args joined → TTS pipeline
3. `input::get_text()` resolves input source (args, stdin, file, clipboard)
4. `backend::get_backend(backend_name)` constructs TTS engine
5. `backend.speak(text, opts)` synthesizes audio
6. `db::log_usage()` records statistics (text length, backend, timestamp)

### Voice Clone Flow
1. `Commands::Clone::Add` → `clone::add_voice_clone(name, audio_path, text)`
2. `db::save_voice_clone()` stores reference audio path + transcription in SQLite
3. Future TTS calls with `-v clone_name` → `db::get_voice_clone(name)` → `SpeakOptions { ref_audio, ref_text }`

### MCP Server Flow
1. `vox serve` → `mcp::run_server()`
2. Reads JSON-RPC requests from stdin
3. Dispatches to 14 tool handlers (speak, clone_add, clone_list, etc.)
4. Writes JSON-RPC responses to stdout
5. Logs operations to SQLite via `db::` module

### Daemon Flow
1. `vox daemon start` → forks background process
2. Daemon loads TTS model into memory (qwen-native or kokoro)
3. Listens on Unix socket for inference requests
4. Client sends text → daemon responds with audio path (avoids cold-start delay)

## Integration Points

### Internal Dependencies
- **backend/**: All modules depend on TTS backend abstraction
- **db.rs**: Used by main.rs (stats), clone.rs (storage), mcp.rs (logging), chat.rs (voice clones)
- **config.rs**: Used by main.rs (default backend), daemon.rs (model selection), mcp.rs (preferences)

### External Dependencies
- **Clap**: CLI argument parsing and validation
- **Rusqlite**: SQLite database for persistent state
- **Tokio**: Async runtime for MCP server and network operations
- **Rodio**: Cross-platform audio playback
- **Whisper-rs**: Speech-to-text (macOS Metal acceleration)
- **Ratatui/Crossterm**: Terminal UI framework
- **Eframe/Egui**: Native GUI framework

### System Integration
- **Claude Code/Desktop**: Via MCP stdio server (`vox serve`)
- **macOS Accessibility**: Clipboard pasting for STT transcript injection
- **SoX**: External `rec` command for audio recording
- **Python Bridge**: qwen backend spawns Python subprocess (mlx-audio, qwen3-tts)

## Platform-Specific Features
- **macOS-only**: `chat`, `stt`, `qwen` backend, `say` backend
- **Cross-platform**: `qwen-native`, `piper`, `kokoro`, `voxtream` backends, daemon, MCP server, GUI
