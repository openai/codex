use crate::condition::{AtomicCondition, Condition};

static ENABLED: AtomicCondition = AtomicCondition::DEFAULT;

/// Unconditionally disables styling globally.
///
/// # Example
///
/// ```rust
/// use yansi::Paint;
///
/// // With styling enabled, ANSI color codes are emitted, thus `ne`.
/// assert_ne!("go".green().to_string(), "go".to_string());
///
/// // With styling disabled, ANSI color codes are _not_ emitted.
/// yansi::disable();
/// assert_eq!("go".green().to_string(), "go".to_string());
/// ```
pub fn disable() {
    ENABLED.store(Condition::NEVER);
}

/// Unconditionally enables styling globally.
///
/// By default, styling is enabled based on [`Condition::DEFAULT`], which checks
/// for operating system support.
///
/// # Example
///
/// ```rust
/// use yansi::Paint;
///
/// // With styling disabled, ANSI color codes are _not_ emitted.
/// yansi::disable();
/// assert_eq!("go".green().to_string(), "go".to_string());
///
/// // Reenabling causes color code to be emitted.
/// yansi::enable();
/// assert_ne!("go".green().to_string(), "go".to_string());
/// ```
pub fn enable() {
    ENABLED.store(Condition::ALWAYS);
}

/// Dynamically enables and disables styling globally based on `condition`.
///
/// `condition` is expected to be fast: it is checked dynamically, each time a
/// [`Painted`](crate::Painted) value is displayed.
///
/// # Example
///
/// ```rust
/// # #[cfg(all(feature = "detect-tty", feature = "detect-env"))] {
/// use yansi::Condition;
///
/// yansi::whenever(Condition::STDOUT_IS_TTY);
///
/// // On each styling, check if we have TTYs.
/// yansi::whenever(Condition::STDOUTERR_ARE_TTY_LIVE);
///
/// // Check `NO_COLOR`, `CLICOLOR`, and if we have TTYs.
/// const HAVE_COLOR: Condition = Condition(|| {
///     std::env::var_os("NO_COLOR").is_none()
///         && (Condition::CLICOLOR_LIVE)()
///         && Condition::stdouterr_are_tty_live()
/// });
///
/// // This will call `HAVE_COLOR` every time styling is needed. In this
/// // example, this means that env vars will be checked on each styling.
/// yansi::whenever(HAVE_COLOR);
///
/// // This instead caches the value (checking `env()` exactly once, now).
/// yansi::whenever(Condition::cached((HAVE_COLOR)()));
///
/// // Is identical to this:
/// match (HAVE_COLOR)() {
///     true => yansi::enable(),
///     false => yansi::disable(),
/// }
/// # }
/// ```
pub fn whenever(condition: Condition) {
    ENABLED.store(condition);
}

/// Returns `true` if styling is globally enabled and `false` otherwise.
///
/// By default, styling is enabled based on [`Condition::DEFAULT`], which checks
/// for operating system support. It can be enabled and disabled on-the-fly with
/// [`enable()`] and [`disable()`] and via a dynamic condition with
/// [`whenever()`].
///
/// # Example
///
/// ```rust
/// // Styling is enabled by default.
/// # yansi::enable();
/// assert!(yansi::is_enabled());
///
/// // Disable it with `Painted::disable()`.
/// yansi::disable();
/// assert!(!yansi::is_enabled());
///
/// // Reenable with `Painted::enable()`.
/// yansi::enable();
/// assert!(yansi::is_enabled());
/// ```
pub fn is_enabled() -> bool {
    ENABLED.read()
}
