use crate::metrics::error::Result;
use crate::metrics::validation::validate_tag_key;
use crate::metrics::validation::validate_tag_value;
use opentelemetry::KeyValue;
use std::collections::BTreeMap;

pub(crate) fn collect_tags(tags: &[(&str, &str)]) -> Result<Vec<(String, String)>> {
    tags.iter()
        .map(|(key, value)| {
            validate_tag_key(key)?;
            validate_tag_value(value)?;
            Ok(((*key).to_string(), (*value).to_string()))
        })
        .collect()
}

pub(crate) fn merge_tags(
    default_tags: &BTreeMap<String, String>,
    tags: &[(String, String)],
) -> BTreeMap<String, String> {
    let mut merged = default_tags.clone();
    for (key, value) in tags {
        merged.insert(key.clone(), value.clone());
    }
    merged
}

pub(crate) fn tags_to_attributes(tags: &BTreeMap<String, String>) -> Vec<KeyValue> {
    tags.iter()
        .map(|(key, value)| KeyValue::new(key.clone(), value.clone()))
        .collect()
}
