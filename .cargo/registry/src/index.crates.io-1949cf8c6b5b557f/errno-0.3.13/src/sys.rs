//! A default sys.rs for unrecognized targets.
//!
//! If lib.rs doesn't recognize the target, it defaults to using this file,
//! which issues an explanatory compile error.

// If there is no OS, there's no `errno` or equivalent defined.
#[cfg(any(target_os = "unknown", target_os = "none"))]
compile_error!("The target OS is \"unknown\" or \"none\", so it's unsupported by the errno crate.");

// If there is an OS, support may be added.
#[cfg(not(any(target_os = "unknown", target_os = "none")))]
compile_error!("The target OS is not yet supported in the errno crate.");

// The following define the functions of the normal implementations
// so that the user doesn't see uninteresting errors after the
// errors above.

use crate::Errno;

pub fn with_description<F, T>(_err: Errno, _callback: F) -> T
where
    F: FnOnce(Result<&str, Errno>) -> T,
{
    unreachable!()
}

pub const STRERROR_NAME: &str = "";

pub fn errno() -> Errno {
    unreachable!()
}

pub fn set_errno(_: Errno) {
    unreachable!()
}
