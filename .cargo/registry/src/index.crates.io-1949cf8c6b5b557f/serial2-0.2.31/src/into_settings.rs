use crate::Settings;

/// Trait for objects that can configure a serial port.
///
/// The simplest option is to pass a `u32`, which is used to set the baud rate of the port.
/// That will also disable all OS level input and output modification,
/// configure a character size of 8 bits with 1 stop bit,
/// and it disables paritity checks and flow control.
///
/// For more control, it is possible to pass a `Fn(Settings) -> std::io::Result<Settings>`.
/// If you do, you will generally want to start with a call to [`Settings::set_raw()`].
///
/// To open a serial port without modifying any settings, pass [`KeepSettings`].
pub trait IntoSettings {
	/// Apply the configuration to an existing [`Settings`] struct.
	fn apply_to_settings(self, settings: &mut Settings) -> std::io::Result<()>;
}

impl<F> IntoSettings for F
where
	F: FnOnce(Settings) -> std::io::Result<Settings>,
{
	fn apply_to_settings(self, settings: &mut Settings) -> std::io::Result<()> {
		*settings = (self)(settings.clone())?;
		Ok(())
	}
}

impl IntoSettings for u32 {
	fn apply_to_settings(self, settings: &mut Settings) -> std::io::Result<()> {
		settings.set_raw();
		settings.set_baud_rate(self)?;
		Ok(())
	}
}

/// A serial port "configuration" that simply keeps all existing settings.
///
/// You can pass this to [`SerialPort::open()`][crate::SerialPort::open()] to prevent it from changing any port settings.
///
/// Note: many platforms reset the configuration of a serial port when it is no longer in use.
/// You should normally explicitly configure the settings that you care about.
pub struct KeepSettings;

impl IntoSettings for KeepSettings {
	fn apply_to_settings(self, _settings: &mut Settings) -> std::io::Result<()> {
		Ok(())
	}
}
