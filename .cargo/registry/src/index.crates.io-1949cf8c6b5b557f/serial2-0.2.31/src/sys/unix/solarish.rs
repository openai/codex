use std::path::PathBuf;

// All values taken from:
// https://github.com/illumos/illumos-gate/blob/252adeb303174e992b64771bf9639e63a4d55418/usr/src/uts/common/sys/termios.h

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
	(libc::B921600, 921600),
];

pub fn enumerate() -> std::io::Result<Vec<PathBuf>> {
	use std::os::unix::fs::FileTypeExt;

	// https://illumos.org/man/1M/ports
	// Let's hope Solaris is doing the same.
	// If only Oracle actually had navigatable documentation.
	let cua = std::fs::read_dir("/dev/cua")?;
	let term = std::fs::read_dir("/dev/cua")?;

	let serial_ports = cua
		.chain(term)
		.filter_map(|entry| {
			let entry = entry.ok()?;
			let kind = entry.metadata().ok()?.file_type();
			if kind.is_char_device() {
				Some(entry.path())
			} else {
				None
			}
		})
		.collect();
	Ok(serial_ports)
}
