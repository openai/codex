//! RS-422 and RS-485 configuration.

use std::time::Duration;

/// The mode of a transceiver.
///
/// Some transceivers can be configured in different modes (RS-232, RS-422, RS-485) from software.
///
/// Warning: kernel and driver support for your serial port may be incomplete or bugged.
/// Unsupported features may silently be ignored by the kernel or your device.
/// If your device seems to misbehave, consult the documentation of your platform and your device driver to see what features it supports.
///
/// On Linux, also see the kernel documentation about [RS485 Serial Communication](https://www.kernel.org/doc/html/latest/driver-api/serial/serial-rs485.html).
#[derive(Debug, Clone)]
pub enum TransceiverMode {
	/// The default mode.
	///
	/// If the transceiver supports RS-232 and RS-422 or RS-485, the default mode is usually RS-232.
	Default,

	/// RS-422 mode.
	///
	/// Supported in the Linux kernel since version 6.8,
	/// but at the time of writing there are no device drivers included with the Linux kernel that use it.
	///
	/// Note that some device drivers may interpret full duplex RS-485 mode as RS-422 instead.
	Rs422,

	/// RS-485 mode.
	///
	/// In RS-485 mode, the kernel will automatically set the RTS (request-to-send) signal high before each transmission,
	/// and low again after each transmission.
	///
	/// For full-duplex (or 4-wire) RS-485 mode, set [`Rs485Config::set_full_duplex()`] to true.
	/// Otherwise, the receiver will be disabled during transmissions to avoid reading back your own message.
	Rs485(Rs485Config),
}

/// RS-485 specific configuration options.
///
/// Note that devices may silently ignore unsupported options instead of raising an error.
#[derive(Debug, Clone, Default)]
pub struct Rs485Config {
	/// Enable full-duplex (or 4-wire) RS-485 mode.
	///
	/// Enable this if your device is using different twisted pairs for transmitting and receiving data.
	/// With this mode enabled, the receiver will be left enabled while transmitting data.
	full_duplex: bool,

	/// Some transceivers allow enabling or disabling a termination resistor for the RS-485 bus.
	///
	/// If set to true, enable the termination resistor.
	///
	/// Note that this option may be silently ignored by devices that do not support it.
	terminate_bus: bool,

	/// Time in milliseconds to delay after setting the RTS signal, before starting transmission.
	///
	/// May be needed to give some devices on the bus time to activate their receiver.
	delay_before_send: Duration,

	/// Time in milliseconds to delay after finishing a transmission, before clearing the RTS signal.
	///
	/// May be needed to give some devices on the bus time to fully receive the message before they disable their receiver.
	delay_after_send: Duration,

	/// Invert the RTS signal: set it low during transmissions and high after.
	invert_rts: bool,
}

impl Rs485Config {
	/// Create a new RS-485 configuration with all options disabled and all delays set to zero.
	pub fn new() -> Self {
		Self::default()
	}

	/// Enable or disable full-duplex (or 4-wire) mode.
	///
	/// Enable this if your device is using different twisted pairs for transmitting and receiving data.
	/// With this mode enabled, the receiver will be left enabled while transmitting data.
	///
	/// Note that this option may be silently ignored by devices that do not support it.
	pub fn set_full_duplex(&mut self, enable: bool) {
		self.full_duplex = enable;
	}

	/// Check if the full-duplex (or 4-wire) mode is enabled.
	pub fn get_full_duplex(&self) -> bool {
		self.full_duplex
	}

	/// Enable or disable the bus termination resistor.
	///
	/// Note that this option may be silently ignored by devices that do not support it.
	pub fn set_bus_termination(&mut self, enable: bool) {
		self.terminate_bus = enable;
	}

	/// Check if the bus termination resistor is enabled.
	pub fn get_bus_termination(&self) -> bool {
		self.terminate_bus
	}

	/// Set the time to delay after setting the RTS signal, before starting a transmission.
	///
	/// This may be needed to give some devices on the bus time to activate their receiver.
	///
	/// The precision will be truncated to whatever the platform supports.
	/// On Linux, the delay supports millisecond precision.
	///
	/// Note that this option may be silently ignored by devices that do not support it.
	pub fn set_delay_before_send(&mut self, delay: Duration) {
		self.delay_before_send = delay;
	}

	/// Get the delay time after setting the RTS signal, before starting a transmission.
	pub fn get_delay_before_send(&self) -> Duration {
		self.delay_before_send
	}

	/// Set the time to delay after setting the RTS signal, before starting transmission.
	///
	/// This may be needed to give some devices on the bus time to fully receive the message before they disable their receiver.
	///
	/// The precision will be truncated to whatever the platform supports.
	/// On Linux, the delay supports millisecond precision.
	///
	/// Note that this option may be silently ignored by devices that do not support it.
	pub fn set_delay_after_send(&mut self, delay: Duration) {
		self.delay_after_send = delay;
	}

	/// Get the delay time after setting the RTS signal, before starting a transmission.
	pub fn get_delay_after_send(&self) -> Duration {
		self.delay_after_send
	}

	/// Set whether to invert the level of the RTS signal.
	///
	/// If enabled, the RTS signal will be set low during transmissions and high again after each transmission.
	///
	/// Note that this option may be silently ignored by devices that do not support it.
	pub fn set_invert_rts(&mut self, invert: bool) {
		self.invert_rts = invert;
	}

	/// Check if the level of the RTS signal is inverted.
	///
	/// If enabled, the RTS signal will be set low during transmissions and high again after each transmission.
	pub fn get_invert_rts(&self) -> bool {
		self.invert_rts
	}
}

impl From<Rs485Config> for TransceiverMode {
	fn from(other: Rs485Config) -> Self {
		Self::Rs485(other)
	}
}
