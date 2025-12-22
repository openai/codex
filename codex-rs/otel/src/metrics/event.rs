use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub(crate) enum MetricEvent {
    Counter {
        name: String,
        value: i64,
        tags: BTreeMap<String, String>,
    },
    Histogram {
        name: String,
        value: i64,
        tags: BTreeMap<String, String>,
    },
}
