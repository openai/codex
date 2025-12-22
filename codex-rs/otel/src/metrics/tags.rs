use crate::metrics::error::Result;
use crate::metrics::validation::validate_tag_key;
use crate::metrics::validation::validate_tag_value;
use std::collections::BTreeMap;

pub(crate) fn merge_tags(
    default_tags: &BTreeMap<String, String>,
    tags: &[(&str, &str)],
) -> Result<BTreeMap<String, String>> {
    let mut merged = default_tags.clone();
    for (key, value) in tags {
        validate_tag_key(key)?;
        validate_tag_value(value)?;
        merged.insert((*key).to_string(), (*value).to_string());
    }
    Ok(merged)
}
