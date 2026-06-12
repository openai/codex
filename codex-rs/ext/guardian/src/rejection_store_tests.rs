use std::collections::HashMap;

use codex_protocol::protocol::GuardianAssessmentDecisionSource;
use pretty_assertions::assert_eq;

use super::*;

#[test]
fn stores_rejections_by_review_id_and_removes_them_once() {
    let rejection = GuardianRejection {
        rationale: "unsafe command".to_string(),
        source: GuardianAssessmentDecisionSource::Agent,
    };
    let mut store = GuardianRejectionStore::default();

    assert_eq!(
        store.insert("review-1".to_string(), rejection.clone()),
        None
    );
    assert!(store.contains("review-1"));
    assert_eq!(
        store,
        GuardianRejectionStore {
            rejections: HashMap::from([("review-1".to_string(), rejection.clone())]),
        }
    );
    assert_eq!(store.remove("review-1"), Some(rejection));
    assert_eq!(store.remove("review-1"), None);
    assert_eq!(store, GuardianRejectionStore::default());
}
