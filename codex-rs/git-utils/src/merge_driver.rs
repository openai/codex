use crate::git_config::GitConfigEntry;
#[cfg(test)]
use crate::safe_git::isolate_git_command_environment;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io;

pub(crate) fn parse_merge_attributes(
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

pub(crate) fn untrusted_driver_selection(
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
