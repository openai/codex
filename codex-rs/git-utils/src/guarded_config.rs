use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use crate::FsmonitorOverride;
use crate::git_command::GitCommand;
use crate::git_command::GitRunner;
use crate::git_config_sources::ensure_no_worktree_config_sources;
use crate::safe_git::DISABLED_HOOKS_PATH;

/// Proof that one exact Git config invocation has no worktree-controlled
/// source routes for one runner and canonical repository root.
///
/// The capability deliberately owns the ordered base arguments and cannot be
/// cloned or rebound to another runner, root, or command environment.
pub(crate) struct ValidatedConfigSources<'git> {
    git: &'git GitRunner,
    canonical_root: PathBuf,
    base_config_args: Box<[String]>,
}

impl<'git> ValidatedConfigSources<'git> {
    fn authorize(
        git: &'git GitRunner,
        canonical_root: &Path,
        base_config_args: Vec<String>,
    ) -> io::Result<Self> {
        #[cfg(test)]
        CONFIG_SOURCE_AUTHORIZATION_COUNT.with(|count| count.set(count.get() + 1));

        validate_base_config_args(&base_config_args)?;
        let canonical_root = std::fs::canonicalize(canonical_root)?;
        git.ensure_active_worktree_root(&canonical_root)?;
        ensure_no_worktree_config_sources(git, &canonical_root, &base_config_args)?;
        Ok(Self {
            git,
            canonical_root,
            base_config_args: base_config_args.into_boxed_slice(),
        })
    }
}

fn validate_base_config_args(args: &[String]) -> io::Result<()> {
    let mut pairs = args.chunks_exact(2);
    for pair in &mut pairs {
        let Some((key, _value)) = pair[1].split_once('=') else {
            return Err(invalid_base_config_args());
        };
        if pair[0] != "-c" || key.is_empty() {
            return Err(invalid_base_config_args());
        }
    }
    if !pairs.remainder().is_empty() {
        return Err(invalid_base_config_args());
    }
    Ok(())
}

fn invalid_base_config_args() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "guarded Git base config must contain only ordered -c key=value pairs",
    )
}

/// Operation-owned Git configuration capability.
///
/// All operation children are rooted at the authorized repository, inherit
/// the exact frozen base invocation, and receive fixed library safety scalars.
pub(crate) struct GuardedGitConfig<'git> {
    sources: ValidatedConfigSources<'git>,
}

#[derive(Clone, Copy)]
enum BoundSubcommand {
    AddLiteralPathspecs,
    Apply,
    RevParse,
}

impl BoundSubcommand {
    fn as_str(self) -> &'static str {
        match self {
            Self::AddLiteralPathspecs => "add",
            Self::Apply => "apply",
            Self::RevParse => "rev-parse",
        }
    }

    fn append_to(self, command: &mut GitCommand) {
        if matches!(self, Self::AddLiteralPathspecs) {
            command.arg("--literal-pathspecs");
        }
        command.arg(self.as_str());
    }
}

/// A command whose runner, root, config invocation, and fixed subcommand are
/// inseparably bound to one operation capability.
pub(crate) struct GuardedGitCommand<'operation, 'git> {
    operation: &'operation GuardedGitConfig<'git>,
    inner: GitCommand,
}

impl GuardedGitCommand<'_, '_> {
    pub(crate) fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.inner.arg(arg);
        self
    }

    pub(crate) fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.inner.args(args);
        self
    }

    pub(crate) fn output(self) -> io::Result<std::process::Output> {
        self.operation.sources.git.output(self.inner)
    }
}

impl<'git> GuardedGitConfig<'git> {
    pub(crate) fn authorize(
        git: &'git GitRunner,
        canonical_root: &Path,
        base_config_args: Vec<String>,
    ) -> io::Result<Self> {
        Ok(Self {
            sources: ValidatedConfigSources::authorize(git, canonical_root, base_config_args)?,
        })
    }

    pub(crate) fn canonical_root(&self) -> &Path {
        &self.sources.canonical_root
    }

    pub(crate) fn apply_command(&self) -> io::Result<GuardedGitCommand<'_, 'git>> {
        self.guarded_command(BoundSubcommand::Apply)
    }

    pub(crate) fn literal_add_command(&self) -> io::Result<GuardedGitCommand<'_, 'git>> {
        self.guarded_command(BoundSubcommand::AddLiteralPathspecs)
    }

    pub(crate) fn rev_parse_command(&self) -> io::Result<GuardedGitCommand<'_, 'git>> {
        self.guarded_command(BoundSubcommand::RevParse)
    }

    fn guarded_command(
        &self,
        subcommand: BoundSubcommand,
    ) -> io::Result<GuardedGitCommand<'_, 'git>> {
        let mut command = self
            .sources
            .git
            .command_for_cwd(&self.sources.canonical_root)?;
        command.args(&self.sources.base_config_args);
        append_safe_scalar_overrides(&mut command);
        subcommand.append_to(&mut command);
        Ok(GuardedGitCommand {
            operation: self,
            inner: command,
        })
    }

    pub(crate) fn render_command_for_log(&self, args: &[String]) -> io::Result<String> {
        let mut parts = vec!["git".to_string()];
        parts.extend(self.sources.base_config_args.iter().cloned());
        parts.extend(safe_scalar_override_args());
        parts.extend(args.iter().cloned());
        Ok(format!(
            "(cd {} && {})",
            quote_shell(&self.sources.canonical_root.display().to_string()),
            parts
                .into_iter()
                .map(|part| quote_shell(&part))
                .collect::<Vec<_>>()
                .join(" ")
        ))
    }
}

fn append_safe_scalar_overrides(command: &mut GitCommand) {
    command.args(safe_scalar_override_args());
}

fn safe_scalar_override_args() -> [String; 4] {
    [
        "-c".to_string(),
        format!("core.hooksPath={DISABLED_HOOKS_PATH}"),
        "-c".to_string(),
        FsmonitorOverride::Disabled.git_config_arg().to_string(),
    ]
}

fn quote_shell(value: &str) -> String {
    let simple = value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "-_.:/@%+".contains(character));
    if simple {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
thread_local! {
    static CONFIG_SOURCE_AUTHORIZATION_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_config_source_authorization_count() {
    CONFIG_SOURCE_AUTHORIZATION_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn config_source_authorization_count() -> usize {
    CONFIG_SOURCE_AUTHORIZATION_COUNT.with(std::cell::Cell::get)
}

#[cfg(test)]
#[path = "guarded_config_tests.rs"]
mod tests;
