//! TUI adapters for the Carbonyl-backed terminal browser.
//!
//! Application code owns browser lifecycle, visibility, and focus. This module only adapts the
//! shared browser runtime to a frame-assigned panel rectangle, crossterm input, profile approval
//! UI, and app-server dynamic tools.

mod chrome;
mod input;
mod network;
mod panel;
mod profile_approval;
mod tools;

pub(crate) use chrome::BrowserChromeKeyResult;
pub(crate) use chrome::BrowserChromeMouseResult;
pub(crate) use chrome::BrowserChromeState;
pub(crate) use chrome::render_browser_chrome;
pub(crate) use input::BrowserMouseRoute;
pub(crate) use input::browser_key_input;
pub(crate) use input::browser_mouse_input;
pub(crate) use input::browser_mouse_route;
pub(crate) use network::TerminalBrowserNetworkAvailability;
#[cfg(test)]
pub(crate) use panel::BrowserPanelAreas;
pub(crate) use panel::TerminalBrowserPanel;
pub(crate) use panel::browser_panel_areas;
pub(crate) use panel::browser_viewport;
pub(crate) use panel::render_browser_view;
pub(crate) use profile_approval::profile_approval_view_params;
pub(crate) use profile_approval::requested_profile_command;
pub(crate) use tools::TERMINAL_BROWSER_NAMESPACE;
pub(crate) use tools::dynamic_tool_response;
pub(crate) use tools::dynamic_tool_specs;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
