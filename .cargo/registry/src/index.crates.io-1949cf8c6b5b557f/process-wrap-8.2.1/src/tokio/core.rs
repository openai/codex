use std::{
	future::Future,
	io::Result,
	process::{ExitStatus, Output},
};

use futures::future::try_join3;
#[cfg(unix)]
use nix::{
	sys::signal::{kill, Signal},
	unistd::Pid,
};
use tokio::{
	io::{AsyncRead, AsyncReadExt},
	process::{Child, ChildStderr, ChildStdin, ChildStdout, Command},
};

crate::generic_wrap::Wrap!(
	TokioCommandWrap,
	Command,
	TokioCommandWrapper,
	Child,
	TokioChildWrapper,
	|child| child
);

crate::generic_wrap::MaybeAnyTrait! {
/// Wrapper for `tokio::process::Child`.
///
/// This trait exposes most of the functionality of the underlying [`Child`]. It is implemented for
/// [`Child`] and by wrappers.
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
/// use process_wrap::tokio::*;
/// use tokio::process::Child;
///
/// #[derive(Debug)]
/// pub struct YourChildWrapper(Child);
///
/// impl TokioChildWrapper for YourChildWrapper {
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
pub trait TokioChildWrapper {
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
	/// Tokio's `Child` isn't `Clone`. In those cases, this method should be overridden.
	fn try_clone(&self) -> Option<Box<dyn TokioChildWrapper>> {
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
	///
	/// Returns an `Option` to resemble Tokio's API, but isn't expected to be `None` in practice.
	fn id(&self) -> Option<u32> {
		self.inner().id()
	}

	/// Kill the `Child` and wait for it to exit.
	///
	/// By default this calls `start_kill()` and then `wait()`, which is the same way it is done on
	/// the underlying `Child`, but that way implementing either or both of those methods will use
	/// them when calling `kill()`, instead of requiring a stub implementation.
	fn kill(&mut self) -> Box<dyn Future<Output = Result<()>> + Send + '_> {
		Box::new(async {
			self.start_kill()?;
			Box::into_pin(self.wait()).await?;
			Ok(())
		})
	}

	/// Kill the `Child` without waiting for it to exit.
	///
	/// By default this is a passthrough to the underlying `Child`, which:
	/// - on Unix, sends a `SIGKILL` signal to the process;
	/// - otherwise, passes through to the `kill()` method.
	fn start_kill(&mut self) -> Result<()> {
		self.inner_mut().start_kill()
	}

	/// Check if the `Child` has exited without waiting, and if it has, return its exit status.
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
	fn wait(&mut self) -> Box<dyn Future<Output = Result<ExitStatus>> + Send + '_> {
		Box::new(self.inner_mut().wait())
	}

	/// Wait for the `Child` to exit and return its exit status and outputs.
	///
	/// Note that this method reads the child's stdout and stderr to completion into memory.
	///
	/// By default this is a reimplementation of the Tokio method, so that it can use the wrapper's
	/// `wait()` method instead of the underlying `Child`'s `wait()`.
	fn wait_with_output(mut self: Box<Self>) -> Box<dyn Future<Output = Result<Output>> + Send>
	where
		Self: 'static,
	{
		Box::new(async move {
			async fn read_to_end<A: AsyncRead + Unpin>(io: &mut Option<A>) -> Result<Vec<u8>> {
				let mut vec = Vec::new();
				if let Some(io) = io.as_mut() {
					io.read_to_end(&mut vec).await?;
				}
				Ok(vec)
			}

			let mut stdout_pipe = self.stdout().take();
			let mut stderr_pipe = self.stderr().take();

			let stdout_fut = read_to_end(&mut stdout_pipe);
			let stderr_fut = read_to_end(&mut stderr_pipe);

			let (status, stdout, stderr) =
				try_join3(Box::into_pin(self.wait()), stdout_fut, stderr_fut).await?;

			// Drop happens after `try_join` due to <https://github.com/tokio-rs/tokio/issues/4309>
			drop(stdout_pipe);
			drop(stderr_pipe);

			Ok(Output {
				status,
				stdout,
				stderr,
			})
		})
	}

	/// Send a signal to the `Child`.
	///
	/// This method is only available on Unix. It doesn't exist on Tokio's `Child`, nor on std's. It
	/// was introduced by command-group to abstract over the signal behaviour between process groups
	/// and unwrapped processes.
	#[cfg(unix)]
	fn signal(&self, sig: i32) -> Result<()> {
		if let Some(id) = self.id() {
			kill(
				Pid::from_raw(i32::try_from(id).map_err(std::io::Error::other)?),
				Signal::try_from(sig)?,
			)
			.map_err(std::io::Error::from)
		} else {
			Ok(())
		}
	}
}
}

impl TokioChildWrapper for Child {
	fn inner(&self) -> &Child {
		self
	}
	fn inner_mut(&mut self) -> &mut Child {
		self
	}
	fn into_inner(self: Box<Self>) -> Child {
		*self
	}
}
