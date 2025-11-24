//! These modules are all glue to support reading the MSVC version from
//! the registry and from COM interfaces.

// This is used in the crate's public API, so don't use #[cfg(windows)]
pub mod find_tools;

#[cfg(windows)]
mod windows_link;
#[cfg(windows)]
pub(crate) mod windows_sys;

#[cfg(windows)]
mod registry;
#[cfg(windows)]
#[macro_use]
mod winapi;
#[cfg(windows)]
mod com;
#[cfg(windows)]
mod setup_config;
#[cfg(windows)]
mod vs_instances;
