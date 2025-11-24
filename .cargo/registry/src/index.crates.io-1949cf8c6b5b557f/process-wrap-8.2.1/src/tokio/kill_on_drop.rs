use std::io::Result;

use tokio::process::Command;

use super::{TokioCommandWrap, TokioCommandWrapper};

/// Shim wrapper which sets kill-on-drop on a `Command`.
///
/// This wrapper exists to be able to set the kill-on-drop flag on a `Command` and also store that
/// fact in the wrapper, so that it can be used by other wrappers. Notably this is used by the
/// `JobObject` wrapper.
#[derive(Clone, Copy, Debug)]
pub struct KillOnDrop;

impl TokioCommandWrapper for KillOnDrop {
	fn pre_spawn(&mut self, command: &mut Command, _core: &TokioCommandWrap) -> Result<()> {
		command.kill_on_drop(true);
		Ok(())
	}
}
