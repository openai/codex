//! Example showcasing the use of `set_text_wait` and spawning a daemon to allow the clipboard's
//! contents to live longer than the process on Linux.

use arboard::Clipboard;
#[cfg(target_os = "linux")]
use arboard::SetExtLinux;
use std::{env, error::Error, process};

// An argument that can be passed into the program to signal that it should daemonize itself. This
// can be anything as long as it is unlikely to be passed in by the user by mistake.
const DAEMONIZE_ARG: &str = "__internal_daemonize";

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
	#[cfg(target_os = "linux")]
	if env::args().nth(1).as_deref() == Some(DAEMONIZE_ARG) {
		Clipboard::new()?.set().wait().text("Hello, world!")?;
		return Ok(());
	}

	env_logger::init();

	if cfg!(target_os = "linux") {
		process::Command::new(env::current_exe()?)
			.arg(DAEMONIZE_ARG)
			.stdin(process::Stdio::null())
			.stdout(process::Stdio::null())
			.stderr(process::Stdio::null())
			.current_dir("/")
			.spawn()?;
	} else {
		Clipboard::new()?.set_text("Hello, world!")?;
	}

	Ok(())
}
