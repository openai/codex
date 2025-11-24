use std::{
	io::{Read, Result},
	process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Output},
};

#[cfg(unix)]
use nix::{
	sys::signal::{kill, Signal},
	unistd::Pid,
};

crate::generic_wrap::Wrap!(
	StdCommandWrap,
	Command,
	StdCommandWrapper,
	Child,
	StdChildWrapper,
	StdChild // |child| StdChild(child)
);

crate::generic_wrap::MaybeAnyTrait! {
/// Wrapper for `std::process::Child`.
///
/// This trait exposes most of the functionality of the underlying [`Child`]. It is implemented for
/// [`StdChild`] (a thin wrapper around [`Child`]) (because implementing directly on [`Child`] would
/// loop) and by wrappers.
///
/// The required methods are `inner`, `inner_mut`, and `into_inner`. That provides access to the
/// underlying `Child` and allows the wrapper to be dropped and the `Child` to be used directly if
/// necessary.
///
/// It also makes it possible for all the other methods to have default implementations. Some are
/// direct passthroughs to the underlying `Child`, while others are more complex.
///
/// Here's a simple example of a wrapper:
///
/// ```rust
/// use process_wrap::std::*;
/// use std::process::Child;
///
/// #[derive(Debug)]
/// pub struct YourChildWrapper(Child);
///
/// impl StdChildWrapper for YourChildWrapper {
///     fn inner(&self) -> &Child {
///         &self.0
///     }
///
///     fn inner_mut(&mut self) -> &mut Child {
///         &mut self.0
///     }
///
///     fn into_inner(self: Box<Self>) -> Child {
///         (*self).0
///     }
/// }
/// ```
pub trait StdChildWrapper {
	/// Obtain a reference to the underlying `Child`.
	fn inner(&self) -> &Child;

	/// Obtain a mutable reference to the underlying `Child`.
	fn inner_mut(&mut self) -> &mut Child;

	/// Consume the wrapper and return the underlying `Child`.
	///
	/// Note that this may disrupt whatever the wrappers were doing. However, wrappers must ensure
	/// that the `Child` is in a consistent state when this is called or they are dropped, so that
	/// this is always safe.
	fn into_inner(self: Box<Self>) -> Child;

	/// Obtain a clone if possible.
	///
	/// Some implementations may make it possible to clone the implementing structure, even though
	/// std's `Child` isn't `Clone`. In those cases, this method should be overridden.
	fn try_clone(&self) -> Option<Box<dyn StdChildWrapper>> {
		None
	}

	/// Obtain the `Child`'s stdin.
	///
	/// By default this is a passthrough to the underlying `Child`.
	fn stdin(&mut self) -> &mut Option<ChildStdin> {
		&mut self.inner_mut().stdin
	}

	/// Obtain the `Child`'s stdout.
	///
	/// By default this is a passthrough to the underlying `Child`.
	fn stdout(&mut self) -> &mut Option<ChildStdout> {
		&mut self.inner_mut().stdout
	}

	/// Obtain the `Child`'s stderr.
	///
	/// By default this is a passthrough to the underlying `Child`.
	fn stderr(&mut self) -> &mut Option<ChildStderr> {
		&mut self.inner_mut().stderr
	}

	/// Obtain the `Child`'s process ID.
	///
	/// In general this should be the PID of the top-level spawned process that was spawned
	/// However, that may vary depending on what a wrapper does.
	fn id(&self) -> u32 {
		self.inner().id()
	}

	/// Kill the `Child` and wait for it to exit.
	///
	/// By default this calls `start_kill()` and then `wait()`, which is the same way it is done on
	/// the underlying `Child`, but that way implementing either or both of those methods will use
	/// them when calling `kill()`, instead of requiring a stub implementation.
	fn kill(&mut self) -> Result<()> {
		self.start_kill()?;
		self.wait()?;
		Ok(())
	}

	/// Kill the `Child` without waiting for it to exit.
	///
	/// By default this is:
	/// - on Unix, sending a `SIGKILL` signal to the process;
	/// - otherwise, a passthrough to the underlying `kill()` method.
	///
	/// The `start_kill()` method doesn't exist on std's `Child`, and was introduced by Tokio. This
	/// library uses it to provide a consistent API across both std and Tokio (and because it's a
	/// generally useful API).
	fn start_kill(&mut self) -> Result<()> {
		#[cfg(unix)]
		{
			self.signal(Signal::SIGKILL as _)
		}

		#[cfg(not(unix))]
		{
			self.inner_mut().kill()
		}
	}

	/// Check if the `Child` has exited without blocking, and if so, return its exit status.
	///
	/// Wrappers must ensure that repeatedly calling this (or other wait methods) after the child
	/// has exited will always return the same result.
	///
	/// By default this is a passthrough to the underlying `Child`.
	fn try_wait(&mut self) -> Result<Option<ExitStatus>> {
		self.inner_mut().try_wait()
	}

	/// Wait for the `Child` to exit and return its exit status.
	///
	/// Wrappers must ensure that repeatedly calling this (or other wait methods) after the child
	/// has exited will always return the same result.
	///
	/// By default this is a passthrough to the underlying `Child`.
	fn wait(&mut self) -> Result<ExitStatus> {
		self.inner_mut().wait()
	}

	/// Wait for the `Child` to exit and return its exit status and outputs.
	///
	/// Note that this method reads the child's stdout and stderr to completion into memory.
	///
	/// On Unix, this reads from stdout and stderr simultaneously. On other platforms, it reads from
	/// stdout first, then stderr (pull requests welcome to improve this).
	///
	/// By default this is a reimplementation of the std method, so that it can use the wrapper's
	/// `wait()` method instead of the underlying `Child`'s `wait()`.
	fn wait_with_output(mut self: Box<Self>) -> Result<Output>
	where
		Self: 'static,
	{
		drop(self.stdin().take());

		let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
		match (self.stdout().take(), self.stderr().take()) {
			(None, None) => {}
			(Some(mut out), None) => {
				let res = out.read_to_end(&mut stdout);
				res.unwrap();
			}
			(None, Some(mut err)) => {
				let res = err.read_to_end(&mut stderr);
				res.unwrap();
			}
			(Some(out), Some(err)) => {
				let res = read2(out, &mut stdout, err, &mut stderr);
				res.unwrap();
			}
		}

		let status = self.wait()?;
		Ok(Output {
			status,
			stdout,
			stderr,
		})
	}

	/// Send a signal to the `Child`.
	///
	/// This method is only available on Unix. It doesn't exist on std's `Child`, nor on Tokio's. It
	/// was introduced by command-group to abstract over the signal behaviour between process groups
	/// and unwrapped processes.
	#[cfg(unix)]
	fn signal(&self, sig: i32) -> Result<()> {
		kill(
			Pid::from_raw(i32::try_from(self.id()).map_err(std::io::Error::other)?),
			Signal::try_from(sig)?,
		)
		.map_err(std::io::Error::from)
	}
}
}

/// A thin wrapper around [`Child`].
///
/// This is used only because implementing [`StdChildWrapper`] directly on std's [`Child`] creates
/// loops in the type system. It is not intended to be used directly, but only to be used internally
/// by the library.
#[derive(Debug)]
pub struct StdChild(pub Child);

impl StdChildWrapper for StdChild {
	fn inner(&self) -> &Child {
		&self.0
	}
	fn inner_mut(&mut self) -> &mut Child {
		&mut self.0
	}
	fn into_inner(self: Box<Self>) -> Child {
		(*self).0
	}
}

#[cfg(unix)]
fn read2(
	mut out_r: ChildStdout,
	out_v: &mut Vec<u8>,
	mut err_r: ChildStderr,
	err_v: &mut Vec<u8>,
) -> Result<()> {
	use nix::{
		errno::Errno,
		libc,
		poll::{poll, PollFd, PollFlags, PollTimeout},
	};
	use std::{
		io::Error,
		os::fd::{AsRawFd, BorrowedFd},
	};

	let out_fd = out_r.as_raw_fd();
	let err_fd = err_r.as_raw_fd();
	// SAFETY: these are dropped at the same time as all other FDs here
	let out_bfd = unsafe { BorrowedFd::borrow_raw(out_fd) };
	let err_bfd = unsafe { BorrowedFd::borrow_raw(err_fd) };

	set_nonblocking(out_bfd, true)?;
	set_nonblocking(err_bfd, true)?;

	let mut fds = [
		PollFd::new(out_bfd, PollFlags::POLLIN),
		PollFd::new(err_bfd, PollFlags::POLLIN),
	];

	loop {
		poll(&mut fds, PollTimeout::NONE)?;

		if fds[0].revents().is_some() && read(&mut out_r, out_v)? {
			set_nonblocking(err_bfd, false)?;
			return err_r.read_to_end(err_v).map(drop);
		}
		if fds[1].revents().is_some() && read(&mut err_r, err_v)? {
			set_nonblocking(out_bfd, false)?;
			return out_r.read_to_end(out_v).map(drop);
		}
	}

	fn read(r: &mut impl Read, dst: &mut Vec<u8>) -> Result<bool> {
		match r.read_to_end(dst) {
			Ok(_) => Ok(true),
			Err(e) => {
				if e.raw_os_error() == Some(libc::EWOULDBLOCK)
					|| e.raw_os_error() == Some(libc::EAGAIN)
				{
					Ok(false)
				} else {
					Err(e)
				}
			}
		}
	}

	#[cfg(target_os = "linux")]
	fn set_nonblocking(fd: BorrowedFd, nonblocking: bool) -> Result<()> {
		let v = nonblocking as libc::c_int;
		let res = unsafe { libc::ioctl(fd.as_raw_fd(), libc::FIONBIO, &v) };

		Errno::result(res).map_err(Error::from).map(drop)
	}

	#[cfg(not(target_os = "linux"))]
	fn set_nonblocking(fd: BorrowedFd, nonblocking: bool) -> Result<()> {
		use nix::fcntl::{fcntl, FcntlArg, OFlag};

		let mut flags = OFlag::from_bits_truncate(fcntl(fd, FcntlArg::F_GETFL)?);
		flags.set(OFlag::O_NONBLOCK, nonblocking);

		fcntl(fd, FcntlArg::F_SETFL(flags))
			.map_err(Error::from)
			.map(drop)
	}
}

// if you're reading this code and despairing, we'd love
// your contribution of a proper read2 for your platform!
#[cfg(not(unix))]
fn read2(
	mut out_r: ChildStdout,
	out_v: &mut Vec<u8>,
	mut err_r: ChildStderr,
	err_v: &mut Vec<u8>,
) -> Result<()> {
	out_r.read_to_end(out_v)?;
	err_r.read_to_end(err_v)?;
	Ok(())
}
