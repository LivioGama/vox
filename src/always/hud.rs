//! HUD (Heads-Up Display) overlay for vox status

#[cfg(target_os = "macos")]
use std::process::Command;

/// Show a status bar notification with sound
#[cfg(target_os = "macos")]
pub fn show_status_bar_indicator(paused: bool) -> anyhow::Result<()> {
    let icon = if paused { "🔇" } else { "🎤" };
    let status = if paused { "PAUSED" } else { "ACTIVE" };

    // Use AppleScript to create a notification with sound
    let script = format!(
        r#"
        set theText to "{} VOX {}"
        tell application "System Events"
            display notification theText with title "VOX Status" sound name "Glass"
        end tell
        "#,
        icon, status
    );

    let mut cmd = Command::new("osascript");
    cmd.arg("-e").arg(script);

    let output = cmd.output()?;
    if !output.status.success() {
        eprintln!("Status notification failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn show_status_bar_indicator(paused: bool) -> anyhow::Result<()> {
    let icon = if paused { "🔇" } else { "🎤" };
    let status = if paused { "PAUSED" } else { "ACTIVE" };
    eprintln!("{} VOX {}", icon, status);
    Ok(())
}

/// Show enhanced visual feedback with notification only
pub fn show_enhanced_status(paused: bool) -> anyhow::Result<()> {
    // Show status bar indicator (notification with sound)
    show_status_bar_indicator(paused)?;
    Ok(())
}