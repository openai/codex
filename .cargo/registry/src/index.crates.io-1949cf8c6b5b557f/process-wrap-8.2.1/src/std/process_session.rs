use std::{
	io::{Error, Result},
	os::unix::process::CommandExt,
	process::Command,
};

use nix::unistd::{setsid, Pid};
#[cfg(feature = "tracing")]
use tracing::instrument;

use super::{StdCommandWrap, StdCommandWrapper};

/// Wrapper which creates a new session and group for the `Command`.
///
/// This wrapper is only available on Unix.
///
/// It creates a new session and new process group and sets the [`Command`] as its leader.
/// See [setsid(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/setsid.html).
///
/// You may find that some programs behave differently or better when running in a session rather
/// than a process group, or vice versa.
///
/// This wrapper uses [the same child wrapper as `ProcessGroup`](super::ProcessGroupChild) and does
/// the same setup (plus the session setup); using both together is unnecessary and may misbehave.
#[derive(Clone, Copy, Debug)]
pub struct ProcessSession;

impl StdCommandWrapper for ProcessSession {
	#[cfg_attr(feature = "tracing", instrument(level = "debug", skip(self)))]
	fn pre_spawn(&mut self, command: &mut Command, _core: &StdCommandWrap) -> Result<()> {
		unsafe {
			command.pre_exec(move || setsid().map_err(Error::from).map(|_| ()));
		}

		Ok(())
	}

	#[cfg_attr(feature = "tracing", instrument(level = "debug", skip(self)))]
	fn wrap_child(
		&mut self,
		inner: Box<dyn super::core::StdChildWrapper>,
		_core: &StdCommandWrap,
	) -> Result<Box<dyn super::core::StdChildWrapper>> {
		let pgid = Pid::from_raw(i32::try_from(inner.id()).expect("Command PID > i32::MAX"));

		Ok(Box::new(super::ProcessGroupChild::new(inner, pgid)))
	}
}
