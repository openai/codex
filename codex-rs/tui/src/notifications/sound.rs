use std::io;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;

pub fn play_notification_sound() {
    if try_spawn_sound_command() {
        return;
    }

    let _ = emit_terminal_bell();
}

fn emit_terminal_bell() -> io::Result<()> {
    let mut stdout = io::stdout();
    stdout.write_all(b"\x07")?;
    stdout.flush()
}

#[cfg(unix)]
fn try_spawn_sound_command() -> bool {
    if spawn_sound_command("canberra-gtk-play", &["-i", "bell"]) {
        return true;
    }

    if let Some(sound_path) = find_freedesktop_sound() {
        if spawn_sound_command("paplay", &[sound_path]) {
            return true;
        }
        if spawn_sound_command("pw-play", &[sound_path]) {
            return true;
        }
    }

    if let Some(sound_path) = find_alsa_sound()
        && spawn_sound_command("aplay", &[sound_path]) {
            return true;
        }

    false
}

#[cfg(not(unix))]
fn try_spawn_sound_command() -> bool {
    false
}

fn spawn_sound_command(program: &str, args: &[&str]) -> bool {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    command.spawn().is_ok()
}

#[cfg(unix)]
fn find_freedesktop_sound() -> Option<&'static str> {
    let path = "/usr/share/sounds/freedesktop/stereo/complete.oga";
    Path::new(path).exists().then_some(path)
}

#[cfg(unix)]
fn find_alsa_sound() -> Option<&'static str> {
    let path = "/usr/share/sounds/alsa/Front_Center.wav";
    Path::new(path).exists().then_some(path)
}
