use std::str::FromStr;

/// Provide shell with hint on how to complete an argument.
///
/// See [`Arg::value_hint`][crate::Arg::value_hint] to set this on an argument.
///
/// See the `clap_complete` crate for completion script generation.
///
/// Overview of which hints are supported by which shell:
///
/// | Hint                   | zsh | fish[^1] | dynamic |
/// | ---------------------- | --- | ---------|---------|
/// | `AnyPath`              | Yes | Yes      | Yes     |
/// | `FilePath`             | Yes | Yes      | Yes     |
/// | `DirPath`              | Yes | Yes      | Yes     |
/// | `ExecutablePath`       | Yes | Partial  | Yes     |
/// | `CommandName`          | Yes | Yes      | No      |
/// | `CommandString`        | Yes | Partial  | No      |
/// | `CommandWithArguments` | Yes |          | No      |
/// | `Username`             | Yes | Yes      | No      |
/// | `Hostname`             | Yes | Yes      | No      |
/// | `Url`                  | Yes |          | No      |
/// | `EmailAddress`         | Yes |          | No      |
///
/// [^1]: fish completions currently only support named arguments (e.g. -o or --opt), not
///       positional arguments.
#[derive(Debug, Default, PartialEq, Eq, Hash, Copy, Clone)]
#[non_exhaustive]
pub enum ValueHint {
    /// Default value if hint is not specified. Follows shell default behavior, which is usually
    /// auto-completing filenames.
    #[default]
    Unknown,
    /// None of the hints below apply. Disables shell completion for this argument.
    Other,
    /// Any existing path.
    AnyPath,
    /// Path to a file.
    FilePath,
    /// Path to a directory.
    DirPath,
    /// Path to an executable file.
    ExecutablePath,
    /// Name of a command, without arguments. May be relative to PATH, or full path to executable.
    CommandName,
    /// A single string containing a command and its arguments.
    CommandString,
    /// Capture the remaining arguments as a command name and arguments for that command. This is
    /// common when writing shell wrappers that execute anther command, for example `sudo` or `env`.
    ///
    /// This hint is special, the argument must be a positional argument and have
    /// [`.num_args(1..)`] and Command must use [`Command::trailing_var_arg(true)`]. The result is that the
    /// command line `my_app ls -la /` will be parsed as `["ls", "-la", "/"]` and clap won't try to
    /// parse the `-la` argument itself.
    ///
    /// [`Command::trailing_var_arg(true)`]: crate::Command::trailing_var_arg
    /// [`.num_args(1..)`]: crate::Arg::num_args()
    CommandWithArguments,
    /// Name of a local operating system user.
    Username,
    /// Host name of a computer.
    /// Shells usually parse `/etc/hosts` and `.ssh/known_hosts` to complete hostnames.
    Hostname,
    /// Complete web address.
    Url,
    /// Email address.
    EmailAddress,
}

#[cfg(feature = "unstable-ext")]
impl crate::builder::ArgExt for ValueHint {}

impl FromStr for ValueHint {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, <Self as FromStr>::Err> {
        Ok(match &*s.to_ascii_lowercase() {
            "unknown" => ValueHint::Unknown,
            "other" => ValueHint::Other,
            "anypath" => ValueHint::AnyPath,
            "filepath" => ValueHint::FilePath,
            "dirpath" => ValueHint::DirPath,
            "executablepath" => ValueHint::ExecutablePath,
            "commandname" => ValueHint::CommandName,
            "commandstring" => ValueHint::CommandString,
            "commandwitharguments" => ValueHint::CommandWithArguments,
            "username" => ValueHint::Username,
            "hostname" => ValueHint::Hostname,
            "url" => ValueHint::Url,
            "emailaddress" => ValueHint::EmailAddress,
            _ => return Err(format!("unknown ValueHint: `{s}`")),
        })
    }
}
