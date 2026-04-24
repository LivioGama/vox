//! SQLite database — preferences, voice clones, usage logging, and statistics.
//!
//! All state is persisted in `~/.config/vox/vox.db` with WAL mode enabled.
//! Schema is auto-migrated on first open.

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::config;

#[derive(Debug, Clone, Default)]
pub struct Preferences {
    pub backend: Option<String>,
    pub voice: Option<String>,
    pub lang: Option<String>,
    pub rate: Option<u32>,
    pub gender: Option<String>,
    pub style: Option<String>,
    pub model: Option<String>,
    pub pack: Option<String>,
    pub stt_threshold: Option<f64>,
    pub stt_silence: Option<f64>,
    pub stt_trim_silence: Option<bool>,
    pub stt_auto_enter: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct VoiceClone {
    pub name: String,
    pub ref_audio: String,
    pub ref_text: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct UsageEntry {
    pub timestamp: String,
    pub backend: String,
    pub voice: Option<String>,
    pub lang: Option<String>,
    pub text_len: usize,
    pub duration_ms: Option<u64>,
}

pub fn open() -> Result<Connection> {
    let path = config::db_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create config directory")?;
    }
    let conn = Connection::open(&path).context("Failed to open database")?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS preferences (
            id      INTEGER PRIMARY KEY CHECK (id = 1),
            backend TEXT,
            voice   TEXT,
            lang    TEXT,
            rate    INTEGER,
            gender  TEXT,
            style   TEXT,
            model   TEXT,
            stt_threshold REAL,
            stt_silence REAL,
            stt_trim_silence INTEGER,
            stt_auto_enter INTEGER
        );

        CREATE TABLE IF NOT EXISTS usage_log (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S','now')),
            backend     TEXT NOT NULL,
            voice       TEXT,
            lang        TEXT,
            text_len    INTEGER NOT NULL,
            duration_ms INTEGER
        );

        CREATE TABLE IF NOT EXISTS voice_clones (
            name       TEXT PRIMARY KEY,
            ref_audio  TEXT NOT NULL,
            ref_text   TEXT,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S','now'))
        );",
    )?;

    // Add pack column if it doesn't exist (migration for existing DBs)
    let has_pack = conn.prepare("SELECT pack FROM preferences LIMIT 0").is_ok();
    if !has_pack {
        conn.execute_batch("ALTER TABLE preferences ADD COLUMN pack TEXT;")?;
    }

    let has_stt_threshold = conn
        .prepare("SELECT stt_threshold FROM preferences LIMIT 0")
        .is_ok();
    if !has_stt_threshold {
        conn.execute_batch("ALTER TABLE preferences ADD COLUMN stt_threshold REAL;")?;
    }

    let has_stt_silence = conn
        .prepare("SELECT stt_silence FROM preferences LIMIT 0")
        .is_ok();
    if !has_stt_silence {
        conn.execute_batch("ALTER TABLE preferences ADD COLUMN stt_silence REAL;")?;
    }

    let has_stt_trim_silence = conn
        .prepare("SELECT stt_trim_silence FROM preferences LIMIT 0")
        .is_ok();
    if !has_stt_trim_silence {
        conn.execute_batch("ALTER TABLE preferences ADD COLUMN stt_trim_silence INTEGER;")?;
    }

    let has_stt_auto_enter = conn
        .prepare("SELECT stt_auto_enter FROM preferences LIMIT 0")
        .is_ok();
    if !has_stt_auto_enter {
        conn.execute_batch("ALTER TABLE preferences ADD COLUMN stt_auto_enter INTEGER;")?;
    }

    Ok(())
}

// --- Preferences ---

pub fn get_preferences(conn: &Connection) -> Result<Preferences> {
    let mut stmt = conn.prepare(
        "SELECT backend, voice, lang, rate, gender, style, model, pack, stt_threshold, stt_silence, stt_trim_silence, stt_auto_enter FROM preferences WHERE id = 1",
    )?;
    let result = stmt.query_row([], |row| {
        Ok(Preferences {
            backend: row.get(0)?,
            voice: row.get(1)?,
            lang: row.get(2)?,
            rate: row.get::<_, Option<u32>>(3)?,
            gender: row.get(4)?,
            style: row.get(5)?,
            model: row.get(6)?,
            pack: row.get(7)?,
            stt_threshold: row.get(8)?,
            stt_silence: row.get(9)?,
            stt_trim_silence: row.get::<_, Option<i64>>(10)?.map(|v| v != 0),
            stt_auto_enter: row.get::<_, Option<i64>>(11)?.map(|v| v != 0),
        })
    });
    match result {
        Ok(prefs) => Ok(prefs),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(Preferences::default()),
        Err(e) => Err(e.into()),
    }
}

pub fn set_preference(conn: &Connection, key: &str, value: &str) -> Result<()> {
    let valid_keys = [
        "backend",
        "voice",
        "lang",
        "rate",
        "gender",
        "style",
        "model",
        "pack",
        "stt_threshold",
        "stt_silence",
        "stt_trim_silence",
        "stt_auto_enter",
    ];
    if !valid_keys.contains(&key) {
        anyhow::bail!(
            "Unknown preference: {key}. Valid keys: {}",
            valid_keys.join(", ")
        );
    }

    // Validate specific keys
    match key {
        "gender" => {
            config::Gender::parse(value)?;
        }
        "style" => {
            config::IntonationStyle::parse(value)?;
        }
        "rate" => {
            value
                .parse::<u32>()
                .context("Rate must be a positive integer")?;
        }
        "lang" => {
            if !config::SUPPORTED_LANGS.contains(&value) {
                anyhow::bail!(
                    "Unsupported language: {value}. Supported: {}",
                    config::SUPPORTED_LANGS.join(", ")
                );
            }
        }
        "stt_threshold" => {
            let parsed = value
                .parse::<f64>()
                .context("stt_threshold must be a number")?;
            if !(0.1..=10.0).contains(&parsed) {
                anyhow::bail!("stt_threshold must be between 0.1 and 10.0 (percent)");
            }
        }
        "stt_silence" => {
            let parsed = value
                .parse::<f64>()
                .context("stt_silence must be a number")?;
            if !(0.2..=15.0).contains(&parsed) {
                anyhow::bail!("stt_silence must be between 0.2 and 15.0 seconds");
            }
        }
        "stt_trim_silence" | "stt_auto_enter" => {
            if !matches!(value, "true" | "false" | "1" | "0") {
                anyhow::bail!("{key} must be one of: true, false, 1, 0");
            }
        }
        "backend" => {
            #[cfg(target_os = "macos")]
            let valid_backends = ["kokoro", "say", "qwen", "qwen-native"];
            #[cfg(not(target_os = "macos"))]
            let valid_backends = ["kokoro", "qwen-native"];
            if !valid_backends.contains(&value) {
                anyhow::bail!(
                    "Unknown backend: {value}. Must be one of: {}",
                    valid_backends.join(", ")
                );
            }
        }
        _ => {}
    }

    // Upsert: insert or update
    conn.execute(
        "INSERT INTO preferences (id, backend, voice, lang, rate, gender, style, model, pack, stt_threshold, stt_silence, stt_trim_silence, stt_auto_enter)
         VALUES (1, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL)
         ON CONFLICT(id) DO NOTHING",
        [],
    )?;
    let sql = format!("UPDATE preferences SET {key} = ?1 WHERE id = 1");
    let normalized = match key {
        "stt_trim_silence" | "stt_auto_enter" => {
            if matches!(value, "true" | "1") {
                "1"
            } else {
                "0"
            }
        }
        _ => value,
    };
    conn.execute(&sql, [normalized])?;
    Ok(())
}

pub fn reset_preferences(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM preferences WHERE id = 1", [])?;
    Ok(())
}

// --- Voice Clones ---

pub fn add_clone(
    conn: &Connection,
    name: &str,
    ref_audio: &str,
    ref_text: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO voice_clones (name, ref_audio, ref_text) VALUES (?1, ?2, ?3)",
        rusqlite::params![name, ref_audio, ref_text],
    )?;
    Ok(())
}

pub fn get_clone(conn: &Connection, name: &str) -> Result<Option<VoiceClone>> {
    let mut stmt = conn.prepare(
        "SELECT name, ref_audio, ref_text, created_at FROM voice_clones WHERE name = ?1",
    )?;
    let result = stmt.query_row([name], |row| {
        Ok(VoiceClone {
            name: row.get(0)?,
            ref_audio: row.get(1)?,
            ref_text: row.get(2)?,
            created_at: row.get(3)?,
        })
    });
    match result {
        Ok(clone) => Ok(Some(clone)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn list_clones(conn: &Connection) -> Result<Vec<VoiceClone>> {
    let mut stmt = conn
        .prepare("SELECT name, ref_audio, ref_text, created_at FROM voice_clones ORDER BY name")?;
    let rows = stmt.query_map([], |row| {
        Ok(VoiceClone {
            name: row.get(0)?,
            ref_audio: row.get(1)?,
            ref_text: row.get(2)?,
            created_at: row.get(3)?,
        })
    })?;
    let mut clones = Vec::new();
    for row in rows {
        clones.push(row?);
    }
    Ok(clones)
}

pub fn remove_clone(conn: &Connection, name: &str) -> Result<bool> {
    let count = conn.execute("DELETE FROM voice_clones WHERE name = ?1", [name])?;
    Ok(count > 0)
}

// --- Usage Log ---

pub fn log_usage(
    conn: &Connection,
    backend: &str,
    voice: Option<&str>,
    lang: Option<&str>,
    text_len: usize,
    duration_ms: Option<u64>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO usage_log (backend, voice, lang, text_len, duration_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![backend, voice, lang, text_len as i64, duration_ms.map(|d| d as i64)],
    )?;
    Ok(())
}

pub fn get_usage_stats(conn: &Connection) -> Result<Vec<UsageEntry>> {
    let mut stmt = conn.prepare(
        "SELECT timestamp, backend, voice, lang, text_len, duration_ms FROM usage_log ORDER BY id DESC LIMIT 50",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(UsageEntry {
            timestamp: row.get(0)?,
            backend: row.get(1)?,
            voice: row.get(2)?,
            lang: row.get(3)?,
            text_len: row.get::<_, i64>(4)? as usize,
            duration_ms: row.get::<_, Option<i64>>(5)?.map(|d| d as u64),
        })
    })?;
    let mut entries = Vec::new();
    for row in rows {
        entries.push(row?);
    }
    Ok(entries)
}

pub fn get_usage_summary(conn: &Connection) -> Result<(u64, u64)> {
    let mut stmt = conn.prepare("SELECT COUNT(*), COALESCE(SUM(text_len), 0) FROM usage_log")?;
    let (count, total_chars) = stmt.query_row([], |row| {
        Ok((row.get::<_, i64>(0)? as u64, row.get::<_, i64>(1)? as u64))
    })?;
    Ok((count, total_chars))
}

#[derive(Debug)]
pub struct BackendStats {
    pub backend: String,
    pub calls: u64,
    pub total_chars: u64,
    pub total_duration_ms: u64,
}

#[derive(Debug)]
pub struct LangStats {
    pub lang: String,
    pub calls: u64,
}

pub fn get_backend_stats(conn: &Connection) -> Result<Vec<BackendStats>> {
    let mut stmt = conn.prepare(
        "SELECT backend, COUNT(*), COALESCE(SUM(text_len), 0), COALESCE(SUM(duration_ms), 0) FROM usage_log GROUP BY backend ORDER BY COUNT(*) DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(BackendStats {
            backend: row.get(0)?,
            calls: row.get::<_, i64>(1)? as u64,
            total_chars: row.get::<_, i64>(2)? as u64,
            total_duration_ms: row.get::<_, i64>(3)? as u64,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_lang_stats(conn: &Connection) -> Result<Vec<LangStats>> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(lang, '?'), COUNT(*) FROM usage_log GROUP BY lang ORDER BY COUNT(*) DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(LangStats {
            lang: row.get(0)?,
            calls: row.get::<_, i64>(1)? as u64,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_total_duration_ms(conn: &Connection) -> Result<u64> {
    let mut stmt = conn.prepare("SELECT COALESCE(SUM(duration_ms), 0) FROM usage_log")?;
    let total = stmt.query_row([], |row| row.get::<_, i64>(0))?;
    Ok(total as u64)
}
