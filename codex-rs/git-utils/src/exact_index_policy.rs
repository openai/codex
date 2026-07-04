//! Index-name and flag policy for exact staging.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io;
use std::path::Path;

use crate::exact_filesystem_policy::filesystem_spelling_conflicts;
use crate::guarded_config::GuardedGitConfig;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ExactIndexPolicy {
    Proceed {
        paths: Vec<String>,
        content_filter_paths: Vec<String>,
    },
    Flagged {
        paths: Vec<String>,
        content_filter_paths: Vec<String>,
        skip_worktree: BTreeSet<String>,
        assume_unchanged: BTreeSet<String>,
    },
    Refuse {
        stderr: String,
    },
}

#[derive(Clone, Copy, Debug)]
struct IndexEntry<'a> {
    tag: u8,
    path: &'a [u8],
}

#[derive(Debug)]
struct CaseConflict {
    reason: &'static str,
    paths: BTreeSet<String>,
}

impl CaseConflict {
    fn new(reason: &'static str, paths: impl IntoIterator<Item = String>) -> Self {
        Self {
            reason,
            paths: paths.into_iter().collect(),
        }
    }

    fn add_index_paths<'a>(&mut self, paths: impl IntoIterator<Item = &'a [u8]>) {
        self.paths.extend(
            paths
                .into_iter()
                .map(|path| String::from_utf8_lossy(path).into_owned()),
        );
    }

    fn message(&self) -> String {
        format!(
            "refusing {} under core.ignoreCase: {}",
            self.reason,
            quote_paths(&self.paths)
        )
    }
}

struct IndexEvidence<'a> {
    full_paths: BTreeMap<Vec<u8>, BTreeSet<&'a [u8]>>,
    directory_prefixes: BTreeMap<Vec<u8>, BTreeSet<&'a [u8]>>,
}

impl<'a> IndexEvidence<'a> {
    fn collect(entries: &[IndexEntry<'a>], requested: &[String]) -> Self {
        let relevant = requested
            .iter()
            .flat_map(|path| path_prefixes(path.as_bytes()))
            .map(ascii_fold)
            .collect::<BTreeSet<_>>();
        let mut full_paths = BTreeMap::<Vec<u8>, BTreeSet<&[u8]>>::new();
        let mut directory_prefixes = BTreeMap::<Vec<u8>, BTreeSet<&[u8]>>::new();
        for entry in entries {
            let folded = ascii_fold(entry.path);
            if relevant.contains(&folded) {
                full_paths.entry(folded).or_default().insert(entry.path);
            }
            for prefix in non_leaf_prefixes(entry.path) {
                let folded = ascii_fold(prefix);
                if relevant.contains(&folded) {
                    directory_prefixes.entry(folded).or_default().insert(prefix);
                }
            }
        }
        Self {
            full_paths,
            directory_prefixes,
        }
    }
}

pub(crate) fn resolve_exact_index_policy(
    config: &GuardedGitConfig<'_>,
    paths: &[String],
    content_filter_paths: &[String],
) -> io::Result<ExactIndexPolicy> {
    let output = read_index(config)?;
    let entries = parse_index_entries(&output)?;
    let ignore_case = config.read_bool("core.ignoreCase")?.unwrap_or(false);
    let git_root = config.canonical_root();

    let (paths, content_filter_paths) = if ignore_case {
        let mapped = match map_ignore_case_paths(git_root, &entries, paths, content_filter_paths) {
            Ok(mapped) => mapped,
            Err(conflict) => {
                return Ok(ExactIndexPolicy::Refuse {
                    stderr: conflict.message(),
                });
            }
        };
        let exact_index_paths = entries
            .iter()
            .map(|entry| entry.path)
            .collect::<BTreeSet<_>>();
        let conflicts = filesystem_spelling_conflicts(git_root, &mapped.0, &exact_index_paths)?;
        if !conflicts.is_empty() {
            return Ok(ExactIndexPolicy::Refuse {
                stderr: CaseConflict::new(
                    "filesystem spelling aliases or unresolved paths",
                    conflicts,
                )
                .message(),
            });
        }
        mapped
    } else {
        (paths.to_vec(), content_filter_paths.to_vec())
    };

    let exact = paths.iter().map(String::as_bytes).collect::<BTreeSet<_>>();
    let mut skipped = BTreeSet::new();
    let mut assumed_unchanged = BTreeSet::new();
    for entry in &entries {
        if !exact.contains(entry.path) {
            continue;
        }
        let path = std::str::from_utf8(entry.path).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "non-UTF-8 exact path in Git index output",
            )
        })?;
        if entry.tag == b'S' {
            skipped.insert(path.to_string());
        }
        if entry.tag.is_ascii_lowercase() {
            assumed_unchanged.insert(path.to_string());
        }
    }
    if skipped.is_empty() && assumed_unchanged.is_empty() {
        return Ok(ExactIndexPolicy::Proceed {
            paths,
            content_filter_paths,
        });
    }

    Ok(ExactIndexPolicy::Flagged {
        paths,
        content_filter_paths,
        skip_worktree: skipped,
        assume_unchanged: assumed_unchanged,
    })
}

fn read_index(config: &GuardedGitConfig<'_>) -> io::Result<Vec<u8>> {
    // One full, byte-oriented scan avoids invoking Git once per patch path and
    // lets one parse enforce aliases, directory prefixes, and flags together.
    let mut command = config.ls_files_command()?;
    command
        .disable_optional_locks()
        .args(["--cached", "-v", "-z"]);
    let output = command.output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "git exact-index probe failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output.stdout)
}

fn parse_index_entries(output: &[u8]) -> io::Result<Vec<IndexEntry<'_>>> {
    if output.is_empty() {
        return Ok(Vec::new());
    }
    let Some(body) = output.strip_suffix(&[0]) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unterminated Git exact-index output",
        ));
    };
    body.split(|byte| *byte == 0)
        .map(|record| {
            let [tag, b' ', path @ ..] = record else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "malformed Git exact-index output",
                ));
            };
            if !valid_index_tag(*tag) || path.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unexpected Git exact-index output",
                ));
            }
            Ok(IndexEntry { tag: *tag, path })
        })
        .collect()
}

fn valid_index_tag(tag: u8) -> bool {
    matches!(
        tag,
        b'H' | b'S' | b'M' | b'R' | b'C' | b'K' | b'h' | b's' | b'm' | b'r' | b'c' | b'k' | b'?'
    )
}

fn map_ignore_case_paths(
    git_root: &Path,
    entries: &[IndexEntry<'_>],
    paths: &[String],
    content_filter_paths: &[String],
) -> Result<(Vec<String>, Vec<String>), CaseConflict> {
    let evidence = IndexEvidence::collect(entries, paths);
    let mut groups = BTreeMap::<Vec<u8>, BTreeSet<String>>::new();
    for path in paths {
        groups
            .entry(ascii_fold(path.as_bytes()))
            .or_default()
            .insert(path.clone());
    }

    let mut mapping = BTreeMap::new();
    for (folded, spellings) in groups {
        let aliases = evidence
            .full_paths
            .get(&folded)
            .cloned()
            .unwrap_or_default();
        let canonical = match (spellings.len(), aliases.len()) {
            (1, 0) => {
                let Some(spelling) = spellings.first() else {
                    return Err(CaseConflict::new("empty request group", []));
                };
                spelling.clone()
            }
            (1, 1) => {
                let Some(alias) = aliases.first() else {
                    return Err(CaseConflict::new("missing index alias", spellings));
                };
                let alias = std::str::from_utf8(alias)
                    .map_err(|_| CaseConflict::new("non-UTF-8 index alias", spellings.clone()))?;
                if !spellings.contains(alias) {
                    let mut conflict =
                        CaseConflict::new("index full-path alias", spellings.clone());
                    conflict.add_index_paths(aliases.iter().copied());
                    return Err(conflict);
                }
                alias.to_string()
            }
            (_, 1) => {
                let Some(alias) = aliases.first() else {
                    return Err(CaseConflict::new("missing index alias", spellings));
                };
                let alias = std::str::from_utf8(alias)
                    .map_err(|_| CaseConflict::new("non-UTF-8 index alias", spellings.clone()))?;
                if !spellings.contains(alias) {
                    let mut conflict =
                        CaseConflict::new("index full-path alias", spellings.clone());
                    conflict.add_index_paths(aliases.iter().copied());
                    return Err(conflict);
                }
                if let Some(missing) = spellings.iter().find(|spelling| {
                    *spelling != alias
                        && std::fs::symlink_metadata(git_root.join(spelling)).is_err()
                }) {
                    return Err(CaseConflict::new(
                        "missing requested case alias",
                        [missing.clone(), alias.to_string()],
                    ));
                }
                alias.to_string()
            }
            (1, _) => {
                let mut conflict = CaseConflict::new("ambiguous index aliases", spellings);
                conflict.add_index_paths(aliases.iter().copied());
                return Err(conflict);
            }
            _ if aliases.is_empty() => {
                return Err(CaseConflict::new(
                    "request-set full-path aliases",
                    spellings,
                ));
            }
            _ => {
                let mut conflict = CaseConflict::new("ambiguous index aliases", spellings);
                conflict.add_index_paths(aliases.iter().copied());
                return Err(conflict);
            }
        };
        for spelling in spellings {
            mapping.insert(spelling, canonical.clone());
        }
    }

    let paths = map_and_dedupe(paths, &mapping)
        .ok_or_else(|| CaseConflict::new("unmapped requested path", paths.to_vec()))?;
    let content_filter_paths = map_and_dedupe(content_filter_paths, &mapping)
        .ok_or_else(|| CaseConflict::new("unmapped content-filter path", paths.clone()))?;
    if let Some(conflict) = requested_paths_conflict(&paths) {
        return Err(conflict);
    }
    if let Some(conflict) = index_paths_conflict(&paths, &evidence) {
        return Err(conflict);
    }
    Ok((paths, content_filter_paths))
}

fn map_and_dedupe(paths: &[String], mapping: &BTreeMap<String, String>) -> Option<Vec<String>> {
    let mut seen = BTreeSet::new();
    let mut mapped = Vec::new();
    for path in paths {
        let path = mapping.get(path)?.clone();
        if seen.insert(path.clone()) {
            mapped.push(path);
        }
    }
    Some(mapped)
}

fn requested_paths_conflict(paths: &[String]) -> Option<CaseConflict> {
    let mut full = BTreeMap::<Vec<u8>, BTreeSet<String>>::new();
    let mut directories = BTreeMap::<Vec<u8>, BTreeSet<String>>::new();
    for path in paths {
        full.entry(ascii_fold(path.as_bytes()))
            .or_default()
            .insert(path.clone());
        for prefix in non_leaf_prefixes(path.as_bytes()) {
            directories
                .entry(ascii_fold(prefix))
                .or_default()
                .insert(String::from_utf8_lossy(prefix).into_owned());
        }
    }
    if let Some(spellings) = full.values().find(|spellings| spellings.len() > 1) {
        return Some(CaseConflict::new(
            "request-set full-path aliases",
            spellings.clone(),
        ));
    }
    if let Some(spellings) = directories.values().find(|spellings| spellings.len() > 1) {
        return Some(CaseConflict::new(
            "request-set directory-prefix aliases",
            spellings.clone(),
        ));
    }
    for (folded, leaf) in full {
        if let Some(prefixes) = directories.get(&folded) {
            let paths = leaf.into_iter().chain(prefixes.iter().cloned());
            return Some(CaseConflict::new(
                "request-set file/directory aliases",
                paths,
            ));
        }
    }
    None
}

fn index_paths_conflict(paths: &[String], evidence: &IndexEvidence<'_>) -> Option<CaseConflict> {
    for path in paths {
        for prefix in non_leaf_prefixes(path.as_bytes()) {
            let folded = ascii_fold(prefix);
            if let Some(ancestors) = evidence.full_paths.get(&folded) {
                let mut conflict =
                    CaseConflict::new("index file/requested-directory alias", [path.clone()]);
                conflict.add_index_paths(ancestors.iter().copied());
                return Some(conflict);
            }
            if let Some(index_prefixes) = evidence.directory_prefixes.get(&folded)
                && index_prefixes.iter().any(|indexed| *indexed != prefix)
            {
                let mut conflict =
                    CaseConflict::new("index directory-prefix alias", [path.clone()]);
                conflict.add_index_paths(index_prefixes.iter().copied());
                return Some(conflict);
            }
        }
        if let Some(descendants) = evidence
            .directory_prefixes
            .get(&ascii_fold(path.as_bytes()))
        {
            let mut conflict =
                CaseConflict::new("requested file/index-directory alias", [path.clone()]);
            conflict.add_index_paths(descendants.iter().copied());
            return Some(conflict);
        }
    }
    None
}

fn path_prefixes(path: &[u8]) -> impl Iterator<Item = &[u8]> {
    non_leaf_prefixes(path).chain(std::iter::once(path))
}

fn non_leaf_prefixes(path: &[u8]) -> impl Iterator<Item = &[u8]> {
    path.iter()
        .enumerate()
        .filter_map(move |(index, byte)| (*byte == b'/').then_some(&path[..index]))
}

fn ascii_fold(value: &[u8]) -> Vec<u8> {
    value.iter().map(u8::to_ascii_lowercase).collect()
}

fn quote_paths(paths: &BTreeSet<String>) -> String {
    let shown = paths
        .iter()
        .take(8)
        .map(|path| {
            let mut characters = path.chars();
            let prefix = characters.by_ref().take(160).collect::<String>();
            if characters.next().is_some() {
                format!("{prefix}...")
            } else {
                path.clone()
            }
        })
        .map(|path| format!("{path:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    if paths.len() > 8 {
        format!("{shown}, ...")
    } else {
        shown
    }
}
