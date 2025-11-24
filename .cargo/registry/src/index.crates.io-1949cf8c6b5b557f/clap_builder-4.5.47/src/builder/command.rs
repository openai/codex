#![cfg_attr(not(feature = "usage"), allow(unused_mut))]

// Std
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::io;
use std::ops::Index;
use std::path::Path;

// Internal
use crate::builder::app_settings::{AppFlags, AppSettings};
use crate::builder::arg_settings::ArgSettings;
use crate::builder::ext::Extension;
use crate::builder::ext::Extensions;
use crate::builder::ArgAction;
use crate::builder::IntoResettable;
use crate::builder::PossibleValue;
use crate::builder::Str;
use crate::builder::StyledStr;
use crate::builder::Styles;
use crate::builder::{Arg, ArgGroup, ArgPredicate};
use crate::error::ErrorKind;
use crate::error::Result as ClapResult;
use crate::mkeymap::MKeyMap;
use crate::output::fmt::Stream;
use crate::output::{fmt::Colorizer, write_help, Usage};
use crate::parser::{ArgMatcher, ArgMatches, Parser};
use crate::util::ChildGraph;
use crate::util::{color::ColorChoice, Id};
use crate::{Error, INTERNAL_ERROR_MSG};

#[cfg(debug_assertions)]
use crate::builder::debug_asserts::assert_app;

/// Build a command-line interface.
///
/// This includes defining arguments, subcommands, parser behavior, and help output.
/// Once all configuration is complete,
/// the [`Command::get_matches`] family of methods starts the runtime-parsing
/// process. These methods then return information about the user supplied
/// arguments (or lack thereof).
///
/// When deriving a [`Parser`][crate::Parser], you can use
/// [`CommandFactory::command`][crate::CommandFactory::command] to access the
/// `Command`.
///
/// - [Basic API][crate::Command#basic-api]
/// - [Application-wide Settings][crate::Command#application-wide-settings]
/// - [Command-specific Settings][crate::Command#command-specific-settings]
/// - [Subcommand-specific Settings][crate::Command#subcommand-specific-settings]
/// - [Reflection][crate::Command#reflection]
///
/// # Examples
///
/// ```no_run
/// # use clap_builder as clap;
/// # use clap::{Command, Arg};
/// let m = Command::new("My Program")
///     .author("Me, me@mail.com")
///     .version("1.0.2")
///     .about("Explains in brief what the program does")
///     .arg(
///         Arg::new("in_file")
///     )
///     .after_help("Longer explanation to appear after the options when \
///                  displaying the help information from --help or -h")
///     .get_matches();
///
/// // Your program logic starts here...
/// ```
/// [`Command::get_matches`]: Command::get_matches()
#[derive(Debug, Clone)]
pub struct Command {
    name: Str,
    long_flag: Option<Str>,
    short_flag: Option<char>,
    display_name: Option<String>,
    bin_name: Option<String>,
    author: Option<Str>,
    version: Option<Str>,
    long_version: Option<Str>,
    about: Option<StyledStr>,
    long_about: Option<StyledStr>,
    before_help: Option<StyledStr>,
    before_long_help: Option<StyledStr>,
    after_help: Option<StyledStr>,
    after_long_help: Option<StyledStr>,
    aliases: Vec<(Str, bool)>,             // (name, visible)
    short_flag_aliases: Vec<(char, bool)>, // (name, visible)
    long_flag_aliases: Vec<(Str, bool)>,   // (name, visible)
    usage_str: Option<StyledStr>,
    usage_name: Option<String>,
    help_str: Option<StyledStr>,
    disp_ord: Option<usize>,
    #[cfg(feature = "help")]
    template: Option<StyledStr>,
    settings: AppFlags,
    g_settings: AppFlags,
    args: MKeyMap,
    subcommands: Vec<Command>,
    groups: Vec<ArgGroup>,
    current_help_heading: Option<Str>,
    current_disp_ord: Option<usize>,
    subcommand_value_name: Option<Str>,
    subcommand_heading: Option<Str>,
    external_value_parser: Option<super::ValueParser>,
    long_help_exists: bool,
    deferred: Option<fn(Command) -> Command>,
    #[cfg(feature = "unstable-ext")]
    ext: Extensions,
    app_ext: Extensions,
}

/// # Basic API
impl Command {
    /// Creates a new instance of an `Command`.
    ///
    /// It is common, but not required, to use binary name as the `name`. This
    /// name will only be displayed to the user when they request to print
    /// version or help and usage information.
    ///
    /// See also [`command!`](crate::command!) and [`crate_name!`](crate::crate_name!).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// Command::new("My Program")
    /// # ;
    /// ```
    pub fn new(name: impl Into<Str>) -> Self {
        /// The actual implementation of `new`, non-generic to save code size.
        ///
        /// If we don't do this rustc will unnecessarily generate multiple versions
        /// of this code.
        fn new_inner(name: Str) -> Command {
            Command {
                name,
                ..Default::default()
            }
        }

        new_inner(name.into())
    }

    /// Adds an [argument] to the list of valid possibilities.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, arg, Arg};
    /// Command::new("myprog")
    ///     // Adding a single "flag" argument with a short and help text, using Arg::new()
    ///     .arg(
    ///         Arg::new("debug")
    ///            .short('d')
    ///            .help("turns on debugging mode")
    ///     )
    ///     // Adding a single "option" argument with a short, a long, and help text using the less
    ///     // verbose Arg::from()
    ///     .arg(
    ///         arg!(-c --config <CONFIG> "Optionally sets a config file to use")
    ///     )
    /// # ;
    /// ```
    /// [argument]: Arg
    #[must_use]
    pub fn arg(mut self, a: impl Into<Arg>) -> Self {
        let arg = a.into();
        self.arg_internal(arg);
        self
    }

    fn arg_internal(&mut self, mut arg: Arg) {
        if let Some(current_disp_ord) = self.current_disp_ord.as_mut() {
            if !arg.is_positional() {
                let current = *current_disp_ord;
                arg.disp_ord.get_or_insert(current);
                *current_disp_ord = current + 1;
            }
        }

        arg.help_heading
            .get_or_insert_with(|| self.current_help_heading.clone());
        self.args.push(arg);
    }

    /// Adds multiple [arguments] to the list of valid possibilities.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, arg, Arg};
    /// Command::new("myprog")
    ///     .args([
    ///         arg!(-d --debug "turns on debugging info"),
    ///         Arg::new("input").help("the input file to use")
    ///     ])
    /// # ;
    /// ```
    /// [arguments]: Arg
    #[must_use]
    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<Arg>>) -> Self {
        for arg in args {
            self = self.arg(arg);
        }
        self
    }

    /// Allows one to mutate an [`Arg`] after it's been added to a [`Command`].
    ///
    /// # Panics
    ///
    /// If the argument is undefined
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgAction};
    ///
    /// let mut cmd = Command::new("foo")
    ///     .arg(Arg::new("bar")
    ///         .short('b')
    ///         .action(ArgAction::SetTrue))
    ///     .mut_arg("bar", |a| a.short('B'));
    ///
    /// let res = cmd.try_get_matches_from_mut(vec!["foo", "-b"]);
    ///
    /// // Since we changed `bar`'s short to "B" this should err as there
    /// // is no `-b` anymore, only `-B`
    ///
    /// assert!(res.is_err());
    ///
    /// let res = cmd.try_get_matches_from_mut(vec!["foo", "-B"]);
    /// assert!(res.is_ok());
    /// ```
    #[must_use]
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn mut_arg<F>(mut self, arg_id: impl AsRef<str>, f: F) -> Self
    where
        F: FnOnce(Arg) -> Arg,
    {
        let id = arg_id.as_ref();
        let a = self
            .args
            .remove_by_name(id)
            .unwrap_or_else(|| panic!("Argument `{id}` is undefined"));

        self.args.push(f(a));
        self
    }

    /// Allows one to mutate all [`Arg`]s after they've been added to a [`Command`].
    ///
    /// This does not affect the built-in `--help` or `--version` arguments.
    ///
    /// # Examples
    ///
    #[cfg_attr(feature = "string", doc = "```")]
    #[cfg_attr(not(feature = "string"), doc = "```ignore")]
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgAction};
    ///
    /// let mut cmd = Command::new("foo")
    ///     .arg(Arg::new("bar")
    ///         .long("bar")
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("baz")
    ///         .long("baz")
    ///         .action(ArgAction::SetTrue))
    ///     .mut_args(|a| {
    ///         if let Some(l) = a.get_long().map(|l| format!("prefix-{l}")) {
    ///             a.long(l)
    ///         } else {
    ///             a
    ///         }
    ///     });
    ///
    /// let res = cmd.try_get_matches_from_mut(vec!["foo", "--bar"]);
    ///
    /// // Since we changed `bar`'s long to "prefix-bar" this should err as there
    /// // is no `--bar` anymore, only `--prefix-bar`.
    ///
    /// assert!(res.is_err());
    ///
    /// let res = cmd.try_get_matches_from_mut(vec!["foo", "--prefix-bar"]);
    /// assert!(res.is_ok());
    /// ```
    #[must_use]
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn mut_args<F>(mut self, f: F) -> Self
    where
        F: FnMut(Arg) -> Arg,
    {
        self.args.mut_args(f);
        self
    }

    /// Allows one to mutate an [`ArgGroup`] after it's been added to a [`Command`].
    ///
    /// # Panics
    ///
    /// If the argument is undefined
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, arg, ArgGroup};
    ///
    /// Command::new("foo")
    ///     .arg(arg!(--"set-ver" <ver> "set the version manually").required(false))
    ///     .arg(arg!(--major "auto increase major"))
    ///     .arg(arg!(--minor "auto increase minor"))
    ///     .arg(arg!(--patch "auto increase patch"))
    ///     .group(ArgGroup::new("vers")
    ///          .args(["set-ver", "major", "minor","patch"])
    ///          .required(true))
    ///     .mut_group("vers", |a| a.required(false));
    /// ```
    #[must_use]
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn mut_group<F>(mut self, arg_id: impl AsRef<str>, f: F) -> Self
    where
        F: FnOnce(ArgGroup) -> ArgGroup,
    {
        let id = arg_id.as_ref();
        let index = self
            .groups
            .iter()
            .position(|g| g.get_id() == id)
            .unwrap_or_else(|| panic!("Group `{id}` is undefined"));
        let a = self.groups.remove(index);

        self.groups.push(f(a));
        self
    }
    /// Allows one to mutate a [`Command`] after it's been added as a subcommand.
    ///
    /// This can be useful for modifying auto-generated arguments of nested subcommands with
    /// [`Command::mut_arg`].
    ///
    /// # Panics
    ///
    /// If the subcommand is undefined
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    ///
    /// let mut cmd = Command::new("foo")
    ///         .subcommand(Command::new("bar"))
    ///         .mut_subcommand("bar", |subcmd| subcmd.disable_help_flag(true));
    ///
    /// let res = cmd.try_get_matches_from_mut(vec!["foo", "bar", "--help"]);
    ///
    /// // Since we disabled the help flag on the "bar" subcommand, this should err.
    ///
    /// assert!(res.is_err());
    ///
    /// let res = cmd.try_get_matches_from_mut(vec!["foo", "bar"]);
    /// assert!(res.is_ok());
    /// ```
    #[must_use]
    pub fn mut_subcommand<F>(mut self, name: impl AsRef<str>, f: F) -> Self
    where
        F: FnOnce(Self) -> Self,
    {
        let name = name.as_ref();
        let pos = self.subcommands.iter().position(|s| s.name == name);

        let subcmd = if let Some(idx) = pos {
            self.subcommands.remove(idx)
        } else {
            panic!("Command `{name}` is undefined")
        };

        self.subcommands.push(f(subcmd));
        self
    }

    /// Allows one to mutate all [`Command`]s after they've been added as subcommands.
    ///
    /// This does not affect the built-in `--help` or `--version` arguments.
    ///
    /// # Examples
    ///
    #[cfg_attr(feature = "string", doc = "```")]
    #[cfg_attr(not(feature = "string"), doc = "```ignore")]
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgAction};
    ///
    /// let mut cmd = Command::new("foo")
    ///     .subcommands([
    ///         Command::new("fetch"),
    ///         Command::new("push"),
    ///     ])
    ///     // Allow title-case subcommands
    ///     .mut_subcommands(|sub| {
    ///         let name = sub.get_name();
    ///         let alias = name.chars().enumerate().map(|(i, c)| {
    ///             if i == 0 {
    ///                 c.to_ascii_uppercase()
    ///             } else {
    ///                 c
    ///             }
    ///         }).collect::<String>();
    ///         sub.alias(alias)
    ///     });
    ///
    /// let res = cmd.try_get_matches_from_mut(vec!["foo", "fetch"]);
    /// assert!(res.is_ok());
    ///
    /// let res = cmd.try_get_matches_from_mut(vec!["foo", "Fetch"]);
    /// assert!(res.is_ok());
    /// ```
    #[must_use]
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn mut_subcommands<F>(mut self, f: F) -> Self
    where
        F: FnMut(Command) -> Command,
    {
        self.subcommands = self.subcommands.into_iter().map(f).collect();
        self
    }

    /// Adds an [`ArgGroup`] to the application.
    ///
    /// [`ArgGroup`]s are a family of related arguments.
    /// By placing them in a logical group, you can build easier requirement and exclusion rules.
    ///
    /// Example use cases:
    /// - Make an entire [`ArgGroup`] required, meaning that one (and *only*
    ///   one) argument from that group must be present at runtime.
    /// - Name an [`ArgGroup`] as a conflict to another argument.
    ///   Meaning any of the arguments that belong to that group will cause a failure if present with
    ///   the conflicting argument.
    /// - Ensure exclusion between arguments.
    /// - Extract a value from a group instead of determining exactly which argument was used.
    ///
    /// # Examples
    ///
    /// The following example demonstrates using an [`ArgGroup`] to ensure that one, and only one,
    /// of the arguments from the specified group is present at runtime.
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, arg, ArgGroup};
    /// Command::new("cmd")
    ///     .arg(arg!(--"set-ver" <ver> "set the version manually").required(false))
    ///     .arg(arg!(--major "auto increase major"))
    ///     .arg(arg!(--minor "auto increase minor"))
    ///     .arg(arg!(--patch "auto increase patch"))
    ///     .group(ArgGroup::new("vers")
    ///          .args(["set-ver", "major", "minor","patch"])
    ///          .required(true))
    /// # ;
    /// ```
    #[inline]
    #[must_use]
    pub fn group(mut self, group: impl Into<ArgGroup>) -> Self {
        self.groups.push(group.into());
        self
    }

    /// Adds multiple [`ArgGroup`]s to the [`Command`] at once.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, arg, ArgGroup};
    /// Command::new("cmd")
    ///     .arg(arg!(--"set-ver" <ver> "set the version manually").required(false))
    ///     .arg(arg!(--major         "auto increase major"))
    ///     .arg(arg!(--minor         "auto increase minor"))
    ///     .arg(arg!(--patch         "auto increase patch"))
    ///     .arg(arg!(-c <FILE>       "a config file").required(false))
    ///     .arg(arg!(-i <IFACE>      "an interface").required(false))
    ///     .groups([
    ///         ArgGroup::new("vers")
    ///             .args(["set-ver", "major", "minor","patch"])
    ///             .required(true),
    ///         ArgGroup::new("input")
    ///             .args(["c", "i"])
    ///     ])
    /// # ;
    /// ```
    #[must_use]
    pub fn groups(mut self, groups: impl IntoIterator<Item = impl Into<ArgGroup>>) -> Self {
        for g in groups {
            self = self.group(g.into());
        }
        self
    }

    /// Adds a subcommand to the list of valid possibilities.
    ///
    /// Subcommands are effectively sub-[`Command`]s, because they can contain their own arguments,
    /// subcommands, version, usage, etc. They also function just like [`Command`]s, in that they get
    /// their own auto generated help, version, and usage.
    ///
    /// A subcommand's [`Command::name`] will be used for:
    /// - The argument the user passes in
    /// - Programmatically looking up the subcommand
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, arg};
    /// Command::new("myprog")
    ///     .subcommand(Command::new("config")
    ///         .about("Controls configuration features")
    ///         .arg(arg!(<config> "Required configuration file to use")))
    /// # ;
    /// ```
    #[inline]
    #[must_use]
    pub fn subcommand(self, subcmd: impl Into<Command>) -> Self {
        let subcmd = subcmd.into();
        self.subcommand_internal(subcmd)
    }

    fn subcommand_internal(mut self, mut subcmd: Self) -> Self {
        if let Some(current_disp_ord) = self.current_disp_ord.as_mut() {
            let current = *current_disp_ord;
            subcmd.disp_ord.get_or_insert(current);
            *current_disp_ord = current + 1;
        }
        self.subcommands.push(subcmd);
        self
    }

    /// Adds multiple subcommands to the list of valid possibilities.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, };
    /// # Command::new("myprog")
    /// .subcommands( [
    ///        Command::new("config").about("Controls configuration functionality")
    ///                                 .arg(Arg::new("config_file")),
    ///        Command::new("debug").about("Controls debug functionality")])
    /// # ;
    /// ```
    /// [`IntoIterator`]: std::iter::IntoIterator
    #[must_use]
    pub fn subcommands(mut self, subcmds: impl IntoIterator<Item = impl Into<Self>>) -> Self {
        for subcmd in subcmds {
            self = self.subcommand(subcmd);
        }
        self
    }

    /// Delay initialization for parts of the `Command`
    ///
    /// This is useful for large applications to delay definitions of subcommands until they are
    /// being invoked.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, arg};
    /// Command::new("myprog")
    ///     .subcommand(Command::new("config")
    ///         .about("Controls configuration features")
    ///         .defer(|cmd| {
    ///             cmd.arg(arg!(<config> "Required configuration file to use"))
    ///         })
    ///     )
    /// # ;
    /// ```
    pub fn defer(mut self, deferred: fn(Command) -> Command) -> Self {
        self.deferred = Some(deferred);
        self
    }

    /// Catch problems earlier in the development cycle.
    ///
    /// Most error states are handled as asserts under the assumption they are programming mistake
    /// and not something to handle at runtime.  Rather than relying on tests (manual or automated)
    /// that exhaustively test your CLI to ensure the asserts are evaluated, this will run those
    /// asserts in a way convenient for running as a test.
    ///
    /// **Note:** This will not help with asserts in [`ArgMatches`], those will need exhaustive
    /// testing of your CLI.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgAction};
    /// fn cmd() -> Command {
    ///     Command::new("foo")
    ///         .arg(
    ///             Arg::new("bar").short('b').action(ArgAction::SetTrue)
    ///         )
    /// }
    ///
    /// #[test]
    /// fn verify_app() {
    ///     cmd().debug_assert();
    /// }
    ///
    /// fn main() {
    ///     let m = cmd().get_matches_from(vec!["foo", "-b"]);
    ///     println!("{}", m.get_flag("bar"));
    /// }
    /// ```
    pub fn debug_assert(mut self) {
        self.build();
    }

    /// Custom error message for post-parsing validation
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, error::ErrorKind};
    /// let mut cmd = Command::new("myprog");
    /// let err = cmd.error(ErrorKind::InvalidValue, "Some failure case");
    /// ```
    pub fn error(&mut self, kind: ErrorKind, message: impl fmt::Display) -> Error {
        Error::raw(kind, message).format(self)
    }

    /// Parse [`env::args_os`], [exiting][Error::exit] on failure.
    ///
    /// # Panics
    ///
    /// If contradictory arguments or settings exist (debug builds).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// let matches = Command::new("myprog")
    ///     // Args and options go here...
    ///     .get_matches();
    /// ```
    /// [`env::args_os`]: std::env::args_os()
    /// [`Command::try_get_matches_from_mut`]: Command::try_get_matches_from_mut()
    #[inline]
    pub fn get_matches(self) -> ArgMatches {
        self.get_matches_from(env::args_os())
    }

    /// Parse [`env::args_os`], [exiting][Error::exit] on failure.
    ///
    /// Like [`Command::get_matches`] but doesn't consume the `Command`.
    ///
    /// # Panics
    ///
    /// If contradictory arguments or settings exist (debug builds).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// let mut cmd = Command::new("myprog")
    ///     // Args and options go here...
    ///     ;
    /// let matches = cmd.get_matches_mut();
    /// ```
    /// [`env::args_os`]: std::env::args_os()
    /// [`Command::get_matches`]: Command::get_matches()
    pub fn get_matches_mut(&mut self) -> ArgMatches {
        self.try_get_matches_from_mut(env::args_os())
            .unwrap_or_else(|e| e.exit())
    }

    /// Parse [`env::args_os`], returning a [`clap::Result`] on failure.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This method WILL NOT exit when `--help` or `--version` (or short versions) are
    /// used. It will return a [`clap::Error`], where the [`kind`] is a
    /// [`ErrorKind::DisplayHelp`] or [`ErrorKind::DisplayVersion`] respectively. You must call
    /// [`Error::exit`] or perform a [`std::process::exit`].
    ///
    /// </div>
    ///
    /// # Panics
    ///
    /// If contradictory arguments or settings exist (debug builds).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// let matches = Command::new("myprog")
    ///     // Args and options go here...
    ///     .try_get_matches()
    ///     .unwrap_or_else(|e| e.exit());
    /// ```
    /// [`env::args_os`]: std::env::args_os()
    /// [`Error::exit`]: crate::Error::exit()
    /// [`std::process::exit`]: std::process::exit()
    /// [`clap::Result`]: Result
    /// [`clap::Error`]: crate::Error
    /// [`kind`]: crate::Error
    /// [`ErrorKind::DisplayHelp`]: crate::error::ErrorKind::DisplayHelp
    /// [`ErrorKind::DisplayVersion`]: crate::error::ErrorKind::DisplayVersion
    #[inline]
    pub fn try_get_matches(self) -> ClapResult<ArgMatches> {
        // Start the parsing
        self.try_get_matches_from(env::args_os())
    }

    /// Parse the specified arguments, [exiting][Error::exit] on failure.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** The first argument will be parsed as the binary name unless
    /// [`Command::no_binary_name`] is used.
    ///
    /// </div>
    ///
    /// # Panics
    ///
    /// If contradictory arguments or settings exist (debug builds).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// let arg_vec = vec!["my_prog", "some", "args", "to", "parse"];
    ///
    /// let matches = Command::new("myprog")
    ///     // Args and options go here...
    ///     .get_matches_from(arg_vec);
    /// ```
    /// [`Command::get_matches`]: Command::get_matches()
    /// [`clap::Result`]: Result
    /// [`Vec`]: std::vec::Vec
    pub fn get_matches_from<I, T>(mut self, itr: I) -> ArgMatches
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        self.try_get_matches_from_mut(itr).unwrap_or_else(|e| {
            drop(self);
            e.exit()
        })
    }

    /// Parse the specified arguments, returning a [`clap::Result`] on failure.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This method WILL NOT exit when `--help` or `--version` (or short versions) are
    /// used. It will return a [`clap::Error`], where the [`kind`] is a [`ErrorKind::DisplayHelp`]
    /// or [`ErrorKind::DisplayVersion`] respectively. You must call [`Error::exit`] or
    /// perform a [`std::process::exit`] yourself.
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** The first argument will be parsed as the binary name unless
    /// [`Command::no_binary_name`] is used.
    ///
    /// </div>
    ///
    /// # Panics
    ///
    /// If contradictory arguments or settings exist (debug builds).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// let arg_vec = vec!["my_prog", "some", "args", "to", "parse"];
    ///
    /// let matches = Command::new("myprog")
    ///     // Args and options go here...
    ///     .try_get_matches_from(arg_vec)
    ///     .unwrap_or_else(|e| e.exit());
    /// ```
    /// [`Command::get_matches_from`]: Command::get_matches_from()
    /// [`Command::try_get_matches`]: Command::try_get_matches()
    /// [`Error::exit`]: crate::Error::exit()
    /// [`std::process::exit`]: std::process::exit()
    /// [`clap::Error`]: crate::Error
    /// [`Error::exit`]: crate::Error::exit()
    /// [`kind`]: crate::Error
    /// [`ErrorKind::DisplayHelp`]: crate::error::ErrorKind::DisplayHelp
    /// [`ErrorKind::DisplayVersion`]: crate::error::ErrorKind::DisplayVersion
    /// [`clap::Result`]: Result
    pub fn try_get_matches_from<I, T>(mut self, itr: I) -> ClapResult<ArgMatches>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        self.try_get_matches_from_mut(itr)
    }

    /// Parse the specified arguments, returning a [`clap::Result`] on failure.
    ///
    /// Like [`Command::try_get_matches_from`] but doesn't consume the `Command`.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This method WILL NOT exit when `--help` or `--version` (or short versions) are
    /// used. It will return a [`clap::Error`], where the [`kind`] is a [`ErrorKind::DisplayHelp`]
    /// or [`ErrorKind::DisplayVersion`] respectively. You must call [`Error::exit`] or
    /// perform a [`std::process::exit`] yourself.
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** The first argument will be parsed as the binary name unless
    /// [`Command::no_binary_name`] is used.
    ///
    /// </div>
    ///
    /// # Panics
    ///
    /// If contradictory arguments or settings exist (debug builds).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// let arg_vec = vec!["my_prog", "some", "args", "to", "parse"];
    ///
    /// let mut cmd = Command::new("myprog");
    ///     // Args and options go here...
    /// let matches = cmd.try_get_matches_from_mut(arg_vec)
    ///     .unwrap_or_else(|e| e.exit());
    /// ```
    /// [`Command::try_get_matches_from`]: Command::try_get_matches_from()
    /// [`clap::Result`]: Result
    /// [`clap::Error`]: crate::Error
    /// [`kind`]: crate::Error
    pub fn try_get_matches_from_mut<I, T>(&mut self, itr: I) -> ClapResult<ArgMatches>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        let mut raw_args = clap_lex::RawArgs::new(itr);
        let mut cursor = raw_args.cursor();

        if self.settings.is_set(AppSettings::Multicall) {
            if let Some(argv0) = raw_args.next_os(&mut cursor) {
                let argv0 = Path::new(&argv0);
                if let Some(command) = argv0.file_stem().and_then(|f| f.to_str()) {
                    // Stop borrowing command so we can get another mut ref to it.
                    let command = command.to_owned();
                    debug!("Command::try_get_matches_from_mut: Parsed command {command} from argv");

                    debug!("Command::try_get_matches_from_mut: Reinserting command into arguments so subcommand parser matches it");
                    raw_args.insert(&cursor, [&command]);
                    debug!("Command::try_get_matches_from_mut: Clearing name and bin_name so that displayed command name starts with applet name");
                    self.name = "".into();
                    self.bin_name = None;
                    return self._do_parse(&mut raw_args, cursor);
                }
            }
        };

        // Get the name of the program (argument 1 of env::args()) and determine the
        // actual file
        // that was used to execute the program. This is because a program called
        // ./target/release/my_prog -a
        // will have two arguments, './target/release/my_prog', '-a' but we don't want
        // to display
        // the full path when displaying help messages and such
        if !self.settings.is_set(AppSettings::NoBinaryName) {
            if let Some(name) = raw_args.next_os(&mut cursor) {
                let p = Path::new(name);

                if let Some(f) = p.file_name() {
                    if let Some(s) = f.to_str() {
                        if self.bin_name.is_none() {
                            self.bin_name = Some(s.to_owned());
                        }
                    }
                }
            }
        }

        self._do_parse(&mut raw_args, cursor)
    }

    /// Prints the short help message (`-h`) to [`io::stdout()`].
    ///
    /// See also [`Command::print_long_help`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// let mut cmd = Command::new("myprog");
    /// cmd.print_help();
    /// ```
    /// [`io::stdout()`]: std::io::stdout()
    pub fn print_help(&mut self) -> io::Result<()> {
        self._build_self(false);
        let color = self.color_help();

        let mut styled = StyledStr::new();
        let usage = Usage::new(self);
        write_help(&mut styled, self, &usage, false);

        let c = Colorizer::new(Stream::Stdout, color).with_content(styled);
        c.print()
    }

    /// Prints the long help message (`--help`) to [`io::stdout()`].
    ///
    /// See also [`Command::print_help`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// let mut cmd = Command::new("myprog");
    /// cmd.print_long_help();
    /// ```
    /// [`io::stdout()`]: std::io::stdout()
    /// [`BufWriter`]: std::io::BufWriter
    /// [`-h` (short)]: Arg::help()
    /// [`--help` (long)]: Arg::long_help()
    pub fn print_long_help(&mut self) -> io::Result<()> {
        self._build_self(false);
        let color = self.color_help();

        let mut styled = StyledStr::new();
        let usage = Usage::new(self);
        write_help(&mut styled, self, &usage, true);

        let c = Colorizer::new(Stream::Stdout, color).with_content(styled);
        c.print()
    }

    /// Render the short help message (`-h`) to a [`StyledStr`]
    ///
    /// See also [`Command::render_long_help`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// use std::io;
    /// let mut cmd = Command::new("myprog");
    /// let mut out = io::stdout();
    /// let help = cmd.render_help();
    /// println!("{help}");
    /// ```
    /// [`io::Write`]: std::io::Write
    /// [`-h` (short)]: Arg::help()
    /// [`--help` (long)]: Arg::long_help()
    pub fn render_help(&mut self) -> StyledStr {
        self._build_self(false);

        let mut styled = StyledStr::new();
        let usage = Usage::new(self);
        write_help(&mut styled, self, &usage, false);
        styled
    }

    /// Render the long help message (`--help`) to a [`StyledStr`].
    ///
    /// See also [`Command::render_help`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// use std::io;
    /// let mut cmd = Command::new("myprog");
    /// let mut out = io::stdout();
    /// let help = cmd.render_long_help();
    /// println!("{help}");
    /// ```
    /// [`io::Write`]: std::io::Write
    /// [`-h` (short)]: Arg::help()
    /// [`--help` (long)]: Arg::long_help()
    pub fn render_long_help(&mut self) -> StyledStr {
        self._build_self(false);

        let mut styled = StyledStr::new();
        let usage = Usage::new(self);
        write_help(&mut styled, self, &usage, true);
        styled
    }

    #[doc(hidden)]
    #[cfg_attr(
        feature = "deprecated",
        deprecated(since = "4.0.0", note = "Replaced with `Command::render_help`")
    )]
    pub fn write_help<W: io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self._build_self(false);

        let mut styled = StyledStr::new();
        let usage = Usage::new(self);
        write_help(&mut styled, self, &usage, false);
        ok!(write!(w, "{styled}"));
        w.flush()
    }

    #[doc(hidden)]
    #[cfg_attr(
        feature = "deprecated",
        deprecated(since = "4.0.0", note = "Replaced with `Command::render_long_help`")
    )]
    pub fn write_long_help<W: io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self._build_self(false);

        let mut styled = StyledStr::new();
        let usage = Usage::new(self);
        write_help(&mut styled, self, &usage, true);
        ok!(write!(w, "{styled}"));
        w.flush()
    }

    /// Version message rendered as if the user ran `-V`.
    ///
    /// See also [`Command::render_long_version`].
    ///
    /// ### Coloring
    ///
    /// This function does not try to color the message nor it inserts any [ANSI escape codes].
    ///
    /// ### Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// use std::io;
    /// let cmd = Command::new("myprog");
    /// println!("{}", cmd.render_version());
    /// ```
    /// [`io::Write`]: std::io::Write
    /// [`-V` (short)]: Command::version()
    /// [`--version` (long)]: Command::long_version()
    /// [ANSI escape codes]: https://en.wikipedia.org/wiki/ANSI_escape_code
    pub fn render_version(&self) -> String {
        self._render_version(false)
    }

    /// Version message rendered as if the user ran `--version`.
    ///
    /// See also [`Command::render_version`].
    ///
    /// ### Coloring
    ///
    /// This function does not try to color the message nor it inserts any [ANSI escape codes].
    ///
    /// ### Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// use std::io;
    /// let cmd = Command::new("myprog");
    /// println!("{}", cmd.render_long_version());
    /// ```
    /// [`io::Write`]: std::io::Write
    /// [`-V` (short)]: Command::version()
    /// [`--version` (long)]: Command::long_version()
    /// [ANSI escape codes]: https://en.wikipedia.org/wiki/ANSI_escape_code
    pub fn render_long_version(&self) -> String {
        self._render_version(true)
    }

    /// Usage statement
    ///
    /// ### Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// use std::io;
    /// let mut cmd = Command::new("myprog");
    /// println!("{}", cmd.render_usage());
    /// ```
    pub fn render_usage(&mut self) -> StyledStr {
        self.render_usage_().unwrap_or_default()
    }

    pub(crate) fn render_usage_(&mut self) -> Option<StyledStr> {
        // If there are global arguments, or settings we need to propagate them down to subcommands
        // before parsing incase we run into a subcommand
        self._build_self(false);

        Usage::new(self).create_usage_with_title(&[])
    }

    /// Extend [`Command`] with [`CommandExt`] data
    #[cfg(feature = "unstable-ext")]
    #[allow(clippy::should_implement_trait)]
    pub fn add<T: CommandExt + Extension>(mut self, tagged: T) -> Self {
        self.ext.set(tagged);
        self
    }
}

/// # Application-wide Settings
///
/// These settings will apply to the top-level command and all subcommands, by default.  Some
/// settings can be overridden in subcommands.
impl Command {
    /// Specifies that the parser should not assume the first argument passed is the binary name.
    ///
    /// This is normally the case when using a "daemon" style mode.  For shells / REPLs, see
    /// [`Command::multicall`][Command::multicall].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, arg};
    /// let m = Command::new("myprog")
    ///     .no_binary_name(true)
    ///     .arg(arg!(<cmd> ... "commands to run"))
    ///     .get_matches_from(vec!["command", "set"]);
    ///
    /// let cmds: Vec<_> = m.get_many::<String>("cmd").unwrap().collect();
    /// assert_eq!(cmds, ["command", "set"]);
    /// ```
    /// [`try_get_matches_from_mut`]: crate::Command::try_get_matches_from_mut()
    #[inline]
    pub fn no_binary_name(self, yes: bool) -> Self {
        if yes {
            self.global_setting(AppSettings::NoBinaryName)
        } else {
            self.unset_global_setting(AppSettings::NoBinaryName)
        }
    }

    /// Try not to fail on parse errors, like missing option values.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This choice is propagated to all child subcommands.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, arg};
    /// let cmd = Command::new("cmd")
    ///   .ignore_errors(true)
    ///   .arg(arg!(-c --config <FILE> "Sets a custom config file"))
    ///   .arg(arg!(-x --stuff <FILE> "Sets a custom stuff file"))
    ///   .arg(arg!(f: -f "Flag"));
    ///
    /// let r = cmd.try_get_matches_from(vec!["cmd", "-c", "file", "-f", "-x"]);
    ///
    /// assert!(r.is_ok(), "unexpected error: {r:?}");
    /// let m = r.unwrap();
    /// assert_eq!(m.get_one::<String>("config").unwrap(), "file");
    /// assert!(m.get_flag("f"));
    /// assert_eq!(m.get_one::<String>("stuff"), None);
    /// ```
    #[inline]
    pub fn ignore_errors(self, yes: bool) -> Self {
        if yes {
            self.global_setting(AppSettings::IgnoreErrors)
        } else {
            self.unset_global_setting(AppSettings::IgnoreErrors)
        }
    }

    /// Replace prior occurrences of arguments rather than error
    ///
    /// For any argument that would conflict with itself by default (e.g.
    /// [`ArgAction::Set`], it will now override itself.
    ///
    /// This is the equivalent to saying the `foo` arg using [`Arg::overrides_with("foo")`] for all
    /// defined arguments.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This choice is propagated to all child subcommands.
    ///
    /// </div>
    ///
    /// [`Arg::overrides_with("foo")`]: crate::Arg::overrides_with()
    #[inline]
    pub fn args_override_self(self, yes: bool) -> Self {
        if yes {
            self.global_setting(AppSettings::AllArgsOverrideSelf)
        } else {
            self.unset_global_setting(AppSettings::AllArgsOverrideSelf)
        }
    }

    /// Disables the automatic delimiting of values after `--` or when [`Arg::trailing_var_arg`]
    /// was used.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** The same thing can be done manually by setting the final positional argument to
    /// [`Arg::value_delimiter(None)`]. Using this setting is safer, because it's easier to locate
    /// when making changes.
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This choice is propagated to all child subcommands.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// Command::new("myprog")
    ///     .dont_delimit_trailing_values(true)
    ///     .get_matches();
    /// ```
    ///
    /// [`Arg::value_delimiter(None)`]: crate::Arg::value_delimiter()
    #[inline]
    pub fn dont_delimit_trailing_values(self, yes: bool) -> Self {
        if yes {
            self.global_setting(AppSettings::DontDelimitTrailingValues)
        } else {
            self.unset_global_setting(AppSettings::DontDelimitTrailingValues)
        }
    }

    /// Sets when to color output.
    ///
    /// To customize how the output is styled, see [`Command::styles`].
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This choice is propagated to all child subcommands.
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** Default behaviour is [`ColorChoice::Auto`].
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use clap_builder as clap;
    /// # use clap::{Command, ColorChoice};
    /// Command::new("myprog")
    ///     .color(ColorChoice::Never)
    ///     .get_matches();
    /// ```
    /// [`ColorChoice::Auto`]: crate::ColorChoice::Auto
    #[cfg(feature = "color")]
    #[inline]
    #[must_use]
    pub fn color(self, color: ColorChoice) -> Self {
        let cmd = self
            .unset_global_setting(AppSettings::ColorAuto)
            .unset_global_setting(AppSettings::ColorAlways)
            .unset_global_setting(AppSettings::ColorNever);
        match color {
            ColorChoice::Auto => cmd.global_setting(AppSettings::ColorAuto),
            ColorChoice::Always => cmd.global_setting(AppSettings::ColorAlways),
            ColorChoice::Never => cmd.global_setting(AppSettings::ColorNever),
        }
    }

    /// Sets the [`Styles`] for terminal output
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This choice is propagated to all child subcommands.
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** Default behaviour is [`Styles::default`].
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use clap_builder as clap;
    /// # use clap::{Command, ColorChoice, builder::styling};
    /// const STYLES: styling::Styles = styling::Styles::styled()
    ///     .header(styling::AnsiColor::Green.on_default().bold())
    ///     .usage(styling::AnsiColor::Green.on_default().bold())
    ///     .literal(styling::AnsiColor::Blue.on_default().bold())
    ///     .placeholder(styling::AnsiColor::Cyan.on_default());
    /// Command::new("myprog")
    ///     .styles(STYLES)
    ///     .get_matches();
    /// ```
    #[cfg(feature = "color")]
    #[inline]
    #[must_use]
    pub fn styles(mut self, styles: Styles) -> Self {
        self.app_ext.set(styles);
        self
    }

    /// Sets the terminal width at which to wrap help messages.
    ///
    /// Using `0` will ignore terminal widths and use source formatting.
    ///
    /// Defaults to current terminal width when `wrap_help` feature flag is enabled.  If current
    /// width cannot be determined, the default is 100.
    ///
    /// **`unstable-v5` feature**: Defaults to unbound, being subject to
    /// [`Command::max_term_width`].
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This setting applies globally and *not* on a per-command basis.
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This requires the `wrap_help` feature
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// Command::new("myprog")
    ///     .term_width(80)
    /// # ;
    /// ```
    #[inline]
    #[must_use]
    #[cfg(any(not(feature = "unstable-v5"), feature = "wrap_help"))]
    pub fn term_width(mut self, width: usize) -> Self {
        self.app_ext.set(TermWidth(width));
        self
    }

    /// Limit the line length for wrapping help when using the current terminal's width.
    ///
    /// This only applies when [`term_width`][Command::term_width] is unset so that the current
    /// terminal's width will be used.  See [`Command::term_width`] for more details.
    ///
    /// Using `0` will ignore this, always respecting [`Command::term_width`] (default).
    ///
    /// **`unstable-v5` feature**: Defaults to 100.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This setting applies globally and *not* on a per-command basis.
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This requires the `wrap_help` feature
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// Command::new("myprog")
    ///     .max_term_width(100)
    /// # ;
    /// ```
    #[inline]
    #[must_use]
    #[cfg(any(not(feature = "unstable-v5"), feature = "wrap_help"))]
    pub fn max_term_width(mut self, width: usize) -> Self {
        self.app_ext.set(MaxTermWidth(width));
        self
    }

    /// Disables `-V` and `--version` flag.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, error::ErrorKind};
    /// let res = Command::new("myprog")
    ///     .version("1.0.0")
    ///     .disable_version_flag(true)
    ///     .try_get_matches_from(vec![
    ///         "myprog", "--version"
    ///     ]);
    /// assert!(res.is_err());
    /// assert_eq!(res.unwrap_err().kind(), ErrorKind::UnknownArgument);
    /// ```
    ///
    /// You can create a custom version flag with [`ArgAction::Version`]
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgAction, error::ErrorKind};
    /// let mut cmd = Command::new("myprog")
    ///     .version("1.0.0")
    ///     // Remove the `-V` short flag
    ///     .disable_version_flag(true)
    ///     .arg(
    ///         Arg::new("version")
    ///             .long("version")
    ///             .action(ArgAction::Version)
    ///             .help("Print version")
    ///     );
    ///
    /// let res = cmd.try_get_matches_from_mut(vec![
    ///         "myprog", "-V"
    ///     ]);
    /// assert!(res.is_err());
    /// assert_eq!(res.unwrap_err().kind(), ErrorKind::UnknownArgument);
    ///
    /// let res = cmd.try_get_matches_from_mut(vec![
    ///         "myprog", "--version"
    ///     ]);
    /// assert!(res.is_err());
    /// assert_eq!(res.unwrap_err().kind(), ErrorKind::DisplayVersion);
    /// ```
    #[inline]
    pub fn disable_version_flag(self, yes: bool) -> Self {
        if yes {
            self.global_setting(AppSettings::DisableVersionFlag)
        } else {
            self.unset_global_setting(AppSettings::DisableVersionFlag)
        }
    }

    /// Specifies to use the version of the current command for all [`subcommands`].
    ///
    /// Defaults to `false`; subcommands have independent version strings from their parents.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This choice is propagated to all child subcommands.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// Command::new("myprog")
    ///     .version("v1.1")
    ///     .propagate_version(true)
    ///     .subcommand(Command::new("test"))
    ///     .get_matches();
    /// // running `$ myprog test --version` will display
    /// // "myprog-test v1.1"
    /// ```
    ///
    /// [`subcommands`]: crate::Command::subcommand()
    #[inline]
    pub fn propagate_version(self, yes: bool) -> Self {
        if yes {
            self.global_setting(AppSettings::PropagateVersion)
        } else {
            self.unset_global_setting(AppSettings::PropagateVersion)
        }
    }

    /// Places the help string for all arguments and subcommands on the line after them.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This choice is propagated to all child subcommands.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// Command::new("myprog")
    ///     .next_line_help(true)
    ///     .get_matches();
    /// ```
    #[inline]
    pub fn next_line_help(self, yes: bool) -> Self {
        if yes {
            self.global_setting(AppSettings::NextLineHelp)
        } else {
            self.unset_global_setting(AppSettings::NextLineHelp)
        }
    }

    /// Disables `-h` and `--help` flag.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This choice is propagated to all child subcommands.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, error::ErrorKind};
    /// let res = Command::new("myprog")
    ///     .disable_help_flag(true)
    ///     .try_get_matches_from(vec![
    ///         "myprog", "-h"
    ///     ]);
    /// assert!(res.is_err());
    /// assert_eq!(res.unwrap_err().kind(), ErrorKind::UnknownArgument);
    /// ```
    ///
    /// You can create a custom help flag with [`ArgAction::Help`], [`ArgAction::HelpShort`], or
    /// [`ArgAction::HelpLong`]
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgAction, error::ErrorKind};
    /// let mut cmd = Command::new("myprog")
    ///     // Change help short flag to `?`
    ///     .disable_help_flag(true)
    ///     .arg(
    ///         Arg::new("help")
    ///             .short('?')
    ///             .long("help")
    ///             .action(ArgAction::Help)
    ///             .help("Print help")
    ///     );
    ///
    /// let res = cmd.try_get_matches_from_mut(vec![
    ///         "myprog", "-h"
    ///     ]);
    /// assert!(res.is_err());
    /// assert_eq!(res.unwrap_err().kind(), ErrorKind::UnknownArgument);
    ///
    /// let res = cmd.try_get_matches_from_mut(vec![
    ///         "myprog", "-?"
    ///     ]);
    /// assert!(res.is_err());
    /// assert_eq!(res.unwrap_err().kind(), ErrorKind::DisplayHelp);
    /// ```
    #[inline]
    pub fn disable_help_flag(self, yes: bool) -> Self {
        if yes {
            self.global_setting(AppSettings::DisableHelpFlag)
        } else {
            self.unset_global_setting(AppSettings::DisableHelpFlag)
        }
    }

    /// Disables the `help` [`subcommand`].
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This choice is propagated to all child subcommands.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, error::ErrorKind};
    /// let res = Command::new("myprog")
    ///     .disable_help_subcommand(true)
    ///     // Normally, creating a subcommand causes a `help` subcommand to automatically
    ///     // be generated as well
    ///     .subcommand(Command::new("test"))
    ///     .try_get_matches_from(vec![
    ///         "myprog", "help"
    ///     ]);
    /// assert!(res.is_err());
    /// assert_eq!(res.unwrap_err().kind(), ErrorKind::InvalidSubcommand);
    /// ```
    ///
    /// [`subcommand`]: crate::Command::subcommand()
    #[inline]
    pub fn disable_help_subcommand(self, yes: bool) -> Self {
        if yes {
            self.global_setting(AppSettings::DisableHelpSubcommand)
        } else {
            self.unset_global_setting(AppSettings::DisableHelpSubcommand)
        }
    }

    /// Disables colorized help messages.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This choice is propagated to all child subcommands.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// Command::new("myprog")
    ///     .disable_colored_help(true)
    ///     .get_matches();
    /// ```
    #[inline]
    pub fn disable_colored_help(self, yes: bool) -> Self {
        if yes {
            self.global_setting(AppSettings::DisableColoredHelp)
        } else {
            self.unset_global_setting(AppSettings::DisableColoredHelp)
        }
    }

    /// Panic if help descriptions are omitted.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** When deriving [`Parser`][crate::Parser], you could instead check this at
    /// compile-time with `#![deny(missing_docs)]`
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This choice is propagated to all child subcommands.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// Command::new("myprog")
    ///     .help_expected(true)
    ///     .arg(
    ///         Arg::new("foo").help("It does foo stuff")
    ///         // As required via `help_expected`, a help message was supplied
    ///      )
    /// #    .get_matches();
    /// ```
    ///
    /// # Panics
    ///
    /// On debug builds:
    /// ```rust,no_run
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// Command::new("myapp")
    ///     .help_expected(true)
    ///     .arg(
    ///         Arg::new("foo")
    ///         // Someone forgot to put .about("...") here
    ///         // Since the setting `help_expected` is activated, this will lead to
    ///         // a panic (if you are in debug mode)
    ///     )
    /// #   .get_matches();
    ///```
    #[inline]
    pub fn help_expected(self, yes: bool) -> Self {
        if yes {
            self.global_setting(AppSettings::HelpExpected)
        } else {
            self.unset_global_setting(AppSettings::HelpExpected)
        }
    }

    #[doc(hidden)]
    #[cfg_attr(
        feature = "deprecated",
        deprecated(since = "4.0.0", note = "This is now the default")
    )]
    pub fn dont_collapse_args_in_usage(self, _yes: bool) -> Self {
        self
    }

    /// Tells `clap` *not* to print possible values when displaying help information.
    ///
    /// This can be useful if there are many values, or they are explained elsewhere.
    ///
    /// To set this per argument, see
    /// [`Arg::hide_possible_values`][crate::Arg::hide_possible_values].
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This choice is propagated to all child subcommands.
    ///
    /// </div>
    #[inline]
    pub fn hide_possible_values(self, yes: bool) -> Self {
        if yes {
            self.global_setting(AppSettings::HidePossibleValues)
        } else {
            self.unset_global_setting(AppSettings::HidePossibleValues)
        }
    }

    /// Allow partial matches of long arguments or their [aliases].
    ///
    /// For example, to match an argument named `--test`, one could use `--t`, `--te`, `--tes`, and
    /// `--test`.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** The match *must not* be ambiguous at all in order to succeed. i.e. to match
    /// `--te` to `--test` there could not also be another argument or alias `--temp` because both
    /// start with `--te`
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This choice is propagated to all child subcommands.
    ///
    /// </div>
    ///
    /// [aliases]: crate::Command::aliases()
    #[inline]
    pub fn infer_long_args(self, yes: bool) -> Self {
        if yes {
            self.global_setting(AppSettings::InferLongArgs)
        } else {
            self.unset_global_setting(AppSettings::InferLongArgs)
        }
    }

    /// Allow partial matches of [subcommand] names and their [aliases].
    ///
    /// For example, to match a subcommand named `test`, one could use `t`, `te`, `tes`, and
    /// `test`.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** The match *must not* be ambiguous at all in order to succeed. i.e. to match `te`
    /// to `test` there could not also be a subcommand or alias `temp` because both start with `te`
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// **WARNING:** This setting can interfere with [positional/free arguments], take care when
    /// designing CLIs which allow inferred subcommands and have potential positional/free
    /// arguments whose values could start with the same characters as subcommands. If this is the
    /// case, it's recommended to use settings such as [`Command::args_conflicts_with_subcommands`] in
    /// conjunction with this setting.
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This choice is propagated to all child subcommands.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// let m = Command::new("prog")
    ///     .infer_subcommands(true)
    ///     .subcommand(Command::new("test"))
    ///     .get_matches_from(vec![
    ///         "prog", "te"
    ///     ]);
    /// assert_eq!(m.subcommand_name(), Some("test"));
    /// ```
    ///
    /// [subcommand]: crate::Command::subcommand()
    /// [positional/free arguments]: crate::Arg::index()
    /// [aliases]: crate::Command::aliases()
    #[inline]
    pub fn infer_subcommands(self, yes: bool) -> Self {
        if yes {
            self.global_setting(AppSettings::InferSubcommands)
        } else {
            self.unset_global_setting(AppSettings::InferSubcommands)
        }
    }
}

/// # Command-specific Settings
///
/// These apply only to the current command and are not inherited by subcommands.
impl Command {
    /// (Re)Sets the program's name.
    ///
    /// See [`Command::new`] for more details.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let cmd = clap::command!()
    ///     .name("foo");
    ///
    /// // continued logic goes here, such as `cmd.get_matches()` etc.
    /// ```
    #[must_use]
    pub fn name(mut self, name: impl Into<Str>) -> Self {
        self.name = name.into();
        self
    }

    /// Overrides the runtime-determined name of the binary for help and error messages.
    ///
    /// This should only be used when absolutely necessary, such as when the binary name for your
    /// application is misleading, or perhaps *not* how the user should invoke your program.
    ///
    /// <div class="warning">
    ///
    /// **TIP:** When building things such as third party `cargo`
    /// subcommands, this setting **should** be used!
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This *does not* change or set the name of the binary file on
    /// disk. It only changes what clap thinks the name is for the purposes of
    /// error or help messages.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// Command::new("My Program")
    ///      .bin_name("my_binary")
    /// # ;
    /// ```
    #[must_use]
    pub fn bin_name(mut self, name: impl IntoResettable<String>) -> Self {
        self.bin_name = name.into_resettable().into_option();
        self
    }

    /// Overrides the runtime-determined display name of the program for help and error messages.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// Command::new("My Program")
    ///      .display_name("my_program")
    /// # ;
    /// ```
    #[must_use]
    pub fn display_name(mut self, name: impl IntoResettable<String>) -> Self {
        self.display_name = name.into_resettable().into_option();
        self
    }

    /// Sets the author(s) for the help message.
    ///
    /// <div class="warning">
    ///
    /// **TIP:** Use `clap`s convenience macro [`crate_authors!`] to
    /// automatically set your application's author(s) to the same thing as your
    /// crate at compile time.
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** A custom [`help_template`][Command::help_template] is needed for author to show
    /// up.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// Command::new("myprog")
    ///      .author("Me, me@mymain.com")
    /// # ;
    /// ```
    #[must_use]
    pub fn author(mut self, author: impl IntoResettable<Str>) -> Self {
        self.author = author.into_resettable().into_option();
        self
    }

    /// Sets the program's description for the short help (`-h`).
    ///
    /// If [`Command::long_about`] is not specified, this message will be displayed for `--help`.
    ///
    /// See also [`crate_description!`](crate::crate_description!).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// Command::new("myprog")
    ///     .about("Does really amazing things for great people")
    /// # ;
    /// ```
    #[must_use]
    pub fn about(mut self, about: impl IntoResettable<StyledStr>) -> Self {
        self.about = about.into_resettable().into_option();
        self
    }

    /// Sets the program's description for the long help (`--help`).
    ///
    /// If not set, [`Command::about`] will be used for long help in addition to short help
    /// (`-h`).
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** Only [`Command::about`] (short format) is used in completion
    /// script generation in order to be concise.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// Command::new("myprog")
    ///     .long_about(
    /// "Does really amazing things to great people. Now let's talk a little
    ///  more in depth about how this subcommand really works. It may take about
    ///  a few lines of text, but that's ok!")
    /// # ;
    /// ```
    /// [`Command::about`]: Command::about()
    #[must_use]
    pub fn long_about(mut self, long_about: impl IntoResettable<StyledStr>) -> Self {
        self.long_about = long_about.into_resettable().into_option();
        self
    }

    /// Free-form help text for after auto-generated short help (`-h`).
    ///
    /// This is often used to describe how to use the arguments, caveats to be noted, or license
    /// and contact information.
    ///
    /// If [`Command::after_long_help`] is not specified, this message will be displayed for `--help`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// Command::new("myprog")
    ///     .after_help("Does really amazing things for great people... but be careful with -R!")
    /// # ;
    /// ```
    ///
    #[must_use]
    pub fn after_help(mut self, help: impl IntoResettable<StyledStr>) -> Self {
        self.after_help = help.into_resettable().into_option();
        self
    }

    /// Free-form help text for after auto-generated long help (`--help`).
    ///
    /// This is often used to describe how to use the arguments, caveats to be noted, or license
    /// and contact information.
    ///
    /// If not set, [`Command::after_help`] will be used for long help in addition to short help
    /// (`-h`).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// Command::new("myprog")
    ///     .after_long_help("Does really amazing things to great people... but be careful with -R, \
    ///                      like, for real, be careful with this!")
    /// # ;
    /// ```
    #[must_use]
    pub fn after_long_help(mut self, help: impl IntoResettable<StyledStr>) -> Self {
        self.after_long_help = help.into_resettable().into_option();
        self
    }

    /// Free-form help text for before auto-generated short help (`-h`).
    ///
    /// This is often used for header, copyright, or license information.
    ///
    /// If [`Command::before_long_help`] is not specified, this message will be displayed for `--help`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// Command::new("myprog")
    ///     .before_help("Some info I'd like to appear before the help info")
    /// # ;
    /// ```
    #[must_use]
    pub fn before_help(mut self, help: impl IntoResettable<StyledStr>) -> Self {
        self.before_help = help.into_resettable().into_option();
        self
    }

    /// Free-form help text for before auto-generated long help (`--help`).
    ///
    /// This is often used for header, copyright, or license information.
    ///
    /// If not set, [`Command::before_help`] will be used for long help in addition to short help
    /// (`-h`).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// Command::new("myprog")
    ///     .before_long_help("Some verbose and long info I'd like to appear before the help info")
    /// # ;
    /// ```
    #[must_use]
    pub fn before_long_help(mut self, help: impl IntoResettable<StyledStr>) -> Self {
        self.before_long_help = help.into_resettable().into_option();
        self
    }

    /// Sets the version for the short version (`-V`) and help messages.
    ///
    /// If [`Command::long_version`] is not specified, this message will be displayed for `--version`.
    ///
    /// <div class="warning">
    ///
    /// **TIP:** Use `clap`s convenience macro [`crate_version!`] to
    /// automatically set your application's version to the same thing as your
    /// crate at compile time.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// Command::new("myprog")
    ///     .version("v0.1.24")
    /// # ;
    /// ```
    #[must_use]
    pub fn version(mut self, ver: impl IntoResettable<Str>) -> Self {
        self.version = ver.into_resettable().into_option();
        self
    }

    /// Sets the version for the long version (`--version`) and help messages.
    ///
    /// If [`Command::version`] is not specified, this message will be displayed for `-V`.
    ///
    /// <div class="warning">
    ///
    /// **TIP:** Use `clap`s convenience macro [`crate_version!`] to
    /// automatically set your application's version to the same thing as your
    /// crate at compile time.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// Command::new("myprog")
    ///     .long_version(
    /// "v0.1.24
    ///  commit: abcdef89726d
    ///  revision: 123
    ///  release: 2
    ///  binary: myprog")
    /// # ;
    /// ```
    #[must_use]
    pub fn long_version(mut self, ver: impl IntoResettable<Str>) -> Self {
        self.long_version = ver.into_resettable().into_option();
        self
    }

    /// Overrides the `clap` generated usage string for help and error messages.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** Using this setting disables `clap`s "context-aware" usage
    /// strings. After this setting is set, this will be *the only* usage string
    /// displayed to the user!
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** Multiple usage lines may be present in the usage argument, but
    /// some rules need to be followed to ensure the usage lines are formatted
    /// correctly by the default help formatter:
    ///
    /// - Do not indent the first usage line.
    /// - Indent all subsequent usage lines with seven spaces.
    /// - The last line must not end with a newline.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// Command::new("myprog")
    ///     .override_usage("myapp [-clDas] <some_file>")
    /// # ;
    /// ```
    ///
    /// Or for multiple usage lines:
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// Command::new("myprog")
    ///     .override_usage(
    ///         "myapp -X [-a] [-b] <file>\n       \
    ///          myapp -Y [-c] <file1> <file2>\n       \
    ///          myapp -Z [-d|-e]"
    ///     )
    /// # ;
    /// ```
    #[must_use]
    pub fn override_usage(mut self, usage: impl IntoResettable<StyledStr>) -> Self {
        self.usage_str = usage.into_resettable().into_option();
        self
    }

    /// Overrides the `clap` generated help message (both `-h` and `--help`).
    ///
    /// This should only be used when the auto-generated message does not suffice.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This **only** replaces the help message for the current
    /// command, meaning if you are using subcommands, those help messages will
    /// still be auto-generated unless you specify a [`Command::override_help`] for
    /// them as well.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// Command::new("myapp")
    ///     .override_help("myapp v1.0\n\
    ///            Does awesome things\n\
    ///            (C) me@mail.com\n\n\
    ///
    ///            Usage: myapp <opts> <command>\n\n\
    ///
    ///            Options:\n\
    ///            -h, --help       Display this message\n\
    ///            -V, --version    Display version info\n\
    ///            -s <stuff>       Do something with stuff\n\
    ///            -v               Be verbose\n\n\
    ///
    ///            Commands:\n\
    ///            help             Print this message\n\
    ///            work             Do some work")
    /// # ;
    /// ```
    #[must_use]
    pub fn override_help(mut self, help: impl IntoResettable<StyledStr>) -> Self {
        self.help_str = help.into_resettable().into_option();
        self
    }

    /// Sets the help template to be used, overriding the default format.
    ///
    /// Tags are given inside curly brackets.
    ///
    /// Valid tags are:
    ///
    ///   * `{name}`                - Display name for the (sub-)command.
    ///   * `{bin}`                 - Binary name.(deprecated)
    ///   * `{version}`             - Version number.
    ///   * `{author}`              - Author information.
    ///   * `{author-with-newline}` - Author followed by `\n`.
    ///   * `{author-section}`      - Author preceded and followed by `\n`.
    ///   * `{about}`               - General description (from [`Command::about`] or
    ///     [`Command::long_about`]).
    ///   * `{about-with-newline}`  - About followed by `\n`.
    ///   * `{about-section}`       - About preceded and followed by '\n'.
    ///   * `{usage-heading}`       - Automatically generated usage heading.
    ///   * `{usage}`               - Automatically generated or given usage string.
    ///   * `{all-args}`            - Help for all arguments (options, flags, positional
    ///     arguments, and subcommands) including titles.
    ///   * `{options}`             - Help for options.
    ///   * `{positionals}`         - Help for positional arguments.
    ///   * `{subcommands}`         - Help for subcommands.
    ///   * `{tab}`                 - Standard tab sized used within clap
    ///   * `{after-help}`          - Help from [`Command::after_help`] or [`Command::after_long_help`].
    ///   * `{before-help}`         - Help from [`Command::before_help`] or [`Command::before_long_help`].
    ///
    /// # Examples
    ///
    /// For a very brief help:
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// Command::new("myprog")
    ///     .version("1.0")
    ///     .help_template("{name} ({version}) - {usage}")
    /// # ;
    /// ```
    ///
    /// For showing more application context:
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// Command::new("myprog")
    ///     .version("1.0")
    ///     .help_template("\
    /// {before-help}{name} {version}
    /// {author-with-newline}{about-with-newline}
    /// {usage-heading} {usage}
    ///
    /// {all-args}{after-help}
    /// ")
    /// # ;
    /// ```
    /// [`Command::about`]: Command::about()
    /// [`Command::long_about`]: Command::long_about()
    /// [`Command::after_help`]: Command::after_help()
    /// [`Command::after_long_help`]: Command::after_long_help()
    /// [`Command::before_help`]: Command::before_help()
    /// [`Command::before_long_help`]: Command::before_long_help()
    #[must_use]
    #[cfg(feature = "help")]
    pub fn help_template(mut self, s: impl IntoResettable<StyledStr>) -> Self {
        self.template = s.into_resettable().into_option();
        self
    }

    #[inline]
    #[must_use]
    pub(crate) fn setting(mut self, setting: AppSettings) -> Self {
        self.settings.set(setting);
        self
    }

    #[inline]
    #[must_use]
    pub(crate) fn unset_setting(mut self, setting: AppSettings) -> Self {
        self.settings.unset(setting);
        self
    }

    #[inline]
    #[must_use]
    pub(crate) fn global_setting(mut self, setting: AppSettings) -> Self {
        self.settings.set(setting);
        self.g_settings.set(setting);
        self
    }

    #[inline]
    #[must_use]
    pub(crate) fn unset_global_setting(mut self, setting: AppSettings) -> Self {
        self.settings.unset(setting);
        self.g_settings.unset(setting);
        self
    }

    /// Flatten subcommand help into the current command's help
    ///
    /// This shows a summary of subcommands within the usage and help for the current command, similar to
    /// `git stash --help` showing information on `push`, `pop`, etc.
    /// To see more information, a user can still pass `--help` to the individual subcommands.
    #[inline]
    #[must_use]
    pub fn flatten_help(self, yes: bool) -> Self {
        if yes {
            self.setting(AppSettings::FlattenHelp)
        } else {
            self.unset_setting(AppSettings::FlattenHelp)
        }
    }

    /// Set the default section heading for future args.
    ///
    /// This will be used for any arg that hasn't had [`Arg::help_heading`] called.
    ///
    /// This is useful if the default `Options` or `Arguments` headings are
    /// not specific enough for one's use case.
    ///
    /// For subcommands, see [`Command::subcommand_help_heading`]
    ///
    /// [`Command::arg`]: Command::arg()
    /// [`Arg::help_heading`]: crate::Arg::help_heading()
    #[inline]
    #[must_use]
    pub fn next_help_heading(mut self, heading: impl IntoResettable<Str>) -> Self {
        self.current_help_heading = heading.into_resettable().into_option();
        self
    }

    /// Change the starting value for assigning future display orders for args.
    ///
    /// This will be used for any arg that hasn't had [`Arg::display_order`] called.
    #[inline]
    #[must_use]
    pub fn next_display_order(mut self, disp_ord: impl IntoResettable<usize>) -> Self {
        self.current_disp_ord = disp_ord.into_resettable().into_option();
        self
    }

    /// Exit gracefully if no arguments are present (e.g. `$ myprog`).
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** [`subcommands`] count as arguments
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command};
    /// Command::new("myprog")
    ///     .arg_required_else_help(true);
    /// ```
    ///
    /// [`subcommands`]: crate::Command::subcommand()
    /// [`Arg::default_value`]: crate::Arg::default_value()
    #[inline]
    pub fn arg_required_else_help(self, yes: bool) -> Self {
        if yes {
            self.setting(AppSettings::ArgRequiredElseHelp)
        } else {
            self.unset_setting(AppSettings::ArgRequiredElseHelp)
        }
    }

    #[doc(hidden)]
    #[cfg_attr(
        feature = "deprecated",
        deprecated(since = "4.0.0", note = "Replaced with `Arg::allow_hyphen_values`")
    )]
    pub fn allow_hyphen_values(self, yes: bool) -> Self {
        if yes {
            self.setting(AppSettings::AllowHyphenValues)
        } else {
            self.unset_setting(AppSettings::AllowHyphenValues)
        }
    }

    #[doc(hidden)]
    #[cfg_attr(
        feature = "deprecated",
        deprecated(since = "4.0.0", note = "Replaced with `Arg::allow_negative_numbers`")
    )]
    pub fn allow_negative_numbers(self, yes: bool) -> Self {
        if yes {
            self.setting(AppSettings::AllowNegativeNumbers)
        } else {
            self.unset_setting(AppSettings::AllowNegativeNumbers)
        }
    }

    #[doc(hidden)]
    #[cfg_attr(
        feature = "deprecated",
        deprecated(since = "4.0.0", note = "Replaced with `Arg::trailing_var_arg`")
    )]
    pub fn trailing_var_arg(self, yes: bool) -> Self {
        if yes {
            self.setting(AppSettings::TrailingVarArg)
        } else {
            self.unset_setting(AppSettings::TrailingVarArg)
        }
    }

    /// Allows one to implement two styles of CLIs where positionals can be used out of order.
    ///
    /// The first example is a CLI where the second to last positional argument is optional, but
    /// the final positional argument is required. Such as `$ prog [optional] <required>` where one
    /// of the two following usages is allowed:
    ///
    /// * `$ prog [optional] <required>`
    /// * `$ prog <required>`
    ///
    /// This would otherwise not be allowed. This is useful when `[optional]` has a default value.
    ///
    /// **Note:** when using this style of "missing positionals" the final positional *must* be
    /// [required] if `--` will not be used to skip to the final positional argument.
    ///
    /// **Note:** This style also only allows a single positional argument to be "skipped" without
    /// the use of `--`. To skip more than one, see the second example.
    ///
    /// The second example is when one wants to skip multiple optional positional arguments, and use
    /// of the `--` operator is OK (but not required if all arguments will be specified anyways).
    ///
    /// For example, imagine a CLI which has three positional arguments `[foo] [bar] [baz]...` where
    /// `baz` accepts multiple values (similar to man `ARGS...` style training arguments).
    ///
    /// With this setting the following invocations are possible:
    ///
    /// * `$ prog foo bar baz1 baz2 baz3`
    /// * `$ prog foo -- baz1 baz2 baz3`
    /// * `$ prog -- baz1 baz2 baz3`
    ///
    /// # Examples
    ///
    /// Style number one from above:
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// // Assume there is an external subcommand named "subcmd"
    /// let m = Command::new("myprog")
    ///     .allow_missing_positional(true)
    ///     .arg(Arg::new("arg1"))
    ///     .arg(Arg::new("arg2")
    ///         .required(true))
    ///     .get_matches_from(vec![
    ///         "prog", "other"
    ///     ]);
    ///
    /// assert_eq!(m.get_one::<String>("arg1"), None);
    /// assert_eq!(m.get_one::<String>("arg2").unwrap(), "other");
    /// ```
    ///
    /// Now the same example, but using a default value for the first optional positional argument
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// // Assume there is an external subcommand named "subcmd"
    /// let m = Command::new("myprog")
    ///     .allow_missing_positional(true)
    ///     .arg(Arg::new("arg1")
    ///         .default_value("something"))
    ///     .arg(Arg::new("arg2")
    ///         .required(true))
    ///     .get_matches_from(vec![
    ///         "prog", "other"
    ///     ]);
    ///
    /// assert_eq!(m.get_one::<String>("arg1").unwrap(), "something");
    /// assert_eq!(m.get_one::<String>("arg2").unwrap(), "other");
    /// ```
    ///
    /// Style number two from above:
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgAction};
    /// // Assume there is an external subcommand named "subcmd"
    /// let m = Command::new("myprog")
    ///     .allow_missing_positional(true)
    ///     .arg(Arg::new("foo"))
    ///     .arg(Arg::new("bar"))
    ///     .arg(Arg::new("baz").action(ArgAction::Set).num_args(1..))
    ///     .get_matches_from(vec![
    ///         "prog", "foo", "bar", "baz1", "baz2", "baz3"
    ///     ]);
    ///
    /// assert_eq!(m.get_one::<String>("foo").unwrap(), "foo");
    /// assert_eq!(m.get_one::<String>("bar").unwrap(), "bar");
    /// assert_eq!(m.get_many::<String>("baz").unwrap().collect::<Vec<_>>(), &["baz1", "baz2", "baz3"]);
    /// ```
    ///
    /// Now nofice if we don't specify `foo` or `baz` but use the `--` operator.
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgAction};
    /// // Assume there is an external subcommand named "subcmd"
    /// let m = Command::new("myprog")
    ///     .allow_missing_positional(true)
    ///     .arg(Arg::new("foo"))
    ///     .arg(Arg::new("bar"))
    ///     .arg(Arg::new("baz").action(ArgAction::Set).num_args(1..))
    ///     .get_matches_from(vec![
    ///         "prog", "--", "baz1", "baz2", "baz3"
    ///     ]);
    ///
    /// assert_eq!(m.get_one::<String>("foo"), None);
    /// assert_eq!(m.get_one::<String>("bar"), None);
    /// assert_eq!(m.get_many::<String>("baz").unwrap().collect::<Vec<_>>(), &["baz1", "baz2", "baz3"]);
    /// ```
    ///
    /// [required]: crate::Arg::required()
    #[inline]
    pub fn allow_missing_positional(self, yes: bool) -> Self {
        if yes {
            self.setting(AppSettings::AllowMissingPositional)
        } else {
            self.unset_setting(AppSettings::AllowMissingPositional)
        }
    }
}

/// # Subcommand-specific Settings
impl Command {
    /// Sets the short version of the subcommand flag without the preceding `-`.
    ///
    /// Allows the subcommand to be used as if it were an [`Arg::short`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgAction};
    /// let matches = Command::new("pacman")
    ///     .subcommand(
    ///         Command::new("sync").short_flag('S').arg(
    ///             Arg::new("search")
    ///                 .short('s')
    ///                 .long("search")
    ///                 .action(ArgAction::SetTrue)
    ///                 .help("search remote repositories for matching strings"),
    ///         ),
    ///     )
    ///     .get_matches_from(vec!["pacman", "-Ss"]);
    ///
    /// assert_eq!(matches.subcommand_name().unwrap(), "sync");
    /// let sync_matches = matches.subcommand_matches("sync").unwrap();
    /// assert!(sync_matches.get_flag("search"));
    /// ```
    /// [`Arg::short`]: Arg::short()
    #[must_use]
    pub fn short_flag(mut self, short: impl IntoResettable<char>) -> Self {
        self.short_flag = short.into_resettable().into_option();
        self
    }

    /// Sets the long version of the subcommand flag without the preceding `--`.
    ///
    /// Allows the subcommand to be used as if it were an [`Arg::long`].
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** Any leading `-` characters will be stripped.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// To set `long_flag` use a word containing valid UTF-8 codepoints. If you supply a double leading
    /// `--` such as `--sync` they will be stripped. Hyphens in the middle of the word; however,
    /// will *not* be stripped (i.e. `sync-file` is allowed).
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgAction};
    /// let matches = Command::new("pacman")
    ///     .subcommand(
    ///         Command::new("sync").long_flag("sync").arg(
    ///             Arg::new("search")
    ///                 .short('s')
    ///                 .long("search")
    ///                 .action(ArgAction::SetTrue)
    ///                 .help("search remote repositories for matching strings"),
    ///         ),
    ///     )
    ///     .get_matches_from(vec!["pacman", "--sync", "--search"]);
    ///
    /// assert_eq!(matches.subcommand_name().unwrap(), "sync");
    /// let sync_matches = matches.subcommand_matches("sync").unwrap();
    /// assert!(sync_matches.get_flag("search"));
    /// ```
    ///
    /// [`Arg::long`]: Arg::long()
    #[must_use]
    pub fn long_flag(mut self, long: impl Into<Str>) -> Self {
        self.long_flag = Some(long.into());
        self
    }

    /// Sets a hidden alias to this subcommand.
    ///
    /// This allows the subcommand to be accessed via *either* the original name, or this given
    /// alias. This is more efficient and easier than creating multiple hidden subcommands as one
    /// only needs to check for the existence of this command, and not all aliased variants.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** Aliases defined with this method are *hidden* from the help
    /// message. If you're looking for aliases that will be displayed in the help
    /// message, see [`Command::visible_alias`].
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** When using aliases and checking for the existence of a
    /// particular subcommand within an [`ArgMatches`] struct, one only needs to
    /// search for the original name and not all aliases.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, };
    /// let m = Command::new("myprog")
    ///     .subcommand(Command::new("test")
    ///         .alias("do-stuff"))
    ///     .get_matches_from(vec!["myprog", "do-stuff"]);
    /// assert_eq!(m.subcommand_name(), Some("test"));
    /// ```
    /// [`Command::visible_alias`]: Command::visible_alias()
    #[must_use]
    pub fn alias(mut self, name: impl IntoResettable<Str>) -> Self {
        if let Some(name) = name.into_resettable().into_option() {
            self.aliases.push((name, false));
        } else {
            self.aliases.clear();
        }
        self
    }

    /// Add an alias, which functions as  "hidden" short flag subcommand
    ///
    /// This will automatically dispatch as if this subcommand was used. This is more efficient,
    /// and easier than creating multiple hidden subcommands as one only needs to check for the
    /// existence of this command, and not all variants.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, };
    /// let m = Command::new("myprog")
    ///             .subcommand(Command::new("test").short_flag('t')
    ///                 .short_flag_alias('d'))
    ///             .get_matches_from(vec!["myprog", "-d"]);
    /// assert_eq!(m.subcommand_name(), Some("test"));
    /// ```
    #[must_use]
    pub fn short_flag_alias(mut self, name: impl IntoResettable<char>) -> Self {
        if let Some(name) = name.into_resettable().into_option() {
            debug_assert!(name != '-', "short alias name cannot be `-`");
            self.short_flag_aliases.push((name, false));
        } else {
            self.short_flag_aliases.clear();
        }
        self
    }

    /// Add an alias, which functions as a "hidden" long flag subcommand.
    ///
    /// This will automatically dispatch as if this subcommand was used. This is more efficient,
    /// and easier than creating multiple hidden subcommands as one only needs to check for the
    /// existence of this command, and not all variants.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, };
    /// let m = Command::new("myprog")
    ///             .subcommand(Command::new("test").long_flag("test")
    ///                 .long_flag_alias("testing"))
    ///             .get_matches_from(vec!["myprog", "--testing"]);
    /// assert_eq!(m.subcommand_name(), Some("test"));
    /// ```
    #[must_use]
    pub fn long_flag_alias(mut self, name: impl IntoResettable<Str>) -> Self {
        if let Some(name) = name.into_resettable().into_option() {
            self.long_flag_aliases.push((name, false));
        } else {
            self.long_flag_aliases.clear();
        }
        self
    }

    /// Sets multiple hidden aliases to this subcommand.
    ///
    /// This allows the subcommand to be accessed via *either* the original name or any of the
    /// given aliases. This is more efficient, and easier than creating multiple hidden subcommands
    /// as one only needs to check for the existence of this command and not all aliased variants.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** Aliases defined with this method are *hidden* from the help
    /// message. If looking for aliases that will be displayed in the help
    /// message, see [`Command::visible_aliases`].
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** When using aliases and checking for the existence of a
    /// particular subcommand within an [`ArgMatches`] struct, one only needs to
    /// search for the original name and not all aliases.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// let m = Command::new("myprog")
    ///     .subcommand(Command::new("test")
    ///         .aliases(["do-stuff", "do-tests", "tests"]))
    ///         .arg(Arg::new("input")
    ///             .help("the file to add")
    ///             .required(false))
    ///     .get_matches_from(vec!["myprog", "do-tests"]);
    /// assert_eq!(m.subcommand_name(), Some("test"));
    /// ```
    /// [`Command::visible_aliases`]: Command::visible_aliases()
    #[must_use]
    pub fn aliases(mut self, names: impl IntoIterator<Item = impl Into<Str>>) -> Self {
        self.aliases
            .extend(names.into_iter().map(|n| (n.into(), false)));
        self
    }

    /// Add aliases, which function as "hidden" short flag subcommands.
    ///
    /// These will automatically dispatch as if this subcommand was used. This is more efficient,
    /// and easier than creating multiple hidden subcommands as one only needs to check for the
    /// existence of this command, and not all variants.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, };
    /// let m = Command::new("myprog")
    ///     .subcommand(Command::new("test").short_flag('t')
    ///         .short_flag_aliases(['a', 'b', 'c']))
    ///         .arg(Arg::new("input")
    ///             .help("the file to add")
    ///             .required(false))
    ///     .get_matches_from(vec!["myprog", "-a"]);
    /// assert_eq!(m.subcommand_name(), Some("test"));
    /// ```
    #[must_use]
    pub fn short_flag_aliases(mut self, names: impl IntoIterator<Item = char>) -> Self {
        for s in names {
            debug_assert!(s != '-', "short alias name cannot be `-`");
            self.short_flag_aliases.push((s, false));
        }
        self
    }

    /// Add aliases, which function as "hidden" long flag subcommands.
    ///
    /// These will automatically dispatch as if this subcommand was used. This is more efficient,
    /// and easier than creating multiple hidden subcommands as one only needs to check for the
    /// existence of this command, and not all variants.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, };
    /// let m = Command::new("myprog")
    ///             .subcommand(Command::new("test").long_flag("test")
    ///                 .long_flag_aliases(["testing", "testall", "test_all"]))
    ///                 .arg(Arg::new("input")
    ///                             .help("the file to add")
    ///                             .required(false))
    ///             .get_matches_from(vec!["myprog", "--testing"]);
    /// assert_eq!(m.subcommand_name(), Some("test"));
    /// ```
    #[must_use]
    pub fn long_flag_aliases(mut self, names: impl IntoIterator<Item = impl Into<Str>>) -> Self {
        for s in names {
            self = self.long_flag_alias(s);
        }
        self
    }

    /// Sets a visible alias to this subcommand.
    ///
    /// This allows the subcommand to be accessed via *either* the
    /// original name or the given alias. This is more efficient and easier
    /// than creating hidden subcommands as one only needs to check for
    /// the existence of this command and not all aliased variants.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** The alias defined with this method is *visible* from the help
    /// message and displayed as if it were just another regular subcommand. If
    /// looking for an alias that will not be displayed in the help message, see
    /// [`Command::alias`].
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** When using aliases and checking for the existence of a
    /// particular subcommand within an [`ArgMatches`] struct, one only needs to
    /// search for the original name and not all aliases.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// let m = Command::new("myprog")
    ///     .subcommand(Command::new("test")
    ///         .visible_alias("do-stuff"))
    ///     .get_matches_from(vec!["myprog", "do-stuff"]);
    /// assert_eq!(m.subcommand_name(), Some("test"));
    /// ```
    /// [`Command::alias`]: Command::alias()
    #[must_use]
    pub fn visible_alias(mut self, name: impl IntoResettable<Str>) -> Self {
        if let Some(name) = name.into_resettable().into_option() {
            self.aliases.push((name, true));
        } else {
            self.aliases.clear();
        }
        self
    }

    /// Add an alias, which functions as  "visible" short flag subcommand
    ///
    /// This will automatically dispatch as if this subcommand was used. This is more efficient,
    /// and easier than creating multiple hidden subcommands as one only needs to check for the
    /// existence of this command, and not all variants.
    ///
    /// See also [`Command::short_flag_alias`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, };
    /// let m = Command::new("myprog")
    ///             .subcommand(Command::new("test").short_flag('t')
    ///                 .visible_short_flag_alias('d'))
    ///             .get_matches_from(vec!["myprog", "-d"]);
    /// assert_eq!(m.subcommand_name(), Some("test"));
    /// ```
    /// [`Command::short_flag_alias`]: Command::short_flag_alias()
    #[must_use]
    pub fn visible_short_flag_alias(mut self, name: impl IntoResettable<char>) -> Self {
        if let Some(name) = name.into_resettable().into_option() {
            debug_assert!(name != '-', "short alias name cannot be `-`");
            self.short_flag_aliases.push((name, true));
        } else {
            self.short_flag_aliases.clear();
        }
        self
    }

    /// Add an alias, which functions as a "visible" long flag subcommand.
    ///
    /// This will automatically dispatch as if this subcommand was used. This is more efficient,
    /// and easier than creating multiple hidden subcommands as one only needs to check for the
    /// existence of this command, and not all variants.
    ///
    /// See also [`Command::long_flag_alias`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, };
    /// let m = Command::new("myprog")
    ///             .subcommand(Command::new("test").long_flag("test")
    ///                 .visible_long_flag_alias("testing"))
    ///             .get_matches_from(vec!["myprog", "--testing"]);
    /// assert_eq!(m.subcommand_name(), Some("test"));
    /// ```
    /// [`Command::long_flag_alias`]: Command::long_flag_alias()
    #[must_use]
    pub fn visible_long_flag_alias(mut self, name: impl IntoResettable<Str>) -> Self {
        if let Some(name) = name.into_resettable().into_option() {
            self.long_flag_aliases.push((name, true));
        } else {
            self.long_flag_aliases.clear();
        }
        self
    }

    /// Sets multiple visible aliases to this subcommand.
    ///
    /// This allows the subcommand to be accessed via *either* the
    /// original name or any of the given aliases. This is more efficient and easier
    /// than creating multiple hidden subcommands as one only needs to check for
    /// the existence of this command and not all aliased variants.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** The alias defined with this method is *visible* from the help
    /// message and displayed as if it were just another regular subcommand. If
    /// looking for an alias that will not be displayed in the help message, see
    /// [`Command::alias`].
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** When using aliases, and checking for the existence of a
    /// particular subcommand within an [`ArgMatches`] struct, one only needs to
    /// search for the original name and not all aliases.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, };
    /// let m = Command::new("myprog")
    ///     .subcommand(Command::new("test")
    ///         .visible_aliases(["do-stuff", "tests"]))
    ///     .get_matches_from(vec!["myprog", "do-stuff"]);
    /// assert_eq!(m.subcommand_name(), Some("test"));
    /// ```
    /// [`Command::alias`]: Command::alias()
    #[must_use]
    pub fn visible_aliases(mut self, names: impl IntoIterator<Item = impl Into<Str>>) -> Self {
        self.aliases
            .extend(names.into_iter().map(|n| (n.into(), true)));
        self
    }

    /// Add aliases, which function as *visible* short flag subcommands.
    ///
    /// See [`Command::short_flag_aliases`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, };
    /// let m = Command::new("myprog")
    ///             .subcommand(Command::new("test").short_flag('b')
    ///                 .visible_short_flag_aliases(['t']))
    ///             .get_matches_from(vec!["myprog", "-t"]);
    /// assert_eq!(m.subcommand_name(), Some("test"));
    /// ```
    /// [`Command::short_flag_aliases`]: Command::short_flag_aliases()
    #[must_use]
    pub fn visible_short_flag_aliases(mut self, names: impl IntoIterator<Item = char>) -> Self {
        for s in names {
            debug_assert!(s != '-', "short alias name cannot be `-`");
            self.short_flag_aliases.push((s, true));
        }
        self
    }

    /// Add aliases, which function as *visible* long flag subcommands.
    ///
    /// See [`Command::long_flag_aliases`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, };
    /// let m = Command::new("myprog")
    ///             .subcommand(Command::new("test").long_flag("test")
    ///                 .visible_long_flag_aliases(["testing", "testall", "test_all"]))
    ///             .get_matches_from(vec!["myprog", "--testing"]);
    /// assert_eq!(m.subcommand_name(), Some("test"));
    /// ```
    /// [`Command::long_flag_aliases`]: Command::long_flag_aliases()
    #[must_use]
    pub fn visible_long_flag_aliases(
        mut self,
        names: impl IntoIterator<Item = impl Into<Str>>,
    ) -> Self {
        for s in names {
            self = self.visible_long_flag_alias(s);
        }
        self
    }

    /// Set the placement of this subcommand within the help.
    ///
    /// Subcommands with a lower value will be displayed first in the help message.
    /// Those with the same display order will be sorted.
    ///
    /// `Command`s are automatically assigned a display order based on the order they are added to
    /// their parent [`Command`].
    /// Overriding this is helpful when the order commands are added in isn't the same as the
    /// display order, whether in one-off cases or to automatically sort commands.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(feature = "help")] {
    /// # use clap_builder as clap;
    /// # use clap::{Command, };
    /// let m = Command::new("cust-ord")
    ///     .subcommand(Command::new("beta")
    ///         .display_order(0)  // Sort
    ///         .about("Some help and text"))
    ///     .subcommand(Command::new("alpha")
    ///         .display_order(0)  // Sort
    ///         .about("I should be first!"))
    ///     .get_matches_from(vec![
    ///         "cust-ord", "--help"
    ///     ]);
    /// # }
    /// ```
    ///
    /// The above example displays the following help message
    ///
    /// ```text
    /// cust-ord
    ///
    /// Usage: cust-ord [OPTIONS]
    ///
    /// Commands:
    ///     alpha    I should be first!
    ///     beta     Some help and text
    ///     help     Print help for the subcommand(s)
    ///
    /// Options:
    ///     -h, --help       Print help
    ///     -V, --version    Print version
    /// ```
    #[inline]
    #[must_use]
    pub fn display_order(mut self, ord: impl IntoResettable<usize>) -> Self {
        self.disp_ord = ord.into_resettable().into_option();
        self
    }

    /// Specifies that this [`subcommand`] should be hidden from help messages
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// Command::new("myprog")
    ///     .subcommand(
    ///         Command::new("test").hide(true)
    ///     )
    /// # ;
    /// ```
    ///
    /// [`subcommand`]: crate::Command::subcommand()
    #[inline]
    pub fn hide(self, yes: bool) -> Self {
        if yes {
            self.setting(AppSettings::Hidden)
        } else {
            self.unset_setting(AppSettings::Hidden)
        }
    }

    /// If no [`subcommand`] is present at runtime, error and exit gracefully.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, error::ErrorKind};
    /// let err = Command::new("myprog")
    ///     .subcommand_required(true)
    ///     .subcommand(Command::new("test"))
    ///     .try_get_matches_from(vec![
    ///         "myprog",
    ///     ]);
    /// assert!(err.is_err());
    /// assert_eq!(err.unwrap_err().kind(), ErrorKind::MissingSubcommand);
    /// # ;
    /// ```
    ///
    /// [`subcommand`]: crate::Command::subcommand()
    pub fn subcommand_required(self, yes: bool) -> Self {
        if yes {
            self.setting(AppSettings::SubcommandRequired)
        } else {
            self.unset_setting(AppSettings::SubcommandRequired)
        }
    }

    /// Assume unexpected positional arguments are a [`subcommand`].
    ///
    /// Arguments will be stored in the `""` argument in the [`ArgMatches`]
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** Use this setting with caution,
    /// as a truly unexpected argument (i.e. one that is *NOT* an external subcommand)
    /// will **not** cause an error and instead be treated as a potential subcommand.
    /// One should check for such cases manually and inform the user appropriately.
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** A built-in subcommand will be parsed as an external subcommand when escaped with
    /// `--`.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use std::ffi::OsString;
    /// # use clap::Command;
    /// // Assume there is an external subcommand named "subcmd"
    /// let m = Command::new("myprog")
    ///     .allow_external_subcommands(true)
    ///     .get_matches_from(vec![
    ///         "myprog", "subcmd", "--option", "value", "-fff", "--flag"
    ///     ]);
    ///
    /// // All trailing arguments will be stored under the subcommand's sub-matches using an empty
    /// // string argument name
    /// match m.subcommand() {
    ///     Some((external, ext_m)) => {
    ///          let ext_args: Vec<_> = ext_m.get_many::<OsString>("").unwrap().collect();
    ///          assert_eq!(external, "subcmd");
    ///          assert_eq!(ext_args, ["--option", "value", "-fff", "--flag"]);
    ///     },
    ///     _ => {},
    /// }
    /// ```
    ///
    /// [`subcommand`]: crate::Command::subcommand()
    /// [`ArgMatches`]: crate::ArgMatches
    /// [`ErrorKind::UnknownArgument`]: crate::error::ErrorKind::UnknownArgument
    pub fn allow_external_subcommands(self, yes: bool) -> Self {
        if yes {
            self.setting(AppSettings::AllowExternalSubcommands)
        } else {
            self.unset_setting(AppSettings::AllowExternalSubcommands)
        }
    }

    /// Specifies how to parse external subcommand arguments.
    ///
    /// The default parser is for `OsString`.  This can be used to switch it to `String` or another
    /// type.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** Setting this requires [`Command::allow_external_subcommands`]
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(unix)] {
    /// # use clap_builder as clap;
    /// # use std::ffi::OsString;
    /// # use clap::Command;
    /// # use clap::value_parser;
    /// // Assume there is an external subcommand named "subcmd"
    /// let m = Command::new("myprog")
    ///     .allow_external_subcommands(true)
    ///     .get_matches_from(vec![
    ///         "myprog", "subcmd", "--option", "value", "-fff", "--flag"
    ///     ]);
    ///
    /// // All trailing arguments will be stored under the subcommand's sub-matches using an empty
    /// // string argument name
    /// match m.subcommand() {
    ///     Some((external, ext_m)) => {
    ///          let ext_args: Vec<_> = ext_m.get_many::<OsString>("").unwrap().collect();
    ///          assert_eq!(external, "subcmd");
    ///          assert_eq!(ext_args, ["--option", "value", "-fff", "--flag"]);
    ///     },
    ///     _ => {},
    /// }
    /// # }
    /// ```
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// # use clap::value_parser;
    /// // Assume there is an external subcommand named "subcmd"
    /// let m = Command::new("myprog")
    ///     .external_subcommand_value_parser(value_parser!(String))
    ///     .get_matches_from(vec![
    ///         "myprog", "subcmd", "--option", "value", "-fff", "--flag"
    ///     ]);
    ///
    /// // All trailing arguments will be stored under the subcommand's sub-matches using an empty
    /// // string argument name
    /// match m.subcommand() {
    ///     Some((external, ext_m)) => {
    ///          let ext_args: Vec<_> = ext_m.get_many::<String>("").unwrap().collect();
    ///          assert_eq!(external, "subcmd");
    ///          assert_eq!(ext_args, ["--option", "value", "-fff", "--flag"]);
    ///     },
    ///     _ => {},
    /// }
    /// ```
    ///
    /// [`subcommands`]: crate::Command::subcommand()
    pub fn external_subcommand_value_parser(
        mut self,
        parser: impl IntoResettable<super::ValueParser>,
    ) -> Self {
        self.external_value_parser = parser.into_resettable().into_option();
        self
    }

    /// Specifies that use of an argument prevents the use of [`subcommands`].
    ///
    /// By default `clap` allows arguments between subcommands such
    /// as `<cmd> [cmd_args] <subcmd> [subcmd_args] <subsubcmd> [subsubcmd_args]`.
    ///
    /// This setting disables that functionality and says that arguments can
    /// only follow the *final* subcommand. For instance using this setting
    /// makes only the following invocations possible:
    ///
    /// * `<cmd> <subcmd> <subsubcmd> [subsubcmd_args]`
    /// * `<cmd> <subcmd> [subcmd_args]`
    /// * `<cmd> [cmd_args]`
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// Command::new("myprog")
    ///     .args_conflicts_with_subcommands(true);
    /// ```
    ///
    /// [`subcommands`]: crate::Command::subcommand()
    pub fn args_conflicts_with_subcommands(self, yes: bool) -> Self {
        if yes {
            self.setting(AppSettings::ArgsNegateSubcommands)
        } else {
            self.unset_setting(AppSettings::ArgsNegateSubcommands)
        }
    }

    /// Prevent subcommands from being consumed as an arguments value.
    ///
    /// By default, if an option taking multiple values is followed by a subcommand, the
    /// subcommand will be parsed as another value.
    ///
    /// ```text
    /// cmd --foo val1 val2 subcommand
    ///           --------- ----------
    ///             values   another value
    /// ```
    ///
    /// This setting instructs the parser to stop when encountering a subcommand instead of
    /// greedily consuming arguments.
    ///
    /// ```text
    /// cmd --foo val1 val2 subcommand
    ///           --------- ----------
    ///             values   subcommand
    /// ```
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgAction};
    /// let cmd = Command::new("cmd").subcommand(Command::new("sub")).arg(
    ///     Arg::new("arg")
    ///         .long("arg")
    ///         .num_args(1..)
    ///         .action(ArgAction::Set),
    /// );
    ///
    /// let matches = cmd
    ///     .clone()
    ///     .try_get_matches_from(&["cmd", "--arg", "1", "2", "3", "sub"])
    ///     .unwrap();
    /// assert_eq!(
    ///     matches.get_many::<String>("arg").unwrap().collect::<Vec<_>>(),
    ///     &["1", "2", "3", "sub"]
    /// );
    /// assert!(matches.subcommand_matches("sub").is_none());
    ///
    /// let matches = cmd
    ///     .subcommand_precedence_over_arg(true)
    ///     .try_get_matches_from(&["cmd", "--arg", "1", "2", "3", "sub"])
    ///     .unwrap();
    /// assert_eq!(
    ///     matches.get_many::<String>("arg").unwrap().collect::<Vec<_>>(),
    ///     &["1", "2", "3"]
    /// );
    /// assert!(matches.subcommand_matches("sub").is_some());
    /// ```
    pub fn subcommand_precedence_over_arg(self, yes: bool) -> Self {
        if yes {
            self.setting(AppSettings::SubcommandPrecedenceOverArg)
        } else {
            self.unset_setting(AppSettings::SubcommandPrecedenceOverArg)
        }
    }

    /// Allows [`subcommands`] to override all requirements of the parent command.
    ///
    /// For example, if you had a subcommand or top level application with a required argument
    /// that is only required as long as there is no subcommand present,
    /// using this setting would allow you to set those arguments to [`Arg::required(true)`]
    /// and yet receive no error so long as the user uses a valid subcommand instead.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This defaults to false (using subcommand does *not* negate requirements)
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// This first example shows that it is an error to not use a required argument
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, error::ErrorKind};
    /// let err = Command::new("myprog")
    ///     .subcommand_negates_reqs(true)
    ///     .arg(Arg::new("opt").required(true))
    ///     .subcommand(Command::new("test"))
    ///     .try_get_matches_from(vec![
    ///         "myprog"
    ///     ]);
    /// assert!(err.is_err());
    /// assert_eq!(err.unwrap_err().kind(), ErrorKind::MissingRequiredArgument);
    /// # ;
    /// ```
    ///
    /// This next example shows that it is no longer error to not use a required argument if a
    /// valid subcommand is used.
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, error::ErrorKind};
    /// let noerr = Command::new("myprog")
    ///     .subcommand_negates_reqs(true)
    ///     .arg(Arg::new("opt").required(true))
    ///     .subcommand(Command::new("test"))
    ///     .try_get_matches_from(vec![
    ///         "myprog", "test"
    ///     ]);
    /// assert!(noerr.is_ok());
    /// # ;
    /// ```
    ///
    /// [`Arg::required(true)`]: crate::Arg::required()
    /// [`subcommands`]: crate::Command::subcommand()
    pub fn subcommand_negates_reqs(self, yes: bool) -> Self {
        if yes {
            self.setting(AppSettings::SubcommandsNegateReqs)
        } else {
            self.unset_setting(AppSettings::SubcommandsNegateReqs)
        }
    }

    /// Multiple-personality program dispatched on the binary name (`argv[0]`)
    ///
    /// A "multicall" executable is a single executable
    /// that contains a variety of applets,
    /// and decides which applet to run based on the name of the file.
    /// The executable can be called from different names by creating hard links
    /// or symbolic links to it.
    ///
    /// This is desirable for:
    /// - Easy distribution, a single binary that can install hardlinks to access the different
    ///   personalities.
    /// - Minimal binary size by sharing common code (e.g. standard library, clap)
    /// - Custom shells or REPLs where there isn't a single top-level command
    ///
    /// Setting `multicall` will cause
    /// - `argv[0]` to be stripped to the base name and parsed as the first argument, as if
    ///   [`Command::no_binary_name`][Command::no_binary_name] was set.
    /// - Help and errors to report subcommands as if they were the top-level command
    ///
    /// When the subcommand is not present, there are several strategies you may employ, depending
    /// on your needs:
    /// - Let the error percolate up normally
    /// - Print a specialized error message using the
    ///   [`Error::context`][crate::Error::context]
    /// - Print the [help][Command::write_help] but this might be ambiguous
    /// - Disable `multicall` and re-parse it
    /// - Disable `multicall` and re-parse it with a specific subcommand
    ///
    /// When detecting the error condition, the [`ErrorKind`] isn't sufficient as a sub-subcommand
    /// might report the same error.  Enable
    /// [`allow_external_subcommands`][Command::allow_external_subcommands] if you want to specifically
    /// get the unrecognized binary name.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** Multicall can't be used with [`no_binary_name`] since they interpret
    /// the command name in incompatible ways.
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** The multicall command cannot have arguments.
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** Applets are slightly semantically different from subcommands,
    /// so it's recommended to use [`Command::subcommand_help_heading`] and
    /// [`Command::subcommand_value_name`] to change the descriptive text as above.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// `hostname` is an example of a multicall executable.
    /// Both `hostname` and `dnsdomainname` are provided by the same executable
    /// and which behaviour to use is based on the executable file name.
    ///
    /// This is desirable when the executable has a primary purpose
    /// but there is related functionality that would be convenient to provide
    /// and implement it to be in the same executable.
    ///
    /// The name of the cmd is essentially unused
    /// and may be the same as the name of a subcommand.
    ///
    /// The names of the immediate subcommands of the Command
    /// are matched against the basename of the first argument,
    /// which is conventionally the path of the executable.
    ///
    /// This does not allow the subcommand to be passed as the first non-path argument.
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, error::ErrorKind};
    /// let mut cmd = Command::new("hostname")
    ///     .multicall(true)
    ///     .subcommand(Command::new("hostname"))
    ///     .subcommand(Command::new("dnsdomainname"));
    /// let m = cmd.try_get_matches_from_mut(&["/usr/bin/hostname", "dnsdomainname"]);
    /// assert!(m.is_err());
    /// assert_eq!(m.unwrap_err().kind(), ErrorKind::UnknownArgument);
    /// let m = cmd.get_matches_from(&["/usr/bin/dnsdomainname"]);
    /// assert_eq!(m.subcommand_name(), Some("dnsdomainname"));
    /// ```
    ///
    /// Busybox is another common example of a multicall executable
    /// with a subcommmand for each applet that can be run directly,
    /// e.g. with the `cat` applet being run by running `busybox cat`,
    /// or with `cat` as a link to the `busybox` binary.
    ///
    /// This is desirable when the launcher program has additional options
    /// or it is useful to run the applet without installing a symlink
    /// e.g. to test the applet without installing it
    /// or there may already be a command of that name installed.
    ///
    /// To make an applet usable as both a multicall link and a subcommand
    /// the subcommands must be defined both in the top-level Command
    /// and as subcommands of the "main" applet.
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// fn applet_commands() -> [Command; 2] {
    ///     [Command::new("true"), Command::new("false")]
    /// }
    /// let mut cmd = Command::new("busybox")
    ///     .multicall(true)
    ///     .subcommand(
    ///         Command::new("busybox")
    ///             .subcommand_value_name("APPLET")
    ///             .subcommand_help_heading("APPLETS")
    ///             .subcommands(applet_commands()),
    ///     )
    ///     .subcommands(applet_commands());
    /// // When called from the executable's canonical name
    /// // its applets can be matched as subcommands.
    /// let m = cmd.try_get_matches_from_mut(&["/usr/bin/busybox", "true"]).unwrap();
    /// assert_eq!(m.subcommand_name(), Some("busybox"));
    /// assert_eq!(m.subcommand().unwrap().1.subcommand_name(), Some("true"));
    /// // When called from a link named after an applet that applet is matched.
    /// let m = cmd.get_matches_from(&["/usr/bin/true"]);
    /// assert_eq!(m.subcommand_name(), Some("true"));
    /// ```
    ///
    /// [`no_binary_name`]: crate::Command::no_binary_name
    /// [`Command::subcommand_value_name`]: crate::Command::subcommand_value_name
    /// [`Command::subcommand_help_heading`]: crate::Command::subcommand_help_heading
    #[inline]
    pub fn multicall(self, yes: bool) -> Self {
        if yes {
            self.setting(AppSettings::Multicall)
        } else {
            self.unset_setting(AppSettings::Multicall)
        }
    }

    /// Sets the value name used for subcommands when printing usage and help.
    ///
    /// By default, this is "COMMAND".
    ///
    /// See also [`Command::subcommand_help_heading`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// Command::new("myprog")
    ///     .subcommand(Command::new("sub1"))
    ///     .print_help()
    /// # ;
    /// ```
    ///
    /// will produce
    ///
    /// ```text
    /// myprog
    ///
    /// Usage: myprog [COMMAND]
    ///
    /// Commands:
    ///     help    Print this message or the help of the given subcommand(s)
    ///     sub1
    ///
    /// Options:
    ///     -h, --help       Print help
    ///     -V, --version    Print version
    /// ```
    ///
    /// but usage of `subcommand_value_name`
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// Command::new("myprog")
    ///     .subcommand(Command::new("sub1"))
    ///     .subcommand_value_name("THING")
    ///     .print_help()
    /// # ;
    /// ```
    ///
    /// will produce
    ///
    /// ```text
    /// myprog
    ///
    /// Usage: myprog [THING]
    ///
    /// Commands:
    ///     help    Print this message or the help of the given subcommand(s)
    ///     sub1
    ///
    /// Options:
    ///     -h, --help       Print help
    ///     -V, --version    Print version
    /// ```
    #[must_use]
    pub fn subcommand_value_name(mut self, value_name: impl IntoResettable<Str>) -> Self {
        self.subcommand_value_name = value_name.into_resettable().into_option();
        self
    }

    /// Sets the help heading used for subcommands when printing usage and help.
    ///
    /// By default, this is "Commands".
    ///
    /// See also [`Command::subcommand_value_name`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// Command::new("myprog")
    ///     .subcommand(Command::new("sub1"))
    ///     .print_help()
    /// # ;
    /// ```
    ///
    /// will produce
    ///
    /// ```text
    /// myprog
    ///
    /// Usage: myprog [COMMAND]
    ///
    /// Commands:
    ///     help    Print this message or the help of the given subcommand(s)
    ///     sub1
    ///
    /// Options:
    ///     -h, --help       Print help
    ///     -V, --version    Print version
    /// ```
    ///
    /// but usage of `subcommand_help_heading`
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// Command::new("myprog")
    ///     .subcommand(Command::new("sub1"))
    ///     .subcommand_help_heading("Things")
    ///     .print_help()
    /// # ;
    /// ```
    ///
    /// will produce
    ///
    /// ```text
    /// myprog
    ///
    /// Usage: myprog [COMMAND]
    ///
    /// Things:
    ///     help    Print this message or the help of the given subcommand(s)
    ///     sub1
    ///
    /// Options:
    ///     -h, --help       Print help
    ///     -V, --version    Print version
    /// ```
    #[must_use]
    pub fn subcommand_help_heading(mut self, heading: impl IntoResettable<Str>) -> Self {
        self.subcommand_heading = heading.into_resettable().into_option();
        self
    }
}

/// # Reflection
impl Command {
    #[inline]
    #[cfg(feature = "usage")]
    pub(crate) fn get_usage_name(&self) -> Option<&str> {
        self.usage_name.as_deref()
    }

    #[inline]
    #[cfg(feature = "usage")]
    pub(crate) fn get_usage_name_fallback(&self) -> &str {
        self.get_usage_name()
            .unwrap_or_else(|| self.get_bin_name_fallback())
    }

    #[inline]
    #[cfg(not(feature = "usage"))]
    #[allow(dead_code)]
    pub(crate) fn get_usage_name_fallback(&self) -> &str {
        self.get_bin_name_fallback()
    }

    /// Get the name of the binary.
    #[inline]
    pub fn get_display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    /// Get the name of the binary.
    #[inline]
    pub fn get_bin_name(&self) -> Option<&str> {
        self.bin_name.as_deref()
    }

    /// Get the name of the binary.
    #[inline]
    pub(crate) fn get_bin_name_fallback(&self) -> &str {
        self.bin_name.as_deref().unwrap_or_else(|| self.get_name())
    }

    /// Set binary name. Uses `&mut self` instead of `self`.
    pub fn set_bin_name(&mut self, name: impl Into<String>) {
        self.bin_name = Some(name.into());
    }

    /// Get the name of the cmd.
    #[inline]
    pub fn get_name(&self) -> &str {
        self.name.as_str()
    }

    #[inline]
    #[cfg(debug_assertions)]
    pub(crate) fn get_name_str(&self) -> &Str {
        &self.name
    }

    /// Get all known names of the cmd (i.e. primary name and visible aliases).
    pub fn get_name_and_visible_aliases(&self) -> Vec<&str> {
        let mut names = vec![self.name.as_str()];
        names.extend(self.get_visible_aliases());
        names
    }

    /// Get the version of the cmd.
    #[inline]
    pub fn get_version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// Get the long version of the cmd.
    #[inline]
    pub fn get_long_version(&self) -> Option<&str> {
        self.long_version.as_deref()
    }

    /// Get the placement within help
    #[inline]
    pub fn get_display_order(&self) -> usize {
        self.disp_ord.unwrap_or(999)
    }

    /// Get the authors of the cmd.
    #[inline]
    pub fn get_author(&self) -> Option<&str> {
        self.author.as_deref()
    }

    /// Get the short flag of the subcommand.
    #[inline]
    pub fn get_short_flag(&self) -> Option<char> {
        self.short_flag
    }

    /// Get the long flag of the subcommand.
    #[inline]
    pub fn get_long_flag(&self) -> Option<&str> {
        self.long_flag.as_deref()
    }

    /// Get the help message specified via [`Command::about`].
    ///
    /// [`Command::about`]: Command::about()
    #[inline]
    pub fn get_about(&self) -> Option<&StyledStr> {
        self.about.as_ref()
    }

    /// Get the help message specified via [`Command::long_about`].
    ///
    /// [`Command::long_about`]: Command::long_about()
    #[inline]
    pub fn get_long_about(&self) -> Option<&StyledStr> {
        self.long_about.as_ref()
    }

    /// Get the custom section heading specified via [`Command::flatten_help`].
    #[inline]
    pub fn is_flatten_help_set(&self) -> bool {
        self.is_set(AppSettings::FlattenHelp)
    }

    /// Get the custom section heading specified via [`Command::next_help_heading`].
    #[inline]
    pub fn get_next_help_heading(&self) -> Option<&str> {
        self.current_help_heading.as_deref()
    }

    /// Iterate through the *visible* aliases for this subcommand.
    #[inline]
    pub fn get_visible_aliases(&self) -> impl Iterator<Item = &str> + '_ {
        self.aliases
            .iter()
            .filter(|(_, vis)| *vis)
            .map(|a| a.0.as_str())
    }

    /// Iterate through the *visible* short aliases for this subcommand.
    #[inline]
    pub fn get_visible_short_flag_aliases(&self) -> impl Iterator<Item = char> + '_ {
        self.short_flag_aliases
            .iter()
            .filter(|(_, vis)| *vis)
            .map(|a| a.0)
    }

    /// Iterate through the *visible* long aliases for this subcommand.
    #[inline]
    pub fn get_visible_long_flag_aliases(&self) -> impl Iterator<Item = &str> + '_ {
        self.long_flag_aliases
            .iter()
            .filter(|(_, vis)| *vis)
            .map(|a| a.0.as_str())
    }

    /// Iterate through the set of *all* the aliases for this subcommand, both visible and hidden.
    #[inline]
    pub fn get_all_aliases(&self) -> impl Iterator<Item = &str> + '_ {
        self.aliases.iter().map(|a| a.0.as_str())
    }

    /// Iterate through the set of *all* the short aliases for this subcommand, both visible and hidden.
    #[inline]
    pub fn get_all_short_flag_aliases(&self) -> impl Iterator<Item = char> + '_ {
        self.short_flag_aliases.iter().map(|a| a.0)
    }

    /// Iterate through the set of *all* the long aliases for this subcommand, both visible and hidden.
    #[inline]
    pub fn get_all_long_flag_aliases(&self) -> impl Iterator<Item = &str> + '_ {
        self.long_flag_aliases.iter().map(|a| a.0.as_str())
    }

    /// Iterate through the *hidden* aliases for this subcommand.
    #[inline]
    pub fn get_aliases(&self) -> impl Iterator<Item = &str> + '_ {
        self.aliases
            .iter()
            .filter(|(_, vis)| !*vis)
            .map(|a| a.0.as_str())
    }

    #[inline]
    pub(crate) fn is_set(&self, s: AppSettings) -> bool {
        self.settings.is_set(s) || self.g_settings.is_set(s)
    }

    /// Should we color the output?
    pub fn get_color(&self) -> ColorChoice {
        debug!("Command::color: Color setting...");

        if cfg!(feature = "color") {
            if self.is_set(AppSettings::ColorNever) {
                debug!("Never");
                ColorChoice::Never
            } else if self.is_set(AppSettings::ColorAlways) {
                debug!("Always");
                ColorChoice::Always
            } else {
                debug!("Auto");
                ColorChoice::Auto
            }
        } else {
            ColorChoice::Never
        }
    }

    /// Return the current `Styles` for the `Command`
    #[inline]
    pub fn get_styles(&self) -> &Styles {
        self.app_ext.get().unwrap_or_default()
    }

    /// Iterate through the set of subcommands, getting a reference to each.
    #[inline]
    pub fn get_subcommands(&self) -> impl Iterator<Item = &Command> {
        self.subcommands.iter()
    }

    /// Iterate through the set of subcommands, getting a mutable reference to each.
    #[inline]
    pub fn get_subcommands_mut(&mut self) -> impl Iterator<Item = &mut Command> {
        self.subcommands.iter_mut()
    }

    /// Returns `true` if this `Command` has subcommands.
    #[inline]
    pub fn has_subcommands(&self) -> bool {
        !self.subcommands.is_empty()
    }

    /// Returns the help heading for listing subcommands.
    #[inline]
    pub fn get_subcommand_help_heading(&self) -> Option<&str> {
        self.subcommand_heading.as_deref()
    }

    /// Returns the subcommand value name.
    #[inline]
    pub fn get_subcommand_value_name(&self) -> Option<&str> {
        self.subcommand_value_name.as_deref()
    }

    /// Returns the help heading for listing subcommands.
    #[inline]
    pub fn get_before_help(&self) -> Option<&StyledStr> {
        self.before_help.as_ref()
    }

    /// Returns the help heading for listing subcommands.
    #[inline]
    pub fn get_before_long_help(&self) -> Option<&StyledStr> {
        self.before_long_help.as_ref()
    }

    /// Returns the help heading for listing subcommands.
    #[inline]
    pub fn get_after_help(&self) -> Option<&StyledStr> {
        self.after_help.as_ref()
    }

    /// Returns the help heading for listing subcommands.
    #[inline]
    pub fn get_after_long_help(&self) -> Option<&StyledStr> {
        self.after_long_help.as_ref()
    }

    /// Find subcommand such that its name or one of aliases equals `name`.
    ///
    /// This does not recurse through subcommands of subcommands.
    #[inline]
    pub fn find_subcommand(&self, name: impl AsRef<std::ffi::OsStr>) -> Option<&Command> {
        let name = name.as_ref();
        self.get_subcommands().find(|s| s.aliases_to(name))
    }

    /// Find subcommand such that its name or one of aliases equals `name`, returning
    /// a mutable reference to the subcommand.
    ///
    /// This does not recurse through subcommands of subcommands.
    #[inline]
    pub fn find_subcommand_mut(
        &mut self,
        name: impl AsRef<std::ffi::OsStr>,
    ) -> Option<&mut Command> {
        let name = name.as_ref();
        self.get_subcommands_mut().find(|s| s.aliases_to(name))
    }

    /// Iterate through the set of groups.
    #[inline]
    pub fn get_groups(&self) -> impl Iterator<Item = &ArgGroup> {
        self.groups.iter()
    }

    /// Iterate through the set of arguments.
    #[inline]
    pub fn get_arguments(&self) -> impl Iterator<Item = &Arg> {
        self.args.args()
    }

    /// Iterate through the *positionals* arguments.
    #[inline]
    pub fn get_positionals(&self) -> impl Iterator<Item = &Arg> {
        self.get_arguments().filter(|a| a.is_positional())
    }

    /// Iterate through the *options*.
    pub fn get_opts(&self) -> impl Iterator<Item = &Arg> {
        self.get_arguments()
            .filter(|a| a.is_takes_value_set() && !a.is_positional())
    }

    /// Get a list of all arguments the given argument conflicts with.
    ///
    /// If the provided argument is declared as global, the conflicts will be determined
    /// based on the propagation rules of global arguments.
    ///
    /// ### Panics
    ///
    /// If the given arg contains a conflict with an argument that is unknown to
    /// this `Command`.
    pub fn get_arg_conflicts_with(&self, arg: &Arg) -> Vec<&Arg> // FIXME: This could probably have been an iterator
    {
        if arg.is_global_set() {
            self.get_global_arg_conflicts_with(arg)
        } else {
            let mut result = Vec::new();
            for id in arg.blacklist.iter() {
                if let Some(arg) = self.find(id) {
                    result.push(arg);
                } else if let Some(group) = self.find_group(id) {
                    result.extend(
                        self.unroll_args_in_group(&group.id)
                            .iter()
                            .map(|id| self.find(id).expect(INTERNAL_ERROR_MSG)),
                    );
                } else {
                    panic!("Command::get_arg_conflicts_with: The passed arg conflicts with an arg unknown to the cmd");
                }
            }
            result
        }
    }

    /// Get a unique list of all arguments of all commands and continuous subcommands the given argument conflicts with.
    ///
    /// This behavior follows the propagation rules of global arguments.
    /// It is useful for finding conflicts for arguments declared as global.
    ///
    /// ### Panics
    ///
    /// If the given arg contains a conflict with an argument that is unknown to
    /// this `Command`.
    fn get_global_arg_conflicts_with(&self, arg: &Arg) -> Vec<&Arg> // FIXME: This could probably have been an iterator
    {
        arg.blacklist
            .iter()
            .map(|id| {
                self.args
                    .args()
                    .chain(
                        self.get_subcommands_containing(arg)
                            .iter()
                            .flat_map(|x| x.args.args()),
                    )
                    .find(|arg| arg.get_id() == id)
                    .expect(
                        "Command::get_arg_conflicts_with: \
                    The passed arg conflicts with an arg unknown to the cmd",
                    )
            })
            .collect()
    }

    /// Get a list of subcommands which contain the provided Argument
    ///
    /// This command will only include subcommands in its list for which the subcommands
    /// parent also contains the Argument.
    ///
    /// This search follows the propagation rules of global arguments.
    /// It is useful to finding subcommands, that have inherited a global argument.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** In this case only `Sucommand_1` will be included
    /// ```text
    ///   Subcommand_1 (contains Arg)
    ///     Subcommand_1.1 (doesn't contain Arg)
    ///       Subcommand_1.1.1 (contains Arg)
    /// ```
    ///
    /// </div>
    fn get_subcommands_containing(&self, arg: &Arg) -> Vec<&Self> {
        let mut vec = Vec::new();
        for idx in 0..self.subcommands.len() {
            if self.subcommands[idx]
                .args
                .args()
                .any(|ar| ar.get_id() == arg.get_id())
            {
                vec.push(&self.subcommands[idx]);
                vec.append(&mut self.subcommands[idx].get_subcommands_containing(arg));
            }
        }
        vec
    }

    /// Report whether [`Command::no_binary_name`] is set
    pub fn is_no_binary_name_set(&self) -> bool {
        self.is_set(AppSettings::NoBinaryName)
    }

    /// Report whether [`Command::ignore_errors`] is set
    pub(crate) fn is_ignore_errors_set(&self) -> bool {
        self.is_set(AppSettings::IgnoreErrors)
    }

    /// Report whether [`Command::dont_delimit_trailing_values`] is set
    pub fn is_dont_delimit_trailing_values_set(&self) -> bool {
        self.is_set(AppSettings::DontDelimitTrailingValues)
    }

    /// Report whether [`Command::disable_version_flag`] is set
    pub fn is_disable_version_flag_set(&self) -> bool {
        self.is_set(AppSettings::DisableVersionFlag)
            || (self.version.is_none() && self.long_version.is_none())
    }

    /// Report whether [`Command::propagate_version`] is set
    pub fn is_propagate_version_set(&self) -> bool {
        self.is_set(AppSettings::PropagateVersion)
    }

    /// Report whether [`Command::next_line_help`] is set
    pub fn is_next_line_help_set(&self) -> bool {
        self.is_set(AppSettings::NextLineHelp)
    }

    /// Report whether [`Command::disable_help_flag`] is set
    pub fn is_disable_help_flag_set(&self) -> bool {
        self.is_set(AppSettings::DisableHelpFlag)
    }

    /// Report whether [`Command::disable_help_subcommand`] is set
    pub fn is_disable_help_subcommand_set(&self) -> bool {
        self.is_set(AppSettings::DisableHelpSubcommand)
    }

    /// Report whether [`Command::disable_colored_help`] is set
    pub fn is_disable_colored_help_set(&self) -> bool {
        self.is_set(AppSettings::DisableColoredHelp)
    }

    /// Report whether [`Command::help_expected`] is set
    #[cfg(debug_assertions)]
    pub(crate) fn is_help_expected_set(&self) -> bool {
        self.is_set(AppSettings::HelpExpected)
    }

    #[doc(hidden)]
    #[cfg_attr(
        feature = "deprecated",
        deprecated(since = "4.0.0", note = "This is now the default")
    )]
    pub fn is_dont_collapse_args_in_usage_set(&self) -> bool {
        true
    }

    /// Report whether [`Command::infer_long_args`] is set
    pub(crate) fn is_infer_long_args_set(&self) -> bool {
        self.is_set(AppSettings::InferLongArgs)
    }

    /// Report whether [`Command::infer_subcommands`] is set
    pub(crate) fn is_infer_subcommands_set(&self) -> bool {
        self.is_set(AppSettings::InferSubcommands)
    }

    /// Report whether [`Command::arg_required_else_help`] is set
    pub fn is_arg_required_else_help_set(&self) -> bool {
        self.is_set(AppSettings::ArgRequiredElseHelp)
    }

    #[doc(hidden)]
    #[cfg_attr(
        feature = "deprecated",
        deprecated(
            since = "4.0.0",
            note = "Replaced with `Arg::is_allow_hyphen_values_set`"
        )
    )]
    pub(crate) fn is_allow_hyphen_values_set(&self) -> bool {
        self.is_set(AppSettings::AllowHyphenValues)
    }

    #[doc(hidden)]
    #[cfg_attr(
        feature = "deprecated",
        deprecated(
            since = "4.0.0",
            note = "Replaced with `Arg::is_allow_negative_numbers_set`"
        )
    )]
    pub fn is_allow_negative_numbers_set(&self) -> bool {
        self.is_set(AppSettings::AllowNegativeNumbers)
    }

    #[doc(hidden)]
    #[cfg_attr(
        feature = "deprecated",
        deprecated(since = "4.0.0", note = "Replaced with `Arg::is_trailing_var_arg_set`")
    )]
    pub fn is_trailing_var_arg_set(&self) -> bool {
        self.is_set(AppSettings::TrailingVarArg)
    }

    /// Report whether [`Command::allow_missing_positional`] is set
    pub fn is_allow_missing_positional_set(&self) -> bool {
        self.is_set(AppSettings::AllowMissingPositional)
    }

    /// Report whether [`Command::hide`] is set
    pub fn is_hide_set(&self) -> bool {
        self.is_set(AppSettings::Hidden)
    }

    /// Report whether [`Command::subcommand_required`] is set
    pub fn is_subcommand_required_set(&self) -> bool {
        self.is_set(AppSettings::SubcommandRequired)
    }

    /// Report whether [`Command::allow_external_subcommands`] is set
    pub fn is_allow_external_subcommands_set(&self) -> bool {
        self.is_set(AppSettings::AllowExternalSubcommands)
    }

    /// Configured parser for values passed to an external subcommand
    ///
    /// # Example
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// let cmd = clap::Command::new("raw")
    ///     .external_subcommand_value_parser(clap::value_parser!(String));
    /// let value_parser = cmd.get_external_subcommand_value_parser();
    /// println!("{value_parser:?}");
    /// ```
    pub fn get_external_subcommand_value_parser(&self) -> Option<&super::ValueParser> {
        if !self.is_allow_external_subcommands_set() {
            None
        } else {
            static DEFAULT: super::ValueParser = super::ValueParser::os_string();
            Some(self.external_value_parser.as_ref().unwrap_or(&DEFAULT))
        }
    }

    /// Report whether [`Command::args_conflicts_with_subcommands`] is set
    pub fn is_args_conflicts_with_subcommands_set(&self) -> bool {
        self.is_set(AppSettings::ArgsNegateSubcommands)
    }

    #[doc(hidden)]
    pub fn is_args_override_self(&self) -> bool {
        self.is_set(AppSettings::AllArgsOverrideSelf)
    }

    /// Report whether [`Command::subcommand_precedence_over_arg`] is set
    pub fn is_subcommand_precedence_over_arg_set(&self) -> bool {
        self.is_set(AppSettings::SubcommandPrecedenceOverArg)
    }

    /// Report whether [`Command::subcommand_negates_reqs`] is set
    pub fn is_subcommand_negates_reqs_set(&self) -> bool {
        self.is_set(AppSettings::SubcommandsNegateReqs)
    }

    /// Report whether [`Command::multicall`] is set
    pub fn is_multicall_set(&self) -> bool {
        self.is_set(AppSettings::Multicall)
    }

    /// Access an [`CommandExt`]
    #[cfg(feature = "unstable-ext")]
    pub fn get<T: CommandExt + Extension>(&self) -> Option<&T> {
        self.ext.get::<T>()
    }

    /// Remove an [`CommandExt`]
    #[cfg(feature = "unstable-ext")]
    pub fn remove<T: CommandExt + Extension>(mut self) -> Option<T> {
        self.ext.remove::<T>()
    }
}

// Internally used only
impl Command {
    pub(crate) fn get_override_usage(&self) -> Option<&StyledStr> {
        self.usage_str.as_ref()
    }

    pub(crate) fn get_override_help(&self) -> Option<&StyledStr> {
        self.help_str.as_ref()
    }

    #[cfg(feature = "help")]
    pub(crate) fn get_help_template(&self) -> Option<&StyledStr> {
        self.template.as_ref()
    }

    #[cfg(feature = "help")]
    pub(crate) fn get_term_width(&self) -> Option<usize> {
        self.app_ext.get::<TermWidth>().map(|e| e.0)
    }

    #[cfg(feature = "help")]
    pub(crate) fn get_max_term_width(&self) -> Option<usize> {
        self.app_ext.get::<MaxTermWidth>().map(|e| e.0)
    }

    pub(crate) fn get_keymap(&self) -> &MKeyMap {
        &self.args
    }

    fn get_used_global_args(&self, matches: &ArgMatches, global_arg_vec: &mut Vec<Id>) {
        global_arg_vec.extend(
            self.args
                .args()
                .filter(|a| a.is_global_set())
                .map(|ga| ga.id.clone()),
        );
        if let Some((id, matches)) = matches.subcommand() {
            if let Some(used_sub) = self.find_subcommand(id) {
                used_sub.get_used_global_args(matches, global_arg_vec);
            }
        }
    }

    fn _do_parse(
        &mut self,
        raw_args: &mut clap_lex::RawArgs,
        args_cursor: clap_lex::ArgCursor,
    ) -> ClapResult<ArgMatches> {
        debug!("Command::_do_parse");

        // If there are global arguments, or settings we need to propagate them down to subcommands
        // before parsing in case we run into a subcommand
        self._build_self(false);

        let mut matcher = ArgMatcher::new(self);

        // do the real parsing
        let mut parser = Parser::new(self);
        if let Err(error) = parser.get_matches_with(&mut matcher, raw_args, args_cursor) {
            if self.is_set(AppSettings::IgnoreErrors) && error.use_stderr() {
                debug!("Command::_do_parse: ignoring error: {error}");
            } else {
                return Err(error);
            }
        }

        let mut global_arg_vec = Default::default();
        self.get_used_global_args(&matcher, &mut global_arg_vec);

        matcher.propagate_globals(&global_arg_vec);

        Ok(matcher.into_inner())
    }

    /// Prepare for introspecting on all included [`Command`]s
    ///
    /// Call this on the top-level [`Command`] when done building and before reading state for
    /// cases like completions, custom help output, etc.
    pub fn build(&mut self) {
        self._build_recursive(true);
        self._build_bin_names_internal();
    }

    pub(crate) fn _build_recursive(&mut self, expand_help_tree: bool) {
        self._build_self(expand_help_tree);
        for subcmd in self.get_subcommands_mut() {
            subcmd._build_recursive(expand_help_tree);
        }
    }

    pub(crate) fn _build_self(&mut self, expand_help_tree: bool) {
        debug!("Command::_build: name={:?}", self.get_name());
        if !self.settings.is_set(AppSettings::Built) {
            if let Some(deferred) = self.deferred.take() {
                *self = (deferred)(std::mem::take(self));
            }

            // Make sure all the globally set flags apply to us as well
            self.settings = self.settings | self.g_settings;

            if self.is_multicall_set() {
                self.settings.set(AppSettings::SubcommandRequired);
                self.settings.set(AppSettings::DisableHelpFlag);
                self.settings.set(AppSettings::DisableVersionFlag);
            }
            if !cfg!(feature = "help") && self.get_override_help().is_none() {
                self.settings.set(AppSettings::DisableHelpFlag);
                self.settings.set(AppSettings::DisableHelpSubcommand);
            }
            if self.is_set(AppSettings::ArgsNegateSubcommands) {
                self.settings.set(AppSettings::SubcommandsNegateReqs);
            }
            if self.external_value_parser.is_some() {
                self.settings.set(AppSettings::AllowExternalSubcommands);
            }
            if !self.has_subcommands() {
                self.settings.set(AppSettings::DisableHelpSubcommand);
            }

            self._propagate();
            self._check_help_and_version(expand_help_tree);
            self._propagate_global_args();

            let mut pos_counter = 1;
            let hide_pv = self.is_set(AppSettings::HidePossibleValues);
            for a in self.args.args_mut() {
                // Fill in the groups
                for g in &a.groups {
                    if let Some(ag) = self.groups.iter_mut().find(|grp| grp.id == *g) {
                        ag.args.push(a.get_id().clone());
                    } else {
                        let mut ag = ArgGroup::new(g);
                        ag.args.push(a.get_id().clone());
                        self.groups.push(ag);
                    }
                }

                // Figure out implied settings
                a._build();
                if hide_pv && a.is_takes_value_set() {
                    a.settings.set(ArgSettings::HidePossibleValues);
                }
                if a.is_positional() && a.index.is_none() {
                    a.index = Some(pos_counter);
                    pos_counter += 1;
                }
            }

            self.args._build();

            #[allow(deprecated)]
            {
                let highest_idx = self
                    .get_keymap()
                    .keys()
                    .filter_map(|x| {
                        if let crate::mkeymap::KeyType::Position(n) = x {
                            Some(*n)
                        } else {
                            None
                        }
                    })
                    .max()
                    .unwrap_or(0);
                let is_trailing_var_arg_set = self.is_trailing_var_arg_set();
                let is_allow_hyphen_values_set = self.is_allow_hyphen_values_set();
                let is_allow_negative_numbers_set = self.is_allow_negative_numbers_set();
                for arg in self.args.args_mut() {
                    if is_allow_hyphen_values_set && arg.is_takes_value_set() {
                        arg.settings.set(ArgSettings::AllowHyphenValues);
                    }
                    if is_allow_negative_numbers_set && arg.is_takes_value_set() {
                        arg.settings.set(ArgSettings::AllowNegativeNumbers);
                    }
                    if is_trailing_var_arg_set && arg.get_index() == Some(highest_idx) {
                        arg.settings.set(ArgSettings::TrailingVarArg);
                    }
                }
            }

            #[cfg(debug_assertions)]
            assert_app(self);
            self.settings.set(AppSettings::Built);
        } else {
            debug!("Command::_build: already built");
        }
    }

    pub(crate) fn _build_subcommand(&mut self, name: &str) -> Option<&mut Self> {
        use std::fmt::Write;

        let mut mid_string = String::from(" ");
        #[cfg(feature = "usage")]
        if !self.is_subcommand_negates_reqs_set() && !self.is_args_conflicts_with_subcommands_set()
        {
            let reqs = Usage::new(self).get_required_usage_from(&[], None, true); // maybe Some(m)

            for s in &reqs {
                mid_string.push_str(&s.to_string());
                mid_string.push(' ');
            }
        }
        let is_multicall_set = self.is_multicall_set();

        let sc = some!(self.subcommands.iter_mut().find(|s| s.name == name));

        // Display subcommand name, short and long in usage
        let mut sc_names = String::new();
        sc_names.push_str(sc.name.as_str());
        let mut flag_subcmd = false;
        if let Some(l) = sc.get_long_flag() {
            write!(sc_names, "|--{l}").unwrap();
            flag_subcmd = true;
        }
        if let Some(s) = sc.get_short_flag() {
            write!(sc_names, "|-{s}").unwrap();
            flag_subcmd = true;
        }

        if flag_subcmd {
            sc_names = format!("{{{sc_names}}}");
        }

        let usage_name = self
            .bin_name
            .as_ref()
            .map(|bin_name| format!("{bin_name}{mid_string}{sc_names}"))
            .unwrap_or(sc_names);
        sc.usage_name = Some(usage_name);

        // bin_name should be parent's bin_name + [<reqs>] + the sc's name separated by
        // a space
        let bin_name = format!(
            "{}{}{}",
            self.bin_name.as_deref().unwrap_or_default(),
            if self.bin_name.is_some() { " " } else { "" },
            &*sc.name
        );
        debug!(
            "Command::_build_subcommand Setting bin_name of {} to {:?}",
            sc.name, bin_name
        );
        sc.bin_name = Some(bin_name);

        if sc.display_name.is_none() {
            let self_display_name = if is_multicall_set {
                self.display_name.as_deref().unwrap_or("")
            } else {
                self.display_name.as_deref().unwrap_or(&self.name)
            };
            let display_name = format!(
                "{}{}{}",
                self_display_name,
                if !self_display_name.is_empty() {
                    "-"
                } else {
                    ""
                },
                &*sc.name
            );
            debug!(
                "Command::_build_subcommand Setting display_name of {} to {:?}",
                sc.name, display_name
            );
            sc.display_name = Some(display_name);
        }

        // Ensure all args are built and ready to parse
        sc._build_self(false);

        Some(sc)
    }

    fn _build_bin_names_internal(&mut self) {
        debug!("Command::_build_bin_names");

        if !self.is_set(AppSettings::BinNameBuilt) {
            let mut mid_string = String::from(" ");
            #[cfg(feature = "usage")]
            if !self.is_subcommand_negates_reqs_set()
                && !self.is_args_conflicts_with_subcommands_set()
            {
                let reqs = Usage::new(self).get_required_usage_from(&[], None, true); // maybe Some(m)

                for s in &reqs {
                    mid_string.push_str(&s.to_string());
                    mid_string.push(' ');
                }
            }
            let is_multicall_set = self.is_multicall_set();

            let self_bin_name = if is_multicall_set {
                self.bin_name.as_deref().unwrap_or("")
            } else {
                self.bin_name.as_deref().unwrap_or(&self.name)
            }
            .to_owned();

            for sc in &mut self.subcommands {
                debug!("Command::_build_bin_names:iter: bin_name set...");

                if sc.usage_name.is_none() {
                    use std::fmt::Write;
                    // Display subcommand name, short and long in usage
                    let mut sc_names = String::new();
                    sc_names.push_str(sc.name.as_str());
                    let mut flag_subcmd = false;
                    if let Some(l) = sc.get_long_flag() {
                        write!(sc_names, "|--{l}").unwrap();
                        flag_subcmd = true;
                    }
                    if let Some(s) = sc.get_short_flag() {
                        write!(sc_names, "|-{s}").unwrap();
                        flag_subcmd = true;
                    }

                    if flag_subcmd {
                        sc_names = format!("{{{sc_names}}}");
                    }

                    let usage_name = format!("{self_bin_name}{mid_string}{sc_names}");
                    debug!(
                        "Command::_build_bin_names:iter: Setting usage_name of {} to {:?}",
                        sc.name, usage_name
                    );
                    sc.usage_name = Some(usage_name);
                } else {
                    debug!(
                        "Command::_build_bin_names::iter: Using existing usage_name of {} ({:?})",
                        sc.name, sc.usage_name
                    );
                }

                if sc.bin_name.is_none() {
                    let bin_name = format!(
                        "{}{}{}",
                        self_bin_name,
                        if !self_bin_name.is_empty() { " " } else { "" },
                        &*sc.name
                    );
                    debug!(
                        "Command::_build_bin_names:iter: Setting bin_name of {} to {:?}",
                        sc.name, bin_name
                    );
                    sc.bin_name = Some(bin_name);
                } else {
                    debug!(
                        "Command::_build_bin_names::iter: Using existing bin_name of {} ({:?})",
                        sc.name, sc.bin_name
                    );
                }

                if sc.display_name.is_none() {
                    let self_display_name = if is_multicall_set {
                        self.display_name.as_deref().unwrap_or("")
                    } else {
                        self.display_name.as_deref().unwrap_or(&self.name)
                    };
                    let display_name = format!(
                        "{}{}{}",
                        self_display_name,
                        if !self_display_name.is_empty() {
                            "-"
                        } else {
                            ""
                        },
                        &*sc.name
                    );
                    debug!(
                        "Command::_build_bin_names:iter: Setting display_name of {} to {:?}",
                        sc.name, display_name
                    );
                    sc.display_name = Some(display_name);
                } else {
                    debug!(
                        "Command::_build_bin_names::iter: Using existing display_name of {} ({:?})",
                        sc.name, sc.display_name
                    );
                }

                sc._build_bin_names_internal();
            }
            self.set(AppSettings::BinNameBuilt);
        } else {
            debug!("Command::_build_bin_names: already built");
        }
    }

    pub(crate) fn _panic_on_missing_help(&self, help_required_globally: bool) {
        if self.is_set(AppSettings::HelpExpected) || help_required_globally {
            let args_missing_help: Vec<Id> = self
                .args
                .args()
                .filter(|arg| arg.get_help().is_none() && arg.get_long_help().is_none())
                .map(|arg| arg.get_id().clone())
                .collect();

            debug_assert!(args_missing_help.is_empty(),
                    "Command::help_expected is enabled for the Command {}, but at least one of its arguments does not have either `help` or `long_help` set. List of such arguments: {}",
                    self.name,
                    args_missing_help.join(", ")
                );
        }

        for sub_app in &self.subcommands {
            sub_app._panic_on_missing_help(help_required_globally);
        }
    }

    /// Returns the first two arguments that match the condition.
    ///
    /// If fewer than two arguments that match the condition, `None` is returned.
    #[cfg(debug_assertions)]
    pub(crate) fn two_args_of<F>(&self, condition: F) -> Option<(&Arg, &Arg)>
    where
        F: Fn(&Arg) -> bool,
    {
        two_elements_of(self.args.args().filter(|a: &&Arg| condition(a)))
    }

    /// Returns the first two groups that match the condition.
    ///
    /// If fewer than two groups that match the condition, `None` is returned.
    #[allow(unused)]
    fn two_groups_of<F>(&self, condition: F) -> Option<(&ArgGroup, &ArgGroup)>
    where
        F: Fn(&ArgGroup) -> bool,
    {
        two_elements_of(self.groups.iter().filter(|a| condition(a)))
    }

    /// Propagate global args
    pub(crate) fn _propagate_global_args(&mut self) {
        debug!("Command::_propagate_global_args:{}", self.name);

        let autogenerated_help_subcommand = !self.is_disable_help_subcommand_set();

        for sc in &mut self.subcommands {
            if sc.get_name() == "help" && autogenerated_help_subcommand {
                // Avoid propagating args to the autogenerated help subtrees used in completion.
                // This prevents args from showing up during help completions like
                // `myapp help subcmd <TAB>`, which should only suggest subcommands and not args,
                // while still allowing args to show up properly on the generated help message.
                continue;
            }

            for a in self.args.args().filter(|a| a.is_global_set()) {
                if sc.find(&a.id).is_some() {
                    debug!(
                        "Command::_propagate skipping {:?} to {}, already exists",
                        a.id,
                        sc.get_name(),
                    );
                    continue;
                }

                debug!(
                    "Command::_propagate pushing {:?} to {}",
                    a.id,
                    sc.get_name(),
                );
                sc.args.push(a.clone());
            }
        }
    }

    /// Propagate settings
    pub(crate) fn _propagate(&mut self) {
        debug!("Command::_propagate:{}", self.name);
        let mut subcommands = std::mem::take(&mut self.subcommands);
        for sc in &mut subcommands {
            self._propagate_subcommand(sc);
        }
        self.subcommands = subcommands;
    }

    fn _propagate_subcommand(&self, sc: &mut Self) {
        // We have to create a new scope in order to tell rustc the borrow of `sc` is
        // done and to recursively call this method
        {
            if self.settings.is_set(AppSettings::PropagateVersion) {
                if let Some(version) = self.version.as_ref() {
                    sc.version.get_or_insert_with(|| version.clone());
                }
                if let Some(long_version) = self.long_version.as_ref() {
                    sc.long_version.get_or_insert_with(|| long_version.clone());
                }
            }

            sc.settings = sc.settings | self.g_settings;
            sc.g_settings = sc.g_settings | self.g_settings;
            sc.app_ext.update(&self.app_ext);
        }
    }

    pub(crate) fn _check_help_and_version(&mut self, expand_help_tree: bool) {
        debug!(
            "Command::_check_help_and_version:{} expand_help_tree={}",
            self.name, expand_help_tree
        );

        self.long_help_exists = self.long_help_exists_();

        if !self.is_disable_help_flag_set() {
            debug!("Command::_check_help_and_version: Building default --help");
            let mut arg = Arg::new(Id::HELP)
                .short('h')
                .long("help")
                .action(ArgAction::Help);
            if self.long_help_exists {
                arg = arg
                    .help("Print help (see more with '--help')")
                    .long_help("Print help (see a summary with '-h')");
            } else {
                arg = arg.help("Print help");
            }
            // Avoiding `arg_internal` to not be sensitive to `next_help_heading` /
            // `next_display_order`
            self.args.push(arg);
        }
        if !self.is_disable_version_flag_set() {
            debug!("Command::_check_help_and_version: Building default --version");
            let arg = Arg::new(Id::VERSION)
                .short('V')
                .long("version")
                .action(ArgAction::Version)
                .help("Print version");
            // Avoiding `arg_internal` to not be sensitive to `next_help_heading` /
            // `next_display_order`
            self.args.push(arg);
        }

        if !self.is_set(AppSettings::DisableHelpSubcommand) {
            debug!("Command::_check_help_and_version: Building help subcommand");
            let help_about = "Print this message or the help of the given subcommand(s)";

            let mut help_subcmd = if expand_help_tree {
                // Slow code path to recursively clone all other subcommand subtrees under help
                let help_subcmd = Command::new("help")
                    .about(help_about)
                    .global_setting(AppSettings::DisableHelpSubcommand)
                    .subcommands(self.get_subcommands().map(Command::_copy_subtree_for_help));

                let mut help_help_subcmd = Command::new("help").about(help_about);
                help_help_subcmd.version = None;
                help_help_subcmd.long_version = None;
                help_help_subcmd = help_help_subcmd
                    .setting(AppSettings::DisableHelpFlag)
                    .setting(AppSettings::DisableVersionFlag);

                help_subcmd.subcommand(help_help_subcmd)
            } else {
                Command::new("help").about(help_about).arg(
                    Arg::new("subcommand")
                        .action(ArgAction::Append)
                        .num_args(..)
                        .value_name("COMMAND")
                        .help("Print help for the subcommand(s)"),
                )
            };
            self._propagate_subcommand(&mut help_subcmd);

            // The parser acts like this is set, so let's set it so we don't falsely
            // advertise it to the user
            help_subcmd.version = None;
            help_subcmd.long_version = None;
            help_subcmd = help_subcmd
                .setting(AppSettings::DisableHelpFlag)
                .setting(AppSettings::DisableVersionFlag)
                .unset_global_setting(AppSettings::PropagateVersion);

            self.subcommands.push(help_subcmd);
        }
    }

    fn _copy_subtree_for_help(&self) -> Command {
        let mut cmd = Command::new(self.name.clone())
            .hide(self.is_hide_set())
            .global_setting(AppSettings::DisableHelpFlag)
            .global_setting(AppSettings::DisableVersionFlag)
            .subcommands(self.get_subcommands().map(Command::_copy_subtree_for_help));
        if self.get_about().is_some() {
            cmd = cmd.about(self.get_about().unwrap().clone());
        }
        cmd
    }

    pub(crate) fn _render_version(&self, use_long: bool) -> String {
        debug!("Command::_render_version");

        let ver = if use_long {
            self.long_version
                .as_deref()
                .or(self.version.as_deref())
                .unwrap_or_default()
        } else {
            self.version
                .as_deref()
                .or(self.long_version.as_deref())
                .unwrap_or_default()
        };
        let display_name = self.get_display_name().unwrap_or_else(|| self.get_name());
        format!("{display_name} {ver}\n")
    }

    pub(crate) fn format_group(&self, g: &Id) -> StyledStr {
        use std::fmt::Write as _;

        let g_string = self
            .unroll_args_in_group(g)
            .iter()
            .filter_map(|x| self.find(x))
            .map(|x| {
                if x.is_positional() {
                    // Print val_name for positional arguments. e.g. <file_name>
                    x.name_no_brackets()
                } else {
                    // Print usage string for flags arguments, e.g. <--help>
                    x.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("|");
        let placeholder = self.get_styles().get_placeholder();
        let mut styled = StyledStr::new();
        write!(&mut styled, "{placeholder}<{g_string}>{placeholder:#}").unwrap();
        styled
    }
}

/// A workaround:
/// <https://github.com/rust-lang/rust/issues/34511#issuecomment-373423999>
pub(crate) trait Captures<'a> {}
impl<T> Captures<'_> for T {}

// Internal Query Methods
impl Command {
    /// Iterate through the *flags* & *options* arguments.
    #[cfg(any(feature = "usage", feature = "help"))]
    pub(crate) fn get_non_positionals(&self) -> impl Iterator<Item = &Arg> {
        self.get_arguments().filter(|a| !a.is_positional())
    }

    pub(crate) fn find(&self, arg_id: &Id) -> Option<&Arg> {
        self.args.args().find(|a| a.get_id() == arg_id)
    }

    #[inline]
    pub(crate) fn contains_short(&self, s: char) -> bool {
        debug_assert!(
            self.is_set(AppSettings::Built),
            "If Command::_build hasn't been called, manually search through Arg shorts"
        );

        self.args.contains(s)
    }

    #[inline]
    pub(crate) fn set(&mut self, s: AppSettings) {
        self.settings.set(s);
    }

    #[inline]
    pub(crate) fn has_positionals(&self) -> bool {
        self.get_positionals().next().is_some()
    }

    #[cfg(any(feature = "usage", feature = "help"))]
    pub(crate) fn has_visible_subcommands(&self) -> bool {
        self.subcommands
            .iter()
            .any(|sc| sc.name != "help" && !sc.is_set(AppSettings::Hidden))
    }

    /// Check if this subcommand can be referred to as `name`. In other words,
    /// check if `name` is the name of this subcommand or is one of its aliases.
    #[inline]
    pub(crate) fn aliases_to(&self, name: impl AsRef<std::ffi::OsStr>) -> bool {
        let name = name.as_ref();
        self.get_name() == name || self.get_all_aliases().any(|alias| alias == name)
    }

    /// Check if this subcommand can be referred to as `name`. In other words,
    /// check if `name` is the name of this short flag subcommand or is one of its short flag aliases.
    #[inline]
    pub(crate) fn short_flag_aliases_to(&self, flag: char) -> bool {
        Some(flag) == self.short_flag
            || self.get_all_short_flag_aliases().any(|alias| flag == alias)
    }

    /// Check if this subcommand can be referred to as `name`. In other words,
    /// check if `name` is the name of this long flag subcommand or is one of its long flag aliases.
    #[inline]
    pub(crate) fn long_flag_aliases_to(&self, flag: &str) -> bool {
        match self.long_flag.as_ref() {
            Some(long_flag) => {
                long_flag == flag || self.get_all_long_flag_aliases().any(|alias| alias == flag)
            }
            None => self.get_all_long_flag_aliases().any(|alias| alias == flag),
        }
    }

    /// Checks if there is an argument or group with the given id.
    #[cfg(debug_assertions)]
    pub(crate) fn id_exists(&self, id: &Id) -> bool {
        self.args.args().any(|x| x.get_id() == id) || self.groups.iter().any(|x| x.id == *id)
    }

    /// Iterate through the groups this arg is member of.
    pub(crate) fn groups_for_arg<'a>(&'a self, arg: &Id) -> impl Iterator<Item = Id> + 'a {
        debug!("Command::groups_for_arg: id={arg:?}");
        let arg = arg.clone();
        self.groups
            .iter()
            .filter(move |grp| grp.args.iter().any(|a| a == &arg))
            .map(|grp| grp.id.clone())
    }

    pub(crate) fn find_group(&self, group_id: &Id) -> Option<&ArgGroup> {
        self.groups.iter().find(|g| g.id == *group_id)
    }

    /// Iterate through all the names of all subcommands (not recursively), including aliases.
    /// Used for suggestions.
    pub(crate) fn all_subcommand_names(&self) -> impl Iterator<Item = &str> + Captures<'_> {
        self.get_subcommands().flat_map(|sc| {
            let name = sc.get_name();
            let aliases = sc.get_all_aliases();
            std::iter::once(name).chain(aliases)
        })
    }

    pub(crate) fn required_graph(&self) -> ChildGraph<Id> {
        let mut reqs = ChildGraph::with_capacity(5);
        for a in self.args.args().filter(|a| a.is_required_set()) {
            reqs.insert(a.get_id().clone());
        }
        for group in &self.groups {
            if group.required {
                let idx = reqs.insert(group.id.clone());
                for a in &group.requires {
                    reqs.insert_child(idx, a.clone());
                }
            }
        }

        reqs
    }

    pub(crate) fn unroll_args_in_group(&self, group: &Id) -> Vec<Id> {
        debug!("Command::unroll_args_in_group: group={group:?}");
        let mut g_vec = vec![group];
        let mut args = vec![];

        while let Some(g) = g_vec.pop() {
            for n in self
                .groups
                .iter()
                .find(|grp| grp.id == *g)
                .expect(INTERNAL_ERROR_MSG)
                .args
                .iter()
            {
                debug!("Command::unroll_args_in_group:iter: entity={n:?}");
                if !args.contains(n) {
                    if self.find(n).is_some() {
                        debug!("Command::unroll_args_in_group:iter: this is an arg");
                        args.push(n.clone());
                    } else {
                        debug!("Command::unroll_args_in_group:iter: this is a group");
                        g_vec.push(n);
                    }
                }
            }
        }

        args
    }

    pub(crate) fn unroll_arg_requires<F>(&self, func: F, arg: &Id) -> Vec<Id>
    where
        F: Fn(&(ArgPredicate, Id)) -> Option<Id>,
    {
        let mut processed = vec![];
        let mut r_vec = vec![arg];
        let mut args = vec![];

        while let Some(a) = r_vec.pop() {
            if processed.contains(&a) {
                continue;
            }

            processed.push(a);

            if let Some(arg) = self.find(a) {
                for r in arg.requires.iter().filter_map(&func) {
                    if let Some(req) = self.find(&r) {
                        if !req.requires.is_empty() {
                            r_vec.push(req.get_id());
                        }
                    }
                    args.push(r);
                }
            }
        }

        args
    }

    /// Find a flag subcommand name by short flag or an alias
    pub(crate) fn find_short_subcmd(&self, c: char) -> Option<&str> {
        self.get_subcommands()
            .find(|sc| sc.short_flag_aliases_to(c))
            .map(|sc| sc.get_name())
    }

    /// Find a flag subcommand name by long flag or an alias
    pub(crate) fn find_long_subcmd(&self, long: &str) -> Option<&str> {
        self.get_subcommands()
            .find(|sc| sc.long_flag_aliases_to(long))
            .map(|sc| sc.get_name())
    }

    pub(crate) fn write_help_err(&self, mut use_long: bool) -> StyledStr {
        debug!(
            "Command::write_help_err: {}, use_long={:?}",
            self.get_display_name().unwrap_or_else(|| self.get_name()),
            use_long && self.long_help_exists(),
        );

        use_long = use_long && self.long_help_exists();
        let usage = Usage::new(self);

        let mut styled = StyledStr::new();
        write_help(&mut styled, self, &usage, use_long);

        styled
    }

    pub(crate) fn write_version_err(&self, use_long: bool) -> StyledStr {
        let msg = self._render_version(use_long);
        StyledStr::from(msg)
    }

    pub(crate) fn long_help_exists(&self) -> bool {
        debug!("Command::long_help_exists: {}", self.long_help_exists);
        self.long_help_exists
    }

    fn long_help_exists_(&self) -> bool {
        debug!("Command::long_help_exists");
        // In this case, both must be checked. This allows the retention of
        // original formatting, but also ensures that the actual -h or --help
        // specified by the user is sent through. If hide_short_help is not included,
        // then items specified with hidden_short_help will also be hidden.
        let should_long = |v: &Arg| {
            !v.is_hide_set()
                && (v.get_long_help().is_some()
                    || v.is_hide_long_help_set()
                    || v.is_hide_short_help_set()
                    || (!v.is_hide_possible_values_set()
                        && v.get_possible_values()
                            .iter()
                            .any(PossibleValue::should_show_help)))
        };

        // Subcommands aren't checked because we prefer short help for them, deferring to
        // `cmd subcmd --help` for more.
        self.get_long_about().is_some()
            || self.get_before_long_help().is_some()
            || self.get_after_long_help().is_some()
            || self.get_arguments().any(should_long)
    }

    // Should we color the help?
    pub(crate) fn color_help(&self) -> ColorChoice {
        #[cfg(feature = "color")]
        if self.is_disable_colored_help_set() {
            return ColorChoice::Never;
        }

        self.get_color()
    }
}

impl Default for Command {
    fn default() -> Self {
        Self {
            name: Default::default(),
            long_flag: Default::default(),
            short_flag: Default::default(),
            display_name: Default::default(),
            bin_name: Default::default(),
            author: Default::default(),
            version: Default::default(),
            long_version: Default::default(),
            about: Default::default(),
            long_about: Default::default(),
            before_help: Default::default(),
            before_long_help: Default::default(),
            after_help: Default::default(),
            after_long_help: Default::default(),
            aliases: Default::default(),
            short_flag_aliases: Default::default(),
            long_flag_aliases: Default::default(),
            usage_str: Default::default(),
            usage_name: Default::default(),
            help_str: Default::default(),
            disp_ord: Default::default(),
            #[cfg(feature = "help")]
            template: Default::default(),
            settings: Default::default(),
            g_settings: Default::default(),
            args: Default::default(),
            subcommands: Default::default(),
            groups: Default::default(),
            current_help_heading: Default::default(),
            current_disp_ord: Some(0),
            subcommand_value_name: Default::default(),
            subcommand_heading: Default::default(),
            external_value_parser: Default::default(),
            long_help_exists: false,
            deferred: None,
            #[cfg(feature = "unstable-ext")]
            ext: Default::default(),
            app_ext: Default::default(),
        }
    }
}

impl Index<&'_ Id> for Command {
    type Output = Arg;

    fn index(&self, key: &Id) -> &Self::Output {
        self.find(key).expect(INTERNAL_ERROR_MSG)
    }
}

impl From<&'_ Command> for Command {
    fn from(cmd: &'_ Command) -> Self {
        cmd.clone()
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// User-provided data that can be attached to an [`Arg`]
#[cfg(feature = "unstable-ext")]
pub trait CommandExt: Extension {}

#[allow(dead_code)] // atm dependent on features enabled
pub(crate) trait AppExt: Extension {}

#[allow(dead_code)] // atm dependent on features enabled
#[derive(Default, Copy, Clone, Debug)]
struct TermWidth(usize);

impl AppExt for TermWidth {}

#[allow(dead_code)] // atm dependent on features enabled
#[derive(Default, Copy, Clone, Debug)]
struct MaxTermWidth(usize);

impl AppExt for MaxTermWidth {}

/// Returns the first two elements of an iterator as an `Option<(T, T)>`.
///
/// If the iterator has fewer than two elements, it returns `None`.
fn two_elements_of<I, T>(mut iter: I) -> Option<(T, T)>
where
    I: Iterator<Item = T>,
{
    let first = iter.next();
    let second = iter.next();

    match (first, second) {
        (Some(first), Some(second)) => Some((first, second)),
        _ => None,
    }
}

#[test]
fn check_auto_traits() {
    static_assertions::assert_impl_all!(Command: Send, Sync, Unpin);
}
