use crate::error::MetricsError;
use crate::error::Result;
use std::collections::BTreeMap;

pub(crate) fn validate_tags(tags: &BTreeMap<String, String>) -> Result<()> {
    for (key, value) in tags {
        validate_tag_component(key, "tag key")?;
        validate_tag_component(value, "tag value")?;
    }
    Ok(())
}

pub(crate) fn validate_metric_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(MetricsError::EmptyMetricName);
    }
    if !name.chars().all(is_metric_char) {
        return Err(MetricsError::InvalidMetricName {
            name: name.to_string(),
        });
    }
    Ok(())
}

pub(crate) fn validate_tag_component(value: &str, label: &str) -> Result<()> {
    if value.is_empty() {
        return Err(MetricsError::EmptyTagComponent {
            label: label.to_string(),
        });
    }
    if !value.chars().all(is_tag_char) {
        return Err(MetricsError::InvalidTagComponent {
            label: label.to_string(),
            value: value.to_string(),
        });
    }
    Ok(())
}

fn is_metric_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')
}

fn is_tag_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/')
}
