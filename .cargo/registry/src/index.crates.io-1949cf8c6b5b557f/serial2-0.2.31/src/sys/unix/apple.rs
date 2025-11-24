use std::path::PathBuf;
use std::os::unix::io::RawFd;

/// A ioctl to set the baud rate of a serial port.
///
/// Value taken from random forum because there is no public documentation.
/// * https://fpc-pascal.freepascal.narkive.com/oI4b0CM2/non-standard-baud-rates-in-os-x-iossiospeed-ioctl
/// * https://github.com/dcuddeback/serial-rs/issues/37
const IOCTL_IOSSIOSPEED: u64 = 0x80045402;

/// Set the baud rate of a serial port using the IOSSIOSPEED ioctl.
///
/// The speed set this way applied to the input and the output speed.
///
/// According to some source, the speed set this way is *not* reported back in the `termios` struct by `tcgetattr`,
/// but according to other sources it *is*.
/// Even the two examples from Apple below contradict each-other on this point.
///
/// Testing seems to suggest that the value *is* reported correctly by `tcgetattr`.
/// So to avoid synchronization problems with other FDs for the same serial port we trust `tcgetattr`.
/// https://github.com/de-vri-es/serial2-rs/issues/38#issuecomment-2182531900
///
/// This is Apple, so there is no public documentation (why would you?).
/// This is the best I could find:
/// * https://opensource.apple.com/source/IOSerialFamily/IOSerialFamily-91/tests/IOSerialTestLib.c.auto.html
/// * https://developer.apple.com/library/archive/samplecode/SerialPortSample/Listings/SerialPortSample_SerialPortSample_c.html
pub fn ioctl_iossiospeed(fd: RawFd, baud_rate: libc::speed_t) -> Result<(), std::io::Error> {
	unsafe {
		super::check(libc::ioctl(fd, IOCTL_IOSSIOSPEED, &baud_rate))?;
		Ok(())
	}
}

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
	// Sigh, closed source doesn't have to mean undocumented.
	// Anyway:
	// https://stackoverflow.com/questions/14074413/serial-port-names-on-mac-os-x
	// https://learn.adafruit.com/ftdi-friend/com-slash-serial-port-name
	name.starts_with(b"tty.") || name.starts_with(b"cu.")
}
