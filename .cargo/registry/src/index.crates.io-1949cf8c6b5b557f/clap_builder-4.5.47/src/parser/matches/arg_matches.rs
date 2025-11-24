// Std
use std::any::Any;
use std::ffi::{OsStr, OsString};
use std::fmt::Debug;
use std::iter::{Cloned, Flatten, Map};
use std::slice::Iter;

// Internal
#[cfg(debug_assertions)]
use crate::builder::Str;
use crate::parser::MatchedArg;
use crate::parser::MatchesError;
use crate::parser::ValueSource;
use crate::util::AnyValue;
use crate::util::AnyValueId;
use crate::util::FlatMap;
use crate::util::Id;
use crate::INTERNAL_ERROR_MSG;

/// Container for parse results.
///
/// Used to get information about the arguments that were supplied to the program at runtime by
/// the user. New instances of this struct are obtained by using the [`Command::get_matches`] family of
/// methods.
///
/// # Examples
///
/// ```no_run
/// # use clap_builder as clap;
/// # use clap::{Command, Arg, ArgAction};
/// # use clap::parser::ValueSource;
/// let matches = Command::new("MyApp")
///     .arg(Arg::new("out")
///         .long("output")
///         .required(true)
///         .action(ArgAction::Set)
///         .default_value("-"))
///     .arg(Arg::new("cfg")
///         .short('c')
///         .action(ArgAction::Set))
///     .get_matches(); // builds the instance of ArgMatches
///
/// // to get information about the "cfg" argument we created, such as the value supplied we use
/// // various ArgMatches methods, such as [ArgMatches::get_one]
/// if let Some(c) = matches.get_one::<String>("cfg") {
///     println!("Value for -c: {c}");
/// }
///
/// // The ArgMatches::get_one method returns an Option because the user may not have supplied
/// // that argument at runtime. But if we specified that the argument was "required" as we did
/// // with the "out" argument, we can safely unwrap because `clap` verifies that was actually
/// // used at runtime.
/// println!("Value for --output: {}", matches.get_one::<String>("out").unwrap());
///
/// // You can check the presence of an argument's values
/// if matches.contains_id("out") {
///     // However, if you want to know where the value came from
///     if matches.value_source("out").expect("checked contains_id") == ValueSource::CommandLine {
///         println!("`out` set by user");
///     } else {
///         println!("`out` is defaulted");
///     }
/// }
/// ```
/// [`Command::get_matches`]: crate::Command::get_matches()
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArgMatches {
    #[cfg(debug_assertions)]
    pub(crate) valid_args: Vec<Id>,
    #[cfg(debug_assertions)]
    pub(crate) valid_subcommands: Vec<Str>,
    pub(crate) args: FlatMap<Id, MatchedArg>,
    pub(crate) subcommand: Option<Box<SubCommand>>,
}

/// # Arguments
impl ArgMatches {
    /// Gets the value of a specific option or positional argument.
    ///
    /// i.e. an argument that [takes an additional value][crate::Arg::num_args] at runtime.
    ///
    /// Returns an error if the wrong type was used.
    ///
    /// Returns `None` if the option wasn't present.
    ///
    /// <div class="warning">
    ///
    /// *NOTE:* This will always return `Some(value)` if [`default_value`] has been set.
    /// [`ArgMatches::value_source`] can be used to check if a value is present at runtime.
    ///
    /// </div>
    ///
    /// # Panic
    ///
    /// If the argument definition and access mismatch.  To handle this case programmatically, see
    /// [`ArgMatches::try_get_one`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, value_parser, ArgAction};
    /// let m = Command::new("myapp")
    ///     .arg(Arg::new("port")
    ///         .value_parser(value_parser!(usize))
    ///         .action(ArgAction::Set)
    ///         .required(true))
    ///     .get_matches_from(vec!["myapp", "2020"]);
    ///
    /// let port: usize = *m
    ///     .get_one("port")
    ///     .expect("`port`is required");
    /// assert_eq!(port, 2020);
    /// ```
    /// [positional]: crate::Arg::index()
    /// [`default_value`]: crate::Arg::default_value()
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn get_one<T: Any + Clone + Send + Sync + 'static>(&self, id: &str) -> Option<&T> {
        MatchesError::unwrap(id, self.try_get_one(id))
    }

    /// Gets the value of a specific [`ArgAction::Count`][crate::ArgAction::Count] flag
    ///
    /// # Panic
    ///
    /// If the argument's action is not [`ArgAction::Count`][crate::ArgAction::Count]
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// # use clap::Arg;
    /// let cmd = Command::new("mycmd")
    ///     .arg(
    ///         Arg::new("flag")
    ///             .long("flag")
    ///             .action(clap::ArgAction::Count)
    ///     );
    ///
    /// let matches = cmd.clone().try_get_matches_from(["mycmd", "--flag", "--flag"]).unwrap();
    /// assert_eq!(
    ///     matches.get_count("flag"),
    ///     2
    /// );
    /// ```
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn get_count(&self, id: &str) -> u8 {
        *self.get_one::<u8>(id).unwrap_or_else(|| {
            panic!("arg `{id}`'s `ArgAction` should be `Count` which should provide a default")
        })
    }

    /// Gets the value of a specific [`ArgAction::SetTrue`][crate::ArgAction::SetTrue] or [`ArgAction::SetFalse`][crate::ArgAction::SetFalse] flag
    ///
    /// # Panic
    ///
    /// If the argument's action is not [`ArgAction::SetTrue`][crate::ArgAction::SetTrue] or [`ArgAction::SetFalse`][crate::ArgAction::SetFalse]
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::Command;
    /// # use clap::Arg;
    /// let cmd = Command::new("mycmd")
    ///     .arg(
    ///         Arg::new("flag")
    ///             .long("flag")
    ///             .action(clap::ArgAction::SetTrue)
    ///     );
    ///
    /// let matches = cmd.clone().try_get_matches_from(["mycmd", "--flag"]).unwrap();
    /// assert!(matches.contains_id("flag"));
    /// assert_eq!(
    ///     matches.get_flag("flag"),
    ///     true
    /// );
    /// ```
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn get_flag(&self, id: &str) -> bool {
        *self
            .get_one::<bool>(id)
            .unwrap_or_else(|| {
                panic!(
                    "arg `{id}`'s `ArgAction` should be one of `SetTrue`, `SetFalse` which should provide a default"
                )
            })
    }

    /// Iterate over values of a specific option or positional argument.
    ///
    /// i.e. an argument that takes multiple values at runtime.
    ///
    /// Returns an error if the wrong type was used.
    ///
    /// Returns `None` if the option wasn't present.
    ///
    /// # Panic
    ///
    /// If the argument definition and access mismatch.  To handle this case programmatically, see
    /// [`ArgMatches::try_get_many`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, value_parser, ArgAction};
    /// let m = Command::new("myprog")
    ///     .arg(Arg::new("ports")
    ///         .action(ArgAction::Append)
    ///         .value_parser(value_parser!(usize))
    ///         .short('p')
    ///         .required(true))
    ///     .get_matches_from(vec![
    ///         "myprog", "-p", "22", "-p", "80", "-p", "2020"
    ///     ]);
    /// let vals: Vec<usize> = m.get_many("ports")
    ///     .expect("`port`is required")
    ///     .copied()
    ///     .collect();
    /// assert_eq!(vals, [22, 80, 2020]);
    /// ```
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn get_many<T: Any + Clone + Send + Sync + 'static>(
        &self,
        id: &str,
    ) -> Option<ValuesRef<'_, T>> {
        MatchesError::unwrap(id, self.try_get_many(id))
    }

    /// Iterate over the values passed to each occurrence of an option.
    ///
    /// Each item is itself an iterator containing the arguments passed to a single occurrence
    /// of the option.
    ///
    /// If the option doesn't support multiple occurrences, or there was only a single occurrence,
    /// the iterator will only contain a single item.
    ///
    /// Returns `None` if the option wasn't present.
    ///
    /// # Panics
    ///
    /// If the argument definition and access mismatch (debug builds). To handle this case programmatically, see
    /// [`ArgMatches::try_get_occurrences`].
    ///
    /// # Examples
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command,Arg, ArgAction, value_parser};
    /// let m = Command::new("myprog")
    ///     .arg(Arg::new("x")
    ///         .short('x')
    ///         .num_args(2)
    ///         .action(ArgAction::Append)
    ///         .value_parser(value_parser!(String)))
    ///     .get_matches_from(vec![
    ///         "myprog", "-x", "a", "b", "-x", "c", "d"]);
    /// let vals: Vec<Vec<&String>> = m.get_occurrences("x").unwrap().map(Iterator::collect).collect();
    /// assert_eq!(vals, [["a", "b"], ["c", "d"]]);
    /// ```
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn get_occurrences<T: Any + Clone + Send + Sync + 'static>(
        &self,
        id: &str,
    ) -> Option<OccurrencesRef<'_, T>> {
        MatchesError::unwrap(id, self.try_get_occurrences(id))
    }

    /// Iterate over the original argument values.
    ///
    /// An `OsStr` on Unix-like systems is any series of bytes, regardless of whether or not they
    /// contain valid UTF-8. Since [`String`]s in Rust are guaranteed to be valid UTF-8, a valid
    /// filename on a Unix system as an argument value may contain invalid UTF-8.
    ///
    /// Returns `None` if the option wasn't present.
    ///
    /// # Panic
    ///
    /// If the argument definition and access mismatch.  To handle this case programmatically, see
    /// [`ArgMatches::try_get_raw`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(unix)] {
    /// # use clap_builder as clap;
    /// # use clap::{Command, arg, value_parser};
    /// # use std::ffi::{OsStr,OsString};
    /// # use std::os::unix::ffi::{OsStrExt,OsStringExt};
    /// use std::path::PathBuf;
    ///
    /// let m = Command::new("utf8")
    ///     .arg(arg!(<arg> ... "some arg").value_parser(value_parser!(PathBuf)))
    ///     .get_matches_from(vec![OsString::from("myprog"),
    ///                                 // "Hi"
    ///                                 OsString::from_vec(vec![b'H', b'i']),
    ///                                 // "{0xe9}!"
    ///                                 OsString::from_vec(vec![0xe9, b'!'])]);
    ///
    /// let mut itr = m.get_raw("arg")
    ///     .expect("`port`is required")
    ///     .into_iter();
    /// assert_eq!(itr.next(), Some(OsStr::new("Hi")));
    /// assert_eq!(itr.next(), Some(OsStr::from_bytes(&[0xe9, b'!'])));
    /// assert_eq!(itr.next(), None);
    /// # }
    /// ```
    /// [`Iterator`]: std::iter::Iterator
    /// [`OsSt`]: std::ffi::OsStr
    /// [`String`]: std::string::String
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn get_raw(&self, id: &str) -> Option<RawValues<'_>> {
        MatchesError::unwrap(id, self.try_get_raw(id))
    }

    /// Iterate over the original values for each occurrence of an option.
    ///
    /// Similar to [`ArgMatches::get_occurrences`] but returns raw values.
    ///
    /// An `OsStr` on Unix-like systems is any series of bytes, regardless of whether or not they
    /// contain valid UTF-8. Since [`String`]s in Rust are guaranteed to be valid UTF-8, a valid
    /// filename on a Unix system as an argument value may contain invalid UTF-8.
    ///
    /// Returns `None` if the option wasn't present.
    ///
    /// # Panic
    ///
    /// If the argument definition and access mismatch.  To handle this case programmatically, see
    /// [`ArgMatches::try_get_raw_occurrences`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(unix)] {
    /// # use clap_builder as clap;
    /// # use clap::{Command, arg, value_parser, ArgAction, Arg};
    /// # use std::ffi::{OsStr,OsString};
    /// # use std::os::unix::ffi::{OsStrExt,OsStringExt};
    /// use std::path::PathBuf;
    ///
    /// let m = Command::new("myprog")
    ///     .arg(Arg::new("x")
    ///         .short('x')
    ///         .num_args(2)
    ///         .action(ArgAction::Append)
    ///         .value_parser(value_parser!(PathBuf)))
    ///     .get_matches_from(vec![OsString::from("myprog"),
    ///                             OsString::from("-x"),
    ///                             OsString::from("a"), OsString::from("b"),
    ///                             OsString::from("-x"),
    ///                             OsString::from("c"),
    ///                             // "{0xe9}!"
    ///                             OsString::from_vec(vec![0xe9, b'!'])]);
    /// let mut itr = m.get_raw_occurrences("x")
    ///     .expect("`-x`is required")
    ///     .map(Iterator::collect::<Vec<_>>);
    /// assert_eq!(itr.next(), Some(vec![OsStr::new("a"), OsStr::new("b")]));
    /// assert_eq!(itr.next(), Some(vec![OsStr::new("c"), OsStr::from_bytes(&[0xe9, b'!'])]));
    /// assert_eq!(itr.next(), None);
    /// # }
    /// ```
    /// [`Iterator`]: std::iter::Iterator
    /// [`OsStr`]: std::ffi::OsStr
    /// [`String`]: std::string::String
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn get_raw_occurrences(&self, id: &str) -> Option<RawOccurrences<'_>> {
        MatchesError::unwrap(id, self.try_get_raw_occurrences(id))
    }

    /// Returns the value of a specific option or positional argument.
    ///
    /// i.e. an argument that [takes an additional value][crate::Arg::num_args] at runtime.
    ///
    /// Returns an error if the wrong type was used.  No item will have been removed.
    ///
    /// Returns `None` if the option wasn't present.
    ///
    /// <div class="warning">
    ///
    /// *NOTE:* This will always return `Some(value)` if [`default_value`] has been set.
    /// [`ArgMatches::value_source`] can be used to check if a value is present at runtime.
    ///
    /// </div>
    ///
    /// # Panic
    ///
    /// If the argument definition and access mismatch.  To handle this case programmatically, see
    /// [`ArgMatches::try_remove_one`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, value_parser, ArgAction};
    /// let mut m = Command::new("myprog")
    ///     .arg(Arg::new("file")
    ///         .required(true)
    ///         .action(ArgAction::Set))
    ///     .get_matches_from(vec![
    ///         "myprog", "file.txt",
    ///     ]);
    /// let vals: String = m.remove_one("file")
    ///     .expect("`file`is required");
    /// assert_eq!(vals, "file.txt");
    /// ```
    /// [positional]: crate::Arg::index()
    /// [`default_value`]: crate::Arg::default_value()
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn remove_one<T: Any + Clone + Send + Sync + 'static>(&mut self, id: &str) -> Option<T> {
        MatchesError::unwrap(id, self.try_remove_one(id))
    }

    /// Return values of a specific option or positional argument.
    ///
    /// i.e. an argument that takes multiple values at runtime.
    ///
    /// Returns an error if the wrong type was used.  No item will have been removed.
    ///
    /// Returns `None` if the option wasn't present.
    ///
    /// # Panic
    ///
    /// If the argument definition and access mismatch.  To handle this case programmatically, see
    /// [`ArgMatches::try_remove_many`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, value_parser, ArgAction};
    /// let mut m = Command::new("myprog")
    ///     .arg(Arg::new("file")
    ///         .action(ArgAction::Append)
    ///         .num_args(1..)
    ///         .required(true))
    ///     .get_matches_from(vec![
    ///         "myprog", "file1.txt", "file2.txt", "file3.txt", "file4.txt",
    ///     ]);
    /// let vals: Vec<String> = m.remove_many("file")
    ///     .expect("`file`is required")
    ///     .collect();
    /// assert_eq!(vals, ["file1.txt", "file2.txt", "file3.txt", "file4.txt"]);
    /// ```
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn remove_many<T: Any + Clone + Send + Sync + 'static>(
        &mut self,
        id: &str,
    ) -> Option<Values<T>> {
        MatchesError::unwrap(id, self.try_remove_many(id))
    }

    /// Return values for each occurrence of an option.
    ///
    /// Each item is itself an iterator containing the arguments passed to a single occurrence of
    /// the option.
    ///
    /// If the option doesn't support multiple occurrences, or there was only a single occurrence,
    /// the iterator will only contain a single item.
    ///
    /// Returns `None` if the option wasn't present.
    ///
    /// # Panic
    ///
    /// If the argument definition and access mismatch.  To handle this case programmatically, see
    /// [`ArgMatches::try_remove_occurrences`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, value_parser, ArgAction};
    /// let mut m = Command::new("myprog")
    ///     .arg(Arg::new("x")
    ///         .short('x')
    ///         .num_args(2)
    ///         .action(ArgAction::Append)
    ///         .value_parser(value_parser!(String)))
    ///     .get_matches_from(vec![
    ///         "myprog", "-x", "a", "b", "-x", "c", "d"]);
    /// let vals: Vec<Vec<String>> = m.remove_occurrences("x").unwrap().map(Iterator::collect).collect();
    /// assert_eq!(vals, [["a", "b"], ["c", "d"]]);
    /// ```
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn remove_occurrences<T: Any + Clone + Send + Sync + 'static>(
        &mut self,
        id: &str,
    ) -> Option<Occurrences<T>> {
        MatchesError::unwrap(id, self.try_remove_occurrences(id))
    }

    /// Check if values are present for the argument or group id
    ///
    /// <div class="warning">
    ///
    /// *NOTE:* This will always return `true` if [`default_value`] has been set.
    /// [`ArgMatches::value_source`] can be used to check if a value is present at runtime.
    ///
    /// </div>
    ///
    /// # Panics
    ///
    /// If `id` is not a valid argument or group name (debug builds).  To handle this case programmatically, see
    /// [`ArgMatches::try_contains_id`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgAction};
    /// let m = Command::new("myprog")
    ///     .arg(Arg::new("debug")
    ///         .short('d')
    ///         .action(ArgAction::SetTrue))
    ///     .get_matches_from(vec![
    ///         "myprog", "-d"
    ///     ]);
    ///
    /// assert!(m.contains_id("debug"));
    /// ```
    ///
    /// [`default_value`]: crate::Arg::default_value()
    pub fn contains_id(&self, id: &str) -> bool {
        MatchesError::unwrap(id, self.try_contains_id(id))
    }

    /// Iterate over [`Arg`][crate::Arg] and [`ArgGroup`][crate::ArgGroup] [`Id`]s via [`ArgMatches::ids`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, arg, value_parser};
    ///
    /// let m = Command::new("myprog")
    ///     .arg(arg!(--color <when>)
    ///         .value_parser(["auto", "always", "never"]))
    ///     .arg(arg!(--config <path>)
    ///         .value_parser(value_parser!(std::path::PathBuf)))
    ///     .get_matches_from(["myprog", "--config=config.toml", "--color=auto"]);
    /// assert_eq!(m.ids().len(), 2);
    /// assert_eq!(
    ///     m.ids()
    ///         .map(|id| id.as_str())
    ///         .collect::<Vec<_>>(),
    ///     ["config", "color"]
    /// );
    /// ```
    pub fn ids(&self) -> IdsRef<'_> {
        IdsRef {
            iter: self.args.keys(),
        }
    }

    /// Check if any [`Arg`][crate::Arg]s were present on the command line
    ///
    /// See [`ArgMatches::subcommand_name()`] or [`ArgMatches::subcommand()`] to check if a
    /// subcommand was present on the command line.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgAction};
    /// let mut cmd = Command::new("myapp")
    ///     .arg(Arg::new("output")
    ///         .action(ArgAction::Set));
    ///
    /// let m = cmd
    ///     .try_get_matches_from_mut(vec!["myapp", "something"])
    ///     .unwrap();
    /// assert!(m.args_present());
    ///
    /// let m = cmd
    ///     .try_get_matches_from_mut(vec!["myapp"])
    ///     .unwrap();
    /// assert!(! m.args_present());
    pub fn args_present(&self) -> bool {
        self.args
            .values()
            .any(|v| v.source().map(|s| s.is_explicit()).unwrap_or(false))
    }

    /// Report where argument value came from
    ///
    /// # Panics
    ///
    /// If `id` is not a valid argument or group id (debug builds).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgAction};
    /// # use clap::parser::ValueSource;
    /// let m = Command::new("myprog")
    ///     .arg(Arg::new("debug")
    ///         .short('d')
    ///         .action(ArgAction::SetTrue))
    ///     .get_matches_from(vec![
    ///         "myprog", "-d"
    ///     ]);
    ///
    /// assert_eq!(m.value_source("debug"), Some(ValueSource::CommandLine));
    /// ```
    ///
    /// [`default_value`]: crate::Arg::default_value()
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn value_source(&self, id: &str) -> Option<ValueSource> {
        let value = self.get_arg(id);

        value.and_then(MatchedArg::source)
    }

    /// The first index of that an argument showed up.
    ///
    /// Indices are similar to argv indices, but are not exactly 1:1.
    ///
    /// For flags (i.e. those arguments which don't have an associated value), indices refer
    /// to occurrence of the switch, such as `-f`, or `--flag`. However, for options the indices
    /// refer to the *values* `-o val` would therefore not represent two distinct indices, only the
    /// index for `val` would be recorded. This is by design.
    ///
    /// Besides the flag/option discrepancy, the primary difference between an argv index and clap
    /// index, is that clap continues counting once all arguments have properly separated, whereas
    /// an argv index does not.
    ///
    /// The examples should clear this up.
    ///
    /// <div class="warning">
    ///
    /// *NOTE:* If an argument is allowed multiple times, this method will only give the *first*
    /// index.  See [`ArgMatches::indices_of`].
    ///
    /// </div>
    ///
    /// # Panics
    ///
    /// If `id` is not a valid argument or group id (debug builds).
    ///
    /// # Examples
    ///
    /// The argv indices are listed in the comments below. See how they correspond to the clap
    /// indices. Note that if it's not listed in a clap index, this is because it's not saved in
    /// in an `ArgMatches` struct for querying.
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgAction};
    /// let m = Command::new("myapp")
    ///     .arg(Arg::new("flag")
    ///         .short('f')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("option")
    ///         .short('o')
    ///         .action(ArgAction::Set))
    ///     .get_matches_from(vec!["myapp", "-f", "-o", "val"]);
    ///            // ARGV indices: ^0       ^1    ^2    ^3
    ///            // clap indices:          ^1          ^3
    ///
    /// assert_eq!(m.index_of("flag"), Some(1));
    /// assert_eq!(m.index_of("option"), Some(3));
    /// ```
    ///
    /// Now notice, if we use one of the other styles of options:
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgAction};
    /// let m = Command::new("myapp")
    ///     .arg(Arg::new("flag")
    ///         .short('f')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("option")
    ///         .short('o')
    ///         .action(ArgAction::Set))
    ///     .get_matches_from(vec!["myapp", "-f", "-o=val"]);
    ///            // ARGV indices: ^0       ^1    ^2
    ///            // clap indices:          ^1       ^3
    ///
    /// assert_eq!(m.index_of("flag"), Some(1));
    /// assert_eq!(m.index_of("option"), Some(3));
    /// ```
    ///
    /// Things become much more complicated, or clear if we look at a more complex combination of
    /// flags. Let's also throw in the final option style for good measure.
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgAction};
    /// let m = Command::new("myapp")
    ///     .arg(Arg::new("flag")
    ///         .short('f')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("flag2")
    ///         .short('F')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("flag3")
    ///         .short('z')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("option")
    ///         .short('o')
    ///         .action(ArgAction::Set))
    ///     .get_matches_from(vec!["myapp", "-fzF", "-oval"]);
    ///            // ARGV indices: ^0      ^1       ^2
    ///            // clap indices:         ^1,2,3    ^5
    ///            //
    ///            // clap sees the above as 'myapp -f -z -F -o val'
    ///            //                         ^0    ^1 ^2 ^3 ^4 ^5
    /// assert_eq!(m.index_of("flag"), Some(1));
    /// assert_eq!(m.index_of("flag2"), Some(3));
    /// assert_eq!(m.index_of("flag3"), Some(2));
    /// assert_eq!(m.index_of("option"), Some(5));
    /// ```
    ///
    /// One final combination of flags/options to see how they combine:
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgAction};
    /// let m = Command::new("myapp")
    ///     .arg(Arg::new("flag")
    ///         .short('f')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("flag2")
    ///         .short('F')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("flag3")
    ///         .short('z')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("option")
    ///         .short('o')
    ///         .action(ArgAction::Set))
    ///     .get_matches_from(vec!["myapp", "-fzFoval"]);
    ///            // ARGV indices: ^0       ^1
    ///            // clap indices:          ^1,2,3^5
    ///            //
    ///            // clap sees the above as 'myapp -f -z -F -o val'
    ///            //                         ^0    ^1 ^2 ^3 ^4 ^5
    /// assert_eq!(m.index_of("flag"), Some(1));
    /// assert_eq!(m.index_of("flag2"), Some(3));
    /// assert_eq!(m.index_of("flag3"), Some(2));
    /// assert_eq!(m.index_of("option"), Some(5));
    /// ```
    ///
    /// The last part to mention is when values are sent in multiple groups with a [delimiter].
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// let m = Command::new("myapp")
    ///     .arg(Arg::new("option")
    ///         .short('o')
    ///         .value_delimiter(',')
    ///         .num_args(1..))
    ///     .get_matches_from(vec!["myapp", "-o=val1,val2,val3"]);
    ///            // ARGV indices: ^0       ^1
    ///            // clap indices:             ^2   ^3   ^4
    ///            //
    ///            // clap sees the above as 'myapp -o val1 val2 val3'
    ///            //                         ^0    ^1 ^2   ^3   ^4
    /// assert_eq!(m.index_of("option"), Some(2));
    /// assert_eq!(m.indices_of("option").unwrap().collect::<Vec<_>>(), &[2, 3, 4]);
    /// ```
    /// [delimiter]: crate::Arg::value_delimiter()
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn index_of(&self, id: &str) -> Option<usize> {
        let arg = some!(self.get_arg(id));
        let i = some!(arg.get_index(0));
        Some(i)
    }

    /// All indices an argument appeared at when parsing.
    ///
    /// Indices are similar to argv indices, but are not exactly 1:1.
    ///
    /// For flags (i.e. those arguments which don't have an associated value), indices refer
    /// to occurrence of the switch, such as `-f`, or `--flag`. However, for options the indices
    /// refer to the *values* `-o val` would therefore not represent two distinct indices, only the
    /// index for `val` would be recorded. This is by design.
    ///
    /// <div class="warning">
    ///
    /// *NOTE:* For more information about how clap indices compared to argv indices, see
    /// [`ArgMatches::index_of`]
    ///
    /// </div>
    ///
    /// # Panics
    ///
    /// If `id` is not a valid argument or group id (debug builds).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg};
    /// let m = Command::new("myapp")
    ///     .arg(Arg::new("option")
    ///         .short('o')
    ///         .value_delimiter(','))
    ///     .get_matches_from(vec!["myapp", "-o=val1,val2,val3"]);
    ///            // ARGV indices: ^0       ^1
    ///            // clap indices:             ^2   ^3   ^4
    ///            //
    ///            // clap sees the above as 'myapp -o val1 val2 val3'
    ///            //                         ^0    ^1 ^2   ^3   ^4
    /// assert_eq!(m.indices_of("option").unwrap().collect::<Vec<_>>(), &[2, 3, 4]);
    /// ```
    ///
    /// Another quick example is when flags and options are used together
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgAction};
    /// let m = Command::new("myapp")
    ///     .arg(Arg::new("option")
    ///         .short('o')
    ///         .action(ArgAction::Set)
    ///         .action(ArgAction::Append))
    ///     .arg(Arg::new("flag")
    ///         .short('f')
    ///         .action(ArgAction::Count))
    ///     .get_matches_from(vec!["myapp", "-o", "val1", "-f", "-o", "val2", "-f"]);
    ///            // ARGV indices: ^0       ^1    ^2      ^3    ^4    ^5      ^6
    ///            // clap indices:                ^2      ^3          ^5      ^6
    ///
    /// assert_eq!(m.indices_of("option").unwrap().collect::<Vec<_>>(), &[2, 5]);
    /// assert_eq!(m.indices_of("flag").unwrap().collect::<Vec<_>>(), &[6]);
    /// ```
    ///
    /// One final example, which is an odd case; if we *don't* use  value delimiter as we did with
    /// the first example above instead of `val1`, `val2` and `val3` all being distinc values, they
    /// would all be a single value of `val1,val2,val3`, in which case they'd only receive a single
    /// index.
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgAction};
    /// let m = Command::new("myapp")
    ///     .arg(Arg::new("option")
    ///         .short('o')
    ///         .action(ArgAction::Set)
    ///         .num_args(1..))
    ///     .get_matches_from(vec!["myapp", "-o=val1,val2,val3"]);
    ///            // ARGV indices: ^0       ^1
    ///            // clap indices:             ^2
    ///            //
    ///            // clap sees the above as 'myapp -o "val1,val2,val3"'
    ///            //                         ^0    ^1  ^2
    /// assert_eq!(m.indices_of("option").unwrap().collect::<Vec<_>>(), &[2]);
    /// ```
    /// [`ArgMatches::index_of`]: ArgMatches::index_of()
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn indices_of(&self, id: &str) -> Option<Indices<'_>> {
        let arg = some!(self.get_arg(id));
        let i = Indices {
            iter: arg.indices(),
            len: arg.num_vals(),
        };
        Some(i)
    }
}

/// # Subcommands
impl ArgMatches {
    /// The name and `ArgMatches` of the current [subcommand].
    ///
    /// Subcommand values are put in a child [`ArgMatches`]
    ///
    /// Returns `None` if the subcommand wasn't present at runtime,
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, };
    ///  let app_m = Command::new("git")
    ///      .subcommand(Command::new("clone"))
    ///      .subcommand(Command::new("push"))
    ///      .subcommand(Command::new("commit"))
    ///      .get_matches();
    ///
    /// match app_m.subcommand() {
    ///     Some(("clone",  sub_m)) => {}, // clone was used
    ///     Some(("push",   sub_m)) => {}, // push was used
    ///     Some(("commit", sub_m)) => {}, // commit was used
    ///     _                       => {}, // Either no subcommand or one not tested for...
    /// }
    /// ```
    ///
    /// Another useful scenario is when you want to support third party, or external, subcommands.
    /// In these cases you can't know the subcommand name ahead of time, so use a variable instead
    /// with pattern matching!
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use std::ffi::OsString;
    /// # use std::ffi::OsStr;
    /// # use clap::Command;
    /// // Assume there is an external subcommand named "subcmd"
    /// let app_m = Command::new("myprog")
    ///     .allow_external_subcommands(true)
    ///     .get_matches_from(vec![
    ///         "myprog", "subcmd", "--option", "value", "-fff", "--flag"
    ///     ]);
    ///
    /// // All trailing arguments will be stored under the subcommand's sub-matches using an empty
    /// // string argument name
    /// match app_m.subcommand() {
    ///     Some((external, sub_m)) => {
    ///          let ext_args: Vec<&OsStr> = sub_m.get_many::<OsString>("")
    ///             .unwrap().map(|s| s.as_os_str()).collect();
    ///          assert_eq!(external, "subcmd");
    ///          assert_eq!(ext_args, ["--option", "value", "-fff", "--flag"]);
    ///     },
    ///     _ => {},
    /// }
    /// ```
    /// [subcommand]: crate::Command::subcommand
    #[inline]
    pub fn subcommand(&self) -> Option<(&str, &ArgMatches)> {
        self.subcommand.as_ref().map(|sc| (&*sc.name, &sc.matches))
    }

    /// Return the name and `ArgMatches` of the current [subcommand].
    ///
    /// Subcommand values are put in a child [`ArgMatches`]
    ///
    /// Returns `None` if the subcommand wasn't present at runtime,
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, };
    ///  let mut app_m = Command::new("git")
    ///      .subcommand(Command::new("clone"))
    ///      .subcommand(Command::new("push"))
    ///      .subcommand(Command::new("commit"))
    ///      .subcommand_required(true)
    ///      .get_matches();
    ///
    /// let (name, sub_m) = app_m.remove_subcommand().expect("required");
    /// match (name.as_str(), sub_m) {
    ///     ("clone",  sub_m) => {}, // clone was used
    ///     ("push",   sub_m) => {}, // push was used
    ///     ("commit", sub_m) => {}, // commit was used
    ///     (name, _)         => unimplemented!("{name}"),
    /// }
    /// ```
    ///
    /// Another useful scenario is when you want to support third party, or external, subcommands.
    /// In these cases you can't know the subcommand name ahead of time, so use a variable instead
    /// with pattern matching!
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use std::ffi::OsString;
    /// # use clap::Command;
    /// // Assume there is an external subcommand named "subcmd"
    /// let mut app_m = Command::new("myprog")
    ///     .allow_external_subcommands(true)
    ///     .get_matches_from(vec![
    ///         "myprog", "subcmd", "--option", "value", "-fff", "--flag"
    ///     ]);
    ///
    /// // All trailing arguments will be stored under the subcommand's sub-matches using an empty
    /// // string argument name
    /// match app_m.remove_subcommand() {
    ///     Some((external, mut sub_m)) => {
    ///          let ext_args: Vec<OsString> = sub_m.remove_many("")
    ///             .expect("`file`is required")
    ///             .collect();
    ///          assert_eq!(external, "subcmd");
    ///          assert_eq!(ext_args, ["--option", "value", "-fff", "--flag"]);
    ///     },
    ///     _ => {},
    /// }
    /// ```
    /// [subcommand]: crate::Command::subcommand
    pub fn remove_subcommand(&mut self) -> Option<(String, ArgMatches)> {
        self.subcommand.take().map(|sc| (sc.name, sc.matches))
    }

    /// The `ArgMatches` for the current [subcommand].
    ///
    /// Subcommand values are put in a child [`ArgMatches`]
    ///
    /// Returns `None` if the subcommand wasn't present at runtime,
    ///
    /// # Panics
    ///
    /// If `id` is not a valid subcommand (debug builds).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgAction};
    /// let app_m = Command::new("myprog")
    ///     .arg(Arg::new("debug")
    ///         .short('d')
    ///         .action(ArgAction::SetTrue)
    ///     )
    ///     .subcommand(Command::new("test")
    ///         .arg(Arg::new("opt")
    ///             .long("option")
    ///             .action(ArgAction::Set)))
    ///     .get_matches_from(vec![
    ///         "myprog", "-d", "test", "--option", "val"
    ///     ]);
    ///
    /// // Both parent commands, and child subcommands can have arguments present at the same times
    /// assert!(app_m.get_flag("debug"));
    ///
    /// // Get the subcommand's ArgMatches instance
    /// if let Some(sub_m) = app_m.subcommand_matches("test") {
    ///     // Use the struct like normal
    ///     assert_eq!(sub_m.get_one::<String>("opt").map(|s| s.as_str()), Some("val"));
    /// }
    /// ```
    ///
    /// [subcommand]: crate::Command::subcommand
    /// [`Command`]: crate::Command
    pub fn subcommand_matches(&self, name: &str) -> Option<&ArgMatches> {
        self.get_subcommand(name).map(|sc| &sc.matches)
    }

    /// The name of the current [subcommand].
    ///
    /// Returns `None` if the subcommand wasn't present at runtime,
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, };
    ///  let app_m = Command::new("git")
    ///      .subcommand(Command::new("clone"))
    ///      .subcommand(Command::new("push"))
    ///      .subcommand(Command::new("commit"))
    ///      .get_matches();
    ///
    /// match app_m.subcommand_name() {
    ///     Some("clone")  => {}, // clone was used
    ///     Some("push")   => {}, // push was used
    ///     Some("commit") => {}, // commit was used
    ///     _              => {}, // Either no subcommand or one not tested for...
    /// }
    /// ```
    /// [subcommand]: crate::Command::subcommand
    /// [`Command`]: crate::Command
    #[inline]
    pub fn subcommand_name(&self) -> Option<&str> {
        self.subcommand.as_ref().map(|sc| &*sc.name)
    }

    /// Check if a subcommand can be queried
    ///
    /// By default, `ArgMatches` functions assert on undefined `Id`s to help catch programmer
    /// mistakes.  In some context, this doesn't work, so users can use this function to check
    /// before they do a query on `ArgMatches`.
    #[inline]
    #[doc(hidden)]
    pub fn is_valid_subcommand(&self, _name: &str) -> bool {
        #[cfg(debug_assertions)]
        {
            _name.is_empty() || self.valid_subcommands.iter().any(|s| *s == _name)
        }
        #[cfg(not(debug_assertions))]
        {
            true
        }
    }
}

/// # Advanced
impl ArgMatches {
    /// Non-panicking version of [`ArgMatches::get_one`]
    pub fn try_get_one<T: Any + Clone + Send + Sync + 'static>(
        &self,
        id: &str,
    ) -> Result<Option<&T>, MatchesError> {
        let arg = ok!(self.try_get_arg_t::<T>(id));
        let value = match arg.and_then(|a| a.first()) {
            Some(value) => value,
            None => {
                return Ok(None);
            }
        };
        Ok(value
            .downcast_ref::<T>()
            .map(Some)
            .expect(INTERNAL_ERROR_MSG)) // enforced by `try_get_arg_t`
    }

    /// Non-panicking version of [`ArgMatches::get_many`]
    pub fn try_get_many<T: Any + Clone + Send + Sync + 'static>(
        &self,
        id: &str,
    ) -> Result<Option<ValuesRef<'_, T>>, MatchesError> {
        let arg = match ok!(self.try_get_arg_t::<T>(id)) {
            Some(arg) => arg,
            None => return Ok(None),
        };
        let len = arg.num_vals();
        let values = arg.vals_flatten();
        let values = ValuesRef {
            // enforced by `try_get_arg_t`
            iter: values.map(unwrap_downcast_ref),
            len,
        };
        Ok(Some(values))
    }

    /// Non-panicking version of [`ArgMatches::get_occurrences`]
    pub fn try_get_occurrences<T: Any + Clone + Send + Sync + 'static>(
        &self,
        id: &str,
    ) -> Result<Option<OccurrencesRef<'_, T>>, MatchesError> {
        let arg = match ok!(self.try_get_arg_t::<T>(id)) {
            Some(arg) => arg,
            None => return Ok(None),
        };
        let values = arg.vals();
        Ok(Some(OccurrencesRef {
            iter: values.map(|g| OccurrenceValuesRef {
                iter: g.iter().map(unwrap_downcast_ref),
            }),
        }))
    }

    /// Non-panicking version of [`ArgMatches::get_raw`]
    pub fn try_get_raw(&self, id: &str) -> Result<Option<RawValues<'_>>, MatchesError> {
        let arg = match ok!(self.try_get_arg(id)) {
            Some(arg) => arg,
            None => return Ok(None),
        };
        let len = arg.num_vals();
        let values = arg.raw_vals_flatten();
        let values = RawValues {
            iter: values.map(OsString::as_os_str),
            len,
        };
        Ok(Some(values))
    }

    /// Non-panicking version of [`ArgMatches::get_raw_occurrences`]
    pub fn try_get_raw_occurrences(
        &self,
        id: &str,
    ) -> Result<Option<RawOccurrences<'_>>, MatchesError> {
        let arg = match ok!(self.try_get_arg(id)) {
            Some(arg) => arg,
            None => return Ok(None),
        };
        let values = arg.raw_vals();
        let occurrences = RawOccurrences {
            iter: values.map(|g| RawOccurrenceValues {
                iter: g.iter().map(OsString::as_os_str),
            }),
        };
        Ok(Some(occurrences))
    }

    /// Non-panicking version of [`ArgMatches::remove_one`]
    pub fn try_remove_one<T: Any + Clone + Send + Sync + 'static>(
        &mut self,
        id: &str,
    ) -> Result<Option<T>, MatchesError> {
        match ok!(self.try_remove_arg_t::<T>(id)) {
            Some(values) => Ok(values
                .into_vals_flatten()
                // enforced by `try_get_arg_t`
                .map(unwrap_downcast_into)
                .next()),
            None => Ok(None),
        }
    }

    /// Non-panicking version of [`ArgMatches::remove_many`]
    pub fn try_remove_many<T: Any + Clone + Send + Sync + 'static>(
        &mut self,
        id: &str,
    ) -> Result<Option<Values<T>>, MatchesError> {
        let arg = match ok!(self.try_remove_arg_t::<T>(id)) {
            Some(arg) => arg,
            None => return Ok(None),
        };
        let len = arg.num_vals();
        let values = arg.into_vals_flatten();
        let values = Values {
            // enforced by `try_get_arg_t`
            iter: values.map(unwrap_downcast_into),
            len,
        };
        Ok(Some(values))
    }

    /// Non-panicking version of [`ArgMatches::remove_occurrences`]
    pub fn try_remove_occurrences<T: Any + Clone + Send + Sync + 'static>(
        &mut self,
        id: &str,
    ) -> Result<Option<Occurrences<T>>, MatchesError> {
        let arg = match ok!(self.try_remove_arg_t::<T>(id)) {
            Some(arg) => arg,
            None => return Ok(None),
        };
        let values = arg.into_vals();
        let occurrences = Occurrences {
            iter: values.into_iter().map(|g| OccurrenceValues {
                iter: g.into_iter().map(unwrap_downcast_into),
            }),
        };
        Ok(Some(occurrences))
    }

    /// Non-panicking version of [`ArgMatches::contains_id`]
    pub fn try_contains_id(&self, id: &str) -> Result<bool, MatchesError> {
        ok!(self.verify_arg(id));

        let presence = self.args.contains_key(id);
        Ok(presence)
    }

    /// Clears the values for the given `id`
    ///
    /// Alternative to [`try_remove_*`][ArgMatches::try_remove_one] when the type is not known.
    ///
    /// Returns `Err([``MatchesError``])` if the given `id` isn't valid for current `ArgMatches` instance.
    ///
    /// Returns `Ok(true)` if there were any matches with the given `id`, `Ok(false)` otherwise.
    pub fn try_clear_id(&mut self, id: &str) -> Result<bool, MatchesError> {
        ok!(self.verify_arg(id));
        Ok(self.args.remove_entry(id).is_some())
    }
}

// Private methods
impl ArgMatches {
    #[inline]
    fn try_get_arg(&self, arg: &str) -> Result<Option<&MatchedArg>, MatchesError> {
        ok!(self.verify_arg(arg));
        Ok(self.args.get(arg))
    }

    #[inline]
    fn try_get_arg_t<T: Any + Send + Sync + 'static>(
        &self,
        arg: &str,
    ) -> Result<Option<&MatchedArg>, MatchesError> {
        let arg = match ok!(self.try_get_arg(arg)) {
            Some(arg) => arg,
            None => {
                return Ok(None);
            }
        };
        ok!(self.verify_arg_t::<T>(arg));
        Ok(Some(arg))
    }

    #[inline]
    fn try_remove_arg_t<T: Any + Send + Sync + 'static>(
        &mut self,
        arg: &str,
    ) -> Result<Option<MatchedArg>, MatchesError> {
        ok!(self.verify_arg(arg));
        let (id, matched) = match self.args.remove_entry(arg) {
            Some((id, matched)) => (id, matched),
            None => {
                return Ok(None);
            }
        };

        let expected = AnyValueId::of::<T>();
        let actual = matched.infer_type_id(expected);
        if actual == expected {
            Ok(Some(matched))
        } else {
            self.args.insert(id, matched);
            Err(MatchesError::Downcast { actual, expected })
        }
    }

    fn verify_arg_t<T: Any + Send + Sync + 'static>(
        &self,
        arg: &MatchedArg,
    ) -> Result<(), MatchesError> {
        let expected = AnyValueId::of::<T>();
        let actual = arg.infer_type_id(expected);
        if expected == actual {
            Ok(())
        } else {
            Err(MatchesError::Downcast { actual, expected })
        }
    }

    #[inline]
    fn verify_arg(&self, _arg: &str) -> Result<(), MatchesError> {
        #[cfg(debug_assertions)]
        {
            if _arg == Id::EXTERNAL || self.valid_args.iter().any(|s| *s == _arg) {
            } else {
                debug!(
                    "`{:?}` is not an id of an argument or a group.\n\
                     Make sure you're using the name of the argument itself \
                     and not the name of short or long flags.",
                    _arg
                );
                return Err(MatchesError::UnknownArgument {});
            }
        }
        Ok(())
    }

    #[inline]
    #[cfg_attr(debug_assertions, track_caller)]
    fn get_arg<'s>(&'s self, arg: &str) -> Option<&'s MatchedArg> {
        #[cfg(debug_assertions)]
        {
            if arg == Id::EXTERNAL || self.valid_args.iter().any(|s| *s == arg) {
            } else {
                panic!(
                    "`{arg:?}` is not an id of an argument or a group.\n\
                     Make sure you're using the name of the argument itself \
                     and not the name of short or long flags."
                );
            }
        }

        self.args.get(arg)
    }

    #[inline]
    #[cfg_attr(debug_assertions, track_caller)]
    fn get_subcommand(&self, name: &str) -> Option<&SubCommand> {
        #[cfg(debug_assertions)]
        {
            if name.is_empty() || self.valid_subcommands.iter().any(|s| *s == name) {
            } else {
                panic!("`{name}` is not a name of a subcommand.");
            }
        }

        if let Some(ref sc) = self.subcommand {
            if sc.name == name {
                return Some(sc);
            }
        }

        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SubCommand {
    pub(crate) name: String,
    pub(crate) matches: ArgMatches,
}

/// Iterate over [`Arg`][crate::Arg] and [`ArgGroup`][crate::ArgGroup] [`Id`]s via [`ArgMatches::ids`].
///
/// # Examples
///
/// ```rust
/// # use clap_builder as clap;
/// # use clap::{Command, arg, value_parser};
///
/// let m = Command::new("myprog")
///     .arg(arg!(--color <when>)
///         .value_parser(["auto", "always", "never"]))
///     .arg(arg!(--config <path>)
///         .value_parser(value_parser!(std::path::PathBuf)))
///     .get_matches_from(["myprog", "--config=config.toml", "--color=auto"]);
/// assert_eq!(
///     m.ids()
///         .map(|id| id.as_str())
///         .collect::<Vec<_>>(),
///     ["config", "color"]
/// );
/// ```
#[derive(Clone, Debug)]
pub struct IdsRef<'a> {
    iter: Iter<'a, Id>,
}

impl<'a> Iterator for IdsRef<'a> {
    type Item = &'a Id;

    fn next(&mut self) -> Option<&'a Id> {
        self.iter.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a> DoubleEndedIterator for IdsRef<'a> {
    fn next_back(&mut self) -> Option<&'a Id> {
        self.iter.next_back()
    }
}

impl ExactSizeIterator for IdsRef<'_> {}

/// Iterate over multiple values for an argument via [`ArgMatches::remove_many`].
///
/// # Examples
///
/// ```rust
/// # use clap_builder as clap;
/// # use clap::{Command, Arg, ArgAction};
/// let mut m = Command::new("myapp")
///     .arg(Arg::new("output")
///         .short('o')
///         .action(ArgAction::Append))
///     .get_matches_from(vec!["myapp", "-o", "val1", "-o", "val2"]);
///
/// let mut values = m.remove_many::<String>("output")
///     .unwrap();
///
/// assert_eq!(values.next(), Some(String::from("val1")));
/// assert_eq!(values.next(), Some(String::from("val2")));
/// assert_eq!(values.next(), None);
/// ```
#[derive(Clone, Debug)]
pub struct Values<T> {
    #[allow(clippy::type_complexity)]
    iter: Map<Flatten<std::vec::IntoIter<Vec<AnyValue>>>, fn(AnyValue) -> T>,
    len: usize,
}

impl<T> Iterator for Values<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next) = self.iter.next() {
            self.len -= 1;
            Some(next)
        } else {
            None
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<T> DoubleEndedIterator for Values<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if let Some(next) = self.iter.next_back() {
            self.len -= 1;
            Some(next)
        } else {
            None
        }
    }
}

impl<T> ExactSizeIterator for Values<T> {}

/// Creates an empty iterator.
impl<T> Default for Values<T> {
    fn default() -> Self {
        let empty: Vec<Vec<AnyValue>> = Default::default();
        Values {
            iter: empty.into_iter().flatten().map(|_| unreachable!()),
            len: 0,
        }
    }
}

/// Iterate over multiple values for an argument via [`ArgMatches::get_many`].
///
/// # Examples
///
/// ```rust
/// # use clap_builder as clap;
/// # use clap::{Command, Arg, ArgAction};
/// let m = Command::new("myapp")
///     .arg(Arg::new("output")
///         .short('o')
///         .action(ArgAction::Append))
///     .get_matches_from(vec!["myapp", "-o", "val1", "-o", "val2"]);
///
/// let mut values = m.get_many::<String>("output")
///     .unwrap()
///     .map(|s| s.as_str());
///
/// assert_eq!(values.next(), Some("val1"));
/// assert_eq!(values.next(), Some("val2"));
/// assert_eq!(values.next(), None);
/// ```
#[derive(Clone, Debug)]
pub struct ValuesRef<'a, T> {
    #[allow(clippy::type_complexity)]
    iter: Map<Flatten<Iter<'a, Vec<AnyValue>>>, fn(&AnyValue) -> &T>,
    len: usize,
}

impl<'a, T: 'a> Iterator for ValuesRef<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next) = self.iter.next() {
            self.len -= 1;
            Some(next)
        } else {
            None
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a, T: 'a> DoubleEndedIterator for ValuesRef<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if let Some(next) = self.iter.next_back() {
            self.len -= 1;
            Some(next)
        } else {
            None
        }
    }
}

impl<'a, T: 'a> ExactSizeIterator for ValuesRef<'a, T> {}

/// Creates an empty iterator.
impl<'a, T: 'a> Default for ValuesRef<'a, T> {
    fn default() -> Self {
        static EMPTY: [Vec<AnyValue>; 0] = [];
        ValuesRef {
            iter: EMPTY[..].iter().flatten().map(|_| unreachable!()),
            len: 0,
        }
    }
}

/// Iterate over raw argument values via [`ArgMatches::get_raw`].
///
/// # Examples
///
/// ```rust
/// # #[cfg(unix)] {
/// # use clap_builder as clap;
/// # use clap::{Command, arg, value_parser};
/// use std::ffi::OsString;
/// use std::os::unix::ffi::{OsStrExt,OsStringExt};
///
/// let m = Command::new("utf8")
///     .arg(arg!(<arg> "some arg")
///         .value_parser(value_parser!(OsString)))
///     .get_matches_from(vec![OsString::from("myprog"),
///                             // "Hi {0xe9}!"
///                             OsString::from_vec(vec![b'H', b'i', b' ', 0xe9, b'!'])]);
/// assert_eq!(
///     &*m.get_raw("arg")
///         .unwrap()
///         .next().unwrap()
///         .as_bytes(),
///     [b'H', b'i', b' ', 0xe9, b'!']
/// );
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct RawValues<'a> {
    #[allow(clippy::type_complexity)]
    iter: Map<Flatten<Iter<'a, Vec<OsString>>>, fn(&OsString) -> &OsStr>,
    len: usize,
}

impl<'a> Iterator for RawValues<'a> {
    type Item = &'a OsStr;

    fn next(&mut self) -> Option<&'a OsStr> {
        if let Some(next) = self.iter.next() {
            self.len -= 1;
            Some(next)
        } else {
            None
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a> DoubleEndedIterator for RawValues<'a> {
    fn next_back(&mut self) -> Option<&'a OsStr> {
        if let Some(next) = self.iter.next_back() {
            self.len -= 1;
            Some(next)
        } else {
            None
        }
    }
}

impl ExactSizeIterator for RawValues<'_> {}

/// Creates an empty iterator.
impl Default for RawValues<'_> {
    fn default() -> Self {
        static EMPTY: [Vec<OsString>; 0] = [];
        RawValues {
            iter: EMPTY[..].iter().flatten().map(|_| unreachable!()),
            len: 0,
        }
    }
}

// The following were taken and adapted from vec_map source
// repo: https://github.com/contain-rs/vec-map
// commit: be5e1fa3c26e351761b33010ddbdaf5f05dbcc33
// license: MIT - Copyright (c) 2015 The Rust Project Developers

#[derive(Clone, Debug)]
pub struct Occurrences<T> {
    #[allow(clippy::type_complexity)]
    iter: Map<std::vec::IntoIter<Vec<AnyValue>>, fn(Vec<AnyValue>) -> OccurrenceValues<T>>,
}

impl<T> Iterator for Occurrences<T> {
    type Item = OccurrenceValues<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<T> DoubleEndedIterator for Occurrences<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back()
    }
}

impl<T> ExactSizeIterator for Occurrences<T> {}

impl<T> Default for Occurrences<T> {
    fn default() -> Self {
        let empty: Vec<Vec<AnyValue>> = Default::default();
        Occurrences {
            iter: empty.into_iter().map(|_| unreachable!()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct OccurrenceValues<T> {
    #[allow(clippy::type_complexity)]
    iter: Map<std::vec::IntoIter<AnyValue>, fn(AnyValue) -> T>,
}

impl<T> Iterator for OccurrenceValues<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<T> DoubleEndedIterator for OccurrenceValues<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back()
    }
}

impl<T> ExactSizeIterator for OccurrenceValues<T> {}

#[derive(Clone, Debug)]
pub struct OccurrencesRef<'a, T> {
    #[allow(clippy::type_complexity)]
    iter: Map<Iter<'a, Vec<AnyValue>>, fn(&Vec<AnyValue>) -> OccurrenceValuesRef<'_, T>>,
}

impl<'a, T> Iterator for OccurrencesRef<'a, T>
where
    Self: 'a,
{
    type Item = OccurrenceValuesRef<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a, T> DoubleEndedIterator for OccurrencesRef<'a, T>
where
    Self: 'a,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back()
    }
}

impl<'a, T> ExactSizeIterator for OccurrencesRef<'a, T> where Self: 'a {}
impl<T> Default for OccurrencesRef<'_, T> {
    fn default() -> Self {
        static EMPTY: [Vec<AnyValue>; 0] = [];
        OccurrencesRef {
            iter: EMPTY[..].iter().map(|_| unreachable!()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct OccurrenceValuesRef<'a, T> {
    #[allow(clippy::type_complexity)]
    iter: Map<Iter<'a, AnyValue>, fn(&AnyValue) -> &T>,
}

impl<'a, T> Iterator for OccurrenceValuesRef<'a, T>
where
    Self: 'a,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a, T> DoubleEndedIterator for OccurrenceValuesRef<'a, T>
where
    Self: 'a,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back()
    }
}

impl<'a, T> ExactSizeIterator for OccurrenceValuesRef<'a, T> where Self: 'a {}

#[derive(Clone, Debug)]
pub struct RawOccurrences<'a> {
    #[allow(clippy::type_complexity)]
    iter: Map<Iter<'a, Vec<OsString>>, fn(&Vec<OsString>) -> RawOccurrenceValues<'_>>,
}

impl<'a> Iterator for RawOccurrences<'a> {
    type Item = RawOccurrenceValues<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl DoubleEndedIterator for RawOccurrences<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back()
    }
}

impl ExactSizeIterator for RawOccurrences<'_> {}

impl Default for RawOccurrences<'_> {
    fn default() -> Self {
        static EMPTY: [Vec<OsString>; 0] = [];
        RawOccurrences {
            iter: EMPTY[..].iter().map(|_| unreachable!()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct RawOccurrenceValues<'a> {
    #[allow(clippy::type_complexity)]
    iter: Map<Iter<'a, OsString>, fn(&OsString) -> &OsStr>,
}

impl<'a> Iterator for RawOccurrenceValues<'a>
where
    Self: 'a,
{
    type Item = &'a OsStr;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a> DoubleEndedIterator for RawOccurrenceValues<'a>
where
    Self: 'a,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back()
    }
}

impl ExactSizeIterator for RawOccurrenceValues<'_> {}

/// Iterate over indices for where an argument appeared when parsing, via [`ArgMatches::indices_of`]
///
/// # Examples
///
/// ```rust
/// # use clap_builder as clap;
/// # use clap::{Command, Arg, ArgAction};
/// let m = Command::new("myapp")
///     .arg(Arg::new("output")
///         .short('o')
///         .num_args(1..)
///         .action(ArgAction::Set))
///     .get_matches_from(vec!["myapp", "-o", "val1", "val2"]);
///
/// let mut indices = m.indices_of("output").unwrap();
///
/// assert_eq!(indices.next(), Some(2));
/// assert_eq!(indices.next(), Some(3));
/// assert_eq!(indices.next(), None);
/// ```
/// [`ArgMatches::indices_of`]: ArgMatches::indices_of()
#[derive(Clone, Debug)]
pub struct Indices<'a> {
    iter: Cloned<Iter<'a, usize>>,
    len: usize,
}

impl Iterator for Indices<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        if let Some(next) = self.iter.next() {
            self.len -= 1;
            Some(next)
        } else {
            None
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl DoubleEndedIterator for Indices<'_> {
    fn next_back(&mut self) -> Option<usize> {
        if let Some(next) = self.iter.next_back() {
            self.len -= 1;
            Some(next)
        } else {
            None
        }
    }
}

impl ExactSizeIterator for Indices<'_> {}

/// Creates an empty iterator.
impl Default for Indices<'_> {
    fn default() -> Self {
        static EMPTY: [usize; 0] = [];
        // This is never called because the iterator is empty:
        Indices {
            iter: EMPTY[..].iter().cloned(),
            len: 0,
        }
    }
}

#[track_caller]
fn unwrap_downcast_ref<T: Any + Clone + Send + Sync + 'static>(value: &AnyValue) -> &T {
    value.downcast_ref().expect(INTERNAL_ERROR_MSG)
}

#[track_caller]
fn unwrap_downcast_into<T: Any + Clone + Send + Sync + 'static>(value: AnyValue) -> T {
    value.downcast_into().expect(INTERNAL_ERROR_MSG)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ArgAction;

    #[test]
    fn check_auto_traits() {
        static_assertions::assert_impl_all!(ArgMatches: Send, Sync, Unpin);
    }

    #[test]
    fn test_default_raw_values() {
        let mut values: RawValues<'_> = Default::default();
        assert_eq!(values.next(), None);
    }

    #[test]
    fn test_default_indices() {
        let mut indices: Indices<'_> = Indices::default();
        assert_eq!(indices.next(), None);
    }

    #[test]
    fn test_default_indices_with_shorter_lifetime() {
        let matches = ArgMatches::default();
        let mut indices = matches.indices_of("").unwrap_or_default();
        assert_eq!(indices.next(), None);
    }

    #[test]
    fn values_exact_size() {
        let l = crate::Command::new("test")
            .arg(
                crate::Arg::new("POTATO")
                    .action(ArgAction::Set)
                    .num_args(1..)
                    .required(true),
            )
            .try_get_matches_from(["test", "one"])
            .unwrap()
            .get_many::<String>("POTATO")
            .expect("present")
            .count();
        assert_eq!(l, 1);
    }

    #[test]
    fn os_values_exact_size() {
        let l = crate::Command::new("test")
            .arg(
                crate::Arg::new("POTATO")
                    .action(ArgAction::Set)
                    .num_args(1..)
                    .value_parser(crate::builder::ValueParser::os_string())
                    .required(true),
            )
            .try_get_matches_from(["test", "one"])
            .unwrap()
            .get_many::<OsString>("POTATO")
            .expect("present")
            .count();
        assert_eq!(l, 1);
    }

    #[test]
    fn indices_exact_size() {
        let l = crate::Command::new("test")
            .arg(
                crate::Arg::new("POTATO")
                    .action(ArgAction::Set)
                    .num_args(1..)
                    .required(true),
            )
            .try_get_matches_from(["test", "one"])
            .unwrap()
            .indices_of("POTATO")
            .expect("present")
            .len();
        assert_eq!(l, 1);
    }

    #[test]
    fn rev_iter() {
        let mut matches = crate::Command::new("myprog")
            .arg(crate::Arg::new("a").short('a').action(ArgAction::Append))
            .arg(crate::Arg::new("b").short('b').action(ArgAction::Append))
            .try_get_matches_from(vec!["myprog", "-a1", "-b1", "-b3"])
            .unwrap();

        let a_index = matches
            .indices_of("a")
            .expect("missing aopt indices")
            .collect::<Vec<_>>();
        dbg!(&a_index);
        let a_value = matches
            .remove_many::<String>("a")
            .expect("missing aopt values");
        dbg!(&a_value);
        let a = a_index.into_iter().zip(a_value).rev().collect::<Vec<_>>();
        dbg!(a);

        let b_index = matches
            .indices_of("b")
            .expect("missing aopt indices")
            .collect::<Vec<_>>();
        dbg!(&b_index);
        let b_value = matches
            .remove_many::<String>("b")
            .expect("missing aopt values");
        dbg!(&b_value);
        let b = b_index.into_iter().zip(b_value).rev().collect::<Vec<_>>();
        dbg!(b);
    }

    #[test]
    fn delete_id_without_returning() {
        let mut matches = crate::Command::new("myprog")
            .arg(crate::Arg::new("a").short('a').action(ArgAction::Append))
            .arg(crate::Arg::new("b").short('b').action(ArgAction::Append))
            .arg(crate::Arg::new("c").short('c').action(ArgAction::Append))
            .try_get_matches_from(vec!["myprog", "-b1", "-a1", "-b2"])
            .unwrap();
        let matches_ids_count = matches.ids().count();
        assert_eq!(matches_ids_count, 2);

        let _ = matches
            .try_clear_id("d")
            .expect_err("should fail due to there is no arg 'd'");

        let c_was_presented = matches
            .try_clear_id("c")
            .expect("doesn't fail because there is no matches for 'c' argument");
        assert!(!c_was_presented);
        let matches_ids_count = matches.ids().count();
        assert_eq!(matches_ids_count, 2);

        let b_was_presented = matches.try_clear_id("b").unwrap();
        assert!(b_was_presented);
        let matches_ids_count = matches.ids().count();
        assert_eq!(matches_ids_count, 1);

        let a_was_presented = matches.try_clear_id("a").unwrap();
        assert!(a_was_presented);
        let matches_ids_count = matches.ids().count();
        assert_eq!(matches_ids_count, 0);
    }
}
