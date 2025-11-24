use std::path::PathBuf;

pub fn enumerate() -> std::io::Result<Vec<PathBuf>> {
	use std::os::unix::ffi::OsStrExt;
	use std::os::unix::fs::FileTypeExt;

	let serial_ports = std::fs::read_dir("/dev")?
		.filter_map(|entry| {
			let entry = entry.ok()?;
			let kind = entry.metadata().ok()?.file_type();
			if kind.is_char_device() && is_tty_name(entry.file_name().as_bytes()) {
				Some(entry.path())
			} else {
				None
			}
		})
		.collect();
	Ok(serial_ports)
}

fn is_tty_name(name: &[u8]) -> bool {
		// For BSD variants, we simply report all entries in /dev that look like a TTY.
		// This may contain a lot of false positives for pseudo-terminals or other fake terminals.
		// If anyone can improve this for a specific BSD they love, by all means send a PR.

		// https://man.dragonflybsd.org/?command=sio&section=4
		// https://leaf.dragonflybsd.org/cgi/web-man?command=ucom&section=ANY
		#[cfg(target_os = "dragonfly")]
		const PREFIXES: [&[u8]; 4] = [b"ttyd", b"cuaa", b"ttyU", b"cuaU"];

		// https://www.freebsd.org/cgi/man.cgi?query=uart&sektion=4&apropos=0&manpath=FreeBSD+13.0-RELEASE+and+Ports
		// https://www.freebsd.org/cgi/man.cgi?query=ucom&sektion=4&apropos=0&manpath=FreeBSD+13.0-RELEASE+and+Ports
		#[cfg(target_os = "freebsd")]
		const PREFIXES: [&[u8]; 5] = [b"ttyu", b"cuau", b"cuad", b"ttyU", b"cuaU"];

		// https://man.netbsd.org/com.4
		// https://man.netbsd.org/ucom.4
		#[cfg(target_os = "netbsd")]
		const PREFIXES: [&[u8]; 4] = [b"tty", b"dty", b"ttyU", b"dtyU"];

		// https://man.openbsd.org/com
		// https://man.openbsd.org/ucom
		#[cfg(target_os = "openbsd")]
		const PREFIXES: [&[u8]; 4] = [b"tty", b"cua", b"ttyU", b"cuaU"];

		for prefix in PREFIXES {
			if let Some(suffix) = name.strip_prefix(prefix) {
				if !suffix.is_empty() && suffix.iter().all(|c| c.is_ascii_digit()) {
					return true;
				}
			}
		}

		false
}
