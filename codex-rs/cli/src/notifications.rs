use std::io::Write;
use std::{fs, path::PathBuf, process::Command, time::Instant};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct NotificationsConfig {
    #[serde(default)]
    pub sound: bool,
    #[serde(default)]
    pub only_on_long_runs_ms: u64,
    #[serde(default = "default_true")]
    pub respect_focus: bool,
    #[serde(default = "default_str_default")]
    pub success_tone: String,
    #[serde(default = "default_str_default")]
    pub error_tone: String,
}

fn default_true() -> bool { true }
fn default_str_default() -> String { "default".to_string() }

pub fn load_notifications_config() -> NotificationsConfig {
    let mut p = home_dir();
    p.push(".codex");
    p.push("config.toml");
    let Ok(raw) = fs::read_to_string(p) else { return NotificationsConfig::default() };
    let Ok(toml_val) = raw.parse::<toml::Value>() else { return NotificationsConfig::default() };
    let Some(notifs) = toml_val.get("notifications") else { return NotificationsConfig::default() };
    let Ok(cfg) = notifs.clone().try_into() else { return NotificationsConfig::default() };
    cfg
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir())
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn should_notify(start: Instant, cfg: &NotificationsConfig) -> bool {
    if !cfg.sound { return false; }
    let min = cfg.only_on_long_runs_ms;
    if min == 0 { return true; }
    let elapsed = Instant::now().saturating_duration_since(start).as_millis() as u64;
    elapsed >= min
}

pub fn play_completion_sound(ok: bool, cfg: &NotificationsConfig) {
    if !cfg.sound { return; }

    #[cfg(target_os = "macos")]
    {
        let tone = if ok { &cfg.success_tone } else { &cfg.error_tone };
        let default_path = if ok { "/System/Library/Sounds/Hero.aiff" } else { "/System/Library/Sounds/Basso.aiff" };
        let path = if tone == "default" { default_path } else { tone };
        if Command::new("afplay").arg(path).spawn().is_err() { bell(); }
        return;
    }

    #[cfg(target_os = "windows")]
    {
        let tone = if ok { &cfg.success_tone } else { &cfg.error_tone };
        let name = if tone == "default" { if ok { "Asterisk" } else { "Hand" } } else { tone };
        let ps = format!(r#"Add-Type -AssemblyName System.Windows.Forms; [System.Media.SystemSounds]::{name}.Play()"#);
        if Command::new("powershell").args(["-NoProfile", "-Command", &ps]).spawn().is_err() { bell(); }
        return;
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        let tone = if ok { &cfg.success_tone } else { &cfg.error_tone };
        let id = if tone == "default" { if ok { "complete" } else { "dialog-error" } } else { tone.as_str() };
        if Command::new("canberra-gtk-play").args(["--id", id]).spawn().is_ok() { return; }
        let path = if ok {
            "/usr/share/sounds/freedesktop/stereo/complete.oga"
        } else {
            "/usr/share/sounds/freedesktop/stereo/dialog-error.oga"
        };
        if Command::new("paplay").arg(path).spawn().is_ok() { return; }
        bell();
    }
}

fn bell() {
    let _ = std::io::Write::write_all(&mut std::io::stdout(), b"\x07");
    let _ = std::io::stdout().flush();
}
