#[cfg(test)]
use crate::plugins::PluginManifestInterface;
pub use codex_core_plugins::marketplace::*;
#[cfg(test)]
use codex_plugin::PluginId;
#[cfg(test)]
use codex_utils_absolute_path::AbsolutePathBuf;
#[cfg(test)]
use std::fs;

#[cfg(test)]
#[path = "marketplace_tests.rs"]
mod tests;
