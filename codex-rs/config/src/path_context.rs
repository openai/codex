//! Path facts for filesystem denial parsing. Native callers and callers resolving
//! another platform use the same resolver after fact capture.

use codex_utils_absolute_path::AbsolutePathBufGuard;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use std::cell::RefCell;

/// The owning environment's path convention, base directory, and home for denials.
#[derive(Clone, Debug)]
pub struct ConfigPathContext {
    convention: PathConvention,
    base_dir: Option<PathUri>,
    user_home_dir: Option<PathUri>,
}

impl ConfigPathContext {
    /// Supplies the grammar and directories of the environment owning the layer.
    /// Relative paths require a base; home-relative paths require a home directory.
    pub fn new(
        convention: PathConvention,
        base_dir: Option<PathUri>,
        user_home_dir: Option<PathUri>,
    ) -> Self {
        Self {
            convention,
            base_dir,
            user_home_dir,
        }
    }

    pub(crate) fn enter(&self) -> PathContextGuard {
        PathContextGuard(PATH_CONTEXT.with(|current| current.replace(Some(self.clone()))))
    }
}

thread_local! {
    static PATH_CONTEXT: RefCell<Option<ConfigPathContext>> = const { RefCell::new(None) };
}

pub(crate) struct PathContextGuard(Option<ConfigPathContext>);

impl Drop for PathContextGuard {
    fn drop(&mut self) {
        PATH_CONTEXT.with(|current| *current.borrow_mut() = self.0.take());
    }
}

pub(crate) fn convention() -> PathConvention {
    PATH_CONTEXT.with(|current| {
        current
            .borrow()
            .as_ref()
            .map_or_else(PathConvention::native, |context| context.convention)
    })
}

pub(crate) fn resolve(input: &str) -> Result<String, String> {
    let context = match PATH_CONTEXT.with(|current| current.borrow().clone()) {
        Some(context) => context,
        None => {
            let convention = PathConvention::native();
            let base = AbsolutePathBufGuard::deserialization_base(std::path::Path::new(input))
                .map_err(str::to_owned)?
                .map(PathUri::from_host_native_path)
                .transpose()
                .map_err(|error| error.to_string())?;
            let home = convention
                .home_relative_suffix(input)
                .and_then(|_| AbsolutePathBufGuard::home_directory())
                .map(PathUri::from_host_native_path)
                .transpose()
                .map_err(|error| error.to_string())?;
            ConfigPathContext::new(convention, base, home)
        }
    };
    PathUri::resolve_config_path(
        input,
        context.convention,
        context.base_dir.as_ref(),
        context.user_home_dir.as_ref(),
    )
    .map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "path_context_tests.rs"]
mod tests;
