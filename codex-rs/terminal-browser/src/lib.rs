#![deny(clippy::print_stdout, clippy::print_stderr)]

mod accessibility;
mod actions;
mod cdp;
mod devtools;
mod diagnostics;
mod error;
mod handles;
mod human_control;
mod human_navigation;
mod input;
mod navigation;
mod network;
mod process;
mod profile;
mod profile_control;
mod runtime;
mod sandbox;
mod screen;
mod scripts;
mod session;
mod terminal_input;
mod tool_dispatch;
mod url_policy;

pub use actions::BrowserToolOutput;
pub use diagnostics::BrowserDoctorReport;
pub use error::BrowserToolFailure;
pub use error::classify_browser_error;
pub use human_control::HumanControlToken;
pub use human_navigation::HumanNavigationAction;
pub use input::BrowserInputModifiers;
pub use input::BrowserKeyInput;
pub use input::BrowserMouseButton;
pub use input::BrowserMouseInput;
pub use input::BrowserMouseKind;
pub use network::BrowserNetworkPolicy;
pub use sandbox::BrowserLaunchContext;
pub use screen::BrowserCell;
pub use screen::BrowserColor;
pub use screen::BrowserScreen;
pub use screen::BrowserStatus;
pub use screen::BrowserView;
pub use screen::TerminalSize;
pub use session::TerminalBrowser;

#[cfg(test)]
#[path = "terminal_browser_tests.rs"]
mod tests;
