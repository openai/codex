/// Command line argument parser kind of error
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// Occurs when an [`Arg`][crate::Arg] has a set of possible values,
    /// and the user provides a value which isn't in that set.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, error::ErrorKind};
    /// let result = Command::new("prog")
    ///     .arg(Arg::new("speed")
    ///         .value_parser(["fast", "slow"]))
    ///     .try_get_matches_from(vec!["prog", "other"]);
    /// assert!(result.is_err());
    /// assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidValue);
    /// ```
    InvalidValue,

    /// Occurs when a user provides a flag, option, argument or subcommand which isn't defined.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, arg, error::ErrorKind};
    /// let result = Command::new("prog")
    ///     .arg(arg!(--flag "some flag"))
    ///     .try_get_matches_from(vec!["prog", "--other"]);
    /// assert!(result.is_err());
    /// assert_eq!(result.unwrap_err().kind(), ErrorKind::UnknownArgument);
    /// ```
    UnknownArgument,

    /// Occurs when the user provides an unrecognized [`Subcommand`] which meets the threshold for
    /// being similar enough to an existing subcommand.
    /// If it doesn't meet the threshold, or the 'suggestions' feature is disabled,
    /// the more general [`UnknownArgument`] error is returned.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(feature = "suggestions")] {
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, error::ErrorKind, };
    /// let result = Command::new("prog")
    ///     .subcommand(Command::new("config")
    ///         .about("Used for configuration")
    ///         .arg(Arg::new("config_file")
    ///             .help("The configuration file to use")))
    ///     .try_get_matches_from(vec!["prog", "confi"]);
    /// assert!(result.is_err());
    /// assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidSubcommand);
    /// # }
    /// ```
    ///
    /// [`Subcommand`]: crate::Subcommand
    /// [`UnknownArgument`]: ErrorKind::UnknownArgument
    InvalidSubcommand,

    /// Occurs when the user doesn't use equals for an option that requires equal
    /// sign to provide values.
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, error::ErrorKind, ArgAction};
    /// let res = Command::new("prog")
    ///     .arg(Arg::new("color")
    ///          .action(ArgAction::Set)
    ///          .require_equals(true)
    ///          .long("color"))
    ///     .try_get_matches_from(vec!["prog", "--color", "red"]);
    /// assert!(res.is_err());
    /// assert_eq!(res.unwrap_err().kind(), ErrorKind::NoEquals);
    /// ```
    NoEquals,

    /// Occurs when the user provides a value for an argument with a custom validation and the
    /// value fails that validation.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, error::ErrorKind, value_parser};
    /// fn is_numeric(val: &str) -> Result<(), String> {
    ///     match val.parse::<i64>() {
    ///         Ok(..) => Ok(()),
    ///         Err(..) => Err(String::from("value wasn't a number!")),
    ///     }
    /// }
    ///
    /// let result = Command::new("prog")
    ///     .arg(Arg::new("num")
    ///          .value_parser(value_parser!(u8)))
    ///     .try_get_matches_from(vec!["prog", "NotANumber"]);
    /// assert!(result.is_err());
    /// assert_eq!(result.unwrap_err().kind(), ErrorKind::ValueValidation);
    /// ```
    ValueValidation,

    /// Occurs when a user provides more values for an argument than were defined by setting
    /// [`Arg::num_args`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, error::ErrorKind};
    /// let result = Command::new("prog")
    ///     .arg(Arg::new("arg")
    ///         .num_args(1..=2))
    ///     .try_get_matches_from(vec!["prog", "too", "many", "values"]);
    /// assert!(result.is_err());
    /// assert_eq!(result.unwrap_err().kind(), ErrorKind::TooManyValues);
    /// ```
    /// [`Arg::num_args`]: crate::Arg::num_args()
    TooManyValues,

    /// Occurs when the user provides fewer values for an argument than were defined by setting
    /// [`Arg::num_args`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, error::ErrorKind};
    /// let result = Command::new("prog")
    ///     .arg(Arg::new("some_opt")
    ///         .long("opt")
    ///         .num_args(3..))
    ///     .try_get_matches_from(vec!["prog", "--opt", "too", "few"]);
    /// assert!(result.is_err());
    /// assert_eq!(result.unwrap_err().kind(), ErrorKind::TooFewValues);
    /// ```
    /// [`Arg::num_args`]: crate::Arg::num_args()
    TooFewValues,

    /// Occurs when the user provides a different number of values for an argument than what's
    /// been defined by setting [`Arg::num_args`] or than was implicitly set by
    /// [`Arg::value_names`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, error::ErrorKind, ArgAction};
    /// let result = Command::new("prog")
    ///     .arg(Arg::new("some_opt")
    ///         .long("opt")
    ///         .action(ArgAction::Set)
    ///         .num_args(2))
    ///     .try_get_matches_from(vec!["prog", "--opt", "wrong"]);
    /// assert!(result.is_err());
    /// assert_eq!(result.unwrap_err().kind(), ErrorKind::WrongNumberOfValues);
    /// ```
    ///
    /// [`Arg::num_args`]: crate::Arg::num_args()
    /// [`Arg::value_names`]: crate::Arg::value_names()
    WrongNumberOfValues,

    /// Occurs when the user provides two values which conflict with each other and can't be used
    /// together.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, error::ErrorKind, ArgAction};
    /// let result = Command::new("prog")
    ///     .arg(Arg::new("debug")
    ///         .long("debug")
    ///         .action(ArgAction::SetTrue)
    ///         .conflicts_with("color"))
    ///     .arg(Arg::new("color")
    ///         .long("color")
    ///         .action(ArgAction::SetTrue))
    ///     .try_get_matches_from(vec!["prog", "--debug", "--color"]);
    /// assert!(result.is_err());
    /// assert_eq!(result.unwrap_err().kind(), ErrorKind::ArgumentConflict);
    /// ```
    ArgumentConflict,

    /// Occurs when the user does not provide one or more required arguments.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, error::ErrorKind};
    /// let result = Command::new("prog")
    ///     .arg(Arg::new("debug")
    ///         .required(true))
    ///     .try_get_matches_from(vec!["prog"]);
    /// assert!(result.is_err());
    /// assert_eq!(result.unwrap_err().kind(), ErrorKind::MissingRequiredArgument);
    /// ```
    MissingRequiredArgument,

    /// Occurs when a subcommand is required (as defined by [`Command::subcommand_required`]),
    /// but the user does not provide one.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, error::ErrorKind};
    /// let err = Command::new("prog")
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
    /// [`Command::subcommand_required`]: crate::Command::subcommand_required
    MissingSubcommand,

    /// Occurs when the user provides a value containing invalid UTF-8.
    ///
    /// To allow arbitrary data
    /// - Set [`Arg::value_parser(value_parser!(OsString))`] for argument values
    /// - Set [`Command::external_subcommand_value_parser`] for external-subcommand
    ///   values
    ///
    /// # Platform Specific
    ///
    /// Non-Windows platforms only (such as Linux, Unix, OSX, etc.)
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(unix)] {
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, error::ErrorKind, ArgAction};
    /// # use std::os::unix::ffi::OsStringExt;
    /// # use std::ffi::OsString;
    /// let result = Command::new("prog")
    ///     .arg(Arg::new("utf8")
    ///         .short('u')
    ///         .action(ArgAction::Set))
    ///     .try_get_matches_from(vec![OsString::from("myprog"),
    ///                                 OsString::from("-u"),
    ///                                 OsString::from_vec(vec![0xE9])]);
    /// assert!(result.is_err());
    /// assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidUtf8);
    /// # }
    /// ```
    ///
    /// [`Command::external_subcommand_value_parser`]: crate::Command::external_subcommand_value_parser
    InvalidUtf8,

    /// Not a true "error" as it means `--help` or similar was used.
    /// The help message will be sent to `stdout`.
    ///
    /// **Note**: If the help is displayed due to an error (such as missing subcommands) it will
    /// be sent to `stderr` instead of `stdout`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(feature = "help")] {
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, error::ErrorKind};
    /// let result = Command::new("prog")
    ///     .try_get_matches_from(vec!["prog", "--help"]);
    /// assert!(result.is_err());
    /// assert_eq!(result.unwrap_err().kind(), ErrorKind::DisplayHelp);
    /// # }
    /// ```
    DisplayHelp,

    /// Occurs when either an argument or a [`Subcommand`] is required, as defined by
    /// [`Command::arg_required_else_help`] , but the user did not provide
    /// one.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, error::ErrorKind, };
    /// let result = Command::new("prog")
    ///     .arg_required_else_help(true)
    ///     .subcommand(Command::new("config")
    ///         .about("Used for configuration")
    ///         .arg(Arg::new("config_file")
    ///             .help("The configuration file to use")))
    ///     .try_get_matches_from(vec!["prog"]);
    /// assert!(result.is_err());
    /// assert_eq!(result.unwrap_err().kind(), ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand);
    /// ```
    ///
    /// [`Subcommand`]: crate::Subcommand
    /// [`Command::arg_required_else_help`]: crate::Command::arg_required_else_help
    DisplayHelpOnMissingArgumentOrSubcommand,

    /// Not a true "error" as it means `--version` or similar was used.
    /// The message will be sent to `stdout`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, error::ErrorKind};
    /// let result = Command::new("prog")
    ///     .version("3.0")
    ///     .try_get_matches_from(vec!["prog", "--version"]);
    /// assert!(result.is_err());
    /// assert_eq!(result.unwrap_err().kind(), ErrorKind::DisplayVersion);
    /// ```
    DisplayVersion,

    /// Represents an [I/O error].
    /// Can occur when writing to `stderr` or `stdout` or reading a configuration file.
    ///
    /// [I/O error]: std::io::Error
    Io,

    /// Represents a [Format error] (which is a part of [`Display`]).
    /// Typically caused by writing to `stderr` or `stdout`.
    ///
    /// [`Display`]: std::fmt::Display
    /// [Format error]: std::fmt::Error
    Format,
}

impl ErrorKind {
    /// End-user description of the error case, where relevant
    pub fn as_str(self) -> Option<&'static str> {
        match self {
            Self::InvalidValue => Some("one of the values isn't valid for an argument"),
            Self::UnknownArgument => Some("unexpected argument found"),
            Self::InvalidSubcommand => Some("unrecognized subcommand"),
            Self::NoEquals => Some("equal is needed when assigning values to one of the arguments"),
            Self::ValueValidation => Some("invalid value for one of the arguments"),
            Self::TooManyValues => Some("unexpected value for an argument found"),
            Self::TooFewValues => Some("more values required for an argument"),
            Self::WrongNumberOfValues => Some("too many or too few values for an argument"),
            Self::ArgumentConflict => {
                Some("an argument cannot be used with one or more of the other specified arguments")
            }
            Self::MissingRequiredArgument => {
                Some("one or more required arguments were not provided")
            }
            Self::MissingSubcommand => Some("a subcommand is required but one was not provided"),
            Self::InvalidUtf8 => Some("invalid UTF-8 was detected in one or more arguments"),
            Self::DisplayHelp => None,
            Self::DisplayHelpOnMissingArgumentOrSubcommand => None,
            Self::DisplayVersion => None,
            Self::Io => None,
            Self::Format => None,
        }
    }
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_str().unwrap_or_default().fmt(f)
    }
}
