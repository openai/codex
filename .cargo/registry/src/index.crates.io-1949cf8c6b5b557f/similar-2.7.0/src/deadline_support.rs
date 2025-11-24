use std::time::Duration;

#[cfg(not(feature = "wasm32_web_time"))]
pub use std::time::Instant;

/// WASM (browser) specific instant type.
///
/// This type is only available when the `wasm32_web_time` feature is enabled.  In that
/// case this is an alias for [`web_time::Instant`].
#[cfg(feature = "wasm32_web_time")]
pub use web_time::Instant;

/// Checks if a deadline was exeeded.
pub fn deadline_exceeded(deadline: Option<Instant>) -> bool {
    #[allow(unreachable_code)]
    match deadline {
        Some(deadline) => {
            #[cfg(all(target_arch = "wasm32", not(feature = "wasm32_web_time")))]
            {
                return false;
            }
            Instant::now() > deadline
        }
        None => false,
    }
}

/// Converst a duration into a deadline.  This can be a noop on wasm
#[allow(unused)]
pub fn duration_to_deadline(add: Duration) -> Option<Instant> {
    #[allow(unreachable_code)]
    #[cfg(all(target_arch = "wasm32", not(feature = "wasm32_web_time")))]
    {
        return None;
    }
    Instant::now().checked_add(add)
}
