use core::fmt;
use core::sync::atomic::AtomicU8;
use core::sync::atomic::{AtomicPtr, Ordering};

/// A function that decides whether styling should be applied.
///
/// A styling `Condition` can be specified globally via
/// [`yansi::whenever()`](crate::whenever()) or locally to a specific style via
/// the [`whenever()`](crate::Style::whenever()) builder method. Any time a
/// [`Painted`](crate::Painted) value is formatted, both the local and global
/// conditions are checked, and only when both evaluate to `true` is styling
/// actually applied.
///
/// A `Condition` is nothing more than a function that returns a `bool`. The
/// function is called each and every time a `Painted` is formatted, and so it
/// is expected to be fast. All of the built-in conditions (except for their
/// "live" variants) cache their first evaluation as a result: the
/// [`Condition::cached()`] constructor can do the same for your conditions.
///
/// # Built-In Conditions
///
/// `yansi` comes with built-in conditions for common scenarios that can be
/// enabled via crate features:
///
/// | feature(s)                   | condition                       | implication            |
/// |------------------------------|---------------------------------|------------------------|
/// | `detect-tty`                 | [TTY Detectors]                 | `std`, [`is-terminal`] |
/// | `detect-env`                 | [Environment Variable Checkers] | `std`                  |
/// | [`detect-tty`, `detect-env`] | All Above, [Combo Detectors]    | `std`, [`is-terminal`] |
///
/// [`is-terminal`]: https://docs.rs/is-terminal
///
/// For example, to enable the TTY detectors, enable the `detect-tty` feature:
///
/// ```toml
/// yansi = { version = "...", features = ["detect-tty"] }
/// ```
///
/// To enable the TTY detectors, env-var checkers, and combo detectors, enable
/// `detect-tty` _and_ `detect-env`:
///
/// ```toml
/// yansi = { version = "...", features = ["detect-tty", "detect-env"] }
/// ```
///
/// ```rust
/// # #[cfg(all(feature = "detect-tty", feature = "detect-env"))] {
/// use yansi::Condition;
///
/// yansi::whenever(Condition::TTY_AND_COLOR);
/// # }
/// ```
///
/// [TTY detectors]: Condition#impl-Condition-1
/// [Environment Variable Checkers]: Condition#impl-Condition-2
/// [Combo Detectors]: Condition#impl-Condition-3
///
/// # Custom Conditions
///
/// Custom, arbitrary conditions can be created with [`Condition::from()`] or
/// [`Condition::cached()`].
///
/// ```rust
/// # #[cfg(all(feature = "detect-tty", feature = "detect-env"))] {
/// use yansi::{Condition, Style, Color::*};
///
/// // Combine two conditions (`stderr` is a TTY, `CLICOLOR` is set) into one.
/// static STDERR_COLOR: Condition = Condition::from(||
///     Condition::stderr_is_tty() && Condition::clicolor()
/// );
///
/// static DEBUG: Style = Yellow.bold().on_primary().invert().whenever(STDERR_COLOR);
/// # }
/// ```
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Condition(
    /// The function that gets called to check the condition.
    pub fn() -> bool
);

#[repr(transparent)]
pub struct AtomicCondition(AtomicPtr<()>);

#[allow(unused)]
#[repr(transparent)]
pub struct CachedBool(AtomicU8);

impl Condition {
    /// A condition that evaluates to `true` if the OS supports coloring.
    ///
    /// Uses [`Condition::os_support()`]. On Windows, this condition tries to
    /// enable coloring support on the first call and caches the result for
    /// subsequent calls. Outside of Windows, this always evaluates to `true`.
    pub const DEFAULT: Condition = Condition(Condition::os_support);

    /// A condition that always evaluates to `true`.
    pub const ALWAYS: Condition = Condition(Condition::always);

    /// A condition that always evaluated to `false`.
    pub const NEVER: Condition = Condition(Condition::never);

    /// Creates a dynamically checked condition from a function `f`.
    ///
    /// The function `f` is called anytime the condition is checked, including
    /// every time a style with the condition is used.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use yansi::Condition;
    ///
    /// fn some_function() -> bool {
    ///     /* checking arbitrary conditions */
    ///     todo!()
    /// }
    ///
    /// // Create a custom static condition from a function.
    /// static MY_CONDITION: Condition = Condition::from(some_function);
    ///
    /// // Create a condition on the stack from a function.
    /// let my_condition = Condition::from(some_function);
    ///
    /// // Create a static condition from a closure that becomes a `fn`.
    /// static MY_CONDITION_2: Condition = Condition::from(|| false);
    ///
    /// // Create a condition on the stack from a closure that becomes a `fn`.
    /// let my_condition = Condition::from(|| some_function());
    /// ```
    pub const fn from(f: fn() -> bool) -> Self {
        Condition(f)
    }

    /// Creates a condition that is [`ALWAYS`](Self::ALWAYS) when `value` is
    /// `true` and [`NEVER`](Self::NEVER) otherwise.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use yansi::Condition;
    ///
    /// fn some_function() -> bool {
    ///     /* checking arbitrary conditions */
    ///     todo!()
    /// }
    ///
    /// // Cache the result of `some_function()` so it doesn't get called each
    /// // time the condition needs to be checked.
    /// let my_condition = Condition::cached(some_function());
    /// ```
    pub const fn cached(value: bool) -> Self {
        match value {
            true => Condition::ALWAYS,
            false => Condition::NEVER,
        }
    }

    /// The backing function for [`Condition::ALWAYS`]. Returns `true` always.
    pub const fn always() -> bool { true }

    /// The backing function for [`Condition::NEVER`]. Returns `false` always.
    pub const fn never() -> bool { false }

    /// The backing function for [`Condition::DEFAULT`].
    ///
    /// Returns `true` if the current OS supports ANSI escape sequences for
    /// coloring. Outside of Windows, this always returns `true`. On Windows,
    /// the first call to this function attempts to enable support and returns
    /// whether it was successful every time thereafter.
    pub fn os_support() -> bool {
        crate::windows::cache_enable()
    }
}

impl Default for Condition {
    fn default() -> Self {
        Condition::DEFAULT
    }
}

impl core::ops::Deref for Condition {
    type Target = fn() -> bool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AtomicCondition {
    pub const DEFAULT: AtomicCondition = AtomicCondition::from(Condition::DEFAULT);

    pub const fn from(value: Condition) -> Self {
        AtomicCondition(AtomicPtr::new(value.0 as *mut ()))
    }

    pub fn store(&self, cond: Condition) {
        self.0.store(cond.0 as *mut (), Ordering::Release)
    }

    pub fn read(&self) -> bool {
        let condition = unsafe {
            Condition(core::mem::transmute(self.0.load(Ordering::Acquire)))
        };

        condition()
    }
}

#[allow(unused)]
impl CachedBool {
    const TRUE: u8 = 1;
    const UNINIT: u8 = 2;
    const INITING: u8 = 3;

    pub const fn new() -> Self {
        CachedBool(AtomicU8::new(Self::UNINIT))
    }

    pub fn get_or_init(&self, f: impl FnOnce() -> bool) -> bool {
        use core::sync::atomic::Ordering::*;

        match self.0.compare_exchange(Self::UNINIT, Self::INITING, AcqRel, Relaxed) {
            Ok(_) => {
                let new_value = f();
                self.0.store(new_value as u8 /* false = 0, true = 1 */, Release);
                new_value
            }
            Err(Self::INITING) => {
                let mut value;
                while { value = self.0.load(Acquire); value } == Self::INITING {
                    #[cfg(feature = "std")]
                    std::thread::yield_now();
                }

                value == Self::TRUE
            },
            Err(value) => value == Self::TRUE,
        }
    }
}

impl fmt::Debug for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == Condition::DEFAULT {
            f.write_str("Condition::DEFAULT")
        } else if *self == Condition::ALWAYS {
            f.write_str("Condition::ALWAYS")
        } else if *self == Condition::NEVER {
            f.write_str("Condition::NEVER")
        } else {
            f.debug_tuple("Condition").field(&self.0).finish()
        }
    }
}

macro_rules! conditions {
    ($feat:meta $($f:expr, $CACHED:ident: $cached:ident, $LIVE:ident: $live:ident),* $(,)?) => (
        #[cfg($feat)]
        #[cfg_attr(feature = "_nightly", doc(cfg($feat)))]
        /// Feature dependent conditions.
        ///
        /// Available when compiled with
        #[doc = concat!('`', stringify!($feat), "`.")]
        impl Condition {
            $(
                /// Evaluates to `true` if
                #[doc = concat!('`', stringify!($f), "`.")]
                ///
                /// The result of the first check is cached for subsequent
                /// checks. Internally uses
                #[doc = concat!("[`", stringify!($cached), "`](Condition::", stringify!($cached), ").")]
                pub const $CACHED: Condition = Condition(Condition::$cached);
            )*

            $(
                /// Evaluates to `true` if
                #[doc = concat!('`', stringify!($f), "`.")]
                ///
                /// A call is dispatched each time the condition is checked.
                /// This is expensive, so prefer to use
                #[doc = concat!("[`", stringify!($CACHED), "`](Condition::", stringify!($CACHED), ")")]
                /// instead.
                ///
                /// Internally uses
                #[doc = concat!("[`", stringify!($live), "`](Condition::", stringify!($live), ").")]
                pub const $LIVE: Condition = Condition(Condition::$live);
            )*

            $(
                /// Returns `true` if
                #[doc = concat!('`', stringify!($f), "`.")]
                ///
                /// The result of the first check is cached for subsequent
                /// checks. This is the backing function for
                #[doc = concat!("[`", stringify!($CACHED), "`](Condition::", stringify!($CACHED), ").")]
                pub fn $cached() -> bool {
                    static IS_TTY: CachedBool = CachedBool::new();
                    IS_TTY.get_or_init(Condition::$live)
                }
            )*

            $(
                /// Returns `true` if
                #[doc = concat!('`', stringify!($f), "`.")]
                ///
                /// This is the backing function for
                #[doc = concat!("[`", stringify!($LIVE), "`](Condition::", stringify!($LIVE), ").")]
                pub fn $live() -> bool {
                    $f
                }
            )*
        }
    )
}

#[cfg(feature = "detect-tty")]
use is_terminal::is_terminal as is_tty;

conditions! { feature = "detect-tty"
    is_tty(&std::io::stdout()),
        STDOUT_IS_TTY: stdout_is_tty,
        STDOUT_IS_TTY_LIVE: stdout_is_tty_live,

    is_tty(&std::io::stderr()),
        STDERR_IS_TTY: stderr_is_tty,
        STDERR_IS_TTY_LIVE: stderr_is_tty_live,

    is_tty(&std::io::stdin()),
        STDIN_IS_TTY: stdin_is_tty,
        STDIN_IS_TTY_LIVE: stdin_is_tty_live,

    is_tty(&std::io::stdout()) && is_tty(&std::io::stderr()),
        STDOUTERR_ARE_TTY: stdouterr_are_tty,
        STDOUTERR_ARE_TTY_LIVE: stdouterr_are_tty_live,
}

#[cfg(feature = "detect-env")]
pub fn env_set_or(name: &str, default: bool) -> bool {
    std::env::var_os(name).map_or(default, |v| v != "0")
}

conditions! { feature = "detect-env"
    env_set_or("CLICOLOR_FORCE", false) || env_set_or("CLICOLOR", true),
        CLICOLOR: clicolor,
        CLICOLOR_LIVE: clicolor_live,

    !env_set_or("NO_COLOR", false),
        YES_COLOR: no_color,
        YES_COLOR_LIVE: no_color_live,
}

conditions! { all(feature = "detect-env", feature = "detect-tty")
    Condition::stdouterr_are_tty() && Condition::clicolor() && Condition::no_color(),
        TTY_AND_COLOR: tty_and_color,
        TTY_AND_COLOR_LIVE: tty_and_color_live,
}
