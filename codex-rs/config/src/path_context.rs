//! Path facts for configuration parsing. Native callers and callers resolving
//! another platform use the same resolver after fact capture.

use codex_utils_absolute_path::AbsolutePathBufGuard;
use codex_utils_path_uri::LegacyAppPathStringError;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use std::cell::RefCell;

/// The owning environment's path convention, base directory, and home for configuration.
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

    /// Returns the path grammar supplied by the owning environment.
    pub fn convention(&self) -> PathConvention {
        self.convention
    }

    /// Resolves configuration text to a URI using the supplied directory and home facts.
    pub fn resolve_path(&self, input: &str) -> Result<PathUri, LegacyAppPathStringError> {
        PathUri::resolve_config_path_uri(
            input,
            self.convention,
            self.base_dir.as_ref(),
            self.user_home_dir.as_ref(),
        )
    }

    /// Resolves against a supplied base using this context's grammar and home.
    pub fn resolve_against(
        &self,
        input: &str,
        base: &PathUri,
    ) -> Result<PathUri, LegacyAppPathStringError> {
        PathUri::resolve_config_path_uri(
            input,
            self.convention,
            Some(base),
            self.user_home_dir.as_ref(),
        )
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
    context
        .resolve_path(input)
        .and_then(|path| path.to_config_path_string(context.convention()))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "path_context_tests.rs"]
mod tests;
