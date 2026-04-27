//! Loading overlay system for showing voice processing status

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Global loading state
static LOADING_STATE: once_cell::sync::Lazy<Arc<AtomicBool>> =
    once_cell::sync::Lazy::new(|| Arc::new(AtomicBool::new(false)));

/// Show a loading overlay while processing voice input
#[cfg(target_os = "macos")]
pub fn show_loading_overlay() -> anyhow::Result<()> {
    LOADING_STATE.store(true, Ordering::Relaxed);
    // Skip notifications for speed - just update state for other components
    eprint!("🎤");
    std::io::Write::flush(&mut std::io::stderr())?;
    Ok(())
}

/// Hide the loading overlay
#[cfg(target_os = "macos")]
pub fn hide_loading_overlay() -> anyhow::Result<()> {
    LOADING_STATE.store(false, Ordering::Relaxed);
    Ok(())
}

/// Show an animated loading indicator with dots
#[cfg(target_os = "macos")]
pub fn show_animated_loading() -> anyhow::Result<std::thread::JoinHandle<()>> {
    LOADING_STATE.store(true, Ordering::Relaxed);

    let handle = thread::spawn(|| {
        let dots = ["🎤 Processing   ", "🎤 Processing.  ", "🎤 Processing.. ", "🎤 Processing..."];
        let mut counter = 0;

        while LOADING_STATE.load(Ordering::Relaxed) {
            let message = dots[counter % 4];

            let script = format!(
                r#"
                tell application "System Events"
                    display notification "{}" with title "VOX"
                end tell
                "#,
                message
            );

            let mut cmd = Command::new("osascript");
            cmd.arg("-e").arg(script);
            let _ = cmd.output();

            counter += 1;
            thread::sleep(Duration::from_millis(500));
        }
    });

    Ok(handle)
}

/// Show a heads-up display overlay with loading dots
#[cfg(target_os = "macos")]
pub fn show_hud_loading() -> anyhow::Result<std::thread::JoinHandle<()>> {
    LOADING_STATE.store(true, Ordering::Relaxed);

    let handle = thread::spawn(|| {
        let script = format!(
            r#"
            set the_dialog to "🎤 Processing..."
            tell application "System Events"
                display dialog the_dialog with title "VOX" buttons {{"Cancel"}} default button "Cancel" giving up after 1 with icon note
            end tell
            "#
        );

        let mut cmd = Command::new("osascript");
        cmd.arg("-e").arg(script);
        let _ = cmd.output();
    });

    Ok(handle)
}

/// Show a simple persistent overlay (most visible option)
#[cfg(target_os = "macos")]
pub fn show_persistent_overlay() -> anyhow::Result<()> {
    // Skip slow notifications - just update stderr for speed
    eprint!("⏳");
    std::io::Write::flush(&mut std::io::stderr())?;
    Ok(())
}

/// Hide any active loading overlays
pub fn stop_loading() {
    LOADING_STATE.store(false, Ordering::Relaxed);
    // Clear the loading indicator
    eprint!("\r   \r");
    let _ = std::io::Write::flush(&mut std::io::stderr());
}

/// Check if loading is currently active
pub fn is_loading() -> bool {
    LOADING_STATE.load(Ordering::Relaxed)
}

#[cfg(not(target_os = "macos"))]
pub fn show_loading_overlay() -> anyhow::Result<()> {
    LOADING_STATE.store(true, Ordering::Relaxed);
    eprintln!("🎤 Processing audio...");
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn hide_loading_overlay() -> anyhow::Result<()> {
    LOADING_STATE.store(false, Ordering::Relaxed);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn show_persistent_overlay() -> anyhow::Result<()> {
    eprintln!("🎤 Processing audio...");
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn show_animated_loading() -> anyhow::Result<std::thread::JoinHandle<()>> {
    LOADING_STATE.store(true, Ordering::Relaxed);

    let handle = thread::spawn(|| {
        let dots = ["🎤 Processing   ", "🎤 Processing.  ", "🎤 Processing.. ", "🎤 Processing..."];
        let mut counter = 0;

        while LOADING_STATE.load(Ordering::Relaxed) {
            eprint!("\r{}", dots[counter % 4]);
            std::io::Write::flush(&mut std::io::stderr()).unwrap();
            counter += 1;
            thread::sleep(Duration::from_millis(500));
        }
        eprintln!(); // New line when done
    });

    Ok(handle)
}