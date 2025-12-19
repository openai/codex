use crate::STATSD_CONTENT_TYPE;
use crate::error::MetricsError;
use crate::error::Result;
use crate::validation::validate_metric_name;
use crate::validation::validate_tag_key;
use crate::validation::validate_tag_value;
use sentry::types::Dsn;
use std::collections::BTreeMap;

pub(crate) struct StatsdLine {
    name: String,
    value: i64,
    kind: MetricKind,
    tags: Vec<(String, String)>,
}

impl StatsdLine {
    pub(crate) fn counter(name: &str, value: i64, tags: Vec<(String, String)>) -> Result<Self> {
        validate_metric_name(name)?;
        Ok(Self {
            name: name.to_string(),
            value,
            kind: MetricKind::Counter,
            tags,
        })
    }

    pub(crate) fn render(&self, default_tags: &BTreeMap<String, String>) -> Result<String> {
        let tags = merge_tags(default_tags, &self.tags);
        let name = self.name.as_str();
        let value = self.value;
        let kind = self.kind.as_str();
        let mut line = format!("{name}:{value}|{kind}");

        if !tags.is_empty() {
            let taglist = tags
                .iter()
                .map(|(key, value)| format!("{key}:{value}"))
                .collect::<Vec<_>>()
                .join(",");
            line.push_str("|#");
            line.push_str(&taglist);
        }

        Ok(line)
    }
}

enum MetricKind {
    Counter,
}

impl MetricKind {
    fn as_str(&self) -> &'static str {
        match self {
            MetricKind::Counter => "c",
        }
    }
}

pub(crate) fn build_statsd_envelope(dsn: &Dsn, payload: &str) -> Result<Vec<u8>> {
    let header = serde_json::json!({
        "dsn": dsn.to_string(),
    });
    let mut bytes = Vec::new();
    serde_json::to_writer(&mut bytes, &header)
        .map_err(|source| MetricsError::SerializeEnvelopeHeader { source })?;
    bytes.push(b'\n');

    let item_header = serde_json::json!({
        "type": "statsd",
        "length": payload.len(),
        "content_type": STATSD_CONTENT_TYPE,
    });
    serde_json::to_writer(&mut bytes, &item_header)
        .map_err(|source| MetricsError::SerializeEnvelopeItemHeader { source })?;
    bytes.push(b'\n');
    bytes.extend_from_slice(payload.as_bytes());
    bytes.push(b'\n');
    Ok(bytes)
}

pub(crate) fn collect_tags(tags: &[(&str, &str)]) -> Result<Vec<(String, String)>> {
    tags.iter()
        .map(|(key, value)| {
            validate_tag_key(key)?;
            validate_tag_value(value)?;
            Ok(((*key).to_string(), (*value).to_string()))
        })
        .collect()
}

fn merge_tags(
    default_tags: &BTreeMap<String, String>,
    tags: &[(String, String)],
) -> BTreeMap<String, String> {
    let mut merged = default_tags.clone();
    for (key, value) in tags {
        merged.insert(key.clone(), value.clone());
    }
    merged
}
