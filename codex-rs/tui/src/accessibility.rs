use std::sync::OnceLock;

const SCREEN_READER_ENV_VARS: [&str; 5] = [
    "NVDA_RUNNING",
    "JAWS",
    "ORCA_RUNNING",
    "SPEECHD_RUNNING",
    "ACCESSIBILITY_ENABLED",
];

static SCREEN_READER_ACTIVE: OnceLock<bool> = OnceLock::new();

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
    *SCREEN_READER_ACTIVE.get_or_init(detect_screen_reader)
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

            !trimmed.eq_ignore_ascii_case("0") && !trimmed.eq_ignore_ascii_case("false")
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::sync::Mutex;

    static TEST_GUARD: Mutex<()> = Mutex::new(());

    fn set_env(var: &str, value: &str) {
        // Safety: Tests run under a mutex so no other threads observe the intermediate state.
        unsafe {
            std::env::set_var(var, value);
        }
    }

    fn remove_env(var: &str) {
        // Safety: See `set_env`; the mutex ensures serialised environment access.
        unsafe {
            std::env::remove_var(var);
        }
    }

    fn with_clean_env<F, R>(f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let _guard = TEST_GUARD.lock().expect("test mutex poisoned");
        let saved: Vec<(&'static str, Option<String>)> = SCREEN_READER_ENV_VARS
            .iter()
            .map(|&var| (var, std::env::var(var).ok()))
            .collect();

        for &var in &SCREEN_READER_ENV_VARS {
            remove_env(var);
        }

        let result = f();

        for (var, value) in saved {
            if let Some(value) = value {
                set_env(var, &value);
            } else {
                remove_env(var);
            }
        }

        result
    }

    #[test]
    fn returns_false_when_no_environment_variables_are_set() {
        with_clean_env(|| {
            assert_eq!(detect_screen_reader(), false);
        });
    }

    #[test]
    fn detects_nvda_running() {
        with_clean_env(|| {
            set_env("NVDA_RUNNING", "1");
            assert_eq!(detect_screen_reader(), true);
        });
    }

    #[test]
    fn detects_jaws_running() {
        with_clean_env(|| {
            set_env("JAWS", "true");
            assert_eq!(detect_screen_reader(), true);
        });
    }

    #[test]
    fn detects_orca_running() {
        with_clean_env(|| {
            set_env("ORCA_RUNNING", "yes");
            assert_eq!(detect_screen_reader(), true);
        });
    }

    #[test]
    fn detects_speech_dispatcher_running() {
        with_clean_env(|| {
            set_env("SPEECHD_RUNNING", "1");
            assert_eq!(detect_screen_reader(), true);
        });
    }

    #[test]
    fn detects_generic_accessibility_flag() {
        with_clean_env(|| {
            set_env("ACCESSIBILITY_ENABLED", "true");
            assert_eq!(detect_screen_reader(), true);
        });
    }

    #[test]
    fn empty_values_are_ignored() {
        with_clean_env(|| {
            set_env("NVDA_RUNNING", "");
            assert_eq!(detect_screen_reader(), false);
        });
    }

    #[test]
    fn whitespace_values_are_ignored() {
        with_clean_env(|| {
            set_env("JAWS", "   ");
            assert_eq!(detect_screen_reader(), false);
        });
    }

    #[test]
    fn multiple_variables_set_returns_true() {
        with_clean_env(|| {
            set_env("NVDA_RUNNING", "1");
            set_env("ORCA_RUNNING", "1");
            assert_eq!(detect_screen_reader(), true);
        });
    }

    #[test]
    fn false_like_values_do_not_trigger_detection() {
        with_clean_env(|| {
            set_env("ORCA_RUNNING", "false");
            set_env("ACCESSIBILITY_ENABLED", "0");
            assert_eq!(detect_screen_reader(), false);
        });
    }

    #[test]
    fn cached_value_is_reused() {
        with_clean_env(|| {
            set_env("SPEECHD_RUNNING", "1");
            assert_eq!(is_screen_reader_active(), true);

            remove_env("SPEECHD_RUNNING");
            assert_eq!(is_screen_reader_active(), true);
        });
    }
}
