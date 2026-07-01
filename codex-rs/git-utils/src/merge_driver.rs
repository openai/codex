use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io;
use std::io::Seek;
use std::io::Write;
use std::path::Path;
use std::process::Stdio;

use crate::git_command::GitRunner;
use crate::git_config::GitConfigEntry;
use crate::safe_git::GitConfigOverrideFile;
#[cfg(test)]
use crate::safe_git::isolate_git_command_environment;
use crate::safe_git::read_effective_config_with_fallback;

const MERGE_CONFIG_PATTERN: &str = r"^(merge\.default|merge\..*\.driver)$";

pub(crate) fn ensure_no_selected_merge_drivers(
    git: &GitRunner,
    cwd: &Path,
    paths: &[String],
    git_config_args: &[String],
) -> io::Result<Option<GitConfigOverrideFile>> {
    let entries = read_merge_config(git, cwd, git_config_args)?;
    let attributes = read_merge_attributes(git, cwd, paths, git_config_args)?;
    if let Some((driver, path)) = untrusted_driver_selection(&entries, &attributes)? {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "refusing to run an internal Git three-way apply with merge driver {driver:?} selected for {path:?}"
            ),
        ));
    }

    let driver_keys = entries
        .values()
        .filter(|entry| entry.key != "merge.default" && !entry.value.is_empty())
        .map(|entry| entry.key.clone())
        .collect::<Vec<_>>();
    if driver_keys.is_empty() {
        return Ok(None);
    }

    let guard = GitConfigOverrideFile::new("merge-driver-neutralization.gitconfig")?;
    for key in driver_keys {
        guard.add_value(
            git,
            cwd,
            &key,
            "",
            &format!("Git merge-driver neutralization for {key:?}"),
        )?;
    }
    Ok(Some(guard))
}

fn read_merge_config(
    git: &GitRunner,
    cwd: &Path,
    git_config_args: &[String],
) -> io::Result<BTreeMap<String, GitConfigEntry>> {
    read_effective_config_with_fallback(git, cwd, git_config_args, MERGE_CONFIG_PATTERN, "merge")
}

fn read_merge_attributes(
    git: &GitRunner,
    cwd: &Path,
    paths: &[String],
    git_config_args: &[String],
) -> io::Result<BTreeMap<String, String>> {
    if paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refusing to inspect merge attributes for an empty patch path set",
        ));
    }
    let mut input = tempfile::tempfile()?;
    for path in paths {
        input.write_all(path.as_bytes())?;
        input.write_all(&[0])?;
    }
    input.rewind()?;

    let mut command = git.command();
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(git_config_args)
        .args(["check-attr", "--stdin", "-z", "merge"])
        .current_dir(cwd)
        .stdin(Stdio::from(input));
    let output = git.output(command)?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "git merge attribute probe failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    parse_merge_attributes(&output.stdout, paths)
}

fn parse_merge_attributes(
    output: &[u8],
    expected_paths: &[String],
) -> io::Result<BTreeMap<String, String>> {
    let Some(body) = output.strip_suffix(&[0]) else {
        return Err(invalid_attribute_output(
            "unterminated Git attribute output",
        ));
    };
    let fields = body.split(|byte| *byte == 0).collect::<Vec<_>>();
    if fields.len() % 3 != 0 {
        return Err(invalid_attribute_output("incomplete Git attribute record"));
    }
    let expected = expected_paths
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut attributes = BTreeMap::new();
    for record in fields.chunks_exact(3) {
        let path = std::str::from_utf8(record[0])
            .map_err(|_| invalid_attribute_output("non-UTF-8 Git attribute path"))?;
        let attribute = std::str::from_utf8(record[1])
            .map_err(|_| invalid_attribute_output("non-UTF-8 Git attribute name"))?;
        let value = std::str::from_utf8(record[2])
            .map_err(|_| invalid_attribute_output("non-UTF-8 Git attribute value"))?;
        if !expected.contains(path) || attribute != "merge" {
            return Err(invalid_attribute_output(
                "unexpected Git merge attribute record",
            ));
        }
        if attributes
            .insert(path.to_string(), value.to_string())
            .is_some()
        {
            return Err(invalid_attribute_output(
                "duplicate Git merge attribute record",
            ));
        }
    }
    if attributes.len() != expected.len() {
        return Err(invalid_attribute_output(
            "missing Git merge attribute record",
        ));
    }
    Ok(attributes)
}

fn untrusted_driver_selection(
    entries: &BTreeMap<String, GitConfigEntry>,
    attributes: &BTreeMap<String, String>,
) -> io::Result<Option<(String, String)>> {
    let mut drivers = BTreeMap::new();
    for entry in entries.values() {
        if entry.key == "merge.default" {
            continue;
        }
        let name = entry
            .key
            .strip_prefix("merge.")
            .and_then(|key| key.strip_suffix(".driver"))
            .ok_or_else(|| invalid_config_entry("malformed merge driver key"))?;
        drivers.insert(name.to_string(), entry.value.as_str());
    }

    let default = entries
        .get("merge.default")
        .map(|entry| entry.value.as_str());
    for (path, attribute) in attributes {
        for name in candidate_driver_names(attribute, default) {
            if drivers.get(name).is_some_and(|value| !value.is_empty()) {
                return Ok(Some((name.to_string(), path.clone())));
            }
        }
    }
    Ok(None)
}

fn candidate_driver_names<'a>(attribute: &'a str, default: Option<&'a str>) -> BTreeSet<&'a str> {
    let mut names = BTreeSet::from([attribute]);
    if attribute == "unspecified"
        && let Some(default) = default
    {
        names.insert(default);
    }
    names
}

fn invalid_attribute_output(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn invalid_config_entry(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
#[path = "merge_driver_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "merge_driver_race_tests.rs"]
mod race_tests;
