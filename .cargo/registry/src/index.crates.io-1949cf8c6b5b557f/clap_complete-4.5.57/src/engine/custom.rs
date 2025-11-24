use std::any::type_name;
use std::ffi::OsStr;
use std::sync::Arc;

use clap::builder::ArgExt;
use clap::builder::CommandExt;
use clap_lex::OsStrExt as _;

use super::CompletionCandidate;

/// Extend [`Arg`][clap::Arg] with a completer
///
/// # Example
///
/// ```rust
/// use clap::Parser;
/// use clap_complete::engine::{ArgValueCompleter, CompletionCandidate};
///
/// fn custom_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
///     let mut completions = vec![];
///     let Some(current) = current.to_str() else {
///         return completions;
///     };
///
///     if "foo".starts_with(current) {
///         completions.push(CompletionCandidate::new("foo"));
///     }
///     if "bar".starts_with(current) {
///         completions.push(CompletionCandidate::new("bar"));
///     }
///     if "baz".starts_with(current) {
///         completions.push(CompletionCandidate::new("baz"));
///     }
///     completions
/// }
///
/// #[derive(Debug, Parser)]
/// struct Cli {
///     #[arg(long, add = ArgValueCompleter::new(custom_completer))]
///     custom: Option<String>,
/// }
/// ```
#[derive(Clone)]
pub struct ArgValueCompleter(Arc<dyn ValueCompleter>);

impl ArgValueCompleter {
    /// Create a new `ArgValueCompleter` with a custom completer
    pub fn new<C>(completer: C) -> Self
    where
        C: ValueCompleter + 'static,
    {
        Self(Arc::new(completer))
    }

    /// Candidates that match `current`
    ///
    /// See [`CompletionCandidate`] for more information.
    pub fn complete(&self, current: &OsStr) -> Vec<CompletionCandidate> {
        self.0.complete(current)
    }
}

impl std::fmt::Debug for ArgValueCompleter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(type_name::<Self>())
    }
}

impl ArgExt for ArgValueCompleter {}

/// User-provided completion candidates for an [`Arg`][clap::Arg], see [`ArgValueCompleter`]
///
/// This is useful when predefined value hints are not enough.
pub trait ValueCompleter: Send + Sync {
    /// All potential candidates for an argument.
    ///
    /// See [`CompletionCandidate`] for more information.
    fn complete(&self, current: &OsStr) -> Vec<CompletionCandidate>;
}

impl<F> ValueCompleter for F
where
    F: Fn(&OsStr) -> Vec<CompletionCandidate> + Send + Sync,
{
    fn complete(&self, current: &OsStr) -> Vec<CompletionCandidate> {
        self(current)
    }
}

/// Extend [`Arg`][clap::Arg] with a [`ValueCandidates`]
///
/// # Example
///
/// ```rust
/// use clap::Parser;
/// use clap_complete::engine::{ArgValueCandidates, CompletionCandidate};
///
/// #[derive(Debug, Parser)]
/// struct Cli {
///     #[arg(long, add = ArgValueCandidates::new(|| { vec![
///         CompletionCandidate::new("foo"),
///         CompletionCandidate::new("bar"),
///         CompletionCandidate::new("baz")] }))]
///     custom: Option<String>,
/// }
/// ```
#[derive(Clone)]
pub struct ArgValueCandidates(Arc<dyn ValueCandidates>);

impl ArgValueCandidates {
    /// Create a new `ArgValueCandidates` with a custom completer
    pub fn new<C>(completer: C) -> Self
    where
        C: ValueCandidates + 'static,
    {
        Self(Arc::new(completer))
    }

    /// All potential candidates for an argument.
    ///
    /// See [`CompletionCandidate`] for more information.
    pub fn candidates(&self) -> Vec<CompletionCandidate> {
        self.0.candidates()
    }
}

impl std::fmt::Debug for ArgValueCandidates {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(type_name::<Self>())
    }
}

impl ArgExt for ArgValueCandidates {}

/// Extend [`Command`][clap::Command] with a [`ValueCandidates`]
///
/// # Example
/// ```rust
/// use clap::Parser;
/// use clap_complete::engine::{SubcommandCandidates, CompletionCandidate};
/// #[derive(Debug, Parser)]
/// #[clap(name = "cli", add = SubcommandCandidates::new(|| { vec![
///     CompletionCandidate::new("foo"),
///     CompletionCandidate::new("bar"),
///     CompletionCandidate::new("baz")] }))]
/// struct Cli {
///     #[arg(long)]
///     input: Option<String>,
/// }
/// ```
#[derive(Clone)]
pub struct SubcommandCandidates(Arc<dyn ValueCandidates>);

impl SubcommandCandidates {
    /// Create a new `SubcommandCandidates` with a custom completer
    pub fn new<C>(completer: C) -> Self
    where
        C: ValueCandidates + 'static,
    {
        Self(Arc::new(completer))
    }

    /// All potential candidates for an external subcommand.
    ///
    /// See [`CompletionCandidate`] for more information.
    pub fn candidates(&self) -> Vec<CompletionCandidate> {
        self.0.candidates()
    }
}

impl std::fmt::Debug for SubcommandCandidates {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(type_name::<Self>())
    }
}

impl CommandExt for SubcommandCandidates {}

/// User-provided completion candidates for an [`Arg`][clap::Arg], see [`ArgValueCandidates`]
///
/// User-provided completion candidates for an [`Subcommand`][clap::Subcommand], see [`SubcommandCandidates`]
///
/// This is useful when predefined value hints are not enough.
pub trait ValueCandidates: Send + Sync {
    /// All potential candidates for an argument.
    ///
    /// See [`CompletionCandidate`] for more information.
    fn candidates(&self) -> Vec<CompletionCandidate>;
}

impl<F> ValueCandidates for F
where
    F: Fn() -> Vec<CompletionCandidate> + Send + Sync,
{
    fn candidates(&self) -> Vec<CompletionCandidate> {
        self()
    }
}

/// Complete a value as a [`std::path::Path`]
///
/// # Example
///
/// ```rust
/// use clap::Parser;
/// use clap_complete::engine::{ArgValueCompleter, PathCompleter};
///
/// #[derive(Debug, Parser)]
/// struct Cli {
///     #[arg(long, add = ArgValueCompleter::new(PathCompleter::file()))]
///     custom: Option<String>,
/// }
/// ```
pub struct PathCompleter {
    current_dir: Option<std::path::PathBuf>,
    #[allow(clippy::type_complexity)]
    filter: Option<Box<dyn Fn(&std::path::Path) -> bool + Send + Sync>>,
    stdio: bool,
}

impl PathCompleter {
    /// Any path is allowed
    pub fn any() -> Self {
        Self {
            filter: None,
            current_dir: None,
            stdio: false,
        }
    }

    /// Complete only files
    pub fn file() -> Self {
        Self::any().filter(|p| p.is_file())
    }

    /// Complete only directories
    pub fn dir() -> Self {
        Self::any().filter(|p| p.is_dir())
    }

    /// Include stdio (`-`)
    pub fn stdio(mut self) -> Self {
        self.stdio = true;
        self
    }

    /// Select which paths should be completed
    pub fn filter(
        mut self,
        filter: impl Fn(&std::path::Path) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.filter = Some(Box::new(filter));
        self
    }

    /// Override [`std::env::current_dir`]
    pub fn current_dir(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.current_dir = Some(path.into());
        self
    }
}

impl Default for PathCompleter {
    fn default() -> Self {
        Self::any()
    }
}

impl ValueCompleter for PathCompleter {
    fn complete(&self, current: &OsStr) -> Vec<CompletionCandidate> {
        let filter = self.filter.as_deref().unwrap_or(&|_| true);
        let mut current_dir_actual = None;
        let current_dir = self.current_dir.as_deref().or_else(|| {
            current_dir_actual = std::env::current_dir().ok();
            current_dir_actual.as_deref()
        });
        let mut candidates = complete_path(current, current_dir, filter);
        if self.stdio && current.is_empty() {
            candidates.push(CompletionCandidate::new("-").help(Some("stdio".into())));
        }
        candidates
    }
}

pub(crate) fn complete_path(
    value_os: &OsStr,
    current_dir: Option<&std::path::Path>,
    is_wanted: &dyn Fn(&std::path::Path) -> bool,
) -> Vec<CompletionCandidate> {
    let mut completions = Vec::new();
    let mut potential = Vec::new();

    let value_path = std::path::Path::new(value_os);
    let (prefix, current) = split_file_name(value_path);
    let current = current.to_string_lossy();
    let search_root = if prefix.is_absolute() {
        prefix.to_owned()
    } else if prefix.iter().next() == Some(OsStr::new("~")) {
        let prefix = prefix.strip_prefix("~").unwrap_or(prefix);
        let home_dir = match std::env::home_dir() {
            Some(home_dir) => home_dir,
            None => {
                // Can't complete without a `home_dir`
                return completions;
            }
        };
        home_dir.join(prefix)
    } else {
        let current_dir = match current_dir {
            Some(current_dir) => current_dir,
            None => {
                // Can't complete without a `current_dir`
                return completions;
            }
        };
        current_dir.join(prefix)
    };
    debug!("complete_path: search_root={search_root:?}, prefix={prefix:?}");

    if value_os.is_empty() && is_wanted(&search_root) {
        completions.push(".".into());
    }

    for entry in std::fs::read_dir(&search_root)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
    {
        let raw_file_name = entry.file_name();
        if !raw_file_name.starts_with(&current) {
            continue;
        }

        if entry.metadata().map(|m| m.is_dir()).unwrap_or(false) {
            let mut suggestion = prefix.join(&raw_file_name);
            suggestion.push(""); // Ensure trailing `/`
            let candidate = CompletionCandidate::new(suggestion.as_os_str().to_owned())
                .hide(is_hidden(&raw_file_name));

            if is_wanted(&entry.path()) {
                completions.push(candidate);
            } else {
                potential.push(candidate);
            }
        } else {
            if is_wanted(&entry.path()) {
                let suggestion = prefix.join(&raw_file_name);
                let candidate = CompletionCandidate::new(suggestion.as_os_str().to_owned())
                    .hide(is_hidden(&raw_file_name));
                completions.push(candidate);
            }
        }
    }
    completions.sort();
    potential.sort();
    completions.extend(potential);

    completions
}

fn is_hidden(file_name: &OsStr) -> bool {
    file_name.starts_with(".")
}

fn split_file_name(path: &std::path::Path) -> (&std::path::Path, &OsStr) {
    // Workaround that `Path::new("name/").file_name()` reports `"name"`
    if path_has_name(path) {
        (
            path.parent().unwrap_or_else(|| std::path::Path::new("")),
            path.file_name().expect("not called with `..`"),
        )
    } else {
        (path, Default::default())
    }
}

fn path_has_name(path: &std::path::Path) -> bool {
    let path_bytes = path.as_os_str().as_encoded_bytes();
    let Some(trailing) = path_bytes.last() else {
        return false;
    };
    let trailing = *trailing as char;
    !std::path::is_separator(trailing) && path.file_name().is_some()
}
