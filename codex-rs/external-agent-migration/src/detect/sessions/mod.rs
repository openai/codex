mod cla;
mod common;
mod connectors_cla;
mod connectors_cur;
mod cur;

use crate::model::DetectedConnectorCandidate;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub use cla::detect_recent_cla_sessions;
pub(crate) use cla::detect_recent_cla_sessions_with_limits;
pub use connectors_cla::ImportedSessionConnectorAttribution;
pub(crate) use connectors_cla::detect_cla_session_connectors;
pub(crate) use connectors_cla::detect_cla_session_connectors_by_source_path;
pub use connectors_cla::detect_imported_cla_session_connectors;
pub use connectors_cla::detect_imported_cla_session_connectors_by_source_path;
pub(crate) use connectors_cur::detect_cur_session_connectors;
pub(crate) use connectors_cur::detect_cur_session_connectors_by_source_path;
pub use cur::detect_recent_cur_sessions;
pub(crate) use cur::detect_recent_cur_sessions_with_limits;

pub(crate) fn aggregate_session_connector_candidates(
    candidates_by_source_path: BTreeMap<PathBuf, Vec<DetectedConnectorCandidate>>,
) -> Vec<DetectedConnectorCandidate> {
    let mut candidates = BTreeMap::<String, DetectedConnectorCandidate>::new();
    for session_candidates in candidates_by_source_path.into_values() {
        for candidate in session_candidates {
            let key = candidate.name.to_lowercase();
            let session_count = candidate.session_count;
            let aggregate = candidates.entry(key).or_insert_with(|| {
                let mut aggregate = candidate;
                aggregate.session_count = 0;
                aggregate
            });
            aggregate.session_count = aggregate.session_count.saturating_add(session_count);
        }
    }
    candidates.into_values().collect()
}
