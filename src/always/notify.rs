pub fn notify(title: &str, body: &str, accepted: bool) {
    let preview = body.chars().take(80).collect::<String>();
    let sound = if accepted { "Glass" } else { "Basso" };
    let script =
        format!("display notification {preview:?} with title {title:?} sound name {sound:?}");
    let _ = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .status();
}
