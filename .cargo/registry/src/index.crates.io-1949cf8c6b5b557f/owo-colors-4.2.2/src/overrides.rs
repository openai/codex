use core::sync::atomic::{AtomicU8, Ordering};

/// Set an override value for whether or not colors are supported using
/// [`set_override`] while executing the closure provided.
///
/// Once the function has executed the value will be reset to the previous set
/// (or unset) override.
///
/// This is especially useful in use-cases where one would like to temporarily
/// override the supported color set, without impacting previous configurations.
///
/// ```
/// # use owo_colors::{Stream, OwoColorize, set_override, unset_override, with_override};
/// # use owo_colors::colors::Black;
/// #
/// set_override(false);
/// assert_eq!("example".if_supports_color(Stream::Stdout, |value| value.bg::<Black>()).to_string(), "example");
///
/// with_override(true, || {
///     assert_eq!("example".if_supports_color(Stream::Stdout, |value| value.bg::<Black>()).to_string(), "\x1b[40mexample\x1b[49m");
/// });
///
/// assert_eq!("example".if_supports_color(Stream::Stdout, |value| value.bg::<Black>()).to_string(), "example");
/// # unset_override() // make sure that other doc tests are not impacted
/// ```
#[cfg(feature = "supports-colors")]
pub fn with_override<T, F: FnOnce() -> T>(enabled: bool, f: F) -> T {
    let previous = OVERRIDE.inner();
    OVERRIDE.set_force(enabled);

    // Use a scope guard to ensure that if `f` panics, the override is still
    // caught.
    let _guard = ResetOverrideGuard { previous };

    f()
}

struct ResetOverrideGuard {
    previous: u8,
}

impl Drop for ResetOverrideGuard {
    fn drop(&mut self) {
        OVERRIDE.set_unchecked(self.previous);
    }
}

/// Set an override value for whether or not colors are supported.
///
/// If `true` is passed,
/// [`if_supports_color`](crate::OwoColorize::if_supports_color) will always act
/// as if colors are supported.
///
/// If `false` is passed,
/// [`if_supports_color`](crate::OwoColorize::if_supports_color) will always act
/// as if colors are **not** supported.
///
/// This behavior can be disabled using [`unset_override`], allowing
/// `owo-colors` to return to inferring if colors are supported.
#[cfg(feature = "supports-colors")]
pub fn set_override(enabled: bool) {
    OVERRIDE.set_force(enabled);
}

/// Remove any override value for whether or not colors are supported. This
/// means [`if_supports_color`](crate::OwoColorize::if_supports_color) will
/// resume checking if the given terminal output ([`Stream`](crate::Stream))
/// supports colors.
///
/// This override can be set using [`set_override`].
#[cfg(feature = "supports-colors")]
pub fn unset_override() {
    OVERRIDE.unset();
}

pub(crate) static OVERRIDE: Override = Override::none();

pub(crate) struct Override(AtomicU8);

const FORCE_MASK: u8 = 0b10;
const FORCE_ENABLE: u8 = 0b11;
const FORCE_DISABLE: u8 = 0b10;
const NO_FORCE: u8 = 0b00;

impl Override {
    const fn none() -> Self {
        Self(AtomicU8::new(NO_FORCE))
    }

    fn inner(&self) -> u8 {
        self.0.load(Ordering::SeqCst)
    }

    pub(crate) fn is_force_enabled_or_disabled(&self) -> (bool, bool) {
        let inner = self.inner();

        (inner == FORCE_ENABLE, inner == FORCE_DISABLE)
    }

    fn set_force(&self, enable: bool) {
        self.set_unchecked(FORCE_MASK | (enable as u8));
    }

    fn unset(&self) {
        self.set_unchecked(0);
    }

    fn set_unchecked(&self, value: u8) {
        self.0.store(value, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_override_on_panic() {
        set_override(false);

        std::panic::catch_unwind(|| {
            with_override(true, || {
                assert_eq!(OVERRIDE.inner(), FORCE_ENABLE);
                panic!("test");
            });
        })
        .expect_err("test should panic");

        assert_eq!(
            OVERRIDE.inner(),
            FORCE_DISABLE,
            "override should have been reset"
        );
    }
}
