use cfg_if::cfg_if;
use std::path::{Path, PathBuf};

#[cfg(feature = "rs4xx")]
mod rs4xx;

#[cfg(feature = "rs4xx")]
pub use rs4xx::*;

cfg_if! {
	if #[cfg(any(target_arch = "sparc", target_arch = "sparc64"))] {
		pub const BAUD_RATES: &[(u32, u32)] = &[
			(libc::B50, 50),
			(libc::B75, 75),
			(libc::B110, 110),
			(libc::B134, 134),
			(libc::B150, 150),
			(libc::B200, 200),
			(libc::B300, 300),
			(libc::B600, 600),
			(libc::B1200, 1200),
			(libc::B1800, 1800),
			(libc::B2400, 2400),
			(libc::B4800, 4800),
			(libc::B9600, 9600),
			(libc::B19200, 19200),
			(libc::B38400, 38400),
			(libc::B57600, 57600),
			(libc::B76800, 76800),
			(libc::B115200, 115200),
			(libc::B153600, 153600),
			(libc::B230400, 230400),
			(libc::B307200, 307200),
			(libc::B460800, 460800),
			(libc::B500000, 500000),
			(libc::B576000, 576000),
			(libc::B614400, 614400),
			(libc::B921600, 921600),
			(libc::B1000000, 1000000),
			(libc::B1152000, 1152000),
			(libc::B1500000, 1500000),
			(libc::B2000000, 2000000),
		];
	} else {
		pub const BAUD_RATES: &[(u32, u32)] = &[
			(libc::B50, 50),
			(libc::B75, 75),
			(libc::B110, 110),
			(libc::B134, 134),
			(libc::B150, 150),
			(libc::B200, 200),
			(libc::B300, 300),
			(libc::B600, 600),
			(libc::B1200, 1200),
			(libc::B1800, 1800),
			(libc::B2400, 2400),
			(libc::B4800, 4800),
			(libc::B9600, 9600),
			(libc::B19200, 19200),
			(libc::B38400, 38400),
			(libc::B57600, 57600),
			(libc::B115200, 115200),
			(libc::B230400, 230400),
			(libc::B460800, 460800),
			(libc::B500000, 500000),
			(libc::B576000, 576000),
			(libc::B921600, 921600),
			(libc::B1000000, 1000000),
			(libc::B1152000, 1152000),
			(libc::B1500000, 1500000),
			(libc::B2000000, 2000000),
			(libc::B2500000, 2500000),
			(libc::B3000000, 3000000),
			(libc::B3500000, 3500000),
			(libc::B4000000, 4000000),
		];
	}
}

pub fn enumerate() -> std::io::Result<Vec<PathBuf>> {
	use std::os::unix::ffi::OsStrExt;
	use std::os::unix::fs::FileTypeExt;

	let dir = std::fs::read_dir("/sys/class/tty")?;
	let mut entries = Vec::with_capacity(32);

	for entry in dir {
		// Skip entries we can't stat.
		let entry = match entry {
			Ok(x) => x,
			Err(_) => continue,
		};

		let name = entry.file_name();

		// Skip everything that doesn't have a matching device node in /dev
		let dev_path = Path::new("/dev").join(&name);
		match dev_path.metadata() {
			Err(_) => continue,
			Ok(metadata) => {
				if !metadata.file_type().is_char_device() {
					continue;
				}
			},
		}

		match name.as_bytes().strip_prefix(b"tty") {
			// Skip entries called "tty";
			Some(b"") => continue,
			// Skip "tty1", "tty2", etc (they are virtual terminals, not serial ports).
			Some(&[c, ..]) if c.is_ascii_digit() => continue,
			// Skip everything that doesn't start with "tty", they are almost certainly not serial ports.
			None => continue,
			// Accept the rest.
			Some(_) => (),
		};

		// There's a bunch of ttyS* ports that are not really serial ports.
		//
		// They have a file called `device/driver_override` set to "(null)".
		if let Ok(driver_override) = std::fs::read(entry.path().join("device/driver_override")) {
			if driver_override == b"(null)\n" {
				continue;
			}
		}

		entries.push(dev_path);
	}

	Ok(entries)
}
