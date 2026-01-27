mod bel;
mod osc9;

use std::io;

use bel::BelBackend;
use codex_core::config::types::NotificationMethod;
use osc9::Osc9Backend;

#[derive(Debug)]
pub enum DesktopNotificationBackend {
    Osc9(Osc9Backend),
    Bel(BelBackend),
}

impl DesktopNotificationBackend {
    pub fn for_method(method: NotificationMethod) -> Self {
        match method {
            NotificationMethod::Osc9 => Self::Osc9(Osc9Backend),
            NotificationMethod::Bel => Self::Bel(BelBackend),
        }
    }

    pub fn method(&self) -> NotificationMethod {
        match self {
            DesktopNotificationBackend::Osc9(_) => NotificationMethod::Osc9,
            DesktopNotificationBackend::Bel(_) => NotificationMethod::Bel,
        }
    }

    pub fn notify(&mut self, message: &str) -> io::Result<()> {
        match self {
            DesktopNotificationBackend::Osc9(backend) => backend.notify(message),
            DesktopNotificationBackend::Bel(backend) => backend.notify(message),
        }
    }
}

pub fn detect_backend(method: NotificationMethod) -> DesktopNotificationBackend {
    DesktopNotificationBackend::for_method(method)
}

#[cfg(test)]
mod tests {
    use super::detect_backend;
    use codex_core::config::types::NotificationMethod;

    #[test]
    fn selects_osc9_method() {
        assert!(matches!(
            detect_backend(NotificationMethod::Osc9),
            super::DesktopNotificationBackend::Osc9(_)
        ));
    }

    #[test]
    fn selects_bel_method() {
        assert!(matches!(
            detect_backend(NotificationMethod::Bel),
            super::DesktopNotificationBackend::Bel(_)
        ));
    }
}
