#[cfg(test)]
use crate::git_config::GitConfigEntry;
#[cfg(test)]
use crate::safe_git::isolate_git_command_environment;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io;

#[cfg(test)]
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

#[cfg(test)]
pub(crate) fn untrusted_driver_selection(
    entries: &BTreeMap<String, GitConfigEntry>,
    attributes: &BTreeMap<String, String>,
) -> io::Result<Option<(String, String)>> {
    let mut driver_names = BTreeSet::new();
    for entry in entries.values() {
        if let Some(name) = merge_driver_subsection(&entry.key)? {
            driver_names.insert(name.to_string());
        }
    }
    let default = entries
        .get("merge.default")
        .map(|entry| entry.value.as_str());
    Ok(
        untrusted_driver_selections(&driver_names, default, attributes)
            .into_iter()
            .next()
            .map(|(path, driver)| (driver, path)),
    )
}

/// Return every path whose effective merge selection names a configured user
/// driver namespace. Git creates that namespace for any subsection key, even
/// when `.driver` is missing or explicitly empty; those cases fail or attempt
/// an empty command rather than falling back to built-in text. The complete
/// set is required by the scratch classifier so every selected namespace is
/// quarantined.
pub(crate) fn untrusted_driver_selections(
    driver_names: &BTreeSet<String>,
    default: Option<&str>,
    attributes: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut selected = BTreeMap::new();
    for (path, attribute) in attributes {
        for name in candidate_driver_names(attribute, default) {
            if driver_names.contains(name) {
                selected.insert(path.clone(), name.to_string());
                break;
            }
        }
    }
    selected
}

/// Parse the user-driver subsection using Git 2.54's last-dot behavior.
/// Empty and dotted subsection names are valid; only `merge.default` has no
/// user-driver subsection in the fixed query.
pub(crate) fn merge_driver_subsection(key: &str) -> io::Result<Option<&str>> {
    if key == "merge.default" {
        return Ok(None);
    }
    let (name, final_key) = key
        .strip_prefix("merge.")
        .and_then(|key| key.rsplit_once('.'))
        .ok_or_else(|| invalid_config_entry("malformed merge driver key"))?;
    if final_key.is_empty() {
        return Err(invalid_config_entry("empty merge driver configuration key"));
    }
    Ok(Some(name))
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

#[cfg(test)]
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
