//! Policy for preserving Git's built-in filesystem monitor.
//!
//! Codex overrides `core.fsmonitor` so repository configuration cannot select
//! an executable helper. Preserve the built-in daemon only when the effective
//! value is boolean true and Git advertises daemon support.
//!
//! The daemon avoids scanning every tracked file and untracked directory:
//! https://github.com/git/git/blob/94f057755b7941b321fd11fec1b2e3ca5313a4e0/Documentation/git-fsmonitor--daemon.adoc#L49-L57
//! https://github.com/git/git/blob/94f057755b7941b321fd11fec1b2e3ca5313a4e0/Documentation/git-update-index.adoc#L545-L550

use std::future::Future;

/// The safe `core.fsmonitor` override for an internal Git command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FsmonitorOverride {
    /// Disable repository-selected filesystem monitor helpers.
    Disabled,
    /// Preserve Git's built-in filesystem monitor daemon.
    BuiltIn,
}

impl FsmonitorOverride {
    /// Returns the complete Git configuration override.
    pub const fn git_config_arg(self) -> &'static str {
        match self {
            Self::Disabled => "core.fsmonitor=false",
            Self::BuiltIn => "core.fsmonitor=true",
        }
    }
}

/// Executes the Git commands required by [`detect_fsmonitor_override`].
///
/// Implementations must return stdout only when Git exits successfully.
/// Timeouts, spawn or transport failures, signal termination, and nonzero exit
/// statuses must return `None`.
pub trait FsmonitorProbeRunner: Send {
    /// Runs one bounded probe in the target repository.
    fn run_probe(&mut self, args: &[&str]) -> impl Future<Output = Option<Vec<u8>>> + Send;
}

/// Returns the safe filesystem monitor override for the target repository.
///
/// This intentionally probes every time. Effective Git configuration is
/// layered, may use conditional includes, and can change while Codex is
/// running:
/// https://git-scm.com/docs/git-config#SCOPES
/// https://git-scm.com/docs/git-config#_conditional_includes
pub async fn detect_fsmonitor_override(
    runner: &mut impl FsmonitorProbeRunner,
) -> FsmonitorOverride {
    // A typed query converts every matching value before `--get` selects the
    // effective one. A shadowed helper path can therefore make a repository-
    // local true fail conversion. Query the raw effective value first.
    // https://github.com/git/git/blob/94f057755b7941b321fd11fec1b2e3ca5313a4e0/builtin/config.c#L482-L514
    // https://github.com/git/git/blob/94f057755b7941b321fd11fec1b2e3ca5313a4e0/builtin/config.c#L611-L614
    let Some(config) = runner
        .run_probe(&["config", "--null", "--get", "core.fsmonitor"])
        .await
    else {
        return FsmonitorOverride::Disabled;
    };
    let Some(config) = config.strip_suffix(b"\0") else {
        return FsmonitorOverride::Disabled;
    };
    if config.contains(&0) {
        return FsmonitorOverride::Disabled;
    }
    let Ok(config) = str::from_utf8(config) else {
        return FsmonitorOverride::Disabled;
    };

    // Git accepts these case-insensitive spellings directly, as well as
    // valueless keys and nonzero integers. Ask Git to normalize uncommon
    // spellings, filtering by the raw effective value before conversion so a
    // shadowed helper pathname cannot make the query fail.
    // https://github.com/git/git/blob/94f057755b7941b321fd11fec1b2e3ca5313a4e0/parse.c#L158-L181
    // https://github.com/git/git/blob/94f057755b7941b321fd11fec1b2e3ca5313a4e0/builtin/config.c#L264-L279
    // https://github.com/git/git/blob/94f057755b7941b321fd11fec1b2e3ca5313a4e0/builtin/config.c#L496-L507
    let configured = if ["true", "yes", "on"]
        .iter()
        .any(|value| config.eq_ignore_ascii_case(value))
    {
        true
    } else if ["false", "no", "off"]
        .iter()
        .any(|value| config.eq_ignore_ascii_case(value))
    {
        false
    } else {
        let typed_args = [
            "config",
            "--null",
            "--type=bool",
            "--fixed-value",
            "--get",
            "core.fsmonitor",
            config,
        ];
        matches!(
            runner.run_probe(&typed_args).await.as_deref(),
            Some(b"true\0")
        )
    };
    if !configured {
        return FsmonitorOverride::Disabled;
    }

    // Git 2.35.1 and older interpret "true" as a hook pathname. Before Git
    // 2.26, a successful empty hook response can hide tracked changes. Require
    // the feature line Git added specifically for capability checks.
    // https://github.com/git/git/blob/94f057755b7941b321fd11fec1b2e3ca5313a4e0/Documentation/config/core.adoc#L90-L99
    // https://github.com/git/git/commit/dd77cf61a1a2fbf52c94d0cd986d555ad2ba8a4b
    let Some(build_options) = runner.run_probe(&["version", "--build-options"]).await else {
        return FsmonitorOverride::Disabled;
    };
    if build_options
        .split(|byte| *byte == b'\n')
        .any(|line| line.trim_ascii() == b"feature: fsmonitor--daemon")
    {
        FsmonitorOverride::BuiltIn
    } else {
        FsmonitorOverride::Disabled
    }
}

#[cfg(test)]
#[path = "fsmonitor_tests.rs"]
mod tests;
