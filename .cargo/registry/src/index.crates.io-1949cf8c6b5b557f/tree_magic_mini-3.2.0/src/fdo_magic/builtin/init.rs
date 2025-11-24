use crate::Mime;
use std::collections::HashMap;

#[cfg(not(feature = "with-gpl-data"))]
use super::runtime;

fn aliases() -> &'static str {
    #[cfg(feature = "with-gpl-data")]
    return tree_magic_db::aliases();
    #[cfg(not(feature = "with-gpl-data"))]
    return runtime::aliases();
}

fn subclasses() -> &'static str {
    #[cfg(feature = "with-gpl-data")]
    return tree_magic_db::subclasses();
    #[cfg(not(feature = "with-gpl-data"))]
    return runtime::subclasses();
}

pub fn get_aliaslist() -> HashMap<Mime, Mime> {
    aliases()
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let mut parts = line.split_whitespace();
            let a = parts.next().unwrap();
            let b = parts.next().unwrap();
            (a, b)
        })
        .collect()
}

/// Get list of supported MIME types
pub fn get_supported() -> Vec<Mime> {
    super::ALL_RULES.keys().cloned().collect()
}

/// Get list of parent -> child subclass links
pub fn get_subclasses() -> Vec<(Mime, Mime)> {
    subclasses()
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let mut parts = line.split_whitespace();

            let child = parts.next().unwrap();
            let child = super::ALIASES.get(child).copied().unwrap_or(child);

            let parent = parts.next().unwrap();
            let parent = super::ALIASES.get(parent).copied().unwrap_or(parent);

            (parent, child)
        })
        .collect()
}
