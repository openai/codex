use std::collections::BTreeMap;
use std::time::Duration;
use std::time::SystemTime;

use codex_extension_api::ExtensionData;
use codex_protocol::security_risk::SecurityRiskScore;
use pretty_assertions::assert_eq;

use super::record_fail_closed_score;

#[test]
fn fail_closed_score_preserves_classification_order() {
    let thread_store = ExtensionData::new("thread-1");
    let newer_sampled_at = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
    let newest_sampled_at = newer_sampled_at + Duration::from_secs(1);
    let newer_score = SecurityRiskScore {
        scores: BTreeMap::from([("action_risk".to_owned(), 0.25)]),
        call_id: None,
        action: None,
        sampled_at: Some(newer_sampled_at.into()),
    };
    thread_store.insert(newer_score.clone());

    record_fail_closed_score(&thread_store, SystemTime::UNIX_EPOCH);
    assert_eq!(
        thread_store.get::<SecurityRiskScore>().as_deref(),
        Some(&newer_score)
    );

    for sampled_at in [newer_sampled_at, newest_sampled_at] {
        thread_store.insert(newer_score.clone());
        record_fail_closed_score(&thread_store, sampled_at);
        let fail_closed_score = SecurityRiskScore {
            scores: BTreeMap::from([("action_risk".to_owned(), 1.0)]),
            call_id: None,
            action: None,
            sampled_at: Some(sampled_at.into()),
        };
        assert_eq!(
            thread_store.get::<SecurityRiskScore>().as_deref(),
            Some(&fail_closed_score)
        );
    }
}
