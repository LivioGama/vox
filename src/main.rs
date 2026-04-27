use anyhow::Result;
use clap::{Parser, Subcommand};

use vox::db;

#[derive(Parser)]
#[command(name = "vox", version, about = "Voice activation daemon — DeepGram STT with intelligent transcription")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage preferences
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Manage vocabulary and context-aware corrections
    Vocab {
        #[command(subcommand)]
        action: VocabAction,
    },
    /// Always-on voice activation mode: speak, transcribe, paste, repeat
    Always {
        #[command(subcommand)]
        action: AlwaysAction,
    },
}

#[derive(Subcommand)]
enum VocabAction {
    /// Extract vocabulary from current project
    Extract {
        /// Project root directory (default: current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current preferences
    Show,
    /// Set a preference
    Set {
        /// Preference key
        key: String,
        /// Preference value
        value: String,
    },
    /// Reset all preferences to defaults
    Reset,
}

#[derive(Subcommand)]
enum AlwaysAction {
    /// Start always-on daemon in background
    Start {
        /// Language code for transcription
        #[arg(short = 'l', long, default_value = "en")]
        lang: String,
        /// Maximum recording duration per phrase in seconds
        #[arg(short = 't', long, default_value = "30")]
        timeout: u32,
        /// Seconds of silence before considering phrase complete
        #[arg(short = 's', long, default_value = "0.4")]
        silence: f64,
        /// Press Enter automatically after pasting transcript
        #[arg(long, default_value_t = false)]
        auto_enter: bool,
    },
    /// Stop always-on daemon
    Stop,
    /// Show always-on daemon status
    Status,
    /// Run always-on in foreground (for debugging)
    #[command(name = "run")]
    RunForeground {
        /// Language code for transcription
        #[arg(short = 'l', long, default_value = "en")]
        lang: String,
        /// Maximum recording duration per phrase in seconds
        #[arg(short = 't', long, default_value = "30")]
        timeout: u32,
        /// Seconds of silence before considering phrase complete
        #[arg(short = 's', long, default_value = "0.4")]
        silence: f64,
        /// Press Enter automatically after pasting transcript
        #[arg(long, default_value_t = false)]
        auto_enter: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Config { action }) => handle_config(action),
        Some(Commands::Vocab { action }) => handle_vocab(action),
        Some(Commands::Always { action }) => handle_always_action(action),
        None => {
            eprintln!("vox: voice activation daemon");
            eprintln!("Usage: vox <COMMAND>");
            eprintln!();
            eprintln!("Commands:");
            eprintln!("  always    Always-on voice activation mode");
            eprintln!("  config    Manage preferences");
            eprintln!("  vocab     Manage vocabulary and corrections");
            eprintln!();
            eprintln!("Use 'vox <COMMAND> --help' for more information on a command.");
            Ok(())
        }
    }
}

fn handle_config(action: ConfigAction) -> Result<()> {
    let conn = db::open()?;

    match action {
        ConfigAction::Show => {
            let prefs = db::get_preferences(&conn)?;
            println!(
                "deepgram_api_key: {}",
                prefs
                    .deepgram_api_key
                    .as_deref()
                    .map(|k| format!("{}...", &k[..k.len().min(12)]))
                    .unwrap_or("(not set)".to_string())
            );
            println!(
                "stt_energy_threshold: {}",
                prefs
                    .stt_energy_threshold
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "0.05".to_string())
            );
            println!(
                "hear_energy_threshold: {}",
                prefs
                    .hear_energy_threshold
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "0.002".to_string())
            );
            println!(
                "stt_cooldown_ms: {}",
                prefs
                    .stt_cooldown_ms
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "1500".to_string())
            );
            println!(
                "always_log_path: {}",
                prefs.always_log_path.as_deref().unwrap_or("(default)")
            );
            println!(
                "stt_silence: {}",
                prefs
                    .stt_silence
                    .map(|v| format!("{v}s"))
                    .unwrap_or_else(|| "2.0s".to_string())
            );
            println!(
                "stt_auto_enter: {}",
                prefs
                    .stt_auto_enter
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "false".to_string())
            );
            println!(
                "groq_api_key: {}",
                prefs
                    .groq_api_key
                    .as_deref()
                    .map(|k| format!("{}...", &k[..k.len().min(12)]))
                    .unwrap_or_else(|| "(not set)".to_string())
            );
        }
        ConfigAction::Set { key, value } => {
            db::set_preference(&conn, &key, &value)?;
            println!("{key} = {value}");
        }
        ConfigAction::Reset => {
            db::reset_preferences(&conn)?;
            println!("Preferences reset to defaults.");
        }
    }
    Ok(())
}

fn handle_vocab(action: VocabAction) -> Result<()> {
    match action {
        VocabAction::Extract { path } => {
            let project_root = path.unwrap_or_else(|| ".".to_string());
            println!("Vocabulary extraction not yet implemented for: {project_root}");
        }
    }
    Ok(())
}

fn handle_always_action(action: AlwaysAction) -> Result<()> {
    match action {
        AlwaysAction::Start {
            lang,
            timeout,
            silence,
            auto_enter,
        } => vox::always::daemon::start(&always_config(
            lang, timeout, silence, auto_enter,
        )?),
        AlwaysAction::Stop => vox::always::daemon::stop(),
        AlwaysAction::Status => vox::always::daemon::status(),
        AlwaysAction::RunForeground {
            lang,
            timeout,
            silence,
            auto_enter,
        } => vox::always::run(&always_config(
            lang, timeout, silence, auto_enter,
        )?),
    }
}

fn always_config(
    lang: String,
    timeout: u32,
    silence: f64,
    auto_enter: bool,
) -> Result<vox::always::AlwaysConfig> {
    vox::always::AlwaysConfig::from_cli(lang, timeout, silence, auto_enter)
}
