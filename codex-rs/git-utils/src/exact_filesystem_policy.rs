//! Filesystem-spelling policy for exact staging.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::path::PathBuf;

/// Find requested paths that are neither exact index names nor composed of
/// physically exact directory-entry spellings.
///
/// Git's byte-oriented ignore-case matching does not cover every alias that a
/// case-insensitive filesystem recognizes. Exact index names remain valid for
/// tracked updates and deletions. Other paths must exist under their requested
/// spelling instead of merely resolving through a case or normalization alias;
/// unresolved paths are refused before a mixed update can partially stage.
pub(crate) fn filesystem_spelling_conflicts(
    git_root: &Path,
    paths: &[String],
    exact_index_paths: &BTreeSet<&[u8]>,
) -> io::Result<BTreeSet<String>> {
    let mut cache = BTreeMap::<PathBuf, Option<BTreeSet<OsString>>>::new();
    let mut conflicts = BTreeSet::new();
    for path in paths {
        if exact_index_paths.contains(path.as_bytes()) {
            continue;
        }
        if !has_exact_directory_entry_spelling(git_root, path, &mut cache)? {
            conflicts.insert(path.clone());
        }
    }
    Ok(conflicts)
}

fn has_exact_directory_entry_spelling(
    git_root: &Path,
    path: &str,
    cache: &mut BTreeMap<PathBuf, Option<BTreeSet<OsString>>>,
) -> io::Result<bool> {
    let mut parent = git_root.to_path_buf();
    for component in path.split('/') {
        if !cache.contains_key(&parent) {
            let entries = match std::fs::read_dir(&parent) {
                Ok(entries) => Some(
                    entries
                        .map(|entry| entry.map(|entry| entry.file_name()))
                        .collect::<io::Result<BTreeSet<_>>>()?,
                ),
                Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                Err(error) => return Err(error),
            };
            cache.insert(parent.clone(), entries);
        }
        let Some(Some(entries)) = cache.get(&parent) else {
            return Ok(false);
        };
        if !entries.contains(OsStr::new(component)) {
            return Ok(false);
        }
        parent.push(component);
    }
    Ok(true)
}
