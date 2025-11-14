use std::fmt;
use std::io::stdout;

use ratatui::crossterm::Command;
use ratatui::crossterm::execute;

pub(crate) fn post_notification(message: &str) -> bool {
    #[cfg(target_os = "macos")]
    if post_macos_notification(message) {
        return true;
    }

    post_ansi_notification(message)
}

fn post_ansi_notification(message: &str) -> bool {
    let _ = execute!(stdout(), PostNotification(message.to_string()));
    true
}

#[cfg(target_os = "macos")]
fn post_macos_notification(message: &str) -> bool {
    macos::post_notification(message)
}

#[cfg(not(target_os = "macos"))]
fn post_macos_notification(_: &str) -> bool {
    false
}

/// Command that emits an OSC 9 desktop notification with a message.
#[derive(Debug, Clone)]
struct PostNotification(pub String);

impl Command for PostNotification {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b]9;{}\x07", self.0)
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        Err(std::io::Error::other(
            "tried to execute PostNotification using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

#[cfg(all(target_os = "macos", test))]
mod tests {
    use super::post_notification;

    #[test]
    #[ignore = "triggers a real macOS notification; run manually when testing"]
    fn smoke_test_macos_notification() {
        assert!(
            post_notification("Codex macOS notification smoke test"),
            "expected post_notification to report success"
        );
    }
}

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
mod macos {
    use objc::class;
    use objc::msg_send;
    use objc::rc::autoreleasepool;
    use objc::runtime::Object;
    use objc::sel;
    use objc::sel_impl;

    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {}

    #[link(name = "Foundation", kind = "framework")]
    unsafe extern "C" {}

    pub(super) fn post_notification(message: &str) -> bool {
        autoreleasepool(|| deliver_notification(message))
    }

    fn deliver_notification(message: &str) -> bool {
        unsafe {
            let notification = match create_notification() {
                Some(notification) => notification,
                None => return false,
            };

            if !set_text(notification, "Codex", message) {
                return false;
            }

            let center_class = class!(NSUserNotificationCenter);
            let center: *mut Object = msg_send![center_class, defaultUserNotificationCenter];
            if center.is_null() {
                return false;
            }

            let _: () = msg_send![center, deliverNotification: notification];
            true
        }
    }

    unsafe fn create_notification() -> Option<*mut Object> {
        let class = class!(NSUserNotification);
        let notification: *mut Object = msg_send![class, alloc];
        if notification.is_null() {
            return None;
        }

        let notification: *mut Object = msg_send![notification, init];
        if notification.is_null() {
            return None;
        }

        Some(msg_send![notification, autorelease])
    }

    unsafe fn set_text(notification: *mut Object, title: &str, body: &str) -> bool {
        let Some(title) = nsstring(title) else {
            return false;
        };
        let Some(body) = nsstring(body) else {
            return false;
        };

        let _: () = msg_send![notification, setTitle: title];
        let _: () = msg_send![notification, setInformativeText: body];
        true
    }

    fn nsstring(value: &str) -> Option<*mut Object> {
        let utf16: Vec<u16> = value.encode_utf16().collect();
        unsafe {
            let string: *mut Object = msg_send![class!(NSString), alloc];
            if string.is_null() {
                return None;
            }

            let string: *mut Object =
                msg_send![string, initWithCharacters:utf16.as_ptr() length:utf16.len()];
            if string.is_null() {
                return None;
            }

            Some(msg_send![string, autorelease])
        }
    }
}
