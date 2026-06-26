#![deny(clippy::print_stdout, clippy::print_stderr)]

mod actions;
mod cdp;
mod network;
mod process;
mod screen;
mod scripts;
mod session;

pub use actions::BrowserToolOutput;
pub use network::BrowserNetworkPolicy;
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
