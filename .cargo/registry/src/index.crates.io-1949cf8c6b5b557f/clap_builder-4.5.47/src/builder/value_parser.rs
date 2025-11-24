use std::convert::TryInto;
use std::ops::RangeBounds;

use crate::builder::Str;
use crate::builder::StyledStr;
use crate::parser::ValueSource;
use crate::util::AnyValue;
use crate::util::AnyValueId;

/// Parse/validate argument values
///
/// Specified with [`Arg::value_parser`][crate::Arg::value_parser].
///
/// `ValueParser` defines how to convert a raw argument value into a validated and typed value for
/// use within an application.
///
/// See
/// - [`value_parser!`][crate::value_parser] for automatically selecting an implementation for a given type
/// - [`ValueParser::new`] for additional [`TypedValueParser`] that can be used
///
/// # Example
///
/// ```rust
/// # use clap_builder as clap;
/// let mut cmd = clap::Command::new("raw")
///     .arg(
///         clap::Arg::new("color")
///             .long("color")
///             .value_parser(["always", "auto", "never"])
///             .default_value("auto")
///     )
///     .arg(
///         clap::Arg::new("hostname")
///             .long("hostname")
///             .value_parser(clap::builder::NonEmptyStringValueParser::new())
///             .action(clap::ArgAction::Set)
///             .required(true)
///     )
///     .arg(
///         clap::Arg::new("port")
///             .long("port")
///             .value_parser(clap::value_parser!(u16).range(3000..))
///             .action(clap::ArgAction::Set)
///             .required(true)
///     );
///
/// let m = cmd.try_get_matches_from_mut(
///     ["cmd", "--hostname", "rust-lang.org", "--port", "3001"]
/// ).unwrap();
///
/// let color: &String = m.get_one("color")
///     .expect("default");
/// assert_eq!(color, "auto");
///
/// let hostname: &String = m.get_one("hostname")
///     .expect("required");
/// assert_eq!(hostname, "rust-lang.org");
///
/// let port: u16 = *m.get_one("port")
///     .expect("required");
/// assert_eq!(port, 3001);
/// ```
pub struct ValueParser(ValueParserInner);

enum ValueParserInner {
    // Common enough to optimize and for possible values
    Bool,
    // Common enough to optimize
    String,
    // Common enough to optimize
    OsString,
    // Common enough to optimize
    PathBuf,
    Other(Box<dyn AnyValueParser>),
}

impl ValueParser {
    /// Custom parser for argument values
    ///
    /// Pre-existing [`TypedValueParser`] implementations include:
    /// - `Fn(&str) -> Result<T, E>`
    /// - [`EnumValueParser`] and  [`PossibleValuesParser`] for static enumerated values
    /// - [`BoolishValueParser`] and [`FalseyValueParser`] for alternative `bool` implementations
    /// - [`RangedI64ValueParser`] and [`RangedU64ValueParser`]
    /// - [`NonEmptyStringValueParser`]
    ///
    /// # Example
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// type EnvVar = (String, Option<String>);
    /// fn parse_env_var(env: &str) -> Result<EnvVar, std::io::Error> {
    ///     if let Some((var, value)) = env.split_once('=') {
    ///         Ok((var.to_owned(), Some(value.to_owned())))
    ///     } else {
    ///         Ok((env.to_owned(), None))
    ///     }
    /// }
    ///
    /// let mut cmd = clap::Command::new("raw")
    ///     .arg(
    ///         clap::Arg::new("env")
    ///             .value_parser(clap::builder::ValueParser::new(parse_env_var))
    ///             .required(true)
    ///     );
    ///
    /// let m = cmd.try_get_matches_from_mut(["cmd", "key=value"]).unwrap();
    /// let port: &EnvVar = m.get_one("env")
    ///     .expect("required");
    /// assert_eq!(*port, ("key".into(), Some("value".into())));
    /// ```
    pub fn new<P>(other: P) -> Self
    where
        P: TypedValueParser,
    {
        Self(ValueParserInner::Other(Box::new(other)))
    }

    /// [`bool`] parser for argument values
    ///
    /// See also:
    /// - [`BoolishValueParser`] for different human readable bool representations
    /// - [`FalseyValueParser`] for assuming non-false is true
    ///
    /// # Example
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// let mut cmd = clap::Command::new("raw")
    ///     .arg(
    ///         clap::Arg::new("download")
    ///             .value_parser(clap::value_parser!(bool))
    ///             .required(true)
    ///     );
    ///
    /// let m = cmd.try_get_matches_from_mut(["cmd", "true"]).unwrap();
    /// let port: bool = *m.get_one("download")
    ///     .expect("required");
    /// assert_eq!(port, true);
    ///
    /// assert!(cmd.try_get_matches_from_mut(["cmd", "forever"]).is_err());
    /// ```
    pub const fn bool() -> Self {
        Self(ValueParserInner::Bool)
    }

    /// [`String`] parser for argument values
    ///
    /// See also:
    /// - [`NonEmptyStringValueParser`]
    ///
    /// # Example
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// let mut cmd = clap::Command::new("raw")
    ///     .arg(
    ///         clap::Arg::new("port")
    ///             .value_parser(clap::value_parser!(String))
    ///             .required(true)
    ///     );
    ///
    /// let m = cmd.try_get_matches_from_mut(["cmd", "80"]).unwrap();
    /// let port: &String = m.get_one("port")
    ///     .expect("required");
    /// assert_eq!(port, "80");
    /// ```
    pub const fn string() -> Self {
        Self(ValueParserInner::String)
    }

    /// [`OsString`][std::ffi::OsString] parser for argument values
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[cfg(unix)] {
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, builder::ValueParser};
    /// use std::ffi::OsString;
    /// use std::os::unix::ffi::{OsStrExt,OsStringExt};
    /// let r = Command::new("myprog")
    ///     .arg(
    ///         Arg::new("arg")
    ///         .required(true)
    ///         .value_parser(ValueParser::os_string())
    ///     )
    ///     .try_get_matches_from(vec![
    ///         OsString::from("myprog"),
    ///         OsString::from_vec(vec![0xe9])
    ///     ]);
    ///
    /// assert!(r.is_ok());
    /// let m = r.unwrap();
    /// let arg: &OsString = m.get_one("arg")
    ///     .expect("required");
    /// assert_eq!(arg.as_bytes(), &[0xe9]);
    /// # }
    /// ```
    pub const fn os_string() -> Self {
        Self(ValueParserInner::OsString)
    }

    /// [`PathBuf`][std::path::PathBuf] parser for argument values
    ///
    /// # Example
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use std::path::PathBuf;
    /// # use std::path::Path;
    /// let mut cmd = clap::Command::new("raw")
    ///     .arg(
    ///         clap::Arg::new("output")
    ///             .value_parser(clap::value_parser!(PathBuf))
    ///             .required(true)
    ///     );
    ///
    /// let m = cmd.try_get_matches_from_mut(["cmd", "hello.txt"]).unwrap();
    /// let port: &PathBuf = m.get_one("output")
    ///     .expect("required");
    /// assert_eq!(port, Path::new("hello.txt"));
    ///
    /// assert!(cmd.try_get_matches_from_mut(["cmd", ""]).is_err());
    /// ```
    pub const fn path_buf() -> Self {
        Self(ValueParserInner::PathBuf)
    }
}

impl ValueParser {
    /// Parse into a `AnyValue`
    ///
    /// When `arg` is `None`, an external subcommand value is being parsed.
    pub(crate) fn parse_ref(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: &std::ffi::OsStr,
        source: ValueSource,
    ) -> Result<AnyValue, crate::Error> {
        self.any_value_parser().parse_ref_(cmd, arg, value, source)
    }

    /// Describes the content of `AnyValue`
    pub fn type_id(&self) -> AnyValueId {
        self.any_value_parser().type_id()
    }

    /// Reflect on enumerated value properties
    ///
    /// Error checking should not be done with this; it is mostly targeted at user-facing
    /// applications like errors and completion.
    pub fn possible_values(
        &self,
    ) -> Option<Box<dyn Iterator<Item = crate::builder::PossibleValue> + '_>> {
        self.any_value_parser().possible_values()
    }

    fn any_value_parser(&self) -> &dyn AnyValueParser {
        match &self.0 {
            ValueParserInner::Bool => &BoolValueParser {},
            ValueParserInner::String => &StringValueParser {},
            ValueParserInner::OsString => &OsStringValueParser {},
            ValueParserInner::PathBuf => &PathBufValueParser {},
            ValueParserInner::Other(o) => o.as_ref(),
        }
    }
}

/// Convert a [`TypedValueParser`] to [`ValueParser`]
///
/// # Example
///
/// ```rust
/// # use clap_builder as clap;
/// let mut cmd = clap::Command::new("raw")
///     .arg(
///         clap::Arg::new("hostname")
///             .long("hostname")
///             .value_parser(clap::builder::NonEmptyStringValueParser::new())
///             .action(clap::ArgAction::Set)
///             .required(true)
///     );
///
/// let m = cmd.try_get_matches_from_mut(
///     ["cmd", "--hostname", "rust-lang.org"]
/// ).unwrap();
///
/// let hostname: &String = m.get_one("hostname")
///     .expect("required");
/// assert_eq!(hostname, "rust-lang.org");
/// ```
impl<P> From<P> for ValueParser
where
    P: TypedValueParser + Send + Sync + 'static,
{
    fn from(p: P) -> Self {
        Self::new(p)
    }
}

impl From<_AnonymousValueParser> for ValueParser {
    fn from(p: _AnonymousValueParser) -> Self {
        p.0
    }
}

/// Create an `i64` [`ValueParser`] from a `N..M` range
///
/// See [`RangedI64ValueParser`] for more control over the output type.
///
/// See also [`RangedU64ValueParser`]
///
/// # Examples
///
/// ```rust
/// # use clap_builder as clap;
/// let mut cmd = clap::Command::new("raw")
///     .arg(
///         clap::Arg::new("port")
///             .long("port")
///             .value_parser(3000..4000)
///             .action(clap::ArgAction::Set)
///             .required(true)
///     );
///
/// let m = cmd.try_get_matches_from_mut(["cmd", "--port", "3001"]).unwrap();
/// let port: i64 = *m.get_one("port")
///     .expect("required");
/// assert_eq!(port, 3001);
/// ```
impl From<std::ops::Range<i64>> for ValueParser {
    fn from(value: std::ops::Range<i64>) -> Self {
        let inner = RangedI64ValueParser::<i64>::new().range(value.start..value.end);
        Self::from(inner)
    }
}

/// Create an `i64` [`ValueParser`] from a `N..=M` range
///
/// See [`RangedI64ValueParser`] for more control over the output type.
///
/// See also [`RangedU64ValueParser`]
///
/// # Examples
///
/// ```rust
/// # use clap_builder as clap;
/// let mut cmd = clap::Command::new("raw")
///     .arg(
///         clap::Arg::new("port")
///             .long("port")
///             .value_parser(3000..=4000)
///             .action(clap::ArgAction::Set)
///             .required(true)
///     );
///
/// let m = cmd.try_get_matches_from_mut(["cmd", "--port", "3001"]).unwrap();
/// let port: i64 = *m.get_one("port")
///     .expect("required");
/// assert_eq!(port, 3001);
/// ```
impl From<std::ops::RangeInclusive<i64>> for ValueParser {
    fn from(value: std::ops::RangeInclusive<i64>) -> Self {
        let inner = RangedI64ValueParser::<i64>::new().range(value.start()..=value.end());
        Self::from(inner)
    }
}

/// Create an `i64` [`ValueParser`] from a `N..` range
///
/// See [`RangedI64ValueParser`] for more control over the output type.
///
/// See also [`RangedU64ValueParser`]
///
/// # Examples
///
/// ```rust
/// # use clap_builder as clap;
/// let mut cmd = clap::Command::new("raw")
///     .arg(
///         clap::Arg::new("port")
///             .long("port")
///             .value_parser(3000..)
///             .action(clap::ArgAction::Set)
///             .required(true)
///     );
///
/// let m = cmd.try_get_matches_from_mut(["cmd", "--port", "3001"]).unwrap();
/// let port: i64 = *m.get_one("port")
///     .expect("required");
/// assert_eq!(port, 3001);
/// ```
impl From<std::ops::RangeFrom<i64>> for ValueParser {
    fn from(value: std::ops::RangeFrom<i64>) -> Self {
        let inner = RangedI64ValueParser::<i64>::new().range(value.start..);
        Self::from(inner)
    }
}

/// Create an `i64` [`ValueParser`] from a `..M` range
///
/// See [`RangedI64ValueParser`] for more control over the output type.
///
/// See also [`RangedU64ValueParser`]
///
/// # Examples
///
/// ```rust
/// # use clap_builder as clap;
/// let mut cmd = clap::Command::new("raw")
///     .arg(
///         clap::Arg::new("port")
///             .long("port")
///             .value_parser(..3000)
///             .action(clap::ArgAction::Set)
///             .required(true)
///     );
///
/// let m = cmd.try_get_matches_from_mut(["cmd", "--port", "80"]).unwrap();
/// let port: i64 = *m.get_one("port")
///     .expect("required");
/// assert_eq!(port, 80);
/// ```
impl From<std::ops::RangeTo<i64>> for ValueParser {
    fn from(value: std::ops::RangeTo<i64>) -> Self {
        let inner = RangedI64ValueParser::<i64>::new().range(..value.end);
        Self::from(inner)
    }
}

/// Create an `i64` [`ValueParser`] from a `..=M` range
///
/// See [`RangedI64ValueParser`] for more control over the output type.
///
/// See also [`RangedU64ValueParser`]
///
/// # Examples
///
/// ```rust
/// # use clap_builder as clap;
/// let mut cmd = clap::Command::new("raw")
///     .arg(
///         clap::Arg::new("port")
///             .long("port")
///             .value_parser(..=3000)
///             .action(clap::ArgAction::Set)
///             .required(true)
///     );
///
/// let m = cmd.try_get_matches_from_mut(["cmd", "--port", "80"]).unwrap();
/// let port: i64 = *m.get_one("port")
///     .expect("required");
/// assert_eq!(port, 80);
/// ```
impl From<std::ops::RangeToInclusive<i64>> for ValueParser {
    fn from(value: std::ops::RangeToInclusive<i64>) -> Self {
        let inner = RangedI64ValueParser::<i64>::new().range(..=value.end);
        Self::from(inner)
    }
}

/// Create an `i64` [`ValueParser`] from a `..` range
///
/// See [`RangedI64ValueParser`] for more control over the output type.
///
/// See also [`RangedU64ValueParser`]
///
/// # Examples
///
/// ```rust
/// # use clap_builder as clap;
/// let mut cmd = clap::Command::new("raw")
///     .arg(
///         clap::Arg::new("port")
///             .long("port")
///             .value_parser(..)
///             .action(clap::ArgAction::Set)
///             .required(true)
///     );
///
/// let m = cmd.try_get_matches_from_mut(["cmd", "--port", "3001"]).unwrap();
/// let port: i64 = *m.get_one("port")
///     .expect("required");
/// assert_eq!(port, 3001);
/// ```
impl From<std::ops::RangeFull> for ValueParser {
    fn from(value: std::ops::RangeFull) -> Self {
        let inner = RangedI64ValueParser::<i64>::new().range(value);
        Self::from(inner)
    }
}

/// Create a [`ValueParser`] with [`PossibleValuesParser`]
///
/// See [`PossibleValuesParser`] for more flexibility in creating the
/// [`PossibleValue`][crate::builder::PossibleValue]s.
///
/// # Examples
///
/// ```rust
/// # use clap_builder as clap;
/// let mut cmd = clap::Command::new("raw")
///     .arg(
///         clap::Arg::new("color")
///             .long("color")
///             .value_parser(["always", "auto", "never"])
///             .default_value("auto")
///     );
///
/// let m = cmd.try_get_matches_from_mut(
///     ["cmd", "--color", "never"]
/// ).unwrap();
///
/// let color: &String = m.get_one("color")
///     .expect("default");
/// assert_eq!(color, "never");
/// ```
impl<P, const C: usize> From<[P; C]> for ValueParser
where
    P: Into<super::PossibleValue>,
{
    fn from(values: [P; C]) -> Self {
        let inner = PossibleValuesParser::from(values);
        Self::from(inner)
    }
}

/// Create a [`ValueParser`] with [`PossibleValuesParser`]
///
/// See [`PossibleValuesParser`] for more flexibility in creating the
/// [`PossibleValue`][crate::builder::PossibleValue]s.
///
/// # Examples
///
/// ```rust
/// # use clap_builder as clap;
/// let possible = vec!["always", "auto", "never"];
/// let mut cmd = clap::Command::new("raw")
///     .arg(
///         clap::Arg::new("color")
///             .long("color")
///             .value_parser(possible)
///             .default_value("auto")
///     );
///
/// let m = cmd.try_get_matches_from_mut(
///     ["cmd", "--color", "never"]
/// ).unwrap();
///
/// let color: &String = m.get_one("color")
///     .expect("default");
/// assert_eq!(color, "never");
/// ```
impl<P> From<Vec<P>> for ValueParser
where
    P: Into<super::PossibleValue>,
{
    fn from(values: Vec<P>) -> Self {
        let inner = PossibleValuesParser::from(values);
        Self::from(inner)
    }
}

impl std::fmt::Debug for ValueParser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match &self.0 {
            ValueParserInner::Bool => f.debug_struct("ValueParser::bool").finish(),
            ValueParserInner::String => f.debug_struct("ValueParser::string").finish(),
            ValueParserInner::OsString => f.debug_struct("ValueParser::os_string").finish(),
            ValueParserInner::PathBuf => f.debug_struct("ValueParser::path_buf").finish(),
            ValueParserInner::Other(o) => write!(f, "ValueParser::other({:?})", o.type_id()),
        }
    }
}

impl Clone for ValueParser {
    fn clone(&self) -> Self {
        Self(match &self.0 {
            ValueParserInner::Bool => ValueParserInner::Bool,
            ValueParserInner::String => ValueParserInner::String,
            ValueParserInner::OsString => ValueParserInner::OsString,
            ValueParserInner::PathBuf => ValueParserInner::PathBuf,
            ValueParserInner::Other(o) => ValueParserInner::Other(o.clone_any()),
        })
    }
}

/// A type-erased wrapper for [`TypedValueParser`].
trait AnyValueParser: Send + Sync + 'static {
    fn parse_ref(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<AnyValue, crate::Error>;

    fn parse_ref_(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: &std::ffi::OsStr,
        _source: ValueSource,
    ) -> Result<AnyValue, crate::Error> {
        self.parse_ref(cmd, arg, value)
    }

    /// Describes the content of `AnyValue`
    fn type_id(&self) -> AnyValueId;

    fn possible_values(
        &self,
    ) -> Option<Box<dyn Iterator<Item = crate::builder::PossibleValue> + '_>>;

    fn clone_any(&self) -> Box<dyn AnyValueParser>;
}

impl<T, P> AnyValueParser for P
where
    T: std::any::Any + Clone + Send + Sync + 'static,
    P: TypedValueParser<Value = T>,
{
    fn parse_ref(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<AnyValue, crate::Error> {
        let value = ok!(TypedValueParser::parse_ref(self, cmd, arg, value));
        Ok(AnyValue::new(value))
    }

    fn parse_ref_(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: &std::ffi::OsStr,
        source: ValueSource,
    ) -> Result<AnyValue, crate::Error> {
        let value = ok!(TypedValueParser::parse_ref_(self, cmd, arg, value, source));
        Ok(AnyValue::new(value))
    }

    fn type_id(&self) -> AnyValueId {
        AnyValueId::of::<T>()
    }

    fn possible_values(
        &self,
    ) -> Option<Box<dyn Iterator<Item = crate::builder::PossibleValue> + '_>> {
        P::possible_values(self)
    }

    fn clone_any(&self) -> Box<dyn AnyValueParser> {
        Box::new(self.clone())
    }
}

/// Parse/validate argument values
///
/// As alternatives to implementing `TypedValueParser`,
/// - Use `Fn(&str) -> Result<T, E>` which implements `TypedValueParser`
/// - [`TypedValueParser::map`] or [`TypedValueParser::try_map`] to adapt an existing `TypedValueParser`
///
/// See `ValueParserFactory` to register `TypedValueParser::Value` with
/// [`value_parser!`][crate::value_parser].
///
/// # Example
///
/// ```rust
/// # #[cfg(feature = "error-context")] {
/// # use clap_builder as clap;
/// # use clap::error::ErrorKind;
/// # use clap::error::ContextKind;
/// # use clap::error::ContextValue;
/// #[derive(Clone)]
/// struct Custom(u32);
///
/// #[derive(Clone)]
/// struct CustomValueParser;
///
/// impl clap::builder::TypedValueParser for CustomValueParser {
///     type Value = Custom;
///
///     fn parse_ref(
///         &self,
///         cmd: &clap::Command,
///         arg: Option<&clap::Arg>,
///         value: &std::ffi::OsStr,
///     ) -> Result<Self::Value, clap::Error> {
///         let inner = clap::value_parser!(u32);
///         let val = inner.parse_ref(cmd, arg, value)?;
///
///         const INVALID_VALUE: u32 = 10;
///         if val == INVALID_VALUE {
///             let mut err = clap::Error::new(ErrorKind::ValueValidation)
///                 .with_cmd(cmd);
///             if let Some(arg) = arg {
///                 err.insert(ContextKind::InvalidArg, ContextValue::String(arg.to_string()));
///             }
///             err.insert(ContextKind::InvalidValue, ContextValue::String(INVALID_VALUE.to_string()));
///             return Err(err);
///         }
///
///         Ok(Custom(val))
///     }
/// }
/// # }
/// ```
pub trait TypedValueParser: Clone + Send + Sync + 'static {
    /// Argument's value type
    type Value: Send + Sync + Clone;

    /// Parse the argument value
    ///
    /// When `arg` is `None`, an external subcommand value is being parsed.
    fn parse_ref(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, crate::Error>;

    /// Parse the argument value
    ///
    /// When `arg` is `None`, an external subcommand value is being parsed.
    fn parse_ref_(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: &std::ffi::OsStr,
        _source: ValueSource,
    ) -> Result<Self::Value, crate::Error> {
        self.parse_ref(cmd, arg, value)
    }

    /// Parse the argument value
    ///
    /// When `arg` is `None`, an external subcommand value is being parsed.
    fn parse(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: std::ffi::OsString,
    ) -> Result<Self::Value, crate::Error> {
        self.parse_ref(cmd, arg, &value)
    }

    /// Parse the argument value
    ///
    /// When `arg` is `None`, an external subcommand value is being parsed.
    fn parse_(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: std::ffi::OsString,
        _source: ValueSource,
    ) -> Result<Self::Value, crate::Error> {
        self.parse(cmd, arg, value)
    }

    /// Reflect on enumerated value properties
    ///
    /// Error checking should not be done with this; it is mostly targeted at user-facing
    /// applications like errors and completion.
    fn possible_values(
        &self,
    ) -> Option<Box<dyn Iterator<Item = crate::builder::PossibleValue> + '_>> {
        None
    }

    /// Adapt a `TypedValueParser` from one value to another
    ///
    /// # Example
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// # use clap::Arg;
    /// # use clap::builder::TypedValueParser as _;
    /// # use clap::builder::BoolishValueParser;
    /// let cmd = Command::new("mycmd")
    ///     .arg(
    ///         Arg::new("flag")
    ///             .long("flag")
    ///             .action(clap::ArgAction::SetTrue)
    ///             .value_parser(
    ///                 BoolishValueParser::new()
    ///                 .map(|b| -> usize {
    ///                     if b { 10 } else { 5 }
    ///                 })
    ///             )
    ///     );
    ///
    /// let matches = cmd.clone().try_get_matches_from(["mycmd", "--flag"]).unwrap();
    /// assert!(matches.contains_id("flag"));
    /// assert_eq!(
    ///     matches.get_one::<usize>("flag").copied(),
    ///     Some(10)
    /// );
    ///
    /// let matches = cmd.try_get_matches_from(["mycmd"]).unwrap();
    /// assert!(matches.contains_id("flag"));
    /// assert_eq!(
    ///     matches.get_one::<usize>("flag").copied(),
    ///     Some(5)
    /// );
    /// ```
    fn map<T, F>(self, func: F) -> MapValueParser<Self, F>
    where
        T: Send + Sync + Clone,
        F: Fn(Self::Value) -> T + Clone,
    {
        MapValueParser::new(self, func)
    }

    /// Adapt a `TypedValueParser` from one value to another
    ///
    /// # Example
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use std::ffi::OsString;
    /// # use std::ffi::OsStr;
    /// # use std::path::PathBuf;
    /// # use std::path::Path;
    /// # use clap::Command;
    /// # use clap::Arg;
    /// # use clap::builder::TypedValueParser as _;
    /// # use clap::builder::OsStringValueParser;
    /// let cmd = Command::new("mycmd")
    ///     .arg(
    ///         Arg::new("flag")
    ///             .long("flag")
    ///             .value_parser(
    ///                 OsStringValueParser::new()
    ///                 .try_map(verify_ext)
    ///             )
    ///     );
    ///
    /// fn verify_ext(os: OsString) -> Result<PathBuf, &'static str> {
    ///     let path = PathBuf::from(os);
    ///     if path.extension() != Some(OsStr::new("rs")) {
    ///         return Err("only Rust files are supported");
    ///     }
    ///     Ok(path)
    /// }
    ///
    /// let error = cmd.clone().try_get_matches_from(["mycmd", "--flag", "foo.txt"]).unwrap_err();
    /// error.print();
    ///
    /// let matches = cmd.try_get_matches_from(["mycmd", "--flag", "foo.rs"]).unwrap();
    /// assert!(matches.contains_id("flag"));
    /// assert_eq!(
    ///     matches.get_one::<PathBuf>("flag").map(|s| s.as_path()),
    ///     Some(Path::new("foo.rs"))
    /// );
    /// ```
    fn try_map<T, E, F>(self, func: F) -> TryMapValueParser<Self, F>
    where
        F: Fn(Self::Value) -> Result<T, E> + Clone + Send + Sync + 'static,
        T: Send + Sync + Clone,
        E: Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
    {
        TryMapValueParser::new(self, func)
    }
}

impl<F, T, E> TypedValueParser for F
where
    F: Fn(&str) -> Result<T, E> + Clone + Send + Sync + 'static,
    E: Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
    T: Send + Sync + Clone,
{
    type Value = T;

    fn parse_ref(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, crate::Error> {
        let value = ok!(value.to_str().ok_or_else(|| {
            crate::Error::invalid_utf8(
                cmd,
                crate::output::Usage::new(cmd).create_usage_with_title(&[]),
            )
        }));
        let value = ok!((self)(value).map_err(|e| {
            let arg = arg
                .map(|a| a.to_string())
                .unwrap_or_else(|| "...".to_owned());
            crate::Error::value_validation(arg, value.to_owned(), e.into()).with_cmd(cmd)
        }));
        Ok(value)
    }
}

/// Implementation for [`ValueParser::string`]
///
/// Useful for composing new [`TypedValueParser`]s
#[derive(Copy, Clone, Debug)]
#[non_exhaustive]
pub struct StringValueParser {}

impl StringValueParser {
    /// Implementation for [`ValueParser::string`]
    pub fn new() -> Self {
        Self {}
    }
}

impl TypedValueParser for StringValueParser {
    type Value = String;

    fn parse_ref(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, crate::Error> {
        TypedValueParser::parse(self, cmd, arg, value.to_owned())
    }

    fn parse(
        &self,
        cmd: &crate::Command,
        _arg: Option<&crate::Arg>,
        value: std::ffi::OsString,
    ) -> Result<Self::Value, crate::Error> {
        let value = ok!(value.into_string().map_err(|_| {
            crate::Error::invalid_utf8(
                cmd,
                crate::output::Usage::new(cmd).create_usage_with_title(&[]),
            )
        }));
        Ok(value)
    }
}

impl Default for StringValueParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Implementation for [`ValueParser::os_string`]
///
/// Useful for composing new [`TypedValueParser`]s
#[derive(Copy, Clone, Debug)]
#[non_exhaustive]
pub struct OsStringValueParser {}

impl OsStringValueParser {
    /// Implementation for [`ValueParser::os_string`]
    pub fn new() -> Self {
        Self {}
    }
}

impl TypedValueParser for OsStringValueParser {
    type Value = std::ffi::OsString;

    fn parse_ref(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, crate::Error> {
        TypedValueParser::parse(self, cmd, arg, value.to_owned())
    }

    fn parse(
        &self,
        _cmd: &crate::Command,
        _arg: Option<&crate::Arg>,
        value: std::ffi::OsString,
    ) -> Result<Self::Value, crate::Error> {
        Ok(value)
    }
}

impl Default for OsStringValueParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Implementation for [`ValueParser::path_buf`]
///
/// Useful for composing new [`TypedValueParser`]s
#[derive(Copy, Clone, Debug)]
#[non_exhaustive]
pub struct PathBufValueParser {}

impl PathBufValueParser {
    /// Implementation for [`ValueParser::path_buf`]
    pub fn new() -> Self {
        Self {}
    }
}

impl TypedValueParser for PathBufValueParser {
    type Value = std::path::PathBuf;

    fn parse_ref(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, crate::Error> {
        TypedValueParser::parse(self, cmd, arg, value.to_owned())
    }

    fn parse(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: std::ffi::OsString,
    ) -> Result<Self::Value, crate::Error> {
        if value.is_empty() {
            return Err(crate::Error::empty_value(
                cmd,
                &[],
                arg.map(ToString::to_string)
                    .unwrap_or_else(|| "...".to_owned()),
            ));
        }
        Ok(Self::Value::from(value))
    }
}

impl Default for PathBufValueParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse an [`ValueEnum`][crate::ValueEnum] value.
///
/// See also:
/// - [`PossibleValuesParser`]
///
/// # Example
///
/// ```rust
/// # use clap_builder as clap;
/// # use std::ffi::OsStr;
/// # use clap::ColorChoice;
/// # use clap::builder::TypedValueParser;
/// # let cmd = clap::Command::new("test");
/// # let arg = None;
///
/// // Usage
/// let mut cmd = clap::Command::new("raw")
///     .arg(
///         clap::Arg::new("color")
///             .value_parser(clap::builder::EnumValueParser::<ColorChoice>::new())
///             .required(true)
///     );
///
/// let m = cmd.try_get_matches_from_mut(["cmd", "always"]).unwrap();
/// let port: ColorChoice = *m.get_one("color")
///     .expect("required");
/// assert_eq!(port, ColorChoice::Always);
///
/// // Semantics
/// let value_parser = clap::builder::EnumValueParser::<ColorChoice>::new();
/// // or
/// let value_parser = clap::value_parser!(ColorChoice);
/// assert!(value_parser.parse_ref(&cmd, arg, OsStr::new("random")).is_err());
/// assert!(value_parser.parse_ref(&cmd, arg, OsStr::new("")).is_err());
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("always")).unwrap(), ColorChoice::Always);
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("auto")).unwrap(), ColorChoice::Auto);
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("never")).unwrap(), ColorChoice::Never);
/// ```
#[derive(Clone, Debug)]
pub struct EnumValueParser<E: crate::ValueEnum + Clone + Send + Sync + 'static>(
    std::marker::PhantomData<E>,
);

impl<E: crate::ValueEnum + Clone + Send + Sync + 'static> EnumValueParser<E> {
    /// Parse an [`ValueEnum`][crate::ValueEnum]
    pub fn new() -> Self {
        let phantom: std::marker::PhantomData<E> = Default::default();
        Self(phantom)
    }
}

impl<E: crate::ValueEnum + Clone + Send + Sync + 'static> TypedValueParser for EnumValueParser<E> {
    type Value = E;

    fn parse_ref(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, crate::Error> {
        let ignore_case = arg.map(|a| a.is_ignore_case_set()).unwrap_or(false);
        let possible_vals = || {
            E::value_variants()
                .iter()
                .filter_map(|v| v.to_possible_value())
                .filter(|v| !v.is_hide_set())
                .map(|v| v.get_name().to_owned())
                .collect::<Vec<_>>()
        };

        let value = ok!(value.to_str().ok_or_else(|| {
            crate::Error::invalid_value(
                cmd,
                value.to_string_lossy().into_owned(),
                &possible_vals(),
                arg.map(ToString::to_string)
                    .unwrap_or_else(|| "...".to_owned()),
            )
        }));
        let value = ok!(E::value_variants()
            .iter()
            .find(|v| {
                v.to_possible_value()
                    .expect("ValueEnum::value_variants contains only values with a corresponding ValueEnum::to_possible_value")
                    .matches(value, ignore_case)
            })
            .ok_or_else(|| {
            crate::Error::invalid_value(
                cmd,
                value.to_owned(),
                &possible_vals(),
                arg.map(ToString::to_string)
                    .unwrap_or_else(|| "...".to_owned()),
            )
            }))
            .clone();
        Ok(value)
    }

    fn possible_values(
        &self,
    ) -> Option<Box<dyn Iterator<Item = crate::builder::PossibleValue> + '_>> {
        Some(Box::new(
            E::value_variants()
                .iter()
                .filter_map(|v| v.to_possible_value()),
        ))
    }
}

impl<E: crate::ValueEnum + Clone + Send + Sync + 'static> Default for EnumValueParser<E> {
    fn default() -> Self {
        Self::new()
    }
}

/// Verify the value is from an enumerated set of [`PossibleValue`][crate::builder::PossibleValue].
///
/// See also:
/// - [`EnumValueParser`] for directly supporting [`ValueEnum`][crate::ValueEnum] types
/// - [`TypedValueParser::map`] for adapting values to a more specialized type, like an external
///   enums that can't implement [`ValueEnum`][crate::ValueEnum]
///
/// # Example
///
/// Usage:
/// ```rust
/// # use clap_builder as clap;
/// let mut cmd = clap::Command::new("raw")
///     .arg(
///         clap::Arg::new("color")
///             .value_parser(clap::builder::PossibleValuesParser::new(["always", "auto", "never"]))
///             .required(true)
///     );
///
/// let m = cmd.try_get_matches_from_mut(["cmd", "always"]).unwrap();
/// let port: &String = m.get_one("color")
///     .expect("required");
/// assert_eq!(port, "always");
/// ```
///
/// Semantics:
/// ```rust
/// # use clap_builder as clap;
/// # use std::ffi::OsStr;
/// # use clap::builder::TypedValueParser;
/// # let cmd = clap::Command::new("test");
/// # let arg = None;
/// let value_parser = clap::builder::PossibleValuesParser::new(["always", "auto", "never"]);
/// assert!(value_parser.parse_ref(&cmd, arg, OsStr::new("random")).is_err());
/// assert!(value_parser.parse_ref(&cmd, arg, OsStr::new("")).is_err());
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("always")).unwrap(), "always");
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("auto")).unwrap(), "auto");
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("never")).unwrap(), "never");
/// ```
#[derive(Clone, Debug)]
pub struct PossibleValuesParser(Vec<super::PossibleValue>);

impl PossibleValuesParser {
    /// Verify the value is from an enumerated set of [`PossibleValue`][crate::builder::PossibleValue].
    pub fn new(values: impl Into<PossibleValuesParser>) -> Self {
        values.into()
    }
}

impl TypedValueParser for PossibleValuesParser {
    type Value = String;

    fn parse_ref(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, crate::Error> {
        TypedValueParser::parse(self, cmd, arg, value.to_owned())
    }

    fn parse(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: std::ffi::OsString,
    ) -> Result<String, crate::Error> {
        let value = ok!(value.into_string().map_err(|_| {
            crate::Error::invalid_utf8(
                cmd,
                crate::output::Usage::new(cmd).create_usage_with_title(&[]),
            )
        }));

        let ignore_case = arg.map(|a| a.is_ignore_case_set()).unwrap_or(false);
        if self.0.iter().any(|v| v.matches(&value, ignore_case)) {
            Ok(value)
        } else {
            let possible_vals = self
                .0
                .iter()
                .filter(|v| !v.is_hide_set())
                .map(|v| v.get_name().to_owned())
                .collect::<Vec<_>>();

            Err(crate::Error::invalid_value(
                cmd,
                value,
                &possible_vals,
                arg.map(ToString::to_string)
                    .unwrap_or_else(|| "...".to_owned()),
            ))
        }
    }

    fn possible_values(
        &self,
    ) -> Option<Box<dyn Iterator<Item = crate::builder::PossibleValue> + '_>> {
        Some(Box::new(self.0.iter().cloned()))
    }
}

impl<I, T> From<I> for PossibleValuesParser
where
    I: IntoIterator<Item = T>,
    T: Into<super::PossibleValue>,
{
    fn from(values: I) -> Self {
        Self(values.into_iter().map(|t| t.into()).collect())
    }
}

/// Parse number that fall within a range of values
///
/// <div class="warning">
///
/// **NOTE:** To capture negative values, you will also need to set
/// [`Arg::allow_negative_numbers`][crate::Arg::allow_negative_numbers] or
/// [`Arg::allow_hyphen_values`][crate::Arg::allow_hyphen_values].
///
/// </div>
///
/// # Example
///
/// Usage:
/// ```rust
/// # use clap_builder as clap;
/// let mut cmd = clap::Command::new("raw")
///     .arg(
///         clap::Arg::new("port")
///             .long("port")
///             .value_parser(clap::value_parser!(u16).range(3000..))
///             .action(clap::ArgAction::Set)
///             .required(true)
///     );
///
/// let m = cmd.try_get_matches_from_mut(["cmd", "--port", "3001"]).unwrap();
/// let port: u16 = *m.get_one("port")
///     .expect("required");
/// assert_eq!(port, 3001);
/// ```
///
/// Semantics:
/// ```rust
/// # use clap_builder as clap;
/// # use std::ffi::OsStr;
/// # use clap::builder::TypedValueParser;
/// # let cmd = clap::Command::new("test");
/// # let arg = None;
/// let value_parser = clap::builder::RangedI64ValueParser::<i32>::new().range(-1..200);
/// assert!(value_parser.parse_ref(&cmd, arg, OsStr::new("random")).is_err());
/// assert!(value_parser.parse_ref(&cmd, arg, OsStr::new("")).is_err());
/// assert!(value_parser.parse_ref(&cmd, arg, OsStr::new("-200")).is_err());
/// assert!(value_parser.parse_ref(&cmd, arg, OsStr::new("300")).is_err());
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("-1")).unwrap(), -1);
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("0")).unwrap(), 0);
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("50")).unwrap(), 50);
/// ```
#[derive(Copy, Clone, Debug)]
pub struct RangedI64ValueParser<T: TryFrom<i64> + Clone + Send + Sync = i64> {
    bounds: (std::ops::Bound<i64>, std::ops::Bound<i64>),
    target: std::marker::PhantomData<T>,
}

impl<T: TryFrom<i64> + Clone + Send + Sync> RangedI64ValueParser<T> {
    /// Select full range of `i64`
    pub fn new() -> Self {
        Self::from(..)
    }

    /// Narrow the supported range
    pub fn range<B: RangeBounds<i64>>(mut self, range: B) -> Self {
        // Consideration: when the user does `value_parser!(u8).range()`
        // - Avoid programming mistakes by accidentally expanding the range
        // - Make it convenient to limit the range like with `..10`
        let start = match range.start_bound() {
            l @ std::ops::Bound::Included(i) => {
                debug_assert!(
                    self.bounds.contains(i),
                    "{} must be in {:?}",
                    i,
                    self.bounds
                );
                l.cloned()
            }
            l @ std::ops::Bound::Excluded(i) => {
                debug_assert!(
                    self.bounds.contains(&i.saturating_add(1)),
                    "{} must be in {:?}",
                    i,
                    self.bounds
                );
                l.cloned()
            }
            std::ops::Bound::Unbounded => self.bounds.start_bound().cloned(),
        };
        let end = match range.end_bound() {
            l @ std::ops::Bound::Included(i) => {
                debug_assert!(
                    self.bounds.contains(i),
                    "{} must be in {:?}",
                    i,
                    self.bounds
                );
                l.cloned()
            }
            l @ std::ops::Bound::Excluded(i) => {
                debug_assert!(
                    self.bounds.contains(&i.saturating_sub(1)),
                    "{} must be in {:?}",
                    i,
                    self.bounds
                );
                l.cloned()
            }
            std::ops::Bound::Unbounded => self.bounds.end_bound().cloned(),
        };
        self.bounds = (start, end);
        self
    }

    fn format_bounds(&self) -> String {
        let mut result = match self.bounds.0 {
            std::ops::Bound::Included(i) => i.to_string(),
            std::ops::Bound::Excluded(i) => i.saturating_add(1).to_string(),
            std::ops::Bound::Unbounded => i64::MIN.to_string(),
        };
        result.push_str("..");
        match self.bounds.1 {
            std::ops::Bound::Included(i) => {
                result.push('=');
                result.push_str(&i.to_string());
            }
            std::ops::Bound::Excluded(i) => {
                result.push_str(&i.to_string());
            }
            std::ops::Bound::Unbounded => {
                result.push_str(&i64::MAX.to_string());
            }
        }
        result
    }
}

impl<T: TryFrom<i64> + Clone + Send + Sync + 'static> TypedValueParser for RangedI64ValueParser<T>
where
    <T as TryFrom<i64>>::Error: Send + Sync + 'static + std::error::Error + ToString,
{
    type Value = T;

    fn parse_ref(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        raw_value: &std::ffi::OsStr,
    ) -> Result<Self::Value, crate::Error> {
        let value = ok!(raw_value.to_str().ok_or_else(|| {
            crate::Error::invalid_utf8(
                cmd,
                crate::output::Usage::new(cmd).create_usage_with_title(&[]),
            )
        }));
        let value = ok!(value.parse::<i64>().map_err(|err| {
            let arg = arg
                .map(|a| a.to_string())
                .unwrap_or_else(|| "...".to_owned());
            crate::Error::value_validation(
                arg,
                raw_value.to_string_lossy().into_owned(),
                err.into(),
            )
            .with_cmd(cmd)
        }));
        if !self.bounds.contains(&value) {
            let arg = arg
                .map(|a| a.to_string())
                .unwrap_or_else(|| "...".to_owned());
            return Err(crate::Error::value_validation(
                arg,
                raw_value.to_string_lossy().into_owned(),
                format!("{} is not in {}", value, self.format_bounds()).into(),
            )
            .with_cmd(cmd));
        }

        let value: Result<Self::Value, _> = value.try_into();
        let value = ok!(value.map_err(|err| {
            let arg = arg
                .map(|a| a.to_string())
                .unwrap_or_else(|| "...".to_owned());
            crate::Error::value_validation(
                arg,
                raw_value.to_string_lossy().into_owned(),
                err.into(),
            )
            .with_cmd(cmd)
        }));

        Ok(value)
    }
}

impl<T: TryFrom<i64> + Clone + Send + Sync, B: RangeBounds<i64>> From<B>
    for RangedI64ValueParser<T>
{
    fn from(range: B) -> Self {
        Self {
            bounds: (range.start_bound().cloned(), range.end_bound().cloned()),
            target: Default::default(),
        }
    }
}

impl<T: TryFrom<i64> + Clone + Send + Sync> Default for RangedI64ValueParser<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse number that fall within a range of values
///
/// # Example
///
/// Usage:
/// ```rust
/// # use clap_builder as clap;
/// let mut cmd = clap::Command::new("raw")
///     .arg(
///         clap::Arg::new("port")
///             .long("port")
///             .value_parser(clap::value_parser!(u64).range(3000..))
///             .action(clap::ArgAction::Set)
///             .required(true)
///     );
///
/// let m = cmd.try_get_matches_from_mut(["cmd", "--port", "3001"]).unwrap();
/// let port: u64 = *m.get_one("port")
///     .expect("required");
/// assert_eq!(port, 3001);
/// ```
///
/// Semantics:
/// ```rust
/// # use clap_builder as clap;
/// # use std::ffi::OsStr;
/// # use clap::builder::TypedValueParser;
/// # let cmd = clap::Command::new("test");
/// # let arg = None;
/// let value_parser = clap::builder::RangedU64ValueParser::<u32>::new().range(0..200);
/// assert!(value_parser.parse_ref(&cmd, arg, OsStr::new("random")).is_err());
/// assert!(value_parser.parse_ref(&cmd, arg, OsStr::new("")).is_err());
/// assert!(value_parser.parse_ref(&cmd, arg, OsStr::new("-200")).is_err());
/// assert!(value_parser.parse_ref(&cmd, arg, OsStr::new("300")).is_err());
/// assert!(value_parser.parse_ref(&cmd, arg, OsStr::new("-1")).is_err());
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("0")).unwrap(), 0);
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("50")).unwrap(), 50);
/// ```
#[derive(Copy, Clone, Debug)]
pub struct RangedU64ValueParser<T: TryFrom<u64> = u64> {
    bounds: (std::ops::Bound<u64>, std::ops::Bound<u64>),
    target: std::marker::PhantomData<T>,
}

impl<T: TryFrom<u64>> RangedU64ValueParser<T> {
    /// Select full range of `u64`
    pub fn new() -> Self {
        Self::from(..)
    }

    /// Narrow the supported range
    pub fn range<B: RangeBounds<u64>>(mut self, range: B) -> Self {
        // Consideration: when the user does `value_parser!(u8).range()`
        // - Avoid programming mistakes by accidentally expanding the range
        // - Make it convenient to limit the range like with `..10`
        let start = match range.start_bound() {
            l @ std::ops::Bound::Included(i) => {
                debug_assert!(
                    self.bounds.contains(i),
                    "{} must be in {:?}",
                    i,
                    self.bounds
                );
                l.cloned()
            }
            l @ std::ops::Bound::Excluded(i) => {
                debug_assert!(
                    self.bounds.contains(&i.saturating_add(1)),
                    "{} must be in {:?}",
                    i,
                    self.bounds
                );
                l.cloned()
            }
            std::ops::Bound::Unbounded => self.bounds.start_bound().cloned(),
        };
        let end = match range.end_bound() {
            l @ std::ops::Bound::Included(i) => {
                debug_assert!(
                    self.bounds.contains(i),
                    "{} must be in {:?}",
                    i,
                    self.bounds
                );
                l.cloned()
            }
            l @ std::ops::Bound::Excluded(i) => {
                debug_assert!(
                    self.bounds.contains(&i.saturating_sub(1)),
                    "{} must be in {:?}",
                    i,
                    self.bounds
                );
                l.cloned()
            }
            std::ops::Bound::Unbounded => self.bounds.end_bound().cloned(),
        };
        self.bounds = (start, end);
        self
    }

    fn format_bounds(&self) -> String {
        let mut result = match self.bounds.0 {
            std::ops::Bound::Included(i) => i.to_string(),
            std::ops::Bound::Excluded(i) => i.saturating_add(1).to_string(),
            std::ops::Bound::Unbounded => u64::MIN.to_string(),
        };
        result.push_str("..");
        match self.bounds.1 {
            std::ops::Bound::Included(i) => {
                result.push('=');
                result.push_str(&i.to_string());
            }
            std::ops::Bound::Excluded(i) => {
                result.push_str(&i.to_string());
            }
            std::ops::Bound::Unbounded => {
                result.push_str(&u64::MAX.to_string());
            }
        }
        result
    }
}

impl<T: TryFrom<u64> + Clone + Send + Sync + 'static> TypedValueParser for RangedU64ValueParser<T>
where
    <T as TryFrom<u64>>::Error: Send + Sync + 'static + std::error::Error + ToString,
{
    type Value = T;

    fn parse_ref(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        raw_value: &std::ffi::OsStr,
    ) -> Result<Self::Value, crate::Error> {
        let value = ok!(raw_value.to_str().ok_or_else(|| {
            crate::Error::invalid_utf8(
                cmd,
                crate::output::Usage::new(cmd).create_usage_with_title(&[]),
            )
        }));
        let value = ok!(value.parse::<u64>().map_err(|err| {
            let arg = arg
                .map(|a| a.to_string())
                .unwrap_or_else(|| "...".to_owned());
            crate::Error::value_validation(
                arg,
                raw_value.to_string_lossy().into_owned(),
                err.into(),
            )
            .with_cmd(cmd)
        }));
        if !self.bounds.contains(&value) {
            let arg = arg
                .map(|a| a.to_string())
                .unwrap_or_else(|| "...".to_owned());
            return Err(crate::Error::value_validation(
                arg,
                raw_value.to_string_lossy().into_owned(),
                format!("{} is not in {}", value, self.format_bounds()).into(),
            )
            .with_cmd(cmd));
        }

        let value: Result<Self::Value, _> = value.try_into();
        let value = ok!(value.map_err(|err| {
            let arg = arg
                .map(|a| a.to_string())
                .unwrap_or_else(|| "...".to_owned());
            crate::Error::value_validation(
                arg,
                raw_value.to_string_lossy().into_owned(),
                err.into(),
            )
            .with_cmd(cmd)
        }));

        Ok(value)
    }
}

impl<T: TryFrom<u64>, B: RangeBounds<u64>> From<B> for RangedU64ValueParser<T> {
    fn from(range: B) -> Self {
        Self {
            bounds: (range.start_bound().cloned(), range.end_bound().cloned()),
            target: Default::default(),
        }
    }
}

impl<T: TryFrom<u64>> Default for RangedU64ValueParser<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Implementation for [`ValueParser::bool`]
///
/// Useful for composing new [`TypedValueParser`]s
#[derive(Copy, Clone, Debug)]
#[non_exhaustive]
pub struct BoolValueParser {}

impl BoolValueParser {
    /// Implementation for [`ValueParser::bool`]
    pub fn new() -> Self {
        Self {}
    }

    fn possible_values() -> impl Iterator<Item = crate::builder::PossibleValue> {
        ["true", "false"]
            .iter()
            .copied()
            .map(crate::builder::PossibleValue::new)
    }
}

impl TypedValueParser for BoolValueParser {
    type Value = bool;

    fn parse_ref(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, crate::Error> {
        let value = if value == std::ffi::OsStr::new("true") {
            true
        } else if value == std::ffi::OsStr::new("false") {
            false
        } else {
            // Intentionally showing hidden as we hide all of them
            let possible_vals = Self::possible_values()
                .map(|v| v.get_name().to_owned())
                .collect::<Vec<_>>();

            return Err(crate::Error::invalid_value(
                cmd,
                value.to_string_lossy().into_owned(),
                &possible_vals,
                arg.map(ToString::to_string)
                    .unwrap_or_else(|| "...".to_owned()),
            ));
        };
        Ok(value)
    }

    fn possible_values(
        &self,
    ) -> Option<Box<dyn Iterator<Item = crate::builder::PossibleValue> + '_>> {
        Some(Box::new(Self::possible_values()))
    }
}

impl Default for BoolValueParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse false-like string values, everything else is `true`
///
/// See also:
/// - [`ValueParser::bool`] for assuming non-false is true
/// - [`BoolishValueParser`] for different human readable bool representations
///
/// # Example
///
/// Usage:
/// ```rust
/// # use clap_builder as clap;
/// let mut cmd = clap::Command::new("raw")
///     .arg(
///         clap::Arg::new("append")
///             .value_parser(clap::builder::FalseyValueParser::new())
///             .required(true)
///     );
///
/// let m = cmd.try_get_matches_from_mut(["cmd", "true"]).unwrap();
/// let port: bool = *m.get_one("append")
///     .expect("required");
/// assert_eq!(port, true);
/// ```
///
/// Semantics:
/// ```rust
/// # use clap_builder as clap;
/// # use std::ffi::OsStr;
/// # use clap::builder::TypedValueParser;
/// # let cmd = clap::Command::new("test");
/// # let arg = None;
/// let value_parser = clap::builder::FalseyValueParser::new();
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("random")).unwrap(), true);
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("100")).unwrap(), true);
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("")).unwrap(), false);
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("false")).unwrap(), false);
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("No")).unwrap(), false);
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("oFF")).unwrap(), false);
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("0")).unwrap(), false);
/// ```
#[derive(Copy, Clone, Debug)]
#[non_exhaustive]
pub struct FalseyValueParser {}

impl FalseyValueParser {
    /// Parse false-like string values, everything else is `true`
    pub fn new() -> Self {
        Self {}
    }

    fn possible_values() -> impl Iterator<Item = crate::builder::PossibleValue> {
        crate::util::TRUE_LITERALS
            .iter()
            .chain(crate::util::FALSE_LITERALS.iter())
            .copied()
            .map(|l| crate::builder::PossibleValue::new(l).hide(l != "true" && l != "false"))
    }
}

impl TypedValueParser for FalseyValueParser {
    type Value = bool;

    fn parse_ref(
        &self,
        cmd: &crate::Command,
        _arg: Option<&crate::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, crate::Error> {
        let value = ok!(value.to_str().ok_or_else(|| {
            crate::Error::invalid_utf8(
                cmd,
                crate::output::Usage::new(cmd).create_usage_with_title(&[]),
            )
        }));
        let value = if value.is_empty() {
            false
        } else {
            crate::util::str_to_bool(value).unwrap_or(true)
        };
        Ok(value)
    }

    fn possible_values(
        &self,
    ) -> Option<Box<dyn Iterator<Item = crate::builder::PossibleValue> + '_>> {
        Some(Box::new(Self::possible_values()))
    }
}

impl Default for FalseyValueParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse bool-like string values
///
/// See also:
/// - [`ValueParser::bool`] for different human readable bool representations
/// - [`FalseyValueParser`] for assuming non-false is true
///
/// # Example
///
/// Usage:
/// ```rust
/// # use clap_builder as clap;
/// let mut cmd = clap::Command::new("raw")
///     .arg(
///         clap::Arg::new("append")
///             .value_parser(clap::builder::BoolishValueParser::new())
///             .required(true)
///     );
///
/// let m = cmd.try_get_matches_from_mut(["cmd", "true"]).unwrap();
/// let port: bool = *m.get_one("append")
///     .expect("required");
/// assert_eq!(port, true);
/// ```
///
/// Semantics:
/// ```rust
/// # use clap_builder as clap;
/// # use std::ffi::OsStr;
/// # use clap::builder::TypedValueParser;
/// # let cmd = clap::Command::new("test");
/// # let arg = None;
/// let value_parser = clap::builder::BoolishValueParser::new();
/// assert!(value_parser.parse_ref(&cmd, arg, OsStr::new("random")).is_err());
/// assert!(value_parser.parse_ref(&cmd, arg, OsStr::new("")).is_err());
/// assert!(value_parser.parse_ref(&cmd, arg, OsStr::new("100")).is_err());
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("true")).unwrap(), true);
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("Yes")).unwrap(), true);
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("oN")).unwrap(), true);
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("1")).unwrap(), true);
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("false")).unwrap(), false);
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("No")).unwrap(), false);
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("oFF")).unwrap(), false);
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("0")).unwrap(), false);
/// ```
#[derive(Copy, Clone, Debug)]
#[non_exhaustive]
pub struct BoolishValueParser {}

impl BoolishValueParser {
    /// Parse bool-like string values
    pub fn new() -> Self {
        Self {}
    }

    fn possible_values() -> impl Iterator<Item = crate::builder::PossibleValue> {
        crate::util::TRUE_LITERALS
            .iter()
            .chain(crate::util::FALSE_LITERALS.iter())
            .copied()
            .map(|l| crate::builder::PossibleValue::new(l).hide(l != "true" && l != "false"))
    }
}

impl TypedValueParser for BoolishValueParser {
    type Value = bool;

    fn parse_ref(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, crate::Error> {
        let value = ok!(value.to_str().ok_or_else(|| {
            crate::Error::invalid_utf8(
                cmd,
                crate::output::Usage::new(cmd).create_usage_with_title(&[]),
            )
        }));
        let value = ok!(crate::util::str_to_bool(value).ok_or_else(|| {
            let arg = arg
                .map(|a| a.to_string())
                .unwrap_or_else(|| "...".to_owned());
            crate::Error::value_validation(arg, value.to_owned(), "value was not a boolean".into())
                .with_cmd(cmd)
        }));
        Ok(value)
    }

    fn possible_values(
        &self,
    ) -> Option<Box<dyn Iterator<Item = crate::builder::PossibleValue> + '_>> {
        Some(Box::new(Self::possible_values()))
    }
}

impl Default for BoolishValueParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse non-empty string values
///
/// See also:
/// - [`ValueParser::string`]
///
/// # Example
///
/// Usage:
/// ```rust
/// # use clap_builder as clap;
/// let mut cmd = clap::Command::new("raw")
///     .arg(
///         clap::Arg::new("append")
///             .value_parser(clap::builder::NonEmptyStringValueParser::new())
///             .required(true)
///     );
///
/// let m = cmd.try_get_matches_from_mut(["cmd", "true"]).unwrap();
/// let port: &String = m.get_one("append")
///     .expect("required");
/// assert_eq!(port, "true");
/// ```
///
/// Semantics:
/// ```rust
/// # use clap_builder as clap;
/// # use std::ffi::OsStr;
/// # use clap::builder::TypedValueParser;
/// # let cmd = clap::Command::new("test");
/// # let arg = None;
/// let value_parser = clap::builder::NonEmptyStringValueParser::new();
/// assert_eq!(value_parser.parse_ref(&cmd, arg, OsStr::new("random")).unwrap(), "random");
/// assert!(value_parser.parse_ref(&cmd, arg, OsStr::new("")).is_err());
/// ```
#[derive(Copy, Clone, Debug)]
#[non_exhaustive]
pub struct NonEmptyStringValueParser {}

impl NonEmptyStringValueParser {
    /// Parse non-empty string values
    pub fn new() -> Self {
        Self {}
    }
}

impl TypedValueParser for NonEmptyStringValueParser {
    type Value = String;

    fn parse_ref(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, crate::Error> {
        if value.is_empty() {
            return Err(crate::Error::empty_value(
                cmd,
                &[],
                arg.map(ToString::to_string)
                    .unwrap_or_else(|| "...".to_owned()),
            ));
        }
        let value = ok!(value.to_str().ok_or_else(|| {
            crate::Error::invalid_utf8(
                cmd,
                crate::output::Usage::new(cmd).create_usage_with_title(&[]),
            )
        }));
        Ok(value.to_owned())
    }
}

impl Default for NonEmptyStringValueParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Adapt a `TypedValueParser` from one value to another
///
/// See [`TypedValueParser::map`]
#[derive(Clone, Debug)]
pub struct MapValueParser<P, F> {
    parser: P,
    func: F,
}

impl<P, F, T> MapValueParser<P, F>
where
    P: TypedValueParser,
    P::Value: Send + Sync + Clone,
    F: Fn(P::Value) -> T + Clone,
    T: Send + Sync + Clone,
{
    fn new(parser: P, func: F) -> Self {
        Self { parser, func }
    }
}

impl<P, F, T> TypedValueParser for MapValueParser<P, F>
where
    P: TypedValueParser,
    P::Value: Send + Sync + Clone,
    F: Fn(P::Value) -> T + Clone + Send + Sync + 'static,
    T: Send + Sync + Clone,
{
    type Value = T;

    fn parse_ref(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, crate::Error> {
        let value = ok!(self.parser.parse_ref(cmd, arg, value));
        let value = (self.func)(value);
        Ok(value)
    }

    fn parse(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: std::ffi::OsString,
    ) -> Result<Self::Value, crate::Error> {
        let value = ok!(self.parser.parse(cmd, arg, value));
        let value = (self.func)(value);
        Ok(value)
    }

    fn possible_values(
        &self,
    ) -> Option<Box<dyn Iterator<Item = crate::builder::PossibleValue> + '_>> {
        self.parser.possible_values()
    }
}

/// Adapt a `TypedValueParser` from one value to another
///
/// See [`TypedValueParser::try_map`]
#[derive(Clone, Debug)]
pub struct TryMapValueParser<P, F> {
    parser: P,
    func: F,
}

impl<P, F, T, E> TryMapValueParser<P, F>
where
    P: TypedValueParser,
    P::Value: Send + Sync + Clone,
    F: Fn(P::Value) -> Result<T, E> + Clone + Send + Sync + 'static,
    T: Send + Sync + Clone,
    E: Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
{
    fn new(parser: P, func: F) -> Self {
        Self { parser, func }
    }
}

impl<P, F, T, E> TypedValueParser for TryMapValueParser<P, F>
where
    P: TypedValueParser,
    P::Value: Send + Sync + Clone,
    F: Fn(P::Value) -> Result<T, E> + Clone + Send + Sync + 'static,
    T: Send + Sync + Clone,
    E: Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
{
    type Value = T;

    fn parse_ref(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, crate::Error> {
        let mid_value = ok!(self.parser.parse_ref(cmd, arg, value));
        let value = ok!((self.func)(mid_value).map_err(|e| {
            let arg = arg
                .map(|a| a.to_string())
                .unwrap_or_else(|| "...".to_owned());
            crate::Error::value_validation(arg, value.to_string_lossy().into_owned(), e.into())
                .with_cmd(cmd)
        }));
        Ok(value)
    }

    fn possible_values(
        &self,
    ) -> Option<Box<dyn Iterator<Item = crate::builder::PossibleValue> + '_>> {
        self.parser.possible_values()
    }
}

/// When encountered, report [`ErrorKind::UnknownArgument`][crate::error::ErrorKind::UnknownArgument]
///
/// Useful to help users migrate, either from old versions or similar tools.
///
/// # Examples
///
/// ```rust
/// # use clap_builder as clap;
/// # use clap::Command;
/// # use clap::Arg;
/// let cmd = Command::new("mycmd")
///     .args([
///         Arg::new("current-dir")
///             .short('C'),
///         Arg::new("current-dir-unknown")
///             .long("cwd")
///             .aliases(["current-dir", "directory", "working-directory", "root"])
///             .value_parser(clap::builder::UnknownArgumentValueParser::suggest_arg("-C"))
///             .hide(true),
///     ]);
///
/// // Use a supported version of the argument
/// let matches = cmd.clone().try_get_matches_from(["mycmd", "-C", ".."]).unwrap();
/// assert!(matches.contains_id("current-dir"));
/// assert_eq!(
///     matches.get_many::<String>("current-dir").unwrap_or_default().map(|v| v.as_str()).collect::<Vec<_>>(),
///     vec![".."]
/// );
///
/// // Use one of the invalid versions
/// let err = cmd.try_get_matches_from(["mycmd", "--cwd", ".."]).unwrap_err();
/// assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);
/// ```
#[derive(Clone, Debug)]
pub struct UnknownArgumentValueParser {
    arg: Option<Str>,
    suggestions: Vec<StyledStr>,
}

impl UnknownArgumentValueParser {
    /// Suggest an alternative argument
    pub fn suggest_arg(arg: impl Into<Str>) -> Self {
        Self {
            arg: Some(arg.into()),
            suggestions: Default::default(),
        }
    }

    /// Provide a general suggestion
    pub fn suggest(text: impl Into<StyledStr>) -> Self {
        Self {
            arg: Default::default(),
            suggestions: vec![text.into()],
        }
    }

    /// Extend the suggestions
    pub fn and_suggest(mut self, text: impl Into<StyledStr>) -> Self {
        self.suggestions.push(text.into());
        self
    }
}

impl TypedValueParser for UnknownArgumentValueParser {
    type Value = String;

    fn parse_ref(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, crate::Error> {
        TypedValueParser::parse_ref_(self, cmd, arg, value, ValueSource::CommandLine)
    }

    fn parse_ref_(
        &self,
        cmd: &crate::Command,
        arg: Option<&crate::Arg>,
        _value: &std::ffi::OsStr,
        source: ValueSource,
    ) -> Result<Self::Value, crate::Error> {
        match source {
            ValueSource::DefaultValue => {
                TypedValueParser::parse_ref_(&StringValueParser::new(), cmd, arg, _value, source)
            }
            ValueSource::EnvVariable | ValueSource::CommandLine => {
                let arg = match arg {
                    Some(arg) => arg.to_string(),
                    None => "..".to_owned(),
                };
                let err = crate::Error::unknown_argument(
                    cmd,
                    arg,
                    self.arg.as_ref().map(|s| (s.as_str().to_owned(), None)),
                    false,
                    crate::output::Usage::new(cmd).create_usage_with_title(&[]),
                );
                #[cfg(feature = "error-context")]
                let err = {
                    debug_assert_eq!(
                        err.get(crate::error::ContextKind::Suggested),
                        None,
                        "Assuming `Error::unknown_argument` doesn't apply any `Suggested` so we can without caution"
                    );
                    err.insert_context_unchecked(
                        crate::error::ContextKind::Suggested,
                        crate::error::ContextValue::StyledStrs(self.suggestions.clone()),
                    )
                };
                Err(err)
            }
        }
    }
}

/// Register a type with [`value_parser!`][crate::value_parser!]
///
/// # Example
///
/// ```rust
/// # use clap_builder as clap;
/// #[derive(Copy, Clone, Debug)]
/// pub struct Custom(u32);
///
/// impl clap::builder::ValueParserFactory for Custom {
///     type Parser = CustomValueParser;
///     fn value_parser() -> Self::Parser {
///         CustomValueParser
///     }
/// }
///
/// #[derive(Clone, Debug)]
/// pub struct CustomValueParser;
/// impl clap::builder::TypedValueParser for CustomValueParser {
///     type Value = Custom;
///
///     fn parse_ref(
///         &self,
///         cmd: &clap::Command,
///         arg: Option<&clap::Arg>,
///         value: &std::ffi::OsStr,
///     ) -> Result<Self::Value, clap::Error> {
///         let inner = clap::value_parser!(u32);
///         let val = inner.parse_ref(cmd, arg, value)?;
///         Ok(Custom(val))
///     }
/// }
///
/// let parser: CustomValueParser = clap::value_parser!(Custom);
/// ```
pub trait ValueParserFactory {
    /// Generated parser, usually [`ValueParser`].
    ///
    /// It should at least be a type that supports `Into<ValueParser>`.  A non-`ValueParser` type
    /// allows the caller to do further initialization on the parser.
    type Parser;

    /// Create the specified [`Self::Parser`]
    fn value_parser() -> Self::Parser;
}
impl ValueParserFactory for String {
    type Parser = ValueParser;
    fn value_parser() -> Self::Parser {
        ValueParser::string() // Default `clap_derive` to optimized implementation
    }
}
impl ValueParserFactory for Box<str> {
    type Parser = MapValueParser<StringValueParser, fn(String) -> Box<str>>;
    fn value_parser() -> Self::Parser {
        StringValueParser::new().map(String::into_boxed_str)
    }
}
impl ValueParserFactory for std::ffi::OsString {
    type Parser = ValueParser;
    fn value_parser() -> Self::Parser {
        ValueParser::os_string() // Default `clap_derive` to optimized implementation
    }
}
impl ValueParserFactory for Box<std::ffi::OsStr> {
    type Parser =
        MapValueParser<OsStringValueParser, fn(std::ffi::OsString) -> Box<std::ffi::OsStr>>;
    fn value_parser() -> Self::Parser {
        OsStringValueParser::new().map(std::ffi::OsString::into_boxed_os_str)
    }
}
impl ValueParserFactory for std::path::PathBuf {
    type Parser = ValueParser;
    fn value_parser() -> Self::Parser {
        ValueParser::path_buf() // Default `clap_derive` to optimized implementation
    }
}
impl ValueParserFactory for Box<std::path::Path> {
    type Parser =
        MapValueParser<PathBufValueParser, fn(std::path::PathBuf) -> Box<std::path::Path>>;
    fn value_parser() -> Self::Parser {
        PathBufValueParser::new().map(std::path::PathBuf::into_boxed_path)
    }
}
impl ValueParserFactory for bool {
    type Parser = ValueParser;
    fn value_parser() -> Self::Parser {
        ValueParser::bool() // Default `clap_derive` to optimized implementation
    }
}
impl ValueParserFactory for u8 {
    type Parser = RangedI64ValueParser<u8>;
    fn value_parser() -> Self::Parser {
        let start: i64 = u8::MIN.into();
        let end: i64 = u8::MAX.into();
        RangedI64ValueParser::new().range(start..=end)
    }
}
impl ValueParserFactory for i8 {
    type Parser = RangedI64ValueParser<i8>;
    fn value_parser() -> Self::Parser {
        let start: i64 = i8::MIN.into();
        let end: i64 = i8::MAX.into();
        RangedI64ValueParser::new().range(start..=end)
    }
}
impl ValueParserFactory for u16 {
    type Parser = RangedI64ValueParser<u16>;
    fn value_parser() -> Self::Parser {
        let start: i64 = u16::MIN.into();
        let end: i64 = u16::MAX.into();
        RangedI64ValueParser::new().range(start..=end)
    }
}
impl ValueParserFactory for i16 {
    type Parser = RangedI64ValueParser<i16>;
    fn value_parser() -> Self::Parser {
        let start: i64 = i16::MIN.into();
        let end: i64 = i16::MAX.into();
        RangedI64ValueParser::new().range(start..=end)
    }
}
impl ValueParserFactory for u32 {
    type Parser = RangedI64ValueParser<u32>;
    fn value_parser() -> Self::Parser {
        let start: i64 = u32::MIN.into();
        let end: i64 = u32::MAX.into();
        RangedI64ValueParser::new().range(start..=end)
    }
}
impl ValueParserFactory for i32 {
    type Parser = RangedI64ValueParser<i32>;
    fn value_parser() -> Self::Parser {
        let start: i64 = i32::MIN.into();
        let end: i64 = i32::MAX.into();
        RangedI64ValueParser::new().range(start..=end)
    }
}
impl ValueParserFactory for u64 {
    type Parser = RangedU64ValueParser<u64>;
    fn value_parser() -> Self::Parser {
        RangedU64ValueParser::new()
    }
}
impl ValueParserFactory for i64 {
    type Parser = RangedI64ValueParser<i64>;
    fn value_parser() -> Self::Parser {
        RangedI64ValueParser::new()
    }
}
impl<T> ValueParserFactory for std::num::Saturating<T>
where
    T: ValueParserFactory,
    <T as ValueParserFactory>::Parser: TypedValueParser<Value = T>,
    T: Send + Sync + Clone,
{
    type Parser =
        MapValueParser<<T as ValueParserFactory>::Parser, fn(T) -> std::num::Saturating<T>>;
    fn value_parser() -> Self::Parser {
        T::value_parser().map(std::num::Saturating)
    }
}
impl<T> ValueParserFactory for std::num::Wrapping<T>
where
    T: ValueParserFactory,
    <T as ValueParserFactory>::Parser: TypedValueParser<Value = T>,
    T: Send + Sync + Clone,
{
    type Parser = MapValueParser<<T as ValueParserFactory>::Parser, fn(T) -> std::num::Wrapping<T>>;
    fn value_parser() -> Self::Parser {
        T::value_parser().map(std::num::Wrapping)
    }
}
impl<T> ValueParserFactory for Box<T>
where
    T: ValueParserFactory,
    <T as ValueParserFactory>::Parser: TypedValueParser<Value = T>,
    T: Send + Sync + Clone,
{
    type Parser = MapValueParser<<T as ValueParserFactory>::Parser, fn(T) -> Box<T>>;
    fn value_parser() -> Self::Parser {
        T::value_parser().map(Box::new)
    }
}
impl<T> ValueParserFactory for std::sync::Arc<T>
where
    T: ValueParserFactory,
    <T as ValueParserFactory>::Parser: TypedValueParser<Value = T>,
    T: Send + Sync + Clone,
{
    type Parser = MapValueParser<<T as ValueParserFactory>::Parser, fn(T) -> std::sync::Arc<T>>;
    fn value_parser() -> Self::Parser {
        T::value_parser().map(std::sync::Arc::new)
    }
}

#[doc(hidden)]
#[derive(Debug)]
#[allow(non_camel_case_types)]
pub struct _infer_ValueParser_for<T>(std::marker::PhantomData<T>);

impl<T> _infer_ValueParser_for<T> {
    #[doc(hidden)]
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self(Default::default())
    }
}

/// Unstable [`ValueParser`]
///
/// Implementation may change to more specific instance in the future
#[doc(hidden)]
#[derive(Debug)]
pub struct _AnonymousValueParser(ValueParser);

#[doc(hidden)]
pub mod impl_prelude {
    use super::*;

    #[doc(hidden)]
    #[allow(non_camel_case_types)]
    pub trait _impls_ValueParserFactory: private::_impls_ValueParserFactorySealed {
        type Parser;
        fn value_parser(&self) -> Self::Parser;
    }
    impl<P: ValueParserFactory> _impls_ValueParserFactory for &&&&&&_infer_ValueParser_for<P> {
        type Parser = P::Parser;
        fn value_parser(&self) -> Self::Parser {
            P::value_parser()
        }
    }

    #[doc(hidden)]
    #[allow(non_camel_case_types)]
    pub trait _impls_ValueEnum: private::_impls_ValueEnumSealed {
        type Output;

        fn value_parser(&self) -> Self::Output;
    }
    impl<E: crate::ValueEnum + Clone + Send + Sync + 'static> _impls_ValueEnum
        for &&&&&_infer_ValueParser_for<E>
    {
        type Output = EnumValueParser<E>;

        fn value_parser(&self) -> Self::Output {
            EnumValueParser::<E>::new()
        }
    }

    #[doc(hidden)]
    #[allow(non_camel_case_types)]
    pub trait _impls_From_OsString: private::_impls_From_OsStringSealed {
        fn value_parser(&self) -> _AnonymousValueParser;
    }
    impl<FromOsString> _impls_From_OsString for &&&&_infer_ValueParser_for<FromOsString>
    where
        FromOsString: From<std::ffi::OsString> + std::any::Any + Clone + Send + Sync + 'static,
    {
        fn value_parser(&self) -> _AnonymousValueParser {
            _AnonymousValueParser(
                OsStringValueParser::new()
                    .map(|s| FromOsString::from(s))
                    .into(),
            )
        }
    }

    #[doc(hidden)]
    #[allow(non_camel_case_types)]
    pub trait _impls_From_OsStr: private::_impls_From_OsStrSealed {
        fn value_parser(&self) -> _AnonymousValueParser;
    }
    impl<FromOsStr> _impls_From_OsStr for &&&_infer_ValueParser_for<FromOsStr>
    where
        FromOsStr:
            for<'s> From<&'s std::ffi::OsStr> + std::any::Any + Clone + Send + Sync + 'static,
    {
        fn value_parser(&self) -> _AnonymousValueParser {
            _AnonymousValueParser(
                OsStringValueParser::new()
                    .map(|s| FromOsStr::from(&s))
                    .into(),
            )
        }
    }

    #[doc(hidden)]
    #[allow(non_camel_case_types)]
    pub trait _impls_From_String: private::_impls_From_StringSealed {
        fn value_parser(&self) -> _AnonymousValueParser;
    }
    impl<FromString> _impls_From_String for &&_infer_ValueParser_for<FromString>
    where
        FromString: From<String> + std::any::Any + Clone + Send + Sync + 'static,
    {
        fn value_parser(&self) -> _AnonymousValueParser {
            _AnonymousValueParser(StringValueParser::new().map(|s| FromString::from(s)).into())
        }
    }

    #[doc(hidden)]
    #[allow(non_camel_case_types)]
    pub trait _impls_From_str: private::_impls_From_strSealed {
        fn value_parser(&self) -> _AnonymousValueParser;
    }
    impl<FromStr> _impls_From_str for &_infer_ValueParser_for<FromStr>
    where
        FromStr: for<'s> From<&'s str> + std::any::Any + Clone + Send + Sync + 'static,
    {
        fn value_parser(&self) -> _AnonymousValueParser {
            _AnonymousValueParser(StringValueParser::new().map(|s| FromStr::from(&s)).into())
        }
    }

    #[doc(hidden)]
    #[allow(non_camel_case_types)]
    pub trait _impls_FromStr: private::_impls_FromStrSealed {
        fn value_parser(&self) -> _AnonymousValueParser;
    }
    impl<Parse> _impls_FromStr for _infer_ValueParser_for<Parse>
    where
        Parse: std::str::FromStr + std::any::Any + Clone + Send + Sync + 'static,
        <Parse as std::str::FromStr>::Err: Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
    {
        fn value_parser(&self) -> _AnonymousValueParser {
            let func: fn(&str) -> Result<Parse, <Parse as std::str::FromStr>::Err> =
                Parse::from_str;
            _AnonymousValueParser(ValueParser::new(func))
        }
    }
}

/// Select a [`ValueParser`] implementation from the intended type
///
/// Supported types
/// - [`ValueParserFactory` types][ValueParserFactory], including
///   - [Native types][ValueParser]: `bool`, `String`, `OsString`, `PathBuf`
///   - [Ranged numeric types][RangedI64ValueParser]: `u8`, `i8`, `u16`, `i16`, `u32`, `i32`, `u64`, `i64`
/// - [`ValueEnum` types][crate::ValueEnum]
/// - [`From<OsString>` types][std::convert::From] and [`From<&OsStr>` types][std::convert::From]
/// - [`From<String>` types][std::convert::From] and [`From<&str>` types][std::convert::From]
/// - [`FromStr` types][std::str::FromStr], including usize, isize
///
/// # Example
///
/// Usage:
/// ```rust
/// # use clap_builder as clap;
/// # use std::path::PathBuf;
/// # use std::path::Path;
/// let mut cmd = clap::Command::new("raw")
///     .arg(
///         clap::Arg::new("output")
///             .value_parser(clap::value_parser!(PathBuf))
///             .required(true)
///     );
///
/// let m = cmd.try_get_matches_from_mut(["cmd", "file.txt"]).unwrap();
/// let port: &PathBuf = m.get_one("output")
///     .expect("required");
/// assert_eq!(port, Path::new("file.txt"));
/// ```
///
/// Example mappings:
/// ```rust
/// # use clap_builder as clap;
/// # use clap::ColorChoice;
/// // Built-in types
/// let parser = clap::value_parser!(String);
/// assert_eq!(format!("{parser:?}"), "ValueParser::string");
/// let parser = clap::value_parser!(std::ffi::OsString);
/// assert_eq!(format!("{parser:?}"), "ValueParser::os_string");
/// let parser = clap::value_parser!(std::path::PathBuf);
/// assert_eq!(format!("{parser:?}"), "ValueParser::path_buf");
/// clap::value_parser!(u16).range(3000..);
/// clap::value_parser!(u64).range(3000..);
///
/// // FromStr types
/// let parser = clap::value_parser!(usize);
/// assert_eq!(format!("{parser:?}"), "_AnonymousValueParser(ValueParser::other(usize))");
///
/// // ValueEnum types
/// clap::value_parser!(ColorChoice);
/// ```
#[macro_export]
macro_rules! value_parser {
    ($name:ty) => {{
        use $crate::builder::impl_prelude::*;
        let auto = $crate::builder::_infer_ValueParser_for::<$name>::new();
        (&&&&&&auto).value_parser()
    }};
}

mod private {
    use super::*;

    #[allow(non_camel_case_types)]
    pub trait _impls_ValueParserFactorySealed {}
    impl<P: ValueParserFactory> _impls_ValueParserFactorySealed for &&&&&&_infer_ValueParser_for<P> {}

    #[allow(non_camel_case_types)]
    pub trait _impls_ValueEnumSealed {}
    impl<E: crate::ValueEnum> _impls_ValueEnumSealed for &&&&&_infer_ValueParser_for<E> {}

    #[allow(non_camel_case_types)]
    pub trait _impls_From_OsStringSealed {}
    impl<FromOsString> _impls_From_OsStringSealed for &&&&_infer_ValueParser_for<FromOsString> where
        FromOsString: From<std::ffi::OsString> + std::any::Any + Send + Sync + 'static
    {
    }

    #[allow(non_camel_case_types)]
    pub trait _impls_From_OsStrSealed {}
    impl<FromOsStr> _impls_From_OsStrSealed for &&&_infer_ValueParser_for<FromOsStr> where
        FromOsStr: for<'s> From<&'s std::ffi::OsStr> + std::any::Any + Send + Sync + 'static
    {
    }

    #[allow(non_camel_case_types)]
    pub trait _impls_From_StringSealed {}
    impl<FromString> _impls_From_StringSealed for &&_infer_ValueParser_for<FromString> where
        FromString: From<String> + std::any::Any + Send + Sync + 'static
    {
    }

    #[allow(non_camel_case_types)]
    pub trait _impls_From_strSealed {}
    impl<FromStr> _impls_From_strSealed for &_infer_ValueParser_for<FromStr> where
        FromStr: for<'s> From<&'s str> + std::any::Any + Send + Sync + 'static
    {
    }

    #[allow(non_camel_case_types)]
    pub trait _impls_FromStrSealed {}
    impl<Parse> _impls_FromStrSealed for _infer_ValueParser_for<Parse>
    where
        Parse: std::str::FromStr + std::any::Any + Send + Sync + 'static,
        <Parse as std::str::FromStr>::Err: Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
    {
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn ensure_typed_applies_to_parse() {
        fn parse(_: &str) -> Result<usize, std::io::Error> {
            Ok(10)
        }
        let cmd = crate::Command::new("cmd");
        let arg = None;
        assert_eq!(
            TypedValueParser::parse_ref(&parse, &cmd, arg, std::ffi::OsStr::new("foo")).unwrap(),
            10
        );
    }
}
