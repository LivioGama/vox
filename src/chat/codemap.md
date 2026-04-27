# src/chat/

## Responsibility
Voice conversation system integrating Claude API streaming with local TTS. Manages conversational state, sentence boundary detection, and real-time speech synthesis during LLM generation. macOS-only module.

## Design Patterns
- **Pipeline Pattern**: Audio input → STT → Claude API → sentence chunking → TTS → audio output
- **Streaming Adapter**: Converts Claude's Server-Sent Events (SSE) into sentence-bounded TTS chunks
- **State Machine**: Conversation loop with history tracking and exit condition detection
- **Builder Pattern**: `build_claude_request()` constructs API payloads with system prompt and message history

## Data & Control Flow
1. **Recording Phase**: `record_until_enter()` spawns `rec` subprocess (SoX), waits for Enter keypress, terminates recording
2. **Transcription**: Recorded WAV passed to `crate::stt` module (Whisper-based STT)
3. **API Request**: `build_claude_request()` constructs payload with:
   - System prompt: `SYSTEM_PROMPT` constant (French conversational assistant)
   - Conversation history: `Vec<Message>` (role + content pairs)
   - Model: `DEFAULT_MODEL` or user-specified via `ChatConfig`
   - Streaming enabled: `stream: true`
4. **Streaming Response**: `streaming::run_chat_loop()` consumes SSE stream from Claude API
5. **Sentence Chunking**: `sentence::` module detects sentence boundaries in real-time
6. **TTS Synthesis**: `speak_text()` invokes `qwen` backend with voice clone options
7. **Exit Detection**: `is_exit()` checks transcribed text against `EXIT_WORDS` list

## Integration Points
- **Consumed by**:
  - `src/main.rs`: `Commands::Chat` variant spawns conversation loop
  - `src/mcp.rs`: MCP tool `vox_chat` exposes conversational interface to AI assistants
- **Depends on**:
  - `crate::backend`: Uses `qwen` backend for TTS (`get_backend("qwen")`)
  - `crate::db`: Fetches voice clone metadata (`VoiceClone` struct with ref_audio/ref_text)
  - `crate::stt`: Whisper-based speech-to-text transcription
  - External API: Anthropic Claude API (`https://api.anthropic.com/v1/messages`)
  - System commands: `rec` from SoX audio toolkit

## Key Abstractions
- `ChatConfig` struct: Encapsulates voice clone, language, API key, model selection
- `Message` struct: Serializable Claude API message format (role + content)
- `claude_api::ClaudeRequest`: Request payload builder
- `sentence::` module: Real-time sentence boundary detection for streaming TTS
- `streaming::run_chat_loop()`: Main conversation loop with state management
- Exit condition: `EXIT_WORDS` array supports multilingual termination ("quit", "au revoir", etc.)
