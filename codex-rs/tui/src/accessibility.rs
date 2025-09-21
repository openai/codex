use std::sync::OnceLock;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

const SCREEN_READER_ENV_VARS: [&str; 5] = [
    "NVDA_RUNNING",
    "JAWS",
    "ORCA_RUNNING",
    "SPEECHD_RUNNING",
    "ACCESSIBILITY_ENABLED",
];

static SCREEN_READER_ACTIVE: OnceLock<RwLock<Option<bool>>> = OnceLock::new();
static CLI_ANIMATIONS_DISABLED: AtomicBool = AtomicBool::new(false);

fn cache() -> &'static RwLock<Option<bool>> {
    SCREEN_READER_ACTIVE.get_or_init(|| RwLock::new(None))
}

/// Determine whether a screen reader is likely active for the current process.
///
/// This helper inspects a handful of well-known environment variables set by
/// popular screen readers and accessibility services across the major desktop
/// platforms. The check is cached for the lifetime of the application because
/// screen reader presence is not expected to change once the TUI has started.
///
/// The following environment variables are considered:
///
/// * `NVDA_RUNNING` – NVDA (Windows)
/// * `JAWS` – JAWS (Windows)
/// * `ORCA_RUNNING` – Orca (Linux)
/// * `SPEECHD_RUNNING` – Speech Dispatcher (Linux)
/// * `ACCESSIBILITY_ENABLED` – generic accessibility flag used by some tools
///
/// # Examples
///
/// ```no_run
/// use codex_tui::is_screen_reader_active;
///
/// if is_screen_reader_active() {
///     // Disable non-essential animations to avoid overwhelming the screen reader
/// }
/// ```
pub fn is_screen_reader_active() -> bool {
    if let Some(value) = *cache().read().expect("screen reader cache poisoned") {
        return value;
    }

    let mut guard = cache().write().expect("screen reader cache poisoned");
    if let Some(value) = *guard {
        value
    } else {
        let value = detect_screen_reader();
        *guard = Some(value);
        value
    }
}

pub(crate) fn set_cli_animations_disabled(value: bool) {
    CLI_ANIMATIONS_DISABLED.store(value, Ordering::Relaxed);
}

pub(crate) fn animations_disabled_by_cli() -> bool {
    CLI_ANIMATIONS_DISABLED.load(Ordering::Relaxed)
}

pub(crate) fn animations_enabled() -> bool {
    !(is_screen_reader_active() || animations_disabled_by_cli())
}

fn detect_screen_reader() -> bool {
    SCREEN_READER_ENV_VARS
        .iter()
        .any(|&name| env_var_indicates_screen_reader(name))
}

fn env_var_indicates_screen_reader(name: &str) -> bool {
    match std::env::var(name) {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return false;
            }

            let normalized = trimmed.to_ascii_lowercase();
            !matches!(
                normalized.as_str(),
                "0" | "false" | "no" | "off" | "disabled"
            )
        }
        Err(_) => false,
    }
}

#[cfg(test)]
pub(crate) fn reset_cache_for_tests() {
    if let Some(cache) = SCREEN_READER_ACTIVE.get() {
        *cache
            .write()
            .expect("screen reader cache poisoned during reset") = None;
    }
}

#[cfg(test)]
pub(crate) fn with_cli_animations_disabled_for_tests<F, R>(value: bool, f: F) -> R
where
    F: FnOnce() -> R,
{
    let previous = animations_disabled_by_cli();
    set_cli_animations_disabled(value);
    let result = f();
    set_cli_animations_disabled(previous);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serial_test::serial;

    fn set_env(var: &str, value: &str) {
        // Safety: Tests are serialised with #[serial], so no other threads mutate the environment.
        unsafe {
            std::env::set_var(var, value);
        }
    }

    fn remove_env(var: &str) {
        // Safety: See `set_env`; #[serial] ensures exclusive access during the test.
        unsafe {
            std::env::remove_var(var);
        }
    }

    fn with_clean_env<F, R>(f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let saved: Vec<(&'static str, Option<String>)> = SCREEN_READER_ENV_VARS
            .iter()
            .map(|&var| (var, std::env::var(var).ok()))
            .collect();

        for &var in &SCREEN_READER_ENV_VARS {
            remove_env(var);
        }

        reset_cache_for_tests();

        let result = f();

        for (var, value) in saved {
            if let Some(value) = value {
                set_env(var, &value);
            } else {
                remove_env(var);
            }
        }

        reset_cache_for_tests();

        result
    }

    #[test]
    #[serial]
    fn returns_false_when_no_environment_variables_are_set() {
        with_clean_env(|| {
            assert_eq!(detect_screen_reader(), false);
        });
    }

    #[test]
    #[serial]
    fn detects_nvda_running() {
        with_clean_env(|| {
            set_env("NVDA_RUNNING", "1");
            assert_eq!(detect_screen_reader(), true);
        });
    }

    #[test]
    #[serial]
    fn detects_jaws_running() {
        with_clean_env(|| {
            set_env("JAWS", "true");
            assert_eq!(detect_screen_reader(), true);
        });
    }

    #[test]
    #[serial]
    fn detects_orca_running() {
        with_clean_env(|| {
            set_env("ORCA_RUNNING", "yes");
            assert_eq!(detect_screen_reader(), true);
        });
    }

    #[test]
    #[serial]
    fn detects_speech_dispatcher_running() {
        with_clean_env(|| {
            set_env("SPEECHD_RUNNING", "1");
            assert_eq!(detect_screen_reader(), true);
        });
    }

    #[test]
    #[serial]
    fn detects_generic_accessibility_flag() {
        with_clean_env(|| {
            set_env("ACCESSIBILITY_ENABLED", "true");
            assert_eq!(detect_screen_reader(), true);
        });
    }

    #[test]
    #[serial]
    fn empty_values_are_ignored() {
        with_clean_env(|| {
            set_env("NVDA_RUNNING", "");
            assert_eq!(detect_screen_reader(), false);
        });
    }

    #[test]
    #[serial]
    fn whitespace_values_are_ignored() {
        with_clean_env(|| {
            set_env("JAWS", "   ");
            assert_eq!(detect_screen_reader(), false);
        });
    }

    #[test]
    #[serial]
    fn multiple_variables_set_returns_true() {
        with_clean_env(|| {
            set_env("NVDA_RUNNING", "1");
            set_env("ORCA_RUNNING", "1");
            assert_eq!(detect_screen_reader(), true);
        });
    }

    #[test]
    #[serial]
    fn false_like_values_do_not_trigger_detection() {
        with_clean_env(|| {
            set_env("ORCA_RUNNING", "false");
            set_env("ACCESSIBILITY_ENABLED", "0");
            assert_eq!(detect_screen_reader(), false);
        });
    }

    #[test]
    #[serial]
    fn cached_value_is_reused() {
        with_clean_env(|| {
            set_env("SPEECHD_RUNNING", "1");
            assert_eq!(is_screen_reader_active(), true);

            remove_env("SPEECHD_RUNNING");
            assert_eq!(is_screen_reader_active(), true);
        });
    }

    #[test]
    #[serial]
    fn extended_false_like_values_do_not_trigger_detection() {
        with_clean_env(|| {
            for value in ["NO", "Off", "disabled"] {
                set_env("ACCESSIBILITY_ENABLED", value);
                assert_eq!(detect_screen_reader(), false, "value {value:?}");
            }
        });
    }
}
