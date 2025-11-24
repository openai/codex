//! OS specific definitions.

/// Unix specific definitions.
#[cfg(any(feature = "doc", all(feature = "unix", unix)))]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "unix")))]
pub mod unix {
	/// Raw Unix specific serial port settings.
	///
	/// On Linux, this is an alias for [`libc::termios2`].
	/// On other Unix platforms it is an alias for [`libc::termios`].
	#[cfg(all(unix, any(target_os = "linux", target_os = "android")))]
	pub type RawTermios = crate::sys::RawTermios;

	/// Raw Unix specific serial port settings.
	///
	/// On Linux, this is an alias for `libc::termios2`.
	/// On other Unix platforms it is an alias for [`libc::termios`].
	#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
	pub type RawTermios = crate::sys::RawTermios;

	/// Raw Unix specific serial port settings.
	///
	/// On Linux, this is an alias for `libc::termios2`.
	/// On other Unix platforms it is an alias for `libc::termios`.
	///
	/// Generate the documentation on a Unix platform to get an overview of the struct fields.
	#[cfg(not(unix))]
	#[repr(C)]
	pub struct RawTermios {
		_priv: (),
	}
}

/// Windows specific definitions.
#[cfg(any(feature = "doc", all(feature = "windows", windows)))]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "windows")))]
pub mod windows {
	/// Raw Windows specific serial port settings.
	///
	/// For more information, see:
	/// [https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-dcb](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-dcb)
	#[cfg(windows)]
	pub use winapi::um::winbase::DCB;

	/// Raw Windows specific serial port settings.
	///
	/// For more information, see:
	/// [https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-dcb](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-dcb)
	///
	/// Generate the documentation on Windows to get an overview of the struct fields.
	#[cfg(not(windows))]
	#[non_exhaustive]
	pub struct DCB;

	/// Windows specific timeouts for a serial port.
	///
	/// Use [`crate::SerialPort::get_windows_timeouts()`] to get the timeouts,
	/// and [`crate::SerialPort::set_windows_timeouts()`] to apply them.
	///
	/// For more information, see:
	/// [https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-commtimeouts](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-commtimeouts)
	///
	/// Note that changing the read timeouts can easily lead to the serial port timing out on every read unless you are very careful.
	/// Please read the whole MSDN article about serial port timeouts linked above, including the remarks.
	///
	/// You are strongly suggested to use [`crate::SerialPort::set_read_timeout()`] and [`crate::SerialPort::set_write_timeout()`] instead.
	#[allow(missing_docs)] // People should read the microsoft docs we link instead.
	#[repr(C)]
	pub struct CommTimeouts {
		pub read_interval_timeout: u32,
		pub read_total_timeout_multiplier: u32,
		pub read_total_timeout_constant: u32,
		pub write_total_timeout_multiplier: u32,
		pub write_total_timeout_constant: u32,
	}
}
