use std::io::{IoSlice, IoSliceMut};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::{sys, IntoSettings, Settings};

#[cfg(any(feature = "doc", all(feature = "rs4xx", target_os = "linux")))]
use crate::rs4xx;

/// A serial port.
pub struct SerialPort {
	inner: sys::SerialPort,
}

impl SerialPort {
	/// Open and configure a serial port by path or name.
	///
	/// On Unix systems, the `name` parameter must be a path to a TTY device.
	/// On Windows, it must be the name of a COM device, such as COM1, COM2, etc.
	///
	/// The second argument is used to configure the serial port.
	/// For simple cases, you pass a `u32` for the baud rate.
	/// See [`IntoSettings`] for more information.
	///
	/// The library automatically uses the win32 device namespace on Windows,
	/// so COM ports above COM9 are supported out of the box.
	///
	/// # Example 1: Open a serial port with a specific baud rate and default settings.
	/// ```
	/// # use serial2::SerialPort;
	/// # fn foo() -> std::io::Result<()> {
	/// SerialPort::open("/dev/ttyUSB0", 115200)?;
	/// #   Ok(())
	/// # }
	/// ```
	///
	/// # Example 2: Open a serial port with full control over the settings.
	/// ```
	/// # use serial2::{CharSize, FlowControl, Parity, SerialPort, Settings, StopBits};
	/// # fn foo() -> std::io::Result<()> {
	/// SerialPort::open("/dev/ttyUSB0", |mut settings: Settings| {
	///    settings.set_raw();
	///    settings.set_baud_rate(115200)?;
	///    settings.set_char_size(CharSize::Bits7);
	///    settings.set_stop_bits(StopBits::Two);
	///    settings.set_parity(Parity::Odd);
	///    settings.set_flow_control(FlowControl::RtsCts);
	///    Ok(settings)
	/// })?;
	/// #   Ok(())
	/// # }
	/// ```
	pub fn open(name: impl AsRef<Path>, settings: impl IntoSettings) -> std::io::Result<Self> {
		let mut serial_port = Self {
			inner: sys::SerialPort::open(name.as_ref())?,
		};
		let mut port_settings = serial_port.get_configuration()?;
		settings.apply_to_settings(&mut port_settings)?;
		serial_port.set_configuration(&port_settings)?;
		Ok(serial_port)
	}

	/// Open a connected pair of pseudo-terminals.
	#[cfg(any(feature = "doc", all(unix, feature = "unix")))]
	#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "unix")))]
	pub fn pair() -> std::io::Result<(Self, Self)> {
		#[cfg(unix)] {
			let (pty_a, pty_b) = sys::SerialPort::pair()?;
			let mut pty_a = Self { inner: pty_a };
			let mut pty_b = Self { inner: pty_b };
			{
				let mut settings = pty_a.get_configuration()?;
				settings.set_raw();
				pty_a.set_configuration(&settings)?;
			}
			{
				let mut settings = pty_b.get_configuration()?;
				settings.set_raw();
				pty_b.set_configuration(&settings)?;
			}

			Ok((pty_a, pty_b))
		}
		#[cfg(windows)] {
			unreachable!("this code is only enabled on Unix platforms or during documentation generation")
		}
	}

	/// Get a list of available serial ports.
	///
	/// Not currently supported on all platforms.
	/// On unsupported platforms, this function always returns an error.
	pub fn available_ports() -> std::io::Result<Vec<PathBuf>> {
		sys::enumerate()
	}

	/// Configure (or reconfigure) the serial port.
	pub fn set_configuration(&mut self, settings: &Settings) -> std::io::Result<()> {
		self.inner.set_configuration(&settings.inner)
	}

	/// Get the current configuration of the serial port.
	///
	/// This function can fail if the underlying syscall fails,
	/// or if the serial port configuration can't be reported using [`Settings`].
	pub fn get_configuration(&self) -> std::io::Result<Settings> {
		Ok(Settings {
			inner: self.inner.get_configuration()?,
		})
	}

	/// Try to clone the serial port handle.
	///
	/// The cloned object refers to the same serial port.
	///
	/// Mixing reads and writes on different handles to the same serial port from different threads may lead to unexpect results.
	/// The data may end up interleaved in unpredictable ways.
	pub fn try_clone(&self) -> std::io::Result<Self> {
		Ok(Self {
			inner: self.inner.try_clone()?,
		})
	}

	/// Read bytes from the serial port.
	///
	/// This is identical to [`std::io::Read::read()`], except that this function takes a const reference `&self`.
	/// This allows you to use the serial port concurrently from multiple threads.
	///
	/// Note that there are no guarantees on which thread receives what data when multiple threads are reading from the serial port.
	/// You should normally limit yourself to a single reading thread and a single writing thread.
	pub fn read(&self, buf: &mut [u8]) -> std::io::Result<usize> {
		self.inner.read(buf)
	}

	/// Read bytes from the serial port into a slice of buffers.
	///
	/// This is identical to [`std::io::Read::read_vectored()`], except that this function takes a const reference `&self`.
	/// This allows you to use the serial port concurrently from multiple threads.
	///
	/// Note that there are no guarantees on which thread receives what data when multiple threads are reading from the serial port.
	/// You should normally limit yourself to a single reading thread and a single writing thread.
	pub fn read_vectored(&self, buf: &mut [IoSliceMut<'_>]) -> std::io::Result<usize> {
		self.inner.read_vectored(buf)
	}

	/// Check if the implementation supports vectored reads.
	///
	/// If this returns false, then [`Self::read_vectored()`] will only use the first buffer of the given slice.
	/// All platforms except for Windows support vectored reads.
	pub fn is_read_vectored(&self) -> bool {
		self.inner.is_read_vectored()
	}

	/// Read the exact number of bytes required to fill the buffer from the serial port.
	///
	/// This will repeatedly call `read()` until the entire buffer is filled.
	/// Errors of the type [`std::io::ErrorKind::Interrupted`] are silently ignored.
	/// Any other errors (including timeouts) will be returned immediately.
	///
	/// If this function returns an error, it may already have read some data from the serial port into the provided buffer.
	///
	/// This function is identical to [`std::io::Read::read_exact()`], except that this function takes a const reference `&self`.
	/// This allows you to use the serial port concurrently from multiple threads.
	///
	/// Note that there are no guarantees on which thread receives what data when multiple threads are reading from the serial port.
	/// You should normally limit yourself to a single reading thread and a single writing thread.
	pub fn read_exact(&self, buf: &mut [u8]) -> std::io::Result<()> {
		let mut buf = buf;
		while !buf.is_empty() {
			match self.read(buf) {
				Ok(0) => return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "failed to fill whole buffer")),
				Ok(n) => buf = &mut buf[n..],
				Err(e) => {
					if e.kind() != std::io::ErrorKind::Interrupted {
						return Err(e);
					} else {
						continue;
					}
				},
			}
		}
		Ok(())
	}

	/// Write bytes to the serial port.
	///
	/// This is identical to [`std::io::Write::write()`], except that this function takes a const reference `&self`.
	/// This allows you to use the serial port concurrently from multiple threads.
	///
	/// Note that data written to the same serial port from multiple threads may end up interleaved at the receiving side.
	/// You should normally limit yourself to a single reading thread and a single writing thread.
	pub fn write(&self, buf: &[u8]) -> std::io::Result<usize> {
		self.inner.write(buf)
	}

	/// Write all bytes to the serial port.
	///
	/// This will repeatedly call [`Self::write()`] until the entire buffer has been written.
	/// Errors of the type [`std::io::ErrorKind::Interrupted`] are silently ignored.
	/// Any other errors (including timeouts) will be returned immediately.
	///
	/// If this function returns an error, it may already have transmitted some data from the buffer over the serial port.
	///
	/// This is identical to [`std::io::Write::write_all()`], except that this function takes a const reference `&self`.
	/// This allows you to use the serial port concurrently from multiple threads.
	///
	/// Note that data written to the same serial port from multiple threads may end up interleaved at the receiving side.
	/// You should normally limit yourself to a single reading thread and a single writing thread.
	pub fn write_all(&self, buf: &[u8]) -> std::io::Result<()> {
		let mut buf = buf;
		while !buf.is_empty() {
			match self.write(buf) {
				Ok(0) => return Err(std::io::Error::new(std::io::ErrorKind::WriteZero, "failed to write whole buffer")),
				Ok(n) => buf = &buf[n..],
				Err(e) => {
					if e.kind() != std::io::ErrorKind::Interrupted {
						return Err(e);
					} else {
						continue;
					}
				},
			}
		}
		Ok(())
	}

	/// Write bytes to the serial port from a slice of buffers.
	///
	/// This is identical to [`std::io::Write::write_vectored()`], except that this function takes a const reference `&self`.
	/// This allows you to use the serial port concurrently from multiple threads.
	///
	/// Note that data written to the same serial port from multiple threads may end up interleaved at the receiving side.
	/// You should normally limit yourself to a single reading thread and a single writing thread.
	pub fn write_vectored(&self, buf: &[IoSlice<'_>]) -> std::io::Result<usize> {
		self.inner.write_vectored(buf)
	}

	/// Check if the implementation supports vectored writes.
	///
	/// If this returns false, then [`Self::write_vectored()`] will only use the first buffer of the given slice.
	/// All platforms except for Windows support vectored writes.
	pub fn is_write_vectored(&self) -> bool {
		self.inner.is_write_vectored()
	}

	/// Flush all data queued to be written.
	///
	/// This will block until the OS buffer has been fully transmitted.
	///
	/// This is identical to [`std::io::Write::flush()`], except that this function takes a const reference `&self`.
	pub fn flush(&self) -> std::io::Result<()> {
		self.inner.flush_output()
	}

	/// Set the read timeout for the serial port.
	///
	/// The timeout set by this function is an upper bound on individual calls to [`read()`][Self::read].
	/// Other platform specific time-outs may trigger before this timeout does.
	/// Additionally, some functions (like [`Self::read_exact`]) perform multiple calls to `read()`.
	pub fn set_read_timeout(&mut self, timeout: Duration) -> std::io::Result<()> {
		self.inner.set_read_timeout(timeout)
	}

	/// Get the read timeout of the serial port.
	///
	/// The timeout set by this function is an upper bound on individual calls to [`read()`][Self::read].
	/// Other platform specific time-outs may trigger before this timeout does.
	/// Additionally, some functions (like [`Self::read_exact`]) perform multiple calls to `read()`.
	pub fn get_read_timeout(&self) -> std::io::Result<Duration> {
		self.inner.get_read_timeout()
	}

	/// Set the write timeout for the serial port.
	///
	/// The timeout set by this function is an upper bound on individual calls to [`write()`][Self::write].
	/// Other platform specific time-outs may trigger before this timeout does.
	/// Additionally, some functions (like [`Self::write_all`]) perform multiple calls to `write()`.
	pub fn set_write_timeout(&mut self, timeout: Duration) -> std::io::Result<()> {
		self.inner.set_write_timeout(timeout)
	}

	/// Get the write timeout of the serial port.
	///
	/// The timeout set by this function is an upper bound on individual calls to [`write()`][Self::write].
	/// Other platform specific time-outs may trigger before this timeout does.
	/// Additionally, some functions (like [`Self::write_all`]) perform multiple calls to `write()`.
	pub fn get_write_timeout(&self) -> std::io::Result<Duration> {
		self.inner.get_write_timeout()
	}

	/// Get the platform specific timeouts of a serial port on Windows.
	///
	/// This allows for full control over the platform specifics timeouts, but it is only available on Windows.
	///
	/// Also note that changing the read timeouts can easily lead to the serial port timing out on every read unless you are very careful.
	/// Please read the whole article about serial port timeouts on MSDN before using this, including all remarks:
	/// [https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-commtimeouts](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-commtimeouts)
	///
	/// You are strongly suggested to use [`Self::get_read_timeout()`] and [`Self::get_write_timeout()`] instead.
	#[cfg(any(feature = "doc", all(feature = "windows", windows)))]
	#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "windows")))]
	pub fn get_windows_timeouts(&self) -> std::io::Result<crate::os::windows::CommTimeouts> {
		#[cfg(windows)] {
			self.inner.get_windows_timeouts()
		}
		#[cfg(not(windows))] {
			unreachable!("this code is only enabled on Windows or during documentation generation")
		}
	}

	/// Set the platform specific timeouts of a serial port on Windows.
	///
	/// This allows for full control over the platform specifics timeouts, but it is only available on Windows.
	///
	/// Also note that changing the read timeouts can easily lead to the serial port timing out on every read unless you are very careful.
	/// Please read the whole article about serial port timeouts on MSDN before using this, including all remarks:
	/// [https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-commtimeouts](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-commtimeouts)
	///
	/// You are strongly suggested to use [`Self::set_read_timeout()`] and [`Self::set_write_timeout()`] instead.
	#[cfg(any(feature = "doc", all(feature = "windows", windows)))]
	#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "windows")))]
	pub fn set_windows_timeouts(&self, timeouts: &crate::os::windows::CommTimeouts) -> std::io::Result<()> {
		#[cfg(windows)] {
			self.inner.set_windows_timeouts(timeouts)
		}
		#[cfg(not(windows))] {
			let _ = timeouts;
			unreachable!("this code is only enabled on Windows or during documentation generation")
		}
	}

	/// Discard the kernel input and output buffers for the serial port.
	///
	/// When you write to a serial port, the data may be put in a buffer by the OS to be transmitted by the actual device later.
	/// Similarly, data received on the device can be put in a buffer by the OS untill you read it.
	/// This function clears both buffers: any untransmitted data and received but unread data is discarded by the OS.
	pub fn discard_buffers(&self) -> std::io::Result<()> {
		self.inner.discard_buffers(true, true)
	}

	/// Discard the kernel input buffers for the serial port.
	///
	/// Data received on the device can be put in a buffer by the OS untill you read it.
	/// This function clears that buffer: received but unread data is discarded by the OS.
	///
	/// This is particularly useful when communicating with a device that only responds to commands that you send to it.
	/// If you discard the input buffer before sending the command, you discard any noise that may have been received after the last command.
	pub fn discard_input_buffer(&self) -> std::io::Result<()> {
		self.inner.discard_buffers(true, false)
	}

	/// Discard the kernel output buffers for the serial port.
	///
	/// When you write to a serial port, the data is generally put in a buffer by the OS to be transmitted by the actual device later.
	/// This function clears that buffer: any untransmitted data is discarded by the OS.
	pub fn discard_output_buffer(&self) -> std::io::Result<()> {
		self.inner.discard_buffers(false, true)
	}

	/// Set the state of the Ready To Send line.
	///
	/// If hardware flow control is enabled on the serial port, it is platform specific what will happen.
	/// The function may fail with an error, or it may silently be ignored.
	/// It may even succeed and interfere with the flow control.
	pub fn set_rts(&self, state: bool) -> std::io::Result<()> {
		self.inner.set_rts(state)
	}

	/// Read the state of the Clear To Send line.
	///
	/// If hardware flow control is enabled on the serial port, it is platform specific what will happen.
	/// The function may fail with an error, it may return a bogus value, or it may return the actual state of the CTS line.
	pub fn read_cts(&self) -> std::io::Result<bool> {
		self.inner.read_cts()
	}

	/// Set the state of the Data Terminal Ready line.
	///
	/// If hardware flow control is enabled on the serial port, it is platform specific what will happen.
	/// The function may fail with an error, or it may silently be ignored.
	pub fn set_dtr(&self, state: bool) -> std::io::Result<()> {
		self.inner.set_dtr(state)
	}

	/// Read the state of the Data Set Ready line.
	///
	/// If hardware flow control is enabled on the serial port, it is platform specific what will happen.
	/// The function may fail with an error, it may return a bogus value, or it may return the actual state of the DSR line.
	pub fn read_dsr(&self) -> std::io::Result<bool> {
		self.inner.read_dsr()
	}

	/// Read the state of the Ring Indicator line.
	///
	/// This line is also sometimes also called the RNG or RING line.
	pub fn read_ri(&self) -> std::io::Result<bool> {
		self.inner.read_ri()
	}

	/// Read the state of the Carrier Detect (CD) line.
	///
	/// This line is also called the Data Carrier Detect (DCD) line
	/// or the Receive Line Signal Detect (RLSD) line.
	pub fn read_cd(&self) -> std::io::Result<bool> {
		self.inner.read_cd()
	}

	/// Set or clear the break state of the serial port.
	///
	/// The serial port will hold the data line in a logical low state while the break state is enabled.
	/// This can be detected as a break condition on the other side of the line.
	pub fn set_break(&self, enable: bool) -> std::io::Result<()> {
		self.inner.set_break(enable)
	}

	/// Get the RS-4xx mode of the serial port transceiver.
	///
	/// This is currently only supported on Linux.
	///
	/// Not all serial ports can be configured in a different mode by software.
	/// Some serial ports are always in RS-485 or RS-422 mode,
	/// and some may have hardware switches or jumpers to configure the transceiver.
	/// In those cases, this function will usually report an error or [`rs4xx::TransceiverMode::Default`],
	/// even though the serial port is configured is RS-485 or RS-422 mode.
	///
	/// Note that driver support for this feature is very limited and sometimes inconsistent.
	/// Please read all the warnings in the [`rs4xx`] module carefully.
	#[cfg(any(feature = "doc", all(feature = "rs4xx", target_os = "linux")))]
	#[cfg_attr(feature = "doc-cfg", doc(cfg(all(feature = "rs4xx", target_os = "linux"))))]
	pub fn get_rs4xx_mode(&self) -> std::io::Result<rs4xx::TransceiverMode> {
		#[cfg(all(feature = "rs4xx", target_os = "linux"))]
		return sys::get_rs4xx_mode(&self.inner);
		#[allow(unreachable_code)] {
			panic!("unsupported platform");
		}
	}

	/// Set the RS-4xx mode of the serial port transceiver.
	///
	/// This is currently only supported on Linux.
	///
	/// Not all serial ports can be configured in a different mode by software.
	/// Some serial ports are always in RS-485 or RS-422 mode,
	/// and some may have hardware switches or jumpers to configure the transceiver.
	/// In that case, this function will usually return an error,
	/// but the port can still be in RS-485 or RS-422 mode.
	///
	/// Note that driver support for this feature is very limited and sometimes inconsistent.
	/// Please read all the warnings in the [`rs4xx`] module carefully.
	#[cfg(any(feature = "doc", all(feature = "rs4xx", target_os = "linux")))]
	#[cfg_attr(feature = "doc-cfg", doc(cfg(all(feature = "rs4xx", target_os = "linux"))))]
	pub fn set_rs4xx_mode(&self, mode: impl Into<rs4xx::TransceiverMode>) -> std::io::Result<()> {
		#[cfg(all(feature = "rs4xx", target_os = "linux"))]
		return sys::set_rs4xx_mode(&self.inner, &mode.into());
		#[allow(unreachable_code)] {
			let  _ = mode;
			panic!("unsupported platform");
		}
	}
}

impl std::fmt::Debug for SerialPort {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Debug::fmt(&self.inner, f)
	}
}

impl std::io::Read for SerialPort {
	fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
		SerialPort::read(self, buf)
	}

	fn read_vectored(&mut self, buf: &mut [IoSliceMut<'_>]) -> std::io::Result<usize> {
		SerialPort::read_vectored(self, buf)
	}
}

impl std::io::Read for &'_ SerialPort {
	fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
		SerialPort::read(self, buf)
	}

	fn read_vectored(&mut self, buf: &mut [IoSliceMut<'_>]) -> std::io::Result<usize> {
		SerialPort::read_vectored(self, buf)
	}
}

impl std::io::Write for SerialPort {
	fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
		SerialPort::write(self, buf)
	}

	fn write_vectored(&mut self, buf: &[IoSlice<'_>]) -> std::io::Result<usize> {
		SerialPort::write_vectored(self, buf)
	}

	fn flush(&mut self) -> std::io::Result<()> {
		SerialPort::flush(self)
	}
}

impl std::io::Write for &'_ SerialPort {
	fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
		SerialPort::write(self, buf)
	}

	fn write_vectored(&mut self, buf: &[IoSlice<'_>]) -> std::io::Result<usize> {
		SerialPort::write_vectored(self, buf)
	}

	fn flush(&mut self) -> std::io::Result<()> {
		SerialPort::flush(self)
	}
}

#[cfg(unix)]
impl From<SerialPort> for std::os::unix::io::OwnedFd {
	fn from(value: SerialPort) -> Self {
		value.inner.file.into()
	}
}

#[cfg(unix)]
impl From<std::os::unix::io::OwnedFd> for SerialPort {
	fn from(value: std::os::unix::io::OwnedFd) -> Self {
		Self {
			inner: sys::SerialPort::from_file(value.into()),
		}
	}
}

#[cfg(unix)]
impl std::os::unix::io::AsFd for SerialPort {
	fn as_fd(&self) -> std::os::unix::io::BorrowedFd<'_> {
		self.inner.file.as_fd()
	}
}

#[cfg(unix)]
impl std::os::unix::io::AsRawFd for SerialPort {
	fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
		self.inner.file.as_raw_fd()
	}
}

#[cfg(unix)]
impl std::os::unix::io::IntoRawFd for SerialPort {
	fn into_raw_fd(self) -> std::os::unix::io::RawFd {
		self.inner.file.into_raw_fd()
	}
}

#[cfg(unix)]
impl std::os::unix::io::FromRawFd for SerialPort {
	unsafe fn from_raw_fd(fd: std::os::unix::io::RawFd) -> Self {
		use std::fs::File;
		Self {
			inner: sys::SerialPort::from_file(File::from_raw_fd(fd)),
		}
	}
}

#[cfg(windows)]
impl From<SerialPort> for std::os::windows::io::OwnedHandle {
	fn from(value: SerialPort) -> Self {
		value.inner.file.into()
	}
}

/// Convert an [`OwnedHandle`][std::os::windows::io::OwnedHandle] into a `SerialPort`.
///
/// The file handle must have been created with the `FILE_FLAG_OVERLAPPED` flag for the serial port to function correctly.
#[cfg(windows)]
impl From<std::os::windows::io::OwnedHandle> for SerialPort {
	fn from(value: std::os::windows::io::OwnedHandle) -> Self {
		Self {
			inner: sys::SerialPort::from_file(value.into()),
		}
	}
}

#[cfg(windows)]
impl std::os::windows::io::AsHandle for SerialPort {
	fn as_handle(&self) -> std::os::windows::io::BorrowedHandle<'_> {
		self.inner.file.as_handle()
	}
}

#[cfg(windows)]
impl std::os::windows::io::AsRawHandle for SerialPort {
	fn as_raw_handle(&self) -> std::os::windows::io::RawHandle {
		self.inner.file.as_raw_handle()
	}
}

#[cfg(windows)]
impl std::os::windows::io::IntoRawHandle for SerialPort {
	fn into_raw_handle(self) -> std::os::windows::io::RawHandle {
		self.inner.file.into_raw_handle()
	}
}

/// Convert an [`RawHandle`][std::os::windows::io::RawHandle] into a `SerialPort`.
///
/// The file handle must have been created with the `FILE_FLAG_OVERLAPPED` flag for the serial port to function correctly.
#[cfg(windows)]
impl std::os::windows::io::FromRawHandle for SerialPort {
	unsafe fn from_raw_handle(handle: std::os::windows::io::RawHandle) -> Self {
		use std::fs::File;
		Self {
			inner: sys::SerialPort::from_file(File::from_raw_handle(handle)),
		}
	}
}
