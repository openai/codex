use crate::builder::IntoResettable;
use crate::builder::Str;
use crate::builder::StyledStr;
use crate::util::eq_ignore_case;

/// A possible value of an argument.
///
/// This is used for specifying [possible values] of [Args].
///
/// See also [`PossibleValuesParser`][crate::builder::PossibleValuesParser]
///
/// <div class="warning">
///
/// **NOTE:** Most likely you can use strings, rather than `PossibleValue` as it is only required
/// to [hide] single values from help messages and shell completions or to attach [help] to
/// possible values.
///
/// </div>
///
/// # Examples
///
/// ```rust
/// # use clap_builder as clap;
/// # use clap::{Arg, builder::PossibleValue, ArgAction};
/// let cfg = Arg::new("config")
///     .action(ArgAction::Set)
///     .value_name("FILE")
///     .value_parser([
///         PossibleValue::new("fast"),
///         PossibleValue::new("slow").help("slower than fast"),
///         PossibleValue::new("secret speed").hide(true)
///     ]);
/// ```
///
/// [Args]: crate::Arg
/// [possible values]: crate::builder::ValueParser::possible_values
/// [hide]: PossibleValue::hide()
/// [help]: PossibleValue::help()
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PossibleValue {
    name: Str,
    help: Option<StyledStr>,
    aliases: Vec<Str>, // (name, visible)
    hide: bool,
}

impl PossibleValue {
    /// Create a [`PossibleValue`] with its name.
    ///
    /// The name will be used to decide whether this value was provided by the user to an argument.
    ///
    /// <div class="warning">
    ///
    /// **NOTE:** In case it is not [hidden] it will also be shown in help messages for arguments
    /// that use it as a [possible value] and have not hidden them through [`Arg::hide_possible_values(true)`].
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::builder::PossibleValue;
    /// PossibleValue::new("fast")
    /// # ;
    /// ```
    /// [hidden]: PossibleValue::hide
    /// [possible value]: crate::builder::PossibleValuesParser
    /// [`Arg::hide_possible_values(true)`]: crate::Arg::hide_possible_values()
    pub fn new(name: impl Into<Str>) -> Self {
        PossibleValue {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Sets the help description of the value.
    ///
    /// This is typically displayed in completions (where supported) and should be a short, one-line
    /// description.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::builder::PossibleValue;
    /// PossibleValue::new("slow")
    ///     .help("not fast")
    /// # ;
    /// ```
    #[inline]
    #[must_use]
    pub fn help(mut self, help: impl IntoResettable<StyledStr>) -> Self {
        self.help = help.into_resettable().into_option();
        self
    }

    /// Hides this value from help and shell completions.
    ///
    /// This is an alternative to hiding through [`Arg::hide_possible_values(true)`], if you only
    /// want to hide some values.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::builder::PossibleValue;
    /// PossibleValue::new("secret")
    ///     .hide(true)
    /// # ;
    /// ```
    /// [`Arg::hide_possible_values(true)`]: crate::Arg::hide_possible_values()
    #[inline]
    #[must_use]
    pub fn hide(mut self, yes: bool) -> Self {
        self.hide = yes;
        self
    }

    /// Sets a *hidden* alias for this argument value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::builder::PossibleValue;
    /// PossibleValue::new("slow")
    ///     .alias("not-fast")
    /// # ;
    /// ```
    #[must_use]
    pub fn alias(mut self, name: impl IntoResettable<Str>) -> Self {
        if let Some(name) = name.into_resettable().into_option() {
            self.aliases.push(name);
        } else {
            self.aliases.clear();
        }
        self
    }

    /// Sets multiple *hidden* aliases for this argument value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::builder::PossibleValue;
    /// PossibleValue::new("slow")
    ///     .aliases(["not-fast", "snake-like"])
    /// # ;
    /// ```
    #[must_use]
    pub fn aliases(mut self, names: impl IntoIterator<Item = impl Into<Str>>) -> Self {
        self.aliases.extend(names.into_iter().map(|a| a.into()));
        self
    }
}

/// Reflection
impl PossibleValue {
    /// Get the name of the argument value
    #[inline]
    pub fn get_name(&self) -> &str {
        self.name.as_str()
    }

    /// Get the help specified for this argument, if any
    #[inline]
    pub fn get_help(&self) -> Option<&StyledStr> {
        self.help.as_ref()
    }

    /// Report if [`PossibleValue::hide`] is set
    #[inline]
    pub fn is_hide_set(&self) -> bool {
        self.hide
    }

    /// Report if `PossibleValue` is not hidden and has a help message
    pub(crate) fn should_show_help(&self) -> bool {
        !self.hide && self.help.is_some()
    }

    /// Get the name if argument value is not hidden, `None` otherwise,
    /// but wrapped in quotes if it contains whitespace
    #[cfg(feature = "help")]
    pub(crate) fn get_visible_quoted_name(&self) -> Option<std::borrow::Cow<'_, str>> {
        if !self.hide {
            Some(if self.name.contains(char::is_whitespace) {
                format!("{:?}", self.name).into()
            } else {
                self.name.as_str().into()
            })
        } else {
            None
        }
    }

    /// Returns all valid values of the argument value.
    ///
    /// Namely the name and all aliases.
    pub fn get_name_and_aliases(&self) -> impl Iterator<Item = &str> + '_ {
        std::iter::once(self.get_name()).chain(self.aliases.iter().map(|s| s.as_str()))
    }

    /// Tests if the value is valid for this argument value
    ///
    /// The value is valid if it is either the name or one of the aliases.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use clap_builder as clap;
    /// # use clap::builder::PossibleValue;
    /// let arg_value = PossibleValue::new("fast").alias("not-slow");
    ///
    /// assert!(arg_value.matches("fast", false));
    /// assert!(arg_value.matches("not-slow", false));
    ///
    /// assert!(arg_value.matches("FAST", true));
    /// assert!(!arg_value.matches("FAST", false));
    /// ```
    pub fn matches(&self, value: &str, ignore_case: bool) -> bool {
        if ignore_case {
            self.get_name_and_aliases()
                .any(|name| eq_ignore_case(name, value))
        } else {
            self.get_name_and_aliases().any(|name| name == value)
        }
    }
}

impl<S: Into<Str>> From<S> for PossibleValue {
    fn from(s: S) -> Self {
        Self::new(s)
    }
}
