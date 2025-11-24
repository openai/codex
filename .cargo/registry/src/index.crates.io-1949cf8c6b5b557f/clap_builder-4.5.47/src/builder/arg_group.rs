// Internal
use crate::builder::IntoResettable;
use crate::util::Id;

/// Family of related [arguments].
///
/// By placing arguments in a logical group, you can create easier requirement and
/// exclusion rules instead of having to list each argument individually, or when you want a rule
/// to apply "any but not all" arguments.
///
/// For instance, you can make an entire `ArgGroup` required. If [`ArgGroup::multiple(true)`] is
/// set, this means that at least one argument from that group must be present. If
/// [`ArgGroup::multiple(false)`] is set (the default), one and *only* one must be present.
///
/// You can also do things such as name an entire `ArgGroup` as a [conflict] or [requirement] for
/// another argument, meaning any of the arguments that belong to that group will cause a failure
/// if present, or must be present respectively.
///
/// Perhaps the most common use of `ArgGroup`s is to require one and *only* one argument to be
/// present out of a given set. Imagine that you had multiple arguments, and you want one of them
/// to be required, but making all of them required isn't feasible because perhaps they conflict
/// with each other. For example, lets say that you were building an application where one could
/// set a given version number by supplying a string with an option argument, i.e.
/// `--set-ver v1.2.3`, you also wanted to support automatically using a previous version number
/// and simply incrementing one of the three numbers. So you create three flags `--major`,
/// `--minor`, and `--patch`. All of these arguments shouldn't be used at one time but you want to
/// specify that *at least one* of them is used. For this, you can create a group.
///
/// Finally, you may use `ArgGroup`s to pull a value from a group of arguments when you don't care
/// exactly which argument was actually used at runtime.
///
/// # Examples
///
/// The following example demonstrates using an `ArgGroup` to ensure that one, and only one, of
/// the arguments from the specified group is present at runtime.
///
/// ```rust
/// # use clap_builder as clap;
/// # use clap::{Command, arg, ArgGroup, error::ErrorKind};
/// let result = Command::new("cmd")
///     .arg(arg!(--"set-ver" <ver> "set the version manually"))
///     .arg(arg!(--major           "auto increase major"))
///     .arg(arg!(--minor           "auto increase minor"))
///     .arg(arg!(--patch           "auto increase patch"))
///     .group(ArgGroup::new("vers")
///          .args(["set-ver", "major", "minor", "patch"])
///          .required(true))
///     .try_get_matches_from(vec!["cmd", "--major", "--patch"]);
/// // Because we used two args in the group it's an error
/// assert!(result.is_err());
/// let err = result.unwrap_err();
/// assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
/// ```
///
/// This next example shows a passing parse of the same scenario
/// ```rust
/// # use clap_builder as clap;
/// # use clap::{Command, arg, ArgGroup, Id};
/// let result = Command::new("cmd")
///     .arg(arg!(--"set-ver" <ver> "set the version manually"))
///     .arg(arg!(--major           "auto increase major"))
///     .arg(arg!(--minor           "auto increase minor"))
///     .arg(arg!(--patch           "auto increase patch"))
///     .group(ArgGroup::new("vers")
///          .args(["set-ver", "major", "minor","patch"])
///          .required(true))
///     .try_get_matches_from(vec!["cmd", "--major"]);
/// assert!(result.is_ok());
/// let matches = result.unwrap();
/// // We may not know which of the args was used, so we can test for the group...
/// assert!(matches.contains_id("vers"));
/// // We can also ask the group which arg was used
/// assert_eq!(matches
///     .get_one::<Id>("vers")
///     .expect("`vers` is required")
///     .as_str(),
///     "major"
/// );
/// // we could also alternatively check each arg individually (not shown here)
/// ```
/// [`ArgGroup::multiple(true)`]: ArgGroup::multiple()
///
/// [`ArgGroup::multiple(false)`]: ArgGroup::multiple()
/// [arguments]: crate::Arg
/// [conflict]: crate::Arg::conflicts_with()
/// [requirement]: crate::Arg::requires()
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct ArgGroup {
    pub(crate) id: Id,
    pub(crate) args: Vec<Id>,
    pub(crate) required: bool,
    pub(crate) requires: Vec<Id>,
    pub(crate) conflicts: Vec<Id>,
    pub(crate) multiple: bool,
}

/// # Builder
impl ArgGroup {
    /// Create a `ArgGroup` using a unique name.
    ///
    /// The name will be used to get values from the group or refer to the group inside of conflict
    /// and requirement rules.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, ArgGroup};
    /// ArgGroup::new("config")
    /// # ;
    /// ```
    pub fn new(id: impl Into<Id>) -> Self {
        ArgGroup::default().id(id)
    }

    /// Sets the group name.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, ArgGroup};
    /// ArgGroup::default().id("config")
    /// # ;
    /// ```
    #[must_use]
    pub fn id(mut self, id: impl Into<Id>) -> Self {
        self.id = id.into();
        self
    }

    /// Adds an [argument] to this group by name
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgGroup, ArgAction};
    /// let m = Command::new("myprog")
    ///     .arg(Arg::new("flag")
    ///         .short('f')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("color")
    ///         .short('c')
    ///         .action(ArgAction::SetTrue))
    ///     .group(ArgGroup::new("req_flags")
    ///         .arg("flag")
    ///         .arg("color"))
    ///     .get_matches_from(vec!["myprog", "-f"]);
    /// // maybe we don't know which of the two flags was used...
    /// assert!(m.contains_id("req_flags"));
    /// // but we can also check individually if needed
    /// assert!(m.contains_id("flag"));
    /// ```
    /// [argument]: crate::Arg
    #[must_use]
    pub fn arg(mut self, arg_id: impl IntoResettable<Id>) -> Self {
        if let Some(arg_id) = arg_id.into_resettable().into_option() {
            self.args.push(arg_id);
        } else {
            self.args.clear();
        }
        self
    }

    /// Adds multiple [arguments] to this group by name
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgGroup, ArgAction};
    /// let m = Command::new("myprog")
    ///     .arg(Arg::new("flag")
    ///         .short('f')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("color")
    ///         .short('c')
    ///         .action(ArgAction::SetTrue))
    ///     .group(ArgGroup::new("req_flags")
    ///         .args(["flag", "color"]))
    ///     .get_matches_from(vec!["myprog", "-f"]);
    /// // maybe we don't know which of the two flags was used...
    /// assert!(m.contains_id("req_flags"));
    /// // but we can also check individually if needed
    /// assert!(m.contains_id("flag"));
    /// ```
    /// [arguments]: crate::Arg
    #[must_use]
    pub fn args(mut self, ns: impl IntoIterator<Item = impl Into<Id>>) -> Self {
        for n in ns {
            self = self.arg(n);
        }
        self
    }

    /// Getters for all args. It will return a vector of `Id`
    ///
    /// # Example
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{ArgGroup};
    /// let args: Vec<&str> = vec!["a1".into(), "a4".into()];
    /// let grp = ArgGroup::new("program").args(&args);
    ///
    /// for (pos, arg) in grp.get_args().enumerate() {
    ///     assert_eq!(*arg, args[pos]);
    /// }
    /// ```
    pub fn get_args(&self) -> impl Iterator<Item = &Id> {
        self.args.iter()
    }

    /// Allows more than one of the [`Arg`]s in this group to be used. (Default: `false`)
    ///
    /// # Examples
    ///
    /// Notice in this example we use *both* the `-f` and `-c` flags which are both part of the
    /// group
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgGroup, ArgAction};
    /// let m = Command::new("myprog")
    ///     .arg(Arg::new("flag")
    ///         .short('f')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("color")
    ///         .short('c')
    ///         .action(ArgAction::SetTrue))
    ///     .group(ArgGroup::new("req_flags")
    ///         .args(["flag", "color"])
    ///         .multiple(true))
    ///     .get_matches_from(vec!["myprog", "-f", "-c"]);
    /// // maybe we don't know which of the two flags was used...
    /// assert!(m.contains_id("req_flags"));
    /// ```
    /// In this next example, we show the default behavior (i.e. `multiple(false)`) which will throw
    /// an error if more than one of the args in the group was used.
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgGroup, error::ErrorKind, ArgAction};
    /// let result = Command::new("myprog")
    ///     .arg(Arg::new("flag")
    ///         .short('f')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("color")
    ///         .short('c')
    ///         .action(ArgAction::SetTrue))
    ///     .group(ArgGroup::new("req_flags")
    ///         .args(["flag", "color"]))
    ///     .try_get_matches_from(vec!["myprog", "-f", "-c"]);
    /// // Because we used both args in the group it's an error
    /// assert!(result.is_err());
    /// let err = result.unwrap_err();
    /// assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
    /// ```
    ///
    /// [`Arg`]: crate::Arg
    #[inline]
    #[must_use]
    pub fn multiple(mut self, yes: bool) -> Self {
        self.multiple = yes;
        self
    }

    /// Return true if the group allows more than one of the arguments
    /// in this group to be used. (Default: `false`)
    ///
    /// # Example
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{ArgGroup};
    /// let mut group = ArgGroup::new("myprog")
    ///     .args(["f", "c"])
    ///     .multiple(true);
    ///
    /// assert!(group.is_multiple());
    /// ```
    pub fn is_multiple(&mut self) -> bool {
        self.multiple
    }

    /// Require an argument from the group to be present when parsing.
    ///
    /// This is unless conflicting with another argument.  A required group will be displayed in
    /// the usage string of the application in the format `<arg|arg2|arg3>`.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** This setting only applies to the current [`Command`] / [`Subcommand`]s, and not
    /// globally.
    ///
    /// </div>
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** By default, [`ArgGroup::multiple`] is set to `false` which when combined with
    /// `ArgGroup::required(true)` states, "One and *only one* arg must be used from this group.
    /// Use of more than one arg is an error." Vice setting `ArgGroup::multiple(true)` which
    /// states, '*At least* one arg from this group must be used. Using multiple is OK."
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgGroup, error::ErrorKind, ArgAction};
    /// let result = Command::new("myprog")
    ///     .arg(Arg::new("flag")
    ///         .short('f')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("color")
    ///         .short('c')
    ///         .action(ArgAction::SetTrue))
    ///     .group(ArgGroup::new("req_flags")
    ///         .args(["flag", "color"])
    ///         .required(true))
    ///     .try_get_matches_from(vec!["myprog"]);
    /// // Because we didn't use any of the args in the group, it's an error
    /// assert!(result.is_err());
    /// let err = result.unwrap_err();
    /// assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
    /// ```
    ///
    /// [`Subcommand`]: crate::Subcommand
    /// [`ArgGroup::multiple`]: ArgGroup::multiple()
    /// [`Command`]: crate::Command
    #[inline]
    #[must_use]
    pub fn required(mut self, yes: bool) -> Self {
        self.required = yes;
        self
    }

    /// Specify an argument or group that must be present when this group is.
    ///
    /// This is not to be confused with a [required group]. Requirement rules function just like
    /// [argument requirement rules], you can name other arguments or groups that must be present
    /// when any one of the arguments from this group is used.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** The name provided may be an argument or group name
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgGroup, error::ErrorKind, ArgAction};
    /// let result = Command::new("myprog")
    ///     .arg(Arg::new("flag")
    ///         .short('f')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("color")
    ///         .short('c')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("debug")
    ///         .short('d')
    ///         .action(ArgAction::SetTrue))
    ///     .group(ArgGroup::new("req_flags")
    ///         .args(["flag", "color"])
    ///         .requires("debug"))
    ///     .try_get_matches_from(vec!["myprog", "-c"]);
    /// // because we used an arg from the group, and the group requires "-d" to be used, it's an
    /// // error
    /// assert!(result.is_err());
    /// let err = result.unwrap_err();
    /// assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
    /// ```
    /// [required group]: ArgGroup::required()
    /// [argument requirement rules]: crate::Arg::requires()
    #[must_use]
    pub fn requires(mut self, id: impl IntoResettable<Id>) -> Self {
        if let Some(id) = id.into_resettable().into_option() {
            self.requires.push(id);
        } else {
            self.requires.clear();
        }
        self
    }

    /// Specify arguments or groups that must be present when this group is.
    ///
    /// This is not to be confused with a [required group]. Requirement rules function just like
    /// [argument requirement rules], you can name other arguments or groups that must be present
    /// when one of the arguments from this group is used.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** The names provided may be an argument or group name
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgGroup, error::ErrorKind, ArgAction};
    /// let result = Command::new("myprog")
    ///     .arg(Arg::new("flag")
    ///         .short('f')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("color")
    ///         .short('c')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("debug")
    ///         .short('d')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("verb")
    ///         .short('v')
    ///         .action(ArgAction::SetTrue))
    ///     .group(ArgGroup::new("req_flags")
    ///         .args(["flag", "color"])
    ///         .requires_all(["debug", "verb"]))
    ///     .try_get_matches_from(vec!["myprog", "-c", "-d"]);
    /// // because we used an arg from the group, and the group requires "-d" and "-v" to be used,
    /// // yet we only used "-d" it's an error
    /// assert!(result.is_err());
    /// let err = result.unwrap_err();
    /// assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
    /// ```
    /// [required group]: ArgGroup::required()
    /// [argument requirement rules]: crate::Arg::requires_ifs()
    #[must_use]
    pub fn requires_all(mut self, ns: impl IntoIterator<Item = impl Into<Id>>) -> Self {
        for n in ns {
            self = self.requires(n);
        }
        self
    }

    /// Specify an argument or group that must **not** be present when this group is.
    ///
    /// Exclusion (aka conflict) rules function just like [argument exclusion rules], you can name
    /// other arguments or groups that must *not* be present when one of the arguments from this
    /// group are used.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** The name provided may be an argument, or group name
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgGroup, error::ErrorKind, ArgAction};
    /// let result = Command::new("myprog")
    ///     .arg(Arg::new("flag")
    ///         .short('f')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("color")
    ///         .short('c')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("debug")
    ///         .short('d')
    ///         .action(ArgAction::SetTrue))
    ///     .group(ArgGroup::new("req_flags")
    ///         .args(["flag", "color"])
    ///         .conflicts_with("debug"))
    ///     .try_get_matches_from(vec!["myprog", "-c", "-d"]);
    /// // because we used an arg from the group, and the group conflicts with "-d", it's an error
    /// assert!(result.is_err());
    /// let err = result.unwrap_err();
    /// assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
    /// ```
    /// [argument exclusion rules]: crate::Arg::conflicts_with()
    #[must_use]
    pub fn conflicts_with(mut self, id: impl IntoResettable<Id>) -> Self {
        if let Some(id) = id.into_resettable().into_option() {
            self.conflicts.push(id);
        } else {
            self.conflicts.clear();
        }
        self
    }

    /// Specify arguments or groups that must **not** be present when this group is.
    ///
    /// Exclusion rules function just like [argument exclusion rules], you can name other arguments
    /// or groups that must *not* be present when one of the arguments from this group are used.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** The names provided may be an argument, or group name
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::{Command, Arg, ArgGroup, error::ErrorKind, ArgAction};
    /// let result = Command::new("myprog")
    ///     .arg(Arg::new("flag")
    ///         .short('f')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("color")
    ///         .short('c')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("debug")
    ///         .short('d')
    ///         .action(ArgAction::SetTrue))
    ///     .arg(Arg::new("verb")
    ///         .short('v')
    ///         .action(ArgAction::SetTrue))
    ///     .group(ArgGroup::new("req_flags")
    ///         .args(["flag", "color"])
    ///         .conflicts_with_all(["debug", "verb"]))
    ///     .try_get_matches_from(vec!["myprog", "-c", "-v"]);
    /// // because we used an arg from the group, and the group conflicts with either "-v" or "-d"
    /// // it's an error
    /// assert!(result.is_err());
    /// let err = result.unwrap_err();
    /// assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
    /// ```
    ///
    /// [argument exclusion rules]: crate::Arg::conflicts_with_all()
    #[must_use]
    pub fn conflicts_with_all(mut self, ns: impl IntoIterator<Item = impl Into<Id>>) -> Self {
        for n in ns {
            self = self.conflicts_with(n);
        }
        self
    }
}

/// # Reflection
impl ArgGroup {
    /// Get the name of the group
    #[inline]
    pub fn get_id(&self) -> &Id {
        &self.id
    }

    /// Reports whether [`ArgGroup::required`] is set
    #[inline]
    pub fn is_required_set(&self) -> bool {
        self.required
    }
}

impl From<&'_ ArgGroup> for ArgGroup {
    fn from(g: &ArgGroup) -> Self {
        g.clone()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn groups() {
        let g = ArgGroup::new("test")
            .arg("a1")
            .arg("a4")
            .args(["a2", "a3"])
            .required(true)
            .conflicts_with("c1")
            .conflicts_with_all(["c2", "c3"])
            .conflicts_with("c4")
            .requires("r1")
            .requires_all(["r2", "r3"])
            .requires("r4");

        let args: Vec<Id> = vec!["a1".into(), "a4".into(), "a2".into(), "a3".into()];
        let reqs: Vec<Id> = vec!["r1".into(), "r2".into(), "r3".into(), "r4".into()];
        let confs: Vec<Id> = vec!["c1".into(), "c2".into(), "c3".into(), "c4".into()];

        assert_eq!(g.args, args);
        assert_eq!(g.requires, reqs);
        assert_eq!(g.conflicts, confs);
    }

    #[test]
    fn test_from() {
        let g = ArgGroup::new("test")
            .arg("a1")
            .arg("a4")
            .args(["a2", "a3"])
            .required(true)
            .conflicts_with("c1")
            .conflicts_with_all(["c2", "c3"])
            .conflicts_with("c4")
            .requires("r1")
            .requires_all(["r2", "r3"])
            .requires("r4");

        let args: Vec<Id> = vec!["a1".into(), "a4".into(), "a2".into(), "a3".into()];
        let reqs: Vec<Id> = vec!["r1".into(), "r2".into(), "r3".into(), "r4".into()];
        let confs: Vec<Id> = vec!["c1".into(), "c2".into(), "c3".into(), "c4".into()];

        let g2 = ArgGroup::from(&g);
        assert_eq!(g2.args, args);
        assert_eq!(g2.requires, reqs);
        assert_eq!(g2.conflicts, confs);
    }

    // This test will *fail to compile* if ArgGroup is not Send + Sync
    #[test]
    fn arg_group_send_sync() {
        fn foo<T: Send + Sync>(_: T) {}
        foo(ArgGroup::new("test"));
    }

    #[test]
    fn arg_group_expose_is_multiple_helper() {
        let args: Vec<Id> = vec!["a1".into(), "a4".into()];

        let mut grp_multiple = ArgGroup::new("test_multiple").args(&args).multiple(true);
        assert!(grp_multiple.is_multiple());

        let mut grp_not_multiple = ArgGroup::new("test_multiple").args(&args).multiple(false);
        assert!(!grp_not_multiple.is_multiple());
    }

    #[test]
    fn arg_group_expose_get_args_helper() {
        let args: Vec<Id> = vec!["a1".into(), "a4".into()];
        let grp = ArgGroup::new("program").args(&args);

        for (pos, arg) in grp.get_args().enumerate() {
            assert_eq!(*arg, args[pos]);
        }
    }
}
