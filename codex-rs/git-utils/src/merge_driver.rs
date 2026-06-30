use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io;
use std::io::Seek;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;

use crate::git_config::GitConfigEntry;
use crate::git_config::parse_effective_config;
use crate::safe_git::isolate_git_command_environment;

const MERGE_CONFIG_PATTERN: &str = r"^(merge\.default|merge\..*\.driver)$";

pub(crate) fn ensure_no_selected_merge_drivers(
    cwd: &Path,
    paths: &[String],
    git_config_args: &[String],
) -> io::Result<()> {
    let entries = read_merge_config(cwd, git_config_args)?;
    let attributes = read_merge_attributes(cwd, paths, git_config_args)?;
    if let Some((driver, path)) = untrusted_driver_selection(&entries, &attributes)? {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "refusing to run an internal Git three-way apply with merge driver {driver:?} selected for {path:?}"
            ),
        ));
    }
    Ok(())
}

fn read_merge_config(
    cwd: &Path,
    git_config_args: &[String],
) -> io::Result<BTreeMap<String, GitConfigEntry>> {
    let mut command = Command::new("git");
    isolate_git_command_environment(&mut command);
    let output = command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(git_config_args)
        .args([
            "config",
            "--null",
            "--show-scope",
            "--show-origin",
            "--includes",
            "--get-regexp",
            MERGE_CONFIG_PATTERN,
        ])
        .current_dir(cwd)
        .output()?;
    if !output
        .status
        .code()
        .is_some_and(|code| code == 0 || code == 1)
    {
        return Err(io::Error::other(format!(
            "git merge config probe failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    parse_effective_config(&output.stdout)
}

fn read_merge_attributes(
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

    let mut command = Command::new("git");
    isolate_git_command_environment(&mut command);
    let output = command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(git_config_args)
        .args(["check-attr", "--stdin", "-z", "merge"])
        .current_dir(cwd)
        .stdin(Stdio::from(input))
        .output()?;
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
            .filter(|name| !name.is_empty())
            .ok_or_else(|| invalid_config_entry("malformed merge driver key"))?;
        if !entry.value.is_empty() && !entry.scope.is_system_or_global() {
            return Ok(Some((name.to_string(), "<Git config>".to_string())));
        }
        drivers.insert(name.to_string(), entry.value.as_str());
    }

    let default = entries
        .get("merge.default")
        .map(|entry| entry.value.as_str())
        .filter(|value| !value.is_empty());
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
