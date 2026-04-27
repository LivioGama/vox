//! Visual notification system for vox pause/resume and auto-enter status

use anyhow::Result;

#[cfg(target_os = "macos")]
mod macos {
    use std::process::Command;

    pub fn show_notification(title: &str, message: &str, sound: bool) -> anyhow::Result<()> {
        let mut cmd = Command::new("osascript");
        cmd.arg("-e");

        let sound_setting = if sound { "sound name \"Glass\"" } else { "" };

        let script = format!(
            r#"display notification "{}" with title "{}" {}"#,
            message, title, sound_setting
        );

        cmd.arg(script);
        let output = cmd.output()?;

        if !output.status.success() {
            eprintln!("Notification failed: {}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
mod fallback {
    pub fn show_notification(title: &str, message: &str, _sound: bool) -> anyhow::Result<()> {
        // For non-macOS systems, try to use notify-send if available
        if let Ok(_) = std::process::Command::new("notify-send")
            .arg(title)
            .arg(message)
            .output()
        {
            return Ok(());
        }

        // Fallback to console output
        eprintln!("🔔 {} - {}", title, message);
        Ok(())
    }
}

/// Show a system notification
pub fn notify(title: &str, message: &str, sound: bool) -> Result<()> {
    #[cfg(target_os = "macos")]
    return macos::show_notification(title, message, sound);

    #[cfg(not(target_os = "macos"))]
    return fallback::show_notification(title, message, sound);
}

/// Show pause status change with notification
pub fn show_pause_status_change(paused: bool) -> Result<()> {
    let (status, icon) = if paused {
        ("PAUSED", "🔇")
    } else {
        ("ACTIVE", "🎤")
    };

    // Show notification with sound
    notify("VOX", &format!("{} VOX {}", icon, status), true)?;

    // Also print to console for daemon logs
    eprintln!("{} VOX {} (Ctrl+Shift+P to toggle)", icon, status.to_lowercase());

    Ok(())
}

/// Show auto-enter status change with notification
pub fn show_auto_enter_status_change(enabled: bool) -> Result<()> {
    let (status, icon) = if enabled {
        ("ENABLED", "⏎")
    } else {
        ("DISABLED", "⏸")
    };

    // Show notification with sound
    notify("VOX Auto-Enter", &format!("{} Auto-Enter {}", icon, status), true)?;

    // Also print to console for daemon logs
    eprintln!("{} Auto-Enter {} (Ctrl+Shift+A to toggle)", icon, status.to_lowercase());

    Ok(())
}