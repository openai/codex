use std::os::unix::io::{AsFd, AsRawFd};
use std::time::Duration;

use crate::rs4xx::{Rs485Config, TransceiverMode};
use crate::sys::unix::{check, SerialPort};

/// Get the RS-485/422 mode of the serial port transceiver.
///
/// Driver and device support is very flaky.
/// See all the warnings in the [`crate::rs4xx`] module.
pub fn get_rs4xx_mode(port: &SerialPort) -> std::io::Result<crate::rs4xx::TransceiverMode> {
	let config = &SerialRs485::get_from_fd(&port.file)?;
	Ok(config.into())
}

/// Set the RS-485/422 mode of the serial port transceiver.
///
/// Driver and device support is very flaky.
/// See all the warnings in the [`crate::rs4xx`] module.
pub fn set_rs4xx_mode(port: &SerialPort, mode: &crate::rs4xx::TransceiverMode) -> std::io::Result<()> {
	let config = SerialRs485::from(mode);
	config.set_on_fd(&port.file)
}

#[rustfmt::skip]
#[allow(dead_code)]
mod flags {
	pub const SER_RS485_ENABLED:        u32 = 1 << 0;
	pub const SER_RS485_RTS_ON_SEND:    u32 = 1 << 1;
	pub const SER_RS485_RTS_AFTER_SEND: u32 = 1 << 2;
	pub const SER_RS485_RX_DURING_TX:   u32 = 1 << 4;
	pub const SER_RS485_TERMINATE_BUS:  u32 = 1 << 5;
	pub const SER_RS485_ADDRB:          u32 = 1 << 6;
	pub const SER_RS485_ADDR_RECV:      u32 = 1 << 7;
	pub const SER_RS485_ADDR_DEST:      u32 = 1 << 8;
	pub const SER_RS485_MODE_RS422:     u32 = 1 << 9;
}

/// RS485 serial configuration
///
/// Internally, this structure is the same as a [`struct serial_rs485`] as defined by the Linux kernel.
/// See <https://docs.kernel.org/driver-api/serial/serial-rs485.html>.
#[derive(Debug, Default, Copy, Clone)]
#[repr(C)]
struct SerialRs485 {
	flags: u32,
	delay_rts_before_send_ms: u32,
	delay_rts_after_send_ms: u32,
	addr_recv: u8,
	addr_dest: u8,
	_padding0: [u8; 2],
	_padding1: [u32; 4],
}

impl SerialRs485 {
	/// Create a [`SerialRs485`] struct with the specified flags and otherwise all zeroed.
	fn new_with_flags(flags: u32) -> Self {
		Self {
			flags,
			delay_rts_before_send_ms: 0,
			delay_rts_after_send_ms: 0,
			addr_recv: 0,
			addr_dest: 0,
			_padding0: [0; 2],
			_padding1: [0; 4],
		}
	}

	/// Create a [`SerialRs485`] struct for RS-422 mode.
	fn new_rs422() -> Self {
		// Add the RX_DURING_TX_FLAG as fallback for devices that do not support the RS422 flag.
		Self::new_with_flags(flags::SER_RS485_ENABLED | flags::SER_RS485_MODE_RS422 | flags::SER_RS485_RX_DURING_TX)
	}

	/// Load settings from file descriptor
	///
	/// Settings will be loaded from the file descriptor, which must be a
	/// valid serial device support RS485 extensions
	pub fn get_from_fd(fd: &impl AsFd) -> std::io::Result<SerialRs485> {
		let fd = fd.as_fd().as_raw_fd();
		let mut conf = SerialRs485::new_with_flags(0);
		unsafe {
			check(libc::ioctl(fd, libc::TIOCGRS485, &mut conf))?;
		}
		Ok(conf)
	}

	/// Apply settings to file descriptor
	///
	/// Applies the constructed configuration a raw file-descriptor using
	/// `ioctl`.
	pub fn set_on_fd(&self, fd: &impl AsFd) -> std::io::Result<()> {
		let fd = fd.as_fd().as_raw_fd();
		unsafe {
			check(libc::ioctl(fd, libc::TIOCSRS485, self))?;
		}
		Ok(())
	}
}

impl From<&'_ TransceiverMode> for SerialRs485 {
	fn from(other: &TransceiverMode) -> Self {
		match other {
			TransceiverMode::Default => Self::new_with_flags(0),
			TransceiverMode::Rs422 => Self::new_rs422(),
			TransceiverMode::Rs485(config) => config.into(),
		}
	}
}

impl From<&'_ Rs485Config> for SerialRs485 {
	fn from(config: &Rs485Config) -> Self {
		let mut flags = flags::SER_RS485_ENABLED;
		if config.get_full_duplex() {
			flags |= flags::SER_RS485_RX_DURING_TX;
		}
		if config.get_bus_termination() {
			flags |= flags::SER_RS485_TERMINATE_BUS;
		}
		if config.get_invert_rts() {
			flags |= flags::SER_RS485_RTS_AFTER_SEND;
		} else {
			flags |= flags::SER_RS485_RTS_ON_SEND;
		}

		let delay_rts_before_send_ms = config
			.get_delay_before_send()
			.as_millis()
			.try_into()
			.unwrap_or(u32::MAX);
		let delay_rts_after_send_ms = config.get_delay_after_send().as_millis().try_into().unwrap_or(u32::MAX);

		Self {
			flags,
			delay_rts_before_send_ms,
			delay_rts_after_send_ms,
			addr_recv: 0,
			addr_dest: 0,
			_padding0: [0; 2],
			_padding1: [0; 4],
		}
	}
}

impl From<&SerialRs485> for TransceiverMode {
	fn from(other: &SerialRs485) -> Self {
		if other.flags & flags::SER_RS485_ENABLED == 0 {
			return TransceiverMode::Default;
		}
		if other.flags & flags::SER_RS485_MODE_RS422 != 0 {
			return TransceiverMode::Rs422;
		}

		let mut config = Rs485Config::new();
		config.set_full_duplex(other.flags & flags::SER_RS485_RX_DURING_TX != 0);
		config.set_bus_termination(other.flags & flags::SER_RS485_TERMINATE_BUS != 0);
		config.set_invert_rts(other.flags & flags::SER_RS485_RTS_ON_SEND == 0);
		config.set_delay_before_send(Duration::from_millis(other.delay_rts_before_send_ms.into()));
		config.set_delay_after_send(Duration::from_millis(other.delay_rts_after_send_ms.into()));
		TransceiverMode::Rs485(config)
	}
}
