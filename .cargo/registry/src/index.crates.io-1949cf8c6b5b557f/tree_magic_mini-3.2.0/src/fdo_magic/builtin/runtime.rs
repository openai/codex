//! Enable loading the magic database files at runtime rather than embedding the GPLed database

use std::collections::HashMap;
use std::env::{split_paths, var_os};
use std::ffi::OsString;
use std::fs::{read, read_to_string};
use std::path::{Path, PathBuf};

use once_cell::sync::OnceCell;
use petgraph::prelude::DiGraph;

use super::MagicRule;
use crate::fdo_magic::ruleset;
use crate::Mime;

fn mime_path(base: &Path, filename: &str) -> PathBuf {
    base.join("mime").join(filename)
}

fn search_paths(filename: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // If the TREE_MAGIC_DIR environment variable is set, use it directly
    // and return so we just use specified directory.
    if let Some(load_path) = var_os("TREE_MAGIC_DIR") {
        return vec![PathBuf::from(load_path).join(filename)];
    }

    let data_dirs = match var_os("XDG_DATA_DIRS") {
        Some(dirs) if !dirs.is_empty() => dirs,
        _ => OsString::from("/usr/local/share/:/usr/share/"),
    };
    paths.extend(split_paths(&data_dirs).map(|base| mime_path(&base, filename)));

    let data_home = match var_os("XDG_DATA_HOME") {
        Some(data_home) if !data_home.is_empty() => Some(PathBuf::from(data_home)),
        _ => var_os("HOME").map(|home| Path::new(&home).join(".local/share")),
    };
    if let Some(data_home) = data_home {
        paths.push(mime_path(&data_home, filename));
    }

    #[cfg(target_os = "macos")]
    paths.push(mime_path(Path::new("/opt/homebrew/share"), filename));

    paths
}

/// Load the magic database from the predefined locations in the XDG standard
fn load_xdg_shared_magic() -> Vec<Vec<u8>> {
    search_paths("magic")
        .iter()
        .map(read)
        .filter_map(Result::ok)
        .collect()
}

/// Load a number of files at `paths` and concatenate them together with a newline
fn load_concat_strings(filename: &str) -> String {
    search_paths(filename)
        .iter()
        .map(read_to_string)
        .filter_map(Result::ok)
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn aliases() -> &'static str {
    static ALIAS_STRING: OnceCell<String> = OnceCell::new();
    ALIAS_STRING.get_or_init(|| load_concat_strings("aliases"))
}

pub fn subclasses() -> &'static str {
    static SUBCLASS_STRING: OnceCell<String> = OnceCell::new();
    SUBCLASS_STRING.get_or_init(|| load_concat_strings("subclasses"))
}

pub fn rules() -> Result<HashMap<Mime, DiGraph<MagicRule<'static>, u32>>, String> {
    static RUNTIME_RULES: OnceCell<Vec<Vec<u8>>> = OnceCell::new();
    let files = RUNTIME_RULES.get_or_init(load_xdg_shared_magic);
    ruleset::from_multiple(files)
}
