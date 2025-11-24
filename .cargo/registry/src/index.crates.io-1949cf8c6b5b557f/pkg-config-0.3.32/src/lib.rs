//! A build dependency for Cargo libraries to find system artifacts through the
//! `pkg-config` utility.
//!
//! This library will shell out to `pkg-config` as part of build scripts and
//! probe the system to determine how to link to a specified library. The
//! `Config` structure serves as a method of configuring how `pkg-config` is
//! invoked in a builder style.
//!
//! After running `pkg-config` all appropriate Cargo metadata will be printed on
//! stdout if the search was successful.
//!
//! # Environment variables
//!
//! A number of environment variables are available to globally configure how
//! this crate will invoke `pkg-config`:
//!
//! * `FOO_NO_PKG_CONFIG` - if set, this will disable running `pkg-config` when
//!   probing for the library named `foo`.
//!
//! ### Linking
//!
//! There are also a number of environment variables which can configure how a
//! library is linked to (dynamically vs statically). These variables control
//! whether the `--static` flag is passed. Note that this behavior can be
//! overridden by configuring explicitly on `Config`. The variables are checked
//! in the following order:
//!
//! * `FOO_STATIC` - pass `--static` for the library `foo`
//! * `FOO_DYNAMIC` - do not pass `--static` for the library `foo`
//! * `PKG_CONFIG_ALL_STATIC` - pass `--static` for all libraries
//! * `PKG_CONFIG_ALL_DYNAMIC` - do not pass `--static` for all libraries
//!
//! ### Cross-compilation
//!
//! In cross-compilation context, it is useful to manage separately
//! `PKG_CONFIG_PATH` and a few other variables for the `host` and the `target`
//! platform.
//!
//! The supported variables are: `PKG_CONFIG_PATH`, `PKG_CONFIG_LIBDIR`, and
//! `PKG_CONFIG_SYSROOT_DIR`.
//!
//! Each of these variables can also be supplied with certain prefixes and
//! suffixes, in the following prioritized order:
//!
//! 1. `<var>_<target>` - for example, `PKG_CONFIG_PATH_x86_64-unknown-linux-gnu`
//! 2. `<var>_<target_with_underscores>` - for example,
//!    `PKG_CONFIG_PATH_x86_64_unknown_linux_gnu`
//! 3. `<build-kind>_<var>` - for example, `HOST_PKG_CONFIG_PATH` or
//!    `TARGET_PKG_CONFIG_PATH`
//! 4. `<var>` - a plain `PKG_CONFIG_PATH`
//!
//! This crate will allow `pkg-config` to be used in cross-compilation
//! if `PKG_CONFIG_SYSROOT_DIR` or `PKG_CONFIG` is set. You can set
//! `PKG_CONFIG_ALLOW_CROSS=1` to bypass the compatibility check, but please
//! note that enabling use of `pkg-config` in cross-compilation without
//! appropriate sysroot and search paths set is likely to break builds.
//!
//! # Example
//!
//! Find the system library named `foo`, with minimum version 1.2.3:
//!
//! ```no_run
//! fn main() {
//!     pkg_config::Config::new().atleast_version("1.2.3").probe("foo").unwrap();
//! }
//! ```
//!
//! Find the system library named `foo`, with no version requirement (not
//! recommended):
//!
//! ```no_run
//! fn main() {
//!     pkg_config::probe_library("foo").unwrap();
//! }
//! ```
//!
//! Configure how library `foo` is linked to.
//!
//! ```no_run
//! fn main() {
//!     pkg_config::Config::new().atleast_version("1.2.3").statik(true).probe("foo").unwrap();
//! }
//! ```

#![doc(html_root_url = "https://docs.rs/pkg-config/0.3")]

use std::collections::HashMap;
use std::env;
use std::error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fmt::Display;
use std::io;
use std::ops::{Bound, RangeBounds};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::str;

/// Wrapper struct to polyfill methods introduced in 1.57 (`get_envs`, `get_args` etc).
/// This is needed to reconstruct the pkg-config command for output in a copy-
/// paste friendly format via `Display`.
struct WrappedCommand {
    inner: Command,
    program: OsString,
    env_vars: Vec<(OsString, OsString)>,
    args: Vec<OsString>,
}

#[derive(Clone, Debug)]
pub struct Config {
    statik: Option<bool>,
    min_version: Bound<String>,
    max_version: Bound<String>,
    extra_args: Vec<OsString>,
    cargo_metadata: bool,
    env_metadata: bool,
    print_system_libs: bool,
    print_system_cflags: bool,
}

#[derive(Clone, Debug)]
pub struct Library {
    /// Libraries specified by -l
    pub libs: Vec<String>,
    /// Library search paths specified by -L
    pub link_paths: Vec<PathBuf>,
    /// Library file paths specified without -l
    pub link_files: Vec<PathBuf>,
    /// Darwin frameworks specified by -framework
    pub frameworks: Vec<String>,
    /// Darwin framework search paths specified by -F
    pub framework_paths: Vec<PathBuf>,
    /// C/C++ header include paths specified by -I
    pub include_paths: Vec<PathBuf>,
    /// Linker options specified by -Wl
    pub ld_args: Vec<Vec<String>>,
    /// C/C++ definitions specified by -D
    pub defines: HashMap<String, Option<String>>,
    /// Version specified by .pc file's Version field
    pub version: String,
    /// Ensure that this struct can only be created via its private `[Library::new]` constructor.
    /// Users of this crate can only access the struct via `[Config::probe]`.
    _priv: (),
}

/// Represents all reasons `pkg-config` might not succeed or be run at all.
pub enum Error {
    /// Aborted because of `*_NO_PKG_CONFIG` environment variable.
    ///
    /// Contains the name of the responsible environment variable.
    EnvNoPkgConfig(String),

    /// Detected cross compilation without a custom sysroot.
    ///
    /// Ignore the error with `PKG_CONFIG_ALLOW_CROSS=1`,
    /// which may let `pkg-config` select libraries
    /// for the host's architecture instead of the target's.
    CrossCompilation,

    /// Failed to run `pkg-config`.
    ///
    /// Contains the command and the cause.
    Command { command: String, cause: io::Error },

    /// `pkg-config` did not exit successfully after probing a library.
    ///
    /// Contains the command and output.
    Failure { command: String, output: Output },

    /// `pkg-config` did not exit successfully on the first attempt to probe a library.
    ///
    /// Contains the command and output.
    ProbeFailure {
        name: String,
        command: String,
        output: Output,
    },

    #[doc(hidden)]
    // please don't match on this, we're likely to add more variants over time
    __Nonexhaustive,
}

impl WrappedCommand {
    fn new<S: AsRef<OsStr>>(program: S) -> Self {
        Self {
            inner: Command::new(program.as_ref()),
            program: program.as_ref().to_os_string(),
            env_vars: Vec::new(),
            args: Vec::new(),
        }
    }

    fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S> + Clone,
        S: AsRef<OsStr>,
    {
        self.inner.args(args.clone());
        self.args
            .extend(args.into_iter().map(|arg| arg.as_ref().to_os_string()));

        self
    }

    fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self {
        self.inner.arg(arg.as_ref());
        self.args.push(arg.as_ref().to_os_string());

        self
    }

    fn env<K, V>(&mut self, key: K, value: V) -> &mut Self
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.inner.env(key.as_ref(), value.as_ref());
        self.env_vars
            .push((key.as_ref().to_os_string(), value.as_ref().to_os_string()));

        self
    }

    fn output(&mut self) -> io::Result<Output> {
        self.inner.output()
    }
}

/// Quote an argument that has spaces in it.
/// When our `WrappedCommand` is printed to the terminal, arguments that contain spaces needed to be quoted.
/// Otherwise, we will have output such as:
/// `pkg-config --libs --cflags foo foo < 3.11`
/// which cannot be used in a terminal - it will attempt to read a file named 3.11 and provide it as stdin for pkg-config.
/// Using this function, we instead get the correct output:
/// `pkg-config --libs --cflags foo 'foo < 3.11'`
fn quote_if_needed(arg: String) -> String {
    if arg.contains(' ') {
        format!("'{}'", arg)
    } else {
        arg
    }
}

/// Output a command invocation that can be copy-pasted into the terminal.
/// `Command`'s existing debug implementation is not used for that reason,
/// as it can sometimes lead to output such as:
/// `PKG_CONFIG_ALLOW_SYSTEM_CFLAGS="1" PKG_CONFIG_ALLOW_SYSTEM_LIBS="1" "pkg-config" "--libs" "--cflags" "mylibrary"`
/// Which cannot be copy-pasted into terminals such as nushell, and is a bit noisy.
/// This will look something like:
/// `PKG_CONFIG_ALLOW_SYSTEM_CFLAGS=1 PKG_CONFIG_ALLOW_SYSTEM_LIBS=1 pkg-config --libs --cflags mylibrary`
impl Display for WrappedCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Format all explicitly defined environment variables
        let envs = self
            .env_vars
            .iter()
            .map(|(env, arg)| format!("{}={}", env.to_string_lossy(), arg.to_string_lossy()))
            .collect::<Vec<String>>()
            .join(" ");

        // Format all pkg-config arguments
        let args = self
            .args
            .iter()
            .map(|arg| quote_if_needed(arg.to_string_lossy().to_string()))
            .collect::<Vec<String>>()
            .join(" ");

        write!(f, "{} {} {}", envs, self.program.to_string_lossy(), args)
    }
}

impl error::Error for Error {}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        // Failed `unwrap()` prints Debug representation, but the default debug format lacks helpful instructions for the end users
        <Error as fmt::Display>::fmt(self, f)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match *self {
            Error::EnvNoPkgConfig(ref name) => write!(f, "Aborted because {} is set", name),
            Error::CrossCompilation => f.write_str(
                "pkg-config has not been configured to support cross-compilation.\n\
                \n\
                Install a sysroot for the target platform and configure it via\n\
                PKG_CONFIG_SYSROOT_DIR and PKG_CONFIG_PATH, or install a\n\
                cross-compiling wrapper for pkg-config and set it via\n\
                PKG_CONFIG environment variable.",
            ),
            Error::Command {
                ref command,
                ref cause,
            } => {
                match cause.kind() {
                    io::ErrorKind::NotFound => {
                        let crate_name =
                            std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "sys".to_owned());
                        let instructions = if cfg!(target_os = "macos") {
                            "Try `brew install pkgconf` if you have Homebrew.\n"
                        } else if cfg!(target_os = "ios") {
                            "" // iOS cross-compilation requires a custom setup, no easy fix
                        } else if cfg!(unix) {
                            "Try `apt install pkg-config`, or `yum install pkg-config`, or `brew install pkgconf`\n\
                            or `pkg install pkg-config`, or `apk add pkgconfig` \
                            depending on your distribution.\n"
                        } else {
                            "" // There's no easy fix for Windows users
                        };
                        write!(f, "Could not run `{command}`\n\
                        The pkg-config command could not be found.\n\
                        \n\
                        Most likely, you need to install a pkg-config package for your OS.\n\
                        {instructions}\
                        \n\
                        If you've already installed it, ensure the pkg-config command is one of the\n\
                        directories in the PATH environment variable.\n\
                        \n\
                        If you did not expect this build to link to a pre-installed system library,\n\
                        then check documentation of the {crate_name} crate for an option to\n\
                        build the library from source, or disable features or dependencies\n\
                        that require pkg-config.", command = command, instructions = instructions, crate_name = crate_name)
                    }
                    _ => write!(f, "Failed to run command `{}`, because: {}", command, cause),
                }
            }
            Error::ProbeFailure {
                ref name,
                ref command,
                ref output,
            } => {
                let crate_name =
                    env::var("CARGO_PKG_NAME").unwrap_or(String::from("<NO CRATE NAME>"));

                writeln!(f, "")?;

                // Give a short explanation of what the error is
                writeln!(
                    f,
                    "pkg-config {}",
                    match output.status.code() {
                        Some(code) => format!("exited with status code {}", code),
                        None => "was terminated by signal".to_string(),
                    }
                )?;

                // Give the command run so users can reproduce the error
                writeln!(f, "> {}\n", command)?;

                // Explain how it was caused
                writeln!(
                    f,
                    "The system library `{}` required by crate `{}` was not found.",
                    name, crate_name
                )?;
                writeln!(
                    f,
                    "The file `{}.pc` needs to be installed and the PKG_CONFIG_PATH environment variable must contain its parent directory.",
                    name
                )?;

                // There will be no status code if terminated by signal
                if let Some(_code) = output.status.code() {
                    // Nix uses a wrapper script for pkg-config that sets the custom
                    // environment variable PKG_CONFIG_PATH_FOR_TARGET
                    let search_locations = ["PKG_CONFIG_PATH_FOR_TARGET", "PKG_CONFIG_PATH"];

                    // Find a search path to use
                    let mut search_data = None;
                    for location in search_locations.iter() {
                        if let Ok(search_path) = env::var(location) {
                            search_data = Some((location, search_path));
                            break;
                        }
                    }

                    // Guess the most reasonable course of action
                    let hint = if let Some((search_location, search_path)) = search_data {
                        writeln!(
                            f,
                            "{} contains the following:\n{}",
                            search_location,
                            search_path
                                .split(':')
                                .map(|path| format!("    - {}", path))
                                .collect::<Vec<String>>()
                                .join("\n"),
                        )?;

                        format!("you may need to install a package such as {name}, {name}-dev or {name}-devel.", name=name)
                    } else {
                        // Even on Nix, setting PKG_CONFIG_PATH seems to be a viable option
                        writeln!(f, "The PKG_CONFIG_PATH environment variable is not set.")?;

                        format!(
                            "if you have installed the library, try setting PKG_CONFIG_PATH to the directory containing `{}.pc`.",
                            name
                        )
                    };

                    // Try and nudge the user in the right direction so they don't get stuck
                    writeln!(f, "\nHINT: {}", hint)?;
                }

                Ok(())
            }
            Error::Failure {
                ref command,
                ref output,
            } => {
                write!(
                    f,
                    "`{}` did not exit successfully: {}",
                    command, output.status
                )?;
                format_output(output, f)
            }
            Error::__Nonexhaustive => panic!(),
        }
    }
}

fn format_output(output: &Output, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.is_empty() {
        write!(f, "\n--- stdout\n{}", stdout)?;
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() {
        write!(f, "\n--- stderr\n{}", stderr)?;
    }
    Ok(())
}

/// Deprecated in favor of the probe_library function
#[doc(hidden)]
pub fn find_library(name: &str) -> Result<Library, String> {
    probe_library(name).map_err(|e| e.to_string())
}

/// Simple shortcut for using all default options for finding a library.
pub fn probe_library(name: &str) -> Result<Library, Error> {
    Config::new().probe(name)
}

#[doc(hidden)]
#[deprecated(note = "use config.target_supported() instance method instead")]
pub fn target_supported() -> bool {
    Config::new().target_supported()
}

/// Run `pkg-config` to get the value of a variable from a package using
/// `--variable`.
///
/// The content of `PKG_CONFIG_SYSROOT_DIR` is not injected in paths that are
/// returned by `pkg-config --variable`, which makes them unsuitable to use
/// during cross-compilation unless specifically designed to be used
/// at that time.
pub fn get_variable(package: &str, variable: &str) -> Result<String, Error> {
    let arg = format!("--variable={}", variable);
    let cfg = Config::new();
    let out = cfg.run(package, &[&arg])?;
    Ok(str::from_utf8(&out).unwrap().trim_end().to_owned())
}

impl Config {
    /// Creates a new set of configuration options which are all initially set
    /// to "blank".
    pub fn new() -> Config {
        Config {
            statik: None,
            min_version: Bound::Unbounded,
            max_version: Bound::Unbounded,
            extra_args: vec![],
            print_system_cflags: true,
            print_system_libs: true,
            cargo_metadata: true,
            env_metadata: true,
        }
    }

    /// Indicate whether the `--static` flag should be passed.
    ///
    /// This will override the inference from environment variables described in
    /// the crate documentation.
    pub fn statik(&mut self, statik: bool) -> &mut Config {
        self.statik = Some(statik);
        self
    }

    /// Indicate that the library must be at least version `vers`.
    pub fn atleast_version(&mut self, vers: &str) -> &mut Config {
        self.min_version = Bound::Included(vers.to_string());
        self.max_version = Bound::Unbounded;
        self
    }

    /// Indicate that the library must be equal to version `vers`.
    pub fn exactly_version(&mut self, vers: &str) -> &mut Config {
        self.min_version = Bound::Included(vers.to_string());
        self.max_version = Bound::Included(vers.to_string());
        self
    }

    /// Indicate that the library's version must be in `range`.
    pub fn range_version<'a, R>(&mut self, range: R) -> &mut Config
    where
        R: RangeBounds<&'a str>,
    {
        self.min_version = match range.start_bound() {
            Bound::Included(vers) => Bound::Included(vers.to_string()),
            Bound::Excluded(vers) => Bound::Excluded(vers.to_string()),
            Bound::Unbounded => Bound::Unbounded,
        };
        self.max_version = match range.end_bound() {
            Bound::Included(vers) => Bound::Included(vers.to_string()),
            Bound::Excluded(vers) => Bound::Excluded(vers.to_string()),
            Bound::Unbounded => Bound::Unbounded,
        };
        self
    }

    /// Add an argument to pass to pkg-config.
    ///
    /// It's placed after all of the arguments generated by this library.
    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Config {
        self.extra_args.push(arg.as_ref().to_os_string());
        self
    }

    /// Define whether metadata should be emitted for cargo allowing it to
    /// automatically link the binary. Defaults to `true`.
    pub fn cargo_metadata(&mut self, cargo_metadata: bool) -> &mut Config {
        self.cargo_metadata = cargo_metadata;
        self
    }

    /// Define whether metadata should be emitted for cargo allowing to
    /// automatically rebuild when environment variables change. Defaults to
    /// `true`.
    pub fn env_metadata(&mut self, env_metadata: bool) -> &mut Config {
        self.env_metadata = env_metadata;
        self
    }

    /// Enable or disable the `PKG_CONFIG_ALLOW_SYSTEM_LIBS` environment
    /// variable.
    ///
    /// This env var is enabled by default.
    pub fn print_system_libs(&mut self, print: bool) -> &mut Config {
        self.print_system_libs = print;
        self
    }

    /// Enable or disable the `PKG_CONFIG_ALLOW_SYSTEM_CFLAGS` environment
    /// variable.
    ///
    /// This env var is enabled by default.
    pub fn print_system_cflags(&mut self, print: bool) -> &mut Config {
        self.print_system_cflags = print;
        self
    }

    /// Deprecated in favor fo the `probe` function
    #[doc(hidden)]
    pub fn find(&self, name: &str) -> Result<Library, String> {
        self.probe(name).map_err(|e| e.to_string())
    }

    /// Run `pkg-config` to find the library `name`.
    ///
    /// This will use all configuration previously set to specify how
    /// `pkg-config` is run.
    pub fn probe(&self, name: &str) -> Result<Library, Error> {
        let abort_var_name = format!("{}_NO_PKG_CONFIG", envify(name));
        if self.env_var_os(&abort_var_name).is_some() {
            return Err(Error::EnvNoPkgConfig(abort_var_name));
        } else if !self.target_supported() {
            return Err(Error::CrossCompilation);
        }

        let mut library = Library::new();

        let output = self
            .run(name, &["--libs", "--cflags"])
            .map_err(|e| match e {
                Error::Failure { command, output } => Error::ProbeFailure {
                    name: name.to_owned(),
                    command,
                    output,
                },
                other => other,
            })?;
        library.parse_libs_cflags(name, &output, self);

        let output = self.run(name, &["--modversion"])?;
        library.parse_modversion(str::from_utf8(&output).unwrap());

        Ok(library)
    }

    /// True if pkg-config is used for the host system, or configured for cross-compilation
    pub fn target_supported(&self) -> bool {
        let target = env::var_os("TARGET").unwrap_or_default();
        let host = env::var_os("HOST").unwrap_or_default();

        // Only use pkg-config in host == target situations by default (allowing an
        // override).
        if host == target {
            return true;
        }

        // pkg-config may not be aware of cross-compilation, and require
        // a wrapper script that sets up platform-specific prefixes.
        match self.targeted_env_var("PKG_CONFIG_ALLOW_CROSS") {
            // don't use pkg-config if explicitly disabled
            Some(ref val) if val == "0" => false,
            Some(_) => true,
            None => {
                // if not disabled, and pkg-config is customized,
                // then assume it's prepared for cross-compilation
                self.targeted_env_var("PKG_CONFIG").is_some()
                    || self.targeted_env_var("PKG_CONFIG_SYSROOT_DIR").is_some()
            }
        }
    }

    /// Deprecated in favor of the top level `get_variable` function
    #[doc(hidden)]
    pub fn get_variable(package: &str, variable: &str) -> Result<String, String> {
        get_variable(package, variable).map_err(|e| e.to_string())
    }

    fn targeted_env_var(&self, var_base: &str) -> Option<OsString> {
        match (env::var("TARGET"), env::var("HOST")) {
            (Ok(target), Ok(host)) => {
                let kind = if host == target { "HOST" } else { "TARGET" };
                let target_u = target.replace('-', "_");

                self.env_var_os(&format!("{}_{}", var_base, target))
                    .or_else(|| self.env_var_os(&format!("{}_{}", var_base, target_u)))
                    .or_else(|| self.env_var_os(&format!("{}_{}", kind, var_base)))
                    .or_else(|| self.env_var_os(var_base))
            }
            (Err(env::VarError::NotPresent), _) | (_, Err(env::VarError::NotPresent)) => {
                self.env_var_os(var_base)
            }
            (Err(env::VarError::NotUnicode(s)), _) | (_, Err(env::VarError::NotUnicode(s))) => {
                panic!(
                    "HOST or TARGET environment variable is not valid unicode: {:?}",
                    s
                )
            }
        }
    }

    fn env_var_os(&self, name: &str) -> Option<OsString> {
        if self.env_metadata {
            println!("cargo:rerun-if-env-changed={}", name);
        }
        env::var_os(name)
    }

    fn is_static(&self, name: &str) -> bool {
        self.statik.unwrap_or_else(|| self.infer_static(name))
    }

    fn run(&self, name: &str, args: &[&str]) -> Result<Vec<u8>, Error> {
        let pkg_config_exe = self.targeted_env_var("PKG_CONFIG");
        let fallback_exe = if pkg_config_exe.is_none() {
            Some(OsString::from("pkgconf"))
        } else {
            None
        };
        let exe = pkg_config_exe.unwrap_or_else(|| OsString::from("pkg-config"));

        let mut cmd = self.command(exe, name, args);

        match cmd.output().or_else(|e| {
            if let Some(exe) = fallback_exe {
                self.command(exe, name, args).output()
            } else {
                Err(e)
            }
        }) {
            Ok(output) => {
                if output.status.success() {
                    Ok(output.stdout)
                } else {
                    Err(Error::Failure {
                        command: format!("{}", cmd),
                        output,
                    })
                }
            }
            Err(cause) => Err(Error::Command {
                command: format!("{}", cmd),
                cause,
            }),
        }
    }

    fn command(&self, exe: OsString, name: &str, args: &[&str]) -> WrappedCommand {
        let mut cmd = WrappedCommand::new(exe);
        if self.is_static(name) {
            cmd.arg("--static");
        }
        cmd.args(args).args(&self.extra_args);

        if let Some(value) = self.targeted_env_var("PKG_CONFIG_PATH") {
            cmd.env("PKG_CONFIG_PATH", value);
        }
        if let Some(value) = self.targeted_env_var("PKG_CONFIG_LIBDIR") {
            cmd.env("PKG_CONFIG_LIBDIR", value);
        }
        if let Some(value) = self.targeted_env_var("PKG_CONFIG_SYSROOT_DIR") {
            cmd.env("PKG_CONFIG_SYSROOT_DIR", value);
        }
        if self.print_system_libs {
            cmd.env("PKG_CONFIG_ALLOW_SYSTEM_LIBS", "1");
        }
        if self.print_system_cflags {
            cmd.env("PKG_CONFIG_ALLOW_SYSTEM_CFLAGS", "1");
        }
        cmd.arg(name);
        match self.min_version {
            Bound::Included(ref version) => {
                cmd.arg(&format!("{} >= {}", name, version));
            }
            Bound::Excluded(ref version) => {
                cmd.arg(&format!("{} > {}", name, version));
            }
            _ => (),
        }
        match self.max_version {
            Bound::Included(ref version) => {
                cmd.arg(&format!("{} <= {}", name, version));
            }
            Bound::Excluded(ref version) => {
                cmd.arg(&format!("{} < {}", name, version));
            }
            _ => (),
        }
        cmd
    }

    fn print_metadata(&self, s: &str) {
        if self.cargo_metadata {
            println!("cargo:{}", s);
        }
    }

    fn infer_static(&self, name: &str) -> bool {
        let name = envify(name);
        if self.env_var_os(&format!("{}_STATIC", name)).is_some() {
            true
        } else if self.env_var_os(&format!("{}_DYNAMIC", name)).is_some() {
            false
        } else if self.env_var_os("PKG_CONFIG_ALL_STATIC").is_some() {
            true
        } else if self.env_var_os("PKG_CONFIG_ALL_DYNAMIC").is_some() {
            false
        } else {
            false
        }
    }
}

// Implement Default manually since Bound does not implement Default.
impl Default for Config {
    fn default() -> Config {
        Config {
            statik: None,
            min_version: Bound::Unbounded,
            max_version: Bound::Unbounded,
            extra_args: vec![],
            print_system_cflags: false,
            print_system_libs: false,
            cargo_metadata: false,
            env_metadata: false,
        }
    }
}

impl Library {
    fn new() -> Library {
        Library {
            libs: Vec::new(),
            link_paths: Vec::new(),
            link_files: Vec::new(),
            include_paths: Vec::new(),
            ld_args: Vec::new(),
            frameworks: Vec::new(),
            framework_paths: Vec::new(),
            defines: HashMap::new(),
            version: String::new(),
            _priv: (),
        }
    }

    /// Extract the &str to pass to cargo:rustc-link-lib from a filename (just the file name, not including directories)
    /// using target-specific logic.
    fn extract_lib_from_filename<'a>(target: &str, filename: &'a str) -> Option<&'a str> {
        fn test_suffixes<'b>(filename: &'b str, suffixes: &[&str]) -> Option<&'b str> {
            for suffix in suffixes {
                if filename.ends_with(suffix) {
                    return Some(&filename[..filename.len() - suffix.len()]);
                }
            }
            None
        }

        let prefix = "lib";
        if target.contains("windows") {
            if target.contains("gnu") && filename.starts_with(prefix) {
                // GNU targets for Windows, including gnullvm, use `LinkerFlavor::Gcc` internally in rustc,
                // which tells rustc to use the GNU linker. rustc does not prepend/append to the string it
                // receives via the -l command line argument before passing it to the linker:
                // https://github.com/rust-lang/rust/blob/657f246812ab2684e3c3954b1c77f98fd59e0b21/compiler/rustc_codegen_ssa/src/back/linker.rs#L446
                // https://github.com/rust-lang/rust/blob/657f246812ab2684e3c3954b1c77f98fd59e0b21/compiler/rustc_codegen_ssa/src/back/linker.rs#L457
                // GNU ld can work with more types of files than just the .lib files that MSVC's link.exe needs.
                // GNU ld will prepend the `lib` prefix to the filename if necessary, so it is okay to remove
                // the `lib` prefix from the filename. The `.a` suffix *requires* the `lib` prefix.
                // https://sourceware.org/binutils/docs-2.39/ld.html#index-direct-linking-to-a-dll
                let filename = &filename[prefix.len()..];
                return test_suffixes(filename, &[".dll.a", ".dll", ".lib", ".a"]);
            } else {
                // According to link.exe documentation:
                // https://learn.microsoft.com/en-us/cpp/build/reference/link-input-files?view=msvc-170
                //
                //   LINK doesn't use file extensions to make assumptions about the contents of a file.
                //   Instead, LINK examines each input file to determine what kind of file it is.
                //
                // However, rustc appends `.lib` to the string it receives from the -l command line argument,
                // which it receives from Cargo via cargo:rustc-link-lib:
                // https://github.com/rust-lang/rust/blob/657f246812ab2684e3c3954b1c77f98fd59e0b21/compiler/rustc_codegen_ssa/src/back/linker.rs#L828
                // https://github.com/rust-lang/rust/blob/657f246812ab2684e3c3954b1c77f98fd59e0b21/compiler/rustc_codegen_ssa/src/back/linker.rs#L843
                // So the only file extension that works for MSVC targets is `.lib`
                // However, for externally created libraries, there's no
                // guarantee that the extension is ".lib" so we need to
                // consider all options.
                // See:
                // https://github.com/mesonbuild/meson/issues/8153
                // https://github.com/rust-lang/rust/issues/114013
                return test_suffixes(filename, &[".dll.a", ".dll", ".lib", ".a"]);
            }
        } else if target.contains("apple") {
            if filename.starts_with(prefix) {
                let filename = &filename[prefix.len()..];
                return test_suffixes(filename, &[".a", ".so", ".dylib"]);
            }
            return None;
        } else {
            if filename.starts_with(prefix) {
                let filename = &filename[prefix.len()..];
                return test_suffixes(filename, &[".a", ".so"]);
            }
            return None;
        }
    }

    fn parse_libs_cflags(&mut self, name: &str, output: &[u8], config: &Config) {
        let target = env::var("TARGET");
        let is_msvc = target
            .as_ref()
            .map(|target| target.contains("msvc"))
            .unwrap_or(false);

        let system_roots = if cfg!(target_os = "macos") {
            vec![PathBuf::from("/Library"), PathBuf::from("/System")]
        } else {
            let sysroot = config
                .env_var_os("PKG_CONFIG_SYSROOT_DIR")
                .or_else(|| config.env_var_os("SYSROOT"))
                .map(PathBuf::from);

            if cfg!(target_os = "windows") {
                if let Some(sysroot) = sysroot {
                    vec![sysroot]
                } else {
                    vec![]
                }
            } else {
                vec![sysroot.unwrap_or_else(|| PathBuf::from("/usr"))]
            }
        };

        let mut dirs = Vec::new();
        let statik = config.is_static(name);

        let words = split_flags(output);

        // Handle single-character arguments like `-I/usr/include`
        let parts = words
            .iter()
            .filter(|l| l.len() > 2)
            .map(|arg| (&arg[0..2], &arg[2..]));
        for (flag, val) in parts {
            match flag {
                "-L" => {
                    let meta = format!("rustc-link-search=native={}", val);
                    config.print_metadata(&meta);
                    dirs.push(PathBuf::from(val));
                    self.link_paths.push(PathBuf::from(val));
                }
                "-F" => {
                    let meta = format!("rustc-link-search=framework={}", val);
                    config.print_metadata(&meta);
                    self.framework_paths.push(PathBuf::from(val));
                }
                "-I" => {
                    self.include_paths.push(PathBuf::from(val));
                }
                "-l" => {
                    // These are provided by the CRT with MSVC
                    if is_msvc && ["m", "c", "pthread"].contains(&val) {
                        continue;
                    }

                    if val.starts_with(':') {
                        // Pass this flag to linker directly.
                        let meta = format!("rustc-link-arg={}{}", flag, val);
                        config.print_metadata(&meta);
                    } else if statik && is_static_available(val, &system_roots, &dirs) {
                        let meta = format!("rustc-link-lib=static={}", val);
                        config.print_metadata(&meta);
                    } else {
                        let meta = format!("rustc-link-lib={}", val);
                        config.print_metadata(&meta);
                    }

                    self.libs.push(val.to_string());
                }
                "-D" => {
                    let mut iter = val.split('=');
                    self.defines.insert(
                        iter.next().unwrap().to_owned(),
                        iter.next().map(|s| s.to_owned()),
                    );
                }
                "-u" => {
                    let meta = format!("rustc-link-arg=-Wl,-u,{}", val);
                    config.print_metadata(&meta);
                }
                _ => {}
            }
        }

        // Handle multi-character arguments with space-separated value like `-framework foo`
        let mut iter = words.iter().flat_map(|arg| {
            if arg.starts_with("-Wl,") {
                arg[4..].split(',').collect()
            } else {
                vec![arg.as_ref()]
            }
        });
        while let Some(part) = iter.next() {
            match part {
                "-framework" => {
                    if let Some(lib) = iter.next() {
                        let meta = format!("rustc-link-lib=framework={}", lib);
                        config.print_metadata(&meta);
                        self.frameworks.push(lib.to_string());
                    }
                }
                "-isystem" | "-iquote" | "-idirafter" => {
                    if let Some(inc) = iter.next() {
                        self.include_paths.push(PathBuf::from(inc));
                    }
                }
                "-undefined" | "--undefined" => {
                    if let Some(symbol) = iter.next() {
                        let meta = format!("rustc-link-arg=-Wl,{},{}", part, symbol);
                        config.print_metadata(&meta);
                    }
                }
                _ => {
                    let path = std::path::Path::new(part);
                    if path.is_file() {
                        // Cargo doesn't have a means to directly specify a file path to link,
                        // so split up the path into the parent directory and library name.
                        // TODO: pass file path directly when link-arg library type is stabilized
                        // https://github.com/rust-lang/rust/issues/99427
                        if let (Some(dir), Some(file_name), Ok(target)) =
                            (path.parent(), path.file_name(), &target)
                        {
                            match Self::extract_lib_from_filename(
                                target,
                                &file_name.to_string_lossy(),
                            ) {
                                Some(lib_basename) => {
                                    let link_search =
                                        format!("rustc-link-search={}", dir.display());
                                    config.print_metadata(&link_search);

                                    let link_lib = format!("rustc-link-lib={}", lib_basename);
                                    config.print_metadata(&link_lib);
                                    self.link_files.push(PathBuf::from(path));
                                }
                                None => {
                                    println!("cargo:warning=File path {} found in pkg-config file for {}, but could not extract library base name to pass to linker command line", path.display(), name);
                                }
                            }
                        }
                    }
                }
            }
        }

        let linker_options = words.iter().filter(|arg| arg.starts_with("-Wl,"));
        for option in linker_options {
            let mut pop = false;
            let mut ld_option = vec![];
            for subopt in option[4..].split(',') {
                if pop {
                    pop = false;
                    continue;
                }

                if subopt == "-framework" {
                    pop = true;
                    continue;
                }

                ld_option.push(subopt);
            }

            let meta = format!("rustc-link-arg=-Wl,{}", ld_option.join(","));
            config.print_metadata(&meta);

            self.ld_args
                .push(ld_option.into_iter().map(String::from).collect());
        }
    }

    fn parse_modversion(&mut self, output: &str) {
        self.version.push_str(output.lines().next().unwrap().trim());
    }
}

fn envify(name: &str) -> String {
    name.chars()
        .map(|c| c.to_ascii_uppercase())
        .map(|c| if c == '-' { '_' } else { c })
        .collect()
}

/// System libraries should only be linked dynamically
fn is_static_available(name: &str, system_roots: &[PathBuf], dirs: &[PathBuf]) -> bool {
    let libnames = {
        let mut names = vec![format!("lib{}.a", name)];

        if cfg!(target_os = "windows") {
            names.push(format!("{}.lib", name));
        }

        names
    };

    dirs.iter().any(|dir| {
        let library_exists = libnames.iter().any(|libname| dir.join(&libname).exists());
        library_exists && !system_roots.iter().any(|sys| dir.starts_with(sys))
    })
}

/// Split output produced by pkg-config --cflags and / or --libs into separate flags.
///
/// Backslash in output is used to preserve literal meaning of following byte.  Different words are
/// separated by unescaped space. Other whitespace characters generally should not occur unescaped
/// at all, apart from the newline at the end of output. For compatibility with what others
/// consumers of pkg-config output would do in this scenario, they are used here for splitting as
/// well.
fn split_flags(output: &[u8]) -> Vec<String> {
    let mut word = Vec::new();
    let mut words = Vec::new();
    let mut escaped = false;

    for &b in output {
        match b {
            _ if escaped => {
                escaped = false;
                word.push(b);
            }
            b'\\' => escaped = true,
            b'\t' | b'\n' | b'\r' | b' ' => {
                if !word.is_empty() {
                    words.push(String::from_utf8(word).unwrap());
                    word = Vec::new();
                }
            }
            _ => word.push(b),
        }
    }

    if !word.is_empty() {
        words.push(String::from_utf8(word).unwrap());
    }

    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn system_library_mac_test() {
        use std::path::Path;

        let system_roots = vec![PathBuf::from("/Library"), PathBuf::from("/System")];

        assert!(!is_static_available(
            "PluginManager",
            &system_roots,
            &[PathBuf::from("/Library/Frameworks")]
        ));
        assert!(!is_static_available(
            "python2.7",
            &system_roots,
            &[PathBuf::from(
                "/System/Library/Frameworks/Python.framework/Versions/2.7/lib/python2.7/config"
            )]
        ));
        assert!(!is_static_available(
            "ffi_convenience",
            &system_roots,
            &[PathBuf::from(
                "/Library/Ruby/Gems/2.0.0/gems/ffi-1.9.10/ext/ffi_c/libffi-x86_64/.libs"
            )]
        ));

        // Homebrew is in /usr/local, and it's not a part of the OS
        if Path::new("/usr/local/lib/libpng16.a").exists() {
            assert!(is_static_available(
                "png16",
                &system_roots,
                &[PathBuf::from("/usr/local/lib")]
            ));

            let libpng = Config::new()
                .range_version("1".."99")
                .probe("libpng16")
                .unwrap();
            assert!(libpng.version.find('\n').is_none());
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn system_library_linux_test() {
        assert!(!is_static_available(
            "util",
            &[PathBuf::from("/usr")],
            &[PathBuf::from("/usr/lib/x86_64-linux-gnu")]
        ));
        assert!(!is_static_available(
            "dialog",
            &[PathBuf::from("/usr")],
            &[PathBuf::from("/usr/lib")]
        ));
    }

    fn test_library_filename(target: &str, filename: &str) {
        assert_eq!(
            Library::extract_lib_from_filename(target, filename),
            Some("foo")
        );
    }

    #[test]
    fn link_filename_linux() {
        let target = "x86_64-unknown-linux-gnu";
        test_library_filename(target, "libfoo.a");
        test_library_filename(target, "libfoo.so");
    }

    #[test]
    fn link_filename_apple() {
        let target = "x86_64-apple-darwin";
        test_library_filename(target, "libfoo.a");
        test_library_filename(target, "libfoo.so");
        test_library_filename(target, "libfoo.dylib");
    }

    #[test]
    fn link_filename_msvc() {
        let target = "x86_64-pc-windows-msvc";
        // static and dynamic libraries have the same .lib suffix
        test_library_filename(target, "foo.lib");
    }

    #[test]
    fn link_filename_mingw() {
        let target = "x86_64-pc-windows-gnu";
        test_library_filename(target, "foo.lib");
        test_library_filename(target, "libfoo.a");
        test_library_filename(target, "foo.dll");
        test_library_filename(target, "foo.dll.a");
    }
}
