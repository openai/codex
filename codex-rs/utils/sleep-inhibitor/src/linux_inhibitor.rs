use crate::PlatformSleepInhibitor;
use dbus::arg::OwnedFd;
use dbus::blocking::Connection;
use std::time::Duration;
use tracing::warn;

const ASSERTION_REASON: &str = "Codex is running an active turn";
const APP_ID: &str = "codex";
const DBUS_TIMEOUT: Duration = Duration::from_secs(2);
const GNOME_INHIBIT_SUSPEND_SESSION_FLAG: u32 = 4;

#[derive(Debug, Default)]
pub(crate) struct LinuxSleepInhibitor {
    state: InhibitState,
}

#[derive(Debug, Default)]
enum InhibitState {
    #[default]
    Inactive,
    Logind(OwnedFd),
    Cookie {
        api: CookieApi,
        cookie: u32,
    },
}

#[derive(Debug, Clone, Copy)]
enum CookieApi {
    Gnome,
    FreedesktopPower,
    FreedesktopScreensaver,
}

impl CookieApi {
    fn service_name(self) -> &'static str {
        match self {
            Self::Gnome => "org.gnome.SessionManager",
            Self::FreedesktopPower => "org.freedesktop.PowerManagement",
            Self::FreedesktopScreensaver => "org.freedesktop.ScreenSaver",
        }
    }

    fn object_path(self) -> &'static str {
        match self {
            Self::Gnome => "/org/gnome/SessionManager",
            Self::FreedesktopPower => "/org/freedesktop/PowerManagement/Inhibit",
            Self::FreedesktopScreensaver => "/org/freedesktop/ScreenSaver",
        }
    }

    fn interface_name(self) -> &'static str {
        match self {
            Self::Gnome => "org.gnome.SessionManager",
            Self::FreedesktopPower => "org.freedesktop.PowerManagement.Inhibit",
            Self::FreedesktopScreensaver => "org.freedesktop.ScreenSaver",
        }
    }

    fn uninhibit_method(self) -> &'static str {
        match self {
            Self::Gnome => "Uninhibit",
            Self::FreedesktopPower | Self::FreedesktopScreensaver => "UnInhibit",
        }
    }
}

impl LinuxSleepInhibitor {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

impl PlatformSleepInhibitor for LinuxSleepInhibitor {
    fn acquire(&mut self) {
        if !matches!(self.state, InhibitState::Inactive) {
            return;
        }

        match acquire_logind_inhibitor() {
            Ok(state) => {
                self.state = state;
                return;
            }
            Err(error) => {
                warn!(
                    error = %error,
                    "Failed to acquire sleep inhibitor via org.freedesktop.login1"
                );
            }
        }

        for api in [
            CookieApi::Gnome,
            CookieApi::FreedesktopPower,
            CookieApi::FreedesktopScreensaver,
        ] {
            match acquire_cookie_inhibitor(api) {
                Ok(state) => {
                    self.state = state;
                    return;
                }
                Err(error) => {
                    warn!(?api, error = %error, "Failed to acquire sleep inhibitor via D-Bus");
                }
            }
        }

        warn!("No Linux sleep inhibition API is available");
    }

    fn release(&mut self) {
        match std::mem::take(&mut self.state) {
            InhibitState::Inactive => {}
            InhibitState::Logind(fd) => drop(fd),
            InhibitState::Cookie { api, cookie } => {
                if let Err(error) = release_cookie_inhibitor(api, cookie) {
                    warn!(?api, error = %error, "Failed to release D-Bus sleep inhibitor");
                }
            }
        }
    }
}

fn acquire_logind_inhibitor() -> Result<InhibitState, dbus::Error> {
    let connection = Connection::new_system()?;
    let proxy = connection.with_proxy(
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        DBUS_TIMEOUT,
    );
    let (fd,): (OwnedFd,) = proxy.method_call(
        "org.freedesktop.login1.Manager",
        "Inhibit",
        ("sleep", APP_ID, ASSERTION_REASON, "block"),
    )?;
    Ok(InhibitState::Logind(fd))
}

fn acquire_cookie_inhibitor(api: CookieApi) -> Result<InhibitState, dbus::Error> {
    let connection = Connection::new_session()?;
    let proxy = connection.with_proxy(api.service_name(), api.object_path(), DBUS_TIMEOUT);
    let (cookie,): (u32,) = match api {
        CookieApi::Gnome => proxy.method_call(
            api.interface_name(),
            "Inhibit",
            (
                APP_ID,
                0_u32,
                ASSERTION_REASON,
                GNOME_INHIBIT_SUSPEND_SESSION_FLAG,
            ),
        )?,
        CookieApi::FreedesktopPower | CookieApi::FreedesktopScreensaver => {
            proxy.method_call(api.interface_name(), "Inhibit", (APP_ID, ASSERTION_REASON))?
        }
    };
    Ok(InhibitState::Cookie { api, cookie })
}

fn release_cookie_inhibitor(api: CookieApi, cookie: u32) -> Result<(), dbus::Error> {
    let connection = Connection::new_session()?;
    let proxy = connection.with_proxy(api.service_name(), api.object_path(), DBUS_TIMEOUT);
    let _: () = proxy.method_call(api.interface_name(), api.uninhibit_method(), (cookie,))?;
    Ok(())
}
