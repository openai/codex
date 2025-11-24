//! Environment variables
use cfg_if::cfg_if;
use std::fmt;

/// Indicates that [`clearenv`] failed for some unknown reason
#[derive(Clone, Copy, Debug)]
pub struct ClearEnvError;

impl fmt::Display for ClearEnvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "clearenv failed")
    }
}

impl std::error::Error for ClearEnvError {}

/// Clear the environment of all name-value pairs.
///
/// On platforms where libc provides `clearenv()`, it will be used. libc's
/// `clearenv()` is documented to return an error code but not set errno; if the
/// return value indicates a failure, this function will return
/// [`ClearEnvError`].
///
/// On platforms where libc does not provide `clearenv()`, a fallback
/// implementation will be used that iterates over all environment variables and
/// removes them one-by-one.
///
/// # Safety
///
/// This function is not threadsafe and can cause undefined behavior in
/// combination with `std::env` or other program components that access the
/// environment. See, for example, the discussion on `std::env::remove_var`; this
/// function is a case of an "inherently unsafe non-threadsafe API" dealing with
/// the environment.
///
///  The caller must ensure no other threads access the process environment while
///  this function executes and that no raw pointers to an element of libc's
///  `environ` is currently held. The latter is not an issue if the only other
///  environment access in the program is via `std::env`, but the requirement on
///  thread safety must still be upheld.
pub unsafe fn clearenv() -> std::result::Result<(), ClearEnvError> {
    cfg_if! {
        if #[cfg(any(linux_android,
                     target_os = "fuchsia",
                     target_os = "wasi",
                     target_env = "uclibc",
                     target_os = "emscripten"))] {
            let ret = unsafe { libc::clearenv() };
        } else {
            use std::env;
            for (name, _) in env::vars_os() {
                env::remove_var(name);
            }
            let ret = 0;
        }
    }

    if ret == 0 {
        Ok(())
    } else {
        Err(ClearEnvError)
    }
}
