use super::{
    directive::{self, Directive},
    EnvFilter, FromEnvError,
};
use crate::sync::RwLock;
use std::env;
use thread_local::ThreadLocal;
use tracing::level_filters::STATIC_MAX_LEVEL;

/// A [builder] for constructing new [`EnvFilter`]s.
///
/// [builder]: https://rust-unofficial.github.io/patterns/patterns/creational/builder.html
#[derive(Debug, Clone)]
#[must_use]
pub struct Builder {
    regex: bool,
    env: Option<String>,
    default_directive: Option<Directive>,
}

impl Builder {
    /// Sets whether span field values can be matched with regular expressions.
    ///
    /// If this is `true`, field filter directives will be interpreted as
    /// regular expressions if they are not able to be interpreted as a `bool`,
    /// `i64`, `u64`, or `f64` literal. If this is `false,` those field values
    /// will be interpreted as literal [`std::fmt::Debug`] output instead.
    ///
    /// By default, regular expressions are enabled.
    ///
    /// **Note**: when [`EnvFilter`]s are constructed from untrusted inputs,
    /// disabling regular expressions is strongly encouraged.
    pub fn with_regex(self, regex: bool) -> Self {
        Self { regex, ..self }
    }

    /// Sets a default [filtering directive] that will be added to the filter if
    /// the parsed string or environment variable contains no filter directives.
    ///
    /// By default, there is no default directive.
    ///
    /// # Examples
    ///
    /// If [`parse`], [`parse_lossy`], [`from_env`], or [`from_env_lossy`] are
    /// called with an empty string or environment variable, the default
    /// directive is used instead:
    ///
    /// ```rust
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use tracing_subscriber::filter::{EnvFilter, LevelFilter};
    ///
    /// let filter = EnvFilter::builder()
    ///     .with_default_directive(LevelFilter::INFO.into())
    ///     .parse("")?;
    ///
    /// assert_eq!(format!("{}", filter), "info");
    /// # Ok(()) }
    /// ```
    ///
    /// Note that the `lossy` variants ([`parse_lossy`] and [`from_env_lossy`])
    /// will ignore any invalid directives. If all directives in a filter
    /// string or environment variable are invalid, those methods will also use
    /// the default directive:
    ///
    /// ```rust
    /// use tracing_subscriber::filter::{EnvFilter, LevelFilter};
    ///
    /// let filter = EnvFilter::builder()
    ///     .with_default_directive(LevelFilter::INFO.into())
    ///     .parse_lossy("some_target=fake level,foo::bar=lolwut");
    ///
    /// assert_eq!(format!("{}", filter), "info");
    /// ```
    ///
    ///
    /// If the string or environment variable contains valid filtering
    /// directives, the default directive is not used:
    ///
    /// ```rust
    /// use tracing_subscriber::filter::{EnvFilter, LevelFilter};
    ///
    /// let filter = EnvFilter::builder()
    ///     .with_default_directive(LevelFilter::INFO.into())
    ///     .parse_lossy("foo=trace");
    ///
    /// // The default directive is *not* used:
    /// assert_eq!(format!("{}", filter), "foo=trace");
    /// ```
    ///
    /// Parsing a more complex default directive from a string:
    ///
    /// ```rust
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use tracing_subscriber::filter::{EnvFilter, LevelFilter};
    ///
    /// let default = "myapp=debug".parse()
    ///     .expect("hard-coded default directive should be valid");
    ///
    /// let filter = EnvFilter::builder()
    ///     .with_default_directive(default)
    ///     .parse("")?;
    ///
    /// assert_eq!(format!("{}", filter), "myapp=debug");
    /// # Ok(()) }
    /// ```
    ///
    /// [`parse_lossy`]: Self::parse_lossy
    /// [`from_env_lossy`]: Self::from_env_lossy
    /// [`parse`]: Self::parse
    /// [`from_env`]: Self::from_env
    pub fn with_default_directive(self, default_directive: Directive) -> Self {
        Self {
            default_directive: Some(default_directive),
            ..self
        }
    }

    /// Sets the name of the environment variable used by the [`from_env`],
    /// [`from_env_lossy`], and [`try_from_env`] methods.
    ///
    /// By default, this is the value of [`EnvFilter::DEFAULT_ENV`]
    /// (`RUST_LOG`).
    ///
    /// [`from_env`]: Self::from_env
    /// [`from_env_lossy`]: Self::from_env_lossy
    /// [`try_from_env`]: Self::try_from_env
    pub fn with_env_var(self, var: impl ToString) -> Self {
        Self {
            env: Some(var.to_string()),
            ..self
        }
    }

    /// Returns a new [`EnvFilter`] from the directives in the given string,
    /// *ignoring* any that are invalid.
    ///
    /// If `parse_lossy` is called with an empty string, then the
    /// [default directive] is used instead.
    ///
    /// [default directive]: Self::with_default_directive
    pub fn parse_lossy<S: AsRef<str>>(&self, dirs: S) -> EnvFilter {
        let directives = dirs
            .as_ref()
            .split(',')
            .filter(|s| !s.is_empty())
            .filter_map(|s| match Directive::parse(s, self.regex) {
                Ok(d) => Some(d),
                Err(err) => {
                    eprintln!("ignoring `{}`: {}", s, err);
                    None
                }
            });
        self.from_directives(directives)
    }

    /// Returns a new [`EnvFilter`] from the directives in the given string,
    /// or an error if any are invalid.
    ///
    /// If `parse` is called with an empty string, then the [default directive]
    /// is used instead.
    ///
    /// [default directive]: Self::with_default_directive
    pub fn parse<S: AsRef<str>>(&self, dirs: S) -> Result<EnvFilter, directive::ParseError> {
        let dirs = dirs.as_ref();
        if dirs.is_empty() {
            return Ok(self.from_directives(std::iter::empty()));
        }
        let directives = dirs
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| Directive::parse(s, self.regex))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self.from_directives(directives))
    }

    /// Returns a new [`EnvFilter`] from the directives in the configured
    /// environment variable, ignoring any directives that are invalid.
    ///
    /// If the environment variable is empty, then the [default directive]
    /// is used instead.
    ///
    /// [default directive]: Self::with_default_directive
    pub fn from_env_lossy(&self) -> EnvFilter {
        let var = env::var(self.env_var_name()).unwrap_or_default();
        self.parse_lossy(var)
    }

    /// Returns a new [`EnvFilter`] from the directives in the configured
    /// environment variable. If the environment variable is unset, no directive is added.
    ///
    /// An error is returned if the environment contains invalid directives.
    ///
    /// If the environment variable is empty, then the [default directive]
    /// is used instead.
    ///
    /// [default directive]: Self::with_default_directive
    pub fn from_env(&self) -> Result<EnvFilter, FromEnvError> {
        let var = env::var(self.env_var_name()).unwrap_or_default();
        self.parse(var).map_err(Into::into)
    }

    /// Returns a new [`EnvFilter`] from the directives in the configured
    /// environment variable, or an error if the environment variable is not set
    /// or contains invalid directives.
    ///
    /// If the environment variable is empty, then the [default directive]
    /// is used instead.
    ///
    /// [default directive]: Self::with_default_directive
    pub fn try_from_env(&self) -> Result<EnvFilter, FromEnvError> {
        let var = env::var(self.env_var_name())?;
        self.parse(var).map_err(Into::into)
    }

    // TODO(eliza): consider making this a public API?
    // Clippy doesn't love this naming, because it suggests that `from_` methods
    // should not take a `Self`...but in this case, it's the `EnvFilter` that is
    // being constructed "from" the directives, rather than the builder itself.
    #[allow(clippy::wrong_self_convention)]
    pub(super) fn from_directives(
        &self,
        directives: impl IntoIterator<Item = Directive>,
    ) -> EnvFilter {
        use tracing::Level;

        let mut directives: Vec<_> = directives.into_iter().collect();
        let mut disabled = Vec::new();
        for directive in &mut directives {
            if directive.level > STATIC_MAX_LEVEL {
                disabled.push(directive.clone());
            }
            if !self.regex {
                directive.deregexify();
            }
        }

        if !disabled.is_empty() {
            #[cfg(feature = "nu-ansi-term")]
            use nu_ansi_term::{Color, Style};
            // NOTE: We can't use a configured `MakeWriter` because the EnvFilter
            // has no knowledge of any underlying subscriber or subscriber, which
            // may or may not use a `MakeWriter`.
            let warn = |msg: &str| {
                #[cfg(not(feature = "nu-ansi-term"))]
                let msg = format!("warning: {}", msg);
                #[cfg(feature = "nu-ansi-term")]
                let msg = {
                    let bold = Style::new().bold();
                    let mut warning = Color::Yellow.paint("warning");
                    warning.style_ref_mut().is_bold = true;
                    format!("{}{} {}", warning, bold.paint(":"), bold.paint(msg))
                };
                eprintln!("{}", msg);
            };
            let ctx_prefixed = |prefix: &str, msg: &str| {
                #[cfg(not(feature = "nu-ansi-term"))]
                let msg = format!("{} {}", prefix, msg);
                #[cfg(feature = "nu-ansi-term")]
                let msg = {
                    let mut equal = Color::Fixed(21).paint("="); // dark blue
                    equal.style_ref_mut().is_bold = true;
                    format!(" {} {} {}", equal, Style::new().bold().paint(prefix), msg)
                };
                eprintln!("{}", msg);
            };
            let ctx_help = |msg| ctx_prefixed("help:", msg);
            let ctx_note = |msg| ctx_prefixed("note:", msg);
            let ctx = |msg: &str| {
                #[cfg(not(feature = "nu-ansi-term"))]
                let msg = format!("note: {}", msg);
                #[cfg(feature = "nu-ansi-term")]
                let msg = {
                    let mut pipe = Color::Fixed(21).paint("|");
                    pipe.style_ref_mut().is_bold = true;
                    format!(" {} {}", pipe, msg)
                };
                eprintln!("{}", msg);
            };
            warn("some trace filter directives would enable traces that are disabled statically");
            for directive in disabled {
                let target = if let Some(target) = &directive.target {
                    format!("the `{}` target", target)
                } else {
                    "all targets".into()
                };
                let level = directive
                    .level
                    .into_level()
                    .expect("=off would not have enabled any filters");
                ctx(&format!(
                    "`{}` would enable the {} level for {}",
                    directive, level, target
                ));
            }
            ctx_note(&format!("the static max level is `{}`", STATIC_MAX_LEVEL));
            let help_msg = || {
                let (feature, filter) = match STATIC_MAX_LEVEL.into_level() {
                    Some(Level::TRACE) => unreachable!(
                        "if the max level is trace, no static filtering features are enabled"
                    ),
                    Some(Level::DEBUG) => ("max_level_debug", Level::TRACE),
                    Some(Level::INFO) => ("max_level_info", Level::DEBUG),
                    Some(Level::WARN) => ("max_level_warn", Level::INFO),
                    Some(Level::ERROR) => ("max_level_error", Level::WARN),
                    None => return ("max_level_off", String::new()),
                };
                (feature, format!("{} ", filter))
            };
            let (feature, earlier_level) = help_msg();
            ctx_help(&format!(
                "to enable {}logging, remove the `{}` feature from the `tracing` crate",
                earlier_level, feature
            ));
        }

        let (dynamics, statics) = Directive::make_tables(directives);
        let has_dynamics = !dynamics.is_empty();

        let mut filter = EnvFilter {
            statics,
            dynamics,
            has_dynamics,
            by_id: RwLock::new(Default::default()),
            by_cs: RwLock::new(Default::default()),
            scope: ThreadLocal::new(),
            regex: self.regex,
        };

        if !has_dynamics && filter.statics.is_empty() {
            if let Some(ref default) = self.default_directive {
                filter = filter.add_directive(default.clone());
            }
        }

        filter
    }

    fn env_var_name(&self) -> &str {
        self.env.as_deref().unwrap_or(EnvFilter::DEFAULT_ENV)
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            regex: true,
            env: None,
            default_directive: None,
        }
    }
}
