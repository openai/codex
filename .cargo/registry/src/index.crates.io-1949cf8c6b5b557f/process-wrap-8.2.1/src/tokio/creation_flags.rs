use std::io::Result;

use tokio::process::Command;
use windows::Win32::System::Threading::PROCESS_CREATION_FLAGS;

use super::{TokioCommandWrap, TokioCommandWrapper};

/// Shim wrapper which sets Windows process creation flags.
///
/// This wrapper is only available on Windows.
///
/// It exists to be able to set creation flags on a `Command` and also store them in the wrapper, so
/// that they're no overwritten by other wrappers. Notably this is the only way to use creation
/// flags and the `JobObject` wrapper together.
///
/// When both `CreationFlags` and `JobObject` are used together, `CreationFlags` must come first.
#[derive(Clone, Copy, Debug)]
pub struct CreationFlags(pub PROCESS_CREATION_FLAGS);

impl TokioCommandWrapper for CreationFlags {
	fn pre_spawn(&mut self, command: &mut Command, _core: &TokioCommandWrap) -> Result<()> {
		command.creation_flags((self.0).0);
		Ok(())
	}
}
