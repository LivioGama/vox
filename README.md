# Vox — Voice Activation Daemon

High-performance voice-to-text automation. Speak naturally and have your words instantly appear in any application.

## Quick Start

1. **Install API Keys**
   ```bash
   vox config set groq_api_key "your-groq-api-key"
   ```

2. **Start Voice Daemon**
   ```bash
   vox always start
   ```

3. **Speak and watch your words appear automatically!**

## What is Vox?

Vox is an always-on voice activation daemon that:
- **Listens continuously** for your voice using advanced Voice Activity Detection
- **Transcribes speech instantly** using Groq's Whisper API 
- **Filters intelligently** to block filler words and politeness phrases
- **Pastes automatically** into any active application
- **Runs in background** without interrupting your workflow

## Architecture

```
Microphone → VAD Detection → Audio Recording → Groq Whisper → Intelligent Filtering → Auto-Paste
```

## Installation

### From Source
```bash
# Fast development builds
cargo build --profile release-fast
cargo install --path .
```

### Using Cargo
```bash
cargo install vox
```

## Usage

### Daemon Management
```bash
# Start always-on voice daemon
vox always start

# Check if daemon is running
vox always status

# Stop daemon
vox always stop

# Run in foreground (for debugging)
vox always run
```

### Configuration Management
```bash
# View current settings
vox config show

# Set API key (required)
vox config set groq_api_key "your-key"

# Adjust sensitivity (lower = more sensitive)
vox config set stt_energy_threshold 0.01

# Set silence timeout (seconds before stopping recording)
vox config set stt_silence 0.4

# Enable auto-enter (press Enter after pasting)
vox config set stt_auto_enter true

# Set cooldown between activations (milliseconds)
vox config set stt_cooldown_ms 150

# Reset all settings to defaults
vox config reset
```

### Keyboard Shortcuts (While Running)
- **Ctrl+Shift+P** — Pause/unpause voice listening
- **Ctrl+Shift+A** — Toggle auto-enter mode
- **Ctrl+C** — Stop daemon (when running in foreground)

## How It Works

### 1. Voice Activity Detection (VAD)
- Uses WebRTC VAD for real-time speech detection
- Configurable energy thresholds for different environments
- Intelligent noise filtering to avoid false triggers

### 2. Speech-to-Text Processing
- Records audio using optimized SoX integration
- Sends to Groq's Whisper API for transcription
- Memory pooling and connection reuse for minimal latency

### 3. Intelligent Filtering
Vox uses a two-tier filtering system:

**Hard Filter (Always Active)**
- Blocks "thank you", "you're welcome", "goodbye" 
- Prevents conversational phrases from being pasted
- Allows "hello" and "ok" (can be part of commands)

**Conversational Filter** 
- Blocks single-word fillers: "uh", "um", "hmm"
- Filters greetings and small talk
- Allows actual commands through

### 4. Vocabulary Enhancement
- Context-aware corrections for technical terms
- Project-specific vocabulary from codebase analysis
- Learns from your corrections over time

### 5. Auto-Paste Integration
- Types directly into any focused application
- Optional auto-enter for command execution
- Respects system accessibility permissions

## Configuration

### Required Settings
```bash
# Groq API key for speech-to-text
vox config set groq_api_key "gsk_..."
```

### Performance Tuning
```bash
# Very sensitive (good for quiet environments)
vox config set stt_energy_threshold 0.01

# Less sensitive (good for noisy environments) 
vox config set stt_energy_threshold 0.05

# Quick response (shorter silence detection)
vox config set stt_silence 0.4

# Longer phrases (more silence tolerance)
vox config set stt_silence 2.0

# Faster activation (shorter cooldown)
vox config set stt_cooldown_ms 150

# Prevent double-activation (longer cooldown)
vox config set stt_cooldown_ms 1500
```

### Advanced Options
```bash
# Custom log file location
vox config set always_log_path "/path/to/custom.log"

# Environment variables
export VOX_VAD_MODE=local              # VAD mode (local only currently)
export VOX_GROQ_MODEL=llama3-8b-8192   # Groq model for post-processing
export VOX_LEARNING_LIMIT=1000         # Max corrections to remember
export VOX_GRAMMAR_CORRECTION=true     # Enable grammar fixes
```

## Files and Locations

### Configuration
- **Database**: `~/.config/vox/vox.db`
- **Log File**: `~/.config/vox/always.log`
- **PID File**: `~/.config/vox/always.pid`

## Troubleshooting

### Daemon Won't Start
```bash
# Check if already running
vox always status

# View logs for errors
tail -f ~/.config/vox/always.log

# Test microphone permissions
vox always run  # Run in foreground to see errors
```

### Audio Issues
```bash
# Install SoX for audio recording
# On macOS: brew install sox
# On Ubuntu/Debian: apt install sox
# On Arch: pacman -S sox

# Test microphone
rec -t wav test.wav trim 0 3  # Record 3 seconds

# Check energy threshold is appropriate
vox config set stt_energy_threshold 0.05  # Less sensitive
```

### API Issues
```bash
# Verify API key is set
vox config show

# Test API connectivity
curl -H "Authorization: Bearer $GROQ_API_KEY" \
     https://api.groq.com/openai/v1/models
```

### Permission Issues
**macOS:**
1. Grant microphone access when prompted
2. Grant accessibility access for auto-paste:
   - System Settings → Privacy & Security → Accessibility
   - Add and enable the application

**Linux:**
- Ensure your user is in the `audio` group
- Check PulseAudio/ALSA permissions

**Windows:**
- Grant microphone permissions in Privacy Settings
- May require running as administrator for global hotkeys

## Development

### Building
```bash
# Fast development builds
cargo build --profile release-fast

# Optimized production builds  
cargo build --release
```

### Testing
```bash
# Run all tests
cargo test

# Test specific module
cargo test filter
cargo test audio
```

### Dependencies
- **Rust** 1.70+ 
- **SoX** (audio recording)
  - macOS: `brew install sox`
  - Ubuntu/Debian: `apt install sox`
  - Arch: `pacman -S sox`
  - Windows: Download from [sox.sourceforge.net](http://sox.sourceforge.net/)
- **Groq API Key** (free at groq.com)

## Architecture Details

### Core Components
- **VAD Module** (`src/always/vad.rs`) — Voice activity detection and recording
- **Audio Module** (`src/always/audio.rs`) — Optimized audio processing with memory pooling
- **Filter Module** (`src/always/filter.rs`) — Two-tier intelligent filtering system
- **Config Module** (`src/always/config.rs`) — Configuration management and defaults
- **Event Loop** (`src/always/event_loop.rs`) — Main daemon processing loop

### Performance Optimizations
- **Memory Pooling** — Reused audio buffers to minimize allocations
- **Connection Reuse** — HTTP client pooling for API calls
- **Optimized VAD** — Fast energy detection for low thresholds
- **Efficient Recording** — Persistent SoX processes to avoid spawn overhead
- **Smart Filtering** — Two-stage filtering to minimize API calls

### Vocabulary System
- **Base Vocabulary** — Common programming terms and patterns
- **Context Vocabulary** — Project-specific terms extracted from codebase
- **Learning System** — Groq-powered grammar and context corrections
- **Pattern Matching** — Smart recognition of code patterns, file paths, etc.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make changes with tests
4. Run the test suite: `cargo test`
5. Build and test: `cargo build --release`
6. Submit a pull request

## License

MIT License - see LICENSE file for details.