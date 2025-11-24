//! [`COMPLETE=$SHELL <bin>`][CompleteEnv] completion integration
//!
//! See [`CompleteEnv`]:
//! ```rust
//! # use clap_complete::CompleteEnv;
//! fn cli() -> clap::Command {
//!     // ...
//! #   clap::Command::new("empty")
//! }
//!
//! fn main() {
//!     CompleteEnv::with_factory(cli)
//!         .complete();
//!
//!     // ... rest of application logic
//! }
//! ```
//!
//! To customize completions, see
//! - [`ValueHint`][crate::ValueHint]
//! - [`ValueEnum`][clap::ValueEnum]
//! - [`ArgValueCandidates`][crate::ArgValueCandidates]
//! - [`ArgValueCompleter`][crate::ArgValueCompleter]
//!
//! To source your completions:
//!
//! <div class="warning">
//!
//! **WARNING:** We recommend re-sourcing your completions on upgrade.
//! These completions work by generating shell code that calls into `your_program` while completing.
//! That interface is unstable and a mismatch between the shell code and `your_program` may result
//! in either invalid completions or no completions being generated.
//! For this reason, we recommend generating the shell code anew on shell startup so that it is
//! "self-correcting" on shell launch, rather than writing the generated completions to a file.
//!
//! </div>
//!
//! Bash
//! ```bash
//! echo "source <(COMPLETE=bash your_program)" >> ~/.bashrc
//! ```
//!
//! Elvish
//! ```elvish
//! echo "eval (E:COMPLETE=elvish your_program | slurp)" >> ~/.elvish/rc.elv
//! ```
//!
//! Fish
//! ```fish
//! echo "source (COMPLETE=fish your_program | psub)" >> ~/.config/fish/config.fish
//! ```
//!
//! Powershell
//! ```powershell
//! $env:COMPLETE = "powershell"
//! echo "your_program | Out-String | Invoke-Expression" >> $PROFILE
//! Remove-Item Env:\COMPLETE
//! ```
//!
//! Zsh
//! ```zsh
//! echo "source <(COMPLETE=zsh your_program)" >> ~/.zshrc
//! ```
//!
//! To disable completions, you can set `COMPLETE=` or `COMPLETE=0`

mod shells;

use std::ffi::OsString;
use std::io::Write as _;

pub use shells::*;

/// Environment-activated completions for your CLI
///
/// Benefits over a CLI completion argument or subcommand
/// - Performance: we don't need to generate [`clap::Command`] twice or parse arguments
/// - Flexibility: there is no concern over it interfering with other CLI logic
///
/// **Warning:** `stdout` should not be written to before [`CompleteEnv::complete`] has had a
/// chance to run.
///
/// # Examples
///
/// ```rust
/// # use clap_complete::CompleteEnv;
/// fn cli() -> clap::Command {
///     // ...
/// #   clap::Command::new("empty")
/// }
///
/// fn main() {
///     CompleteEnv::with_factory(cli)
///         .complete()
///
///     // ... rest of application logic
/// }
/// ```
pub struct CompleteEnv<'s, F> {
    factory: F,
    var: &'static str,
    bin: Option<String>,
    completer: Option<String>,
    shells: Shells<'s>,
}

impl<'s, F: Fn() -> clap::Command> CompleteEnv<'s, F> {
    /// Complete a [`clap::Command`]
    ///
    /// # Example
    ///
    /// Builder:
    /// ```rust
    /// # use clap_complete::CompleteEnv;
    /// fn cli() -> clap::Command {
    ///     // ...
    /// #   clap::Command::new("empty")
    /// }
    ///
    /// fn main() {
    ///     CompleteEnv::with_factory(cli)
    ///         .complete()
    ///
    ///     // ... rest of application logic
    /// }
    /// ```
    ///
    /// Derive:
    /// ```
    /// # use clap::Parser;
    /// # use clap_complete::CompleteEnv;
    /// use clap::CommandFactory as _;
    ///
    /// #[derive(Debug, Parser)]
    /// struct Cli {
    ///     custom: Option<String>,
    /// }
    ///
    /// fn main() {
    ///     CompleteEnv::with_factory(|| Cli::command())
    ///         .complete()
    ///
    ///     // ... rest of application logic
    /// }
    /// ```
    pub fn with_factory(factory: F) -> Self {
        Self {
            factory,
            var: "COMPLETE",
            bin: None,
            completer: None,
            shells: Shells::builtins(),
        }
    }

    /// Override the environment variable used for enabling completions
    pub fn var(mut self, var: &'static str) -> Self {
        self.var = var;
        self
    }

    /// Override the name of the binary to complete
    ///
    /// Default: `Command::get_bin_name`
    pub fn bin(mut self, bin: impl Into<String>) -> Self {
        self.bin = Some(bin.into());
        self
    }

    /// Override the binary to call to get completions
    ///
    /// Default: `args_os()[0]`
    pub fn completer(mut self, completer: impl Into<String>) -> Self {
        self.completer = Some(completer.into());
        self
    }

    /// Override the shells supported for completions
    pub fn shells(mut self, shells: Shells<'s>) -> Self {
        self.shells = shells;
        self
    }
}

impl<'s, F: Fn() -> clap::Command> CompleteEnv<'s, F> {
    /// Process the completion request and exit
    ///
    /// **Warning:** `stdout` should not be written to before this has had a
    /// chance to run.
    pub fn complete(self) {
        let args = std::env::args_os();
        let current_dir = std::env::current_dir().ok();
        if self
            .try_complete(args, current_dir.as_deref())
            .unwrap_or_else(|e| e.exit())
        {
            std::process::exit(0)
        }
    }

    /// Process the completion request
    ///
    /// **Warning:** `stdout` should not be written to before or after this has run.
    ///
    /// Returns `true` if a command was completed and `false` if this is a regular run of your
    /// application
    pub fn try_complete(
        self,
        args: impl IntoIterator<Item = impl Into<OsString>>,
        current_dir: Option<&std::path::Path>,
    ) -> clap::error::Result<bool> {
        self.try_complete_(args.into_iter().map(|a| a.into()).collect(), current_dir)
    }

    fn try_complete_(
        self,
        mut args: Vec<OsString>,
        current_dir: Option<&std::path::Path>,
    ) -> clap::error::Result<bool> {
        let Some(name) = std::env::var_os(self.var) else {
            return Ok(false);
        };
        if name.is_empty() || name == "0" {
            return Ok(false);
        }

        // Ensure any child processes called for custom completers don't activate their own
        // completion logic.
        std::env::remove_var(self.var);

        let shell = self.shell(std::path::Path::new(&name))?;

        let mut cmd = (self.factory)();
        cmd.build();

        let completer = args.remove(0);
        let escape_index = args
            .iter()
            .position(|a| *a == "--")
            .map(|i| i + 1)
            .unwrap_or(args.len());
        args.drain(0..escape_index);
        if args.is_empty() {
            let mut buf = Vec::new();
            self.write_registration(&cmd, current_dir, shell, completer, &mut buf)?;
            std::io::stdout().write_all(&buf)?;
        } else {
            let mut buf = Vec::new();
            shell.write_complete(&mut cmd, args, current_dir, &mut buf)?;
            std::io::stdout().write_all(&buf)?;
        }

        Ok(true)
    }

    fn shell(&self, name: &std::path::Path) -> Result<&dyn EnvCompleter, std::io::Error> {
        // Strip off the parent dir in case `$SHELL` was used
        let name = name.file_stem().unwrap_or(name.as_os_str());
        // lossy won't match but this will delegate to unknown
        // error
        let name = name.to_string_lossy();

        let shell = self.shells.completer(&name).ok_or_else(|| {
            let shells =
                self.shells
                    .names()
                    .enumerate()
                    .fold(String::new(), |mut seed, (i, name)| {
                        use std::fmt::Write as _;
                        let prefix = if i == 0 { "" } else { ", " };
                        let _ = write!(&mut seed, "{prefix}`{name}`");
                        seed
                    });
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("unknown shell `{name}`, expected one of {shells}"),
            )
        })?;
        Ok(shell)
    }

    fn write_registration(
        &self,
        cmd: &clap::Command,
        current_dir: Option<&std::path::Path>,
        shell: &dyn EnvCompleter,
        completer: OsString,
        buf: &mut dyn std::io::Write,
    ) -> Result<(), std::io::Error> {
        let name = cmd.get_name();
        let bin = self
            .bin
            .as_deref()
            .or_else(|| cmd.get_bin_name())
            .unwrap_or_else(|| cmd.get_name());
        let completer = if let Some(completer) = self.completer.as_deref() {
            completer.to_owned()
        } else {
            let mut completer = std::path::PathBuf::from(completer);
            if let Some(current_dir) = current_dir {
                if 1 < completer.components().count() {
                    completer = current_dir.join(completer);
                }
            }
            completer.to_string_lossy().into_owned()
        };

        shell.write_registration(self.var, name, bin, &completer, buf)?;

        Ok(())
    }
}

/// Collection of shell-specific completers
pub struct Shells<'s>(pub &'s [&'s dyn EnvCompleter]);

impl<'s> Shells<'s> {
    /// Select all of the built-in shells
    pub const fn builtins() -> Self {
        Self(&[&Bash, &Elvish, &Fish, &Powershell, &Zsh])
    }

    /// Find the specified [`EnvCompleter`]
    pub fn completer(&self, name: &str) -> Option<&dyn EnvCompleter> {
        self.0.iter().copied().find(|c| c.is(name))
    }

    /// Collect all [`EnvCompleter::name`]s
    pub fn names(&self) -> impl Iterator<Item = &'static str> + 's {
        self.0.iter().map(|c| c.name())
    }

    /// Iterate over [`EnvCompleter`]s
    pub fn iter(&self) -> impl Iterator<Item = &dyn EnvCompleter> {
        self.0.iter().copied()
    }
}

/// Shell-integration for completions
///
/// This will generally be called by [`CompleteEnv`].
///
/// This handles adapting between the shell and [`completer`][crate::engine::complete()].
/// A `EnvCompleter` can choose how much of that lives within the registration script or
/// lives in [`EnvCompleter::write_complete`].
pub trait EnvCompleter {
    /// Canonical name for this shell
    ///
    /// **Post-conditions:**
    /// ```rust,ignore
    /// assert!(completer.is(completer.name()));
    /// ```
    fn name(&self) -> &'static str;
    /// Whether the name matches this shell
    ///
    /// This should match [`EnvCompleter::name`] and any alternative names, particularly used by
    /// `$SHELL`.
    fn is(&self, name: &str) -> bool;
    /// Register for completions
    ///
    /// Write the `buf` the logic needed for calling into `<VAR>=<shell> <cmd> --`, passing needed
    /// arguments to [`EnvCompleter::write_complete`] through the environment.
    ///
    /// - `var`: see [`CompleteEnv::var`]
    /// - `name`: an identifier to use in the script
    /// - `bin`: see [`CompleteEnv::bin`]
    /// - `completer`: see [`CompleteEnv::completer`]
    ///
    /// <div class="warning">
    ///
    /// **WARNING:** There are no stability guarantees between the call to
    /// [`EnvCompleter::write_complete`] that this generates and actually calling [`EnvCompleter::write_complete`].
    /// Caching the results of this call may result in invalid or no completions to be generated.
    ///
    /// </div>
    fn write_registration(
        &self,
        var: &str,
        name: &str,
        bin: &str,
        completer: &str,
        buf: &mut dyn std::io::Write,
    ) -> Result<(), std::io::Error>;
    /// Complete the given command
    ///
    /// Adapt information from arguments and [`EnvCompleter::write_registration`]-defined env
    /// variables to what is needed for [`completer`][crate::engine::complete()].
    ///
    /// Write out the [`CompletionCandidate`][crate::engine::CompletionCandidate]s in a way the shell will understand.
    fn write_complete(
        &self,
        cmd: &mut clap::Command,
        args: Vec<OsString>,
        current_dir: Option<&std::path::Path>,
        buf: &mut dyn std::io::Write,
    ) -> Result<(), std::io::Error>;
}
