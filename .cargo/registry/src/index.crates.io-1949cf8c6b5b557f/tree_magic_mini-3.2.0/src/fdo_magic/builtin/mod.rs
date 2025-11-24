//! Read magic file bundled in crate

use super::MagicRule;
use crate::Mime;
use once_cell::sync::Lazy;
use petgraph::prelude::*;
use std::collections::HashMap;

pub mod check;
pub mod init;

#[cfg(not(feature = "with-gpl-data"))]
mod runtime;

/// Preload alias list
static ALIASES: Lazy<HashMap<Mime, Mime>> = Lazy::new(init::get_aliaslist);

/// Load magic file before anything else.
static ALL_RULES: Lazy<HashMap<Mime, DiGraph<MagicRule<'static>, u32>>> = Lazy::new(|| {
    #[cfg(feature = "with-gpl-data")]
    return super::ruleset::from_u8(tree_magic_db::magic()).unwrap_or_default();
    #[cfg(not(feature = "with-gpl-data"))]
    return runtime::rules().unwrap_or_default();
});
