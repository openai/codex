//! Read the lock state of the calling process's macOS login session.

use chrono::Utc;
use core_foundation::base::CFType;
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::dictionary::CFDictionaryRef;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGSessionCopyCurrentDictionary() -> CFDictionaryRef;
}

pub(super) fn is_locked() -> anyhow::Result<bool> {
    // SAFETY: The function takes no arguments and returns an owned dictionary or null.
    let session = unsafe { CGSessionCopyCurrentDictionary() };
    anyhow::ensure!(!session.is_null(), "macOS login session unavailable");
    // SAFETY: CGSession follows the CF create rule. The dictionary owns its CF strings and values.
    let session: CFDictionary<CFString, CFType> =
        unsafe { TCFType::wrap_under_create_rule(session) };
    Ok(requires_authentication(&session, Utc::now().timestamp()))
}

fn requires_authentication(session: &CFDictionary<CFString, CFType>, now: i64) -> bool {
    let lock_screen_active = session
        .find(CFString::new("CGSSessionScreenIsLocked"))
        .and_then(|locked| locked.downcast::<CFBoolean>())
        .is_some_and(bool::from);
    // loginwindow sets the flag as soon as the saver starts. XNU enforces the password
    // after this per-session Unix timestamp, including when the grace period expires.
    let password_required_at = session
        .find(CFString::new("CGSSessionScreenLockedTime"))
        .and_then(|time| time.downcast::<CFNumber>())
        .and_then(|time| time.to_i64());
    lock_screen_active && password_required_at.is_some_and(|time| (1..=now).contains(&time))
}

#[cfg(test)]
#[path = "macos_tests.rs"]
mod tests;
