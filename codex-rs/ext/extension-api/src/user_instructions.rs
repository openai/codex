use std::future::Future;
use std::pin::Pin;

use codex_utils_absolute_path::AbsolutePathBuf;

/// Instructions supplied by the host.
///
/// Filesystem-backed instructions retain their absolute source path for the
/// app-server `instructionSources` API. Other host-provided instructions do
/// not report a filesystem source.
// TODO(anp): Replace the absolute path with a more general instruction-source
// abstraction when non-filesystem providers need first-class attribution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Instructions {
    /// Model-visible instruction text.
    pub text: String,
    /// Absolute filesystem path reported through `instructionSources`, if any.
    pub source: Option<AbsolutePathBuf>,
}

/// Result of loading host-provided user instructions.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LoadedUserInstructions {
    /// Loaded instructions, or `None` when the provider has no applicable text.
    pub instructions: Option<Instructions>,
    /// Recoverable loading problems that should be surfaced to the host.
    /// Providers own suppression of recurring warnings; Core forwards each returned warning.
    pub warnings: Vec<String>,
}

/// Future returned by an instruction provider.
pub type LoadInstructionsFuture<'a> =
    Pin<Box<dyn Future<Output = LoadedUserInstructions> + Send + 'a>>;

/// Loads host-provided instructions that apply to one root thread.
///
/// These instructions follow the global [`UserInstructionsProvider`] snapshot
/// and precede repository instructions. A result with no instructions or
/// blank instructions clears only the thread-scoped contribution. Core retains
/// the provider and reads it at startup and when capturing model-request context.
/// Implementations own fetching and caching; repeated reads should be cheap and
/// return a coherent snapshot. On a recoverable fetch failure, return the last
/// usable snapshot with warnings rather than an empty result that clears it.
pub trait ThreadInstructionsProvider: Send + Sync {
    /// Loads the current snapshot for the provider's root thread.
    fn load_thread_instructions(&self) -> LoadInstructionsFuture<'_>;
}

/// Loads global user instructions shared across root threads.
///
/// Core reads this provider at startup and when capturing model-request context.
/// Implementations own fetching and caching, so repeated reads should be cheap.
/// Implementations should return any recoverable loading problems as warnings
/// while still returning usable fallback instructions when available.
pub trait UserInstructionsProvider: Send + Sync {
    /// Loads the current global snapshot for a root runtime.
    fn load_user_instructions(&self) -> LoadInstructionsFuture<'_>;
}
