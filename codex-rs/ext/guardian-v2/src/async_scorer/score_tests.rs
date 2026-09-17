use std::collections::BTreeMap;
use std::time::Duration;
use std::time::SystemTime;

use codex_extension_api::ExtensionData;
use codex_protocol::security_risk::SecurityRiskScore;
use pretty_assertions::assert_eq;

use super::GuardianV2ScoreProgress;

pub(in crate::async_scorer) fn cached_score(store: &ExtensionData) -> Option<SecurityRiskScore> {
    let progress = store.get::<GuardianV2ScoreProgress>()?;
    let state = progress.state.lock().unwrap();
    state.score.clone()
}

// Replace only the score so fixtures can exercise authorization and coverage independently.
pub(in crate::async_scorer) fn set_cached_score(store: &ExtensionData, score: SecurityRiskScore) {
    let progress = store.get_or_init(GuardianV2ScoreProgress::default);
    progress.state.lock().unwrap().score = Some(score);
}

#[test]
fn fail_closed_score_preserves_classification_order() {
    let thread_store = ExtensionData::new("thread-1");
    let progress = thread_store.get_or_init(GuardianV2ScoreProgress::default);
    let newer_sampled_at = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
    let newest_sampled_at = newer_sampled_at + Duration::from_secs(1);
    let newer_score = SecurityRiskScore {
        scores: BTreeMap::from([("action_risk".to_owned(), 0.25)]),
        call_id: None,
        action: None,
        sampled_at: Some(newer_sampled_at.into()),
    };
    set_cached_score(&thread_store, newer_score.clone());

    progress.fail_closed(SystemTime::UNIX_EPOCH);
    assert_eq!(cached_score(&thread_store).as_ref(), Some(&newer_score));

    for sampled_at in [newer_sampled_at, newest_sampled_at] {
        set_cached_score(&thread_store, newer_score.clone());
        progress.fail_closed(sampled_at);
        let fail_closed_score = SecurityRiskScore {
            scores: BTreeMap::from([("action_risk".to_owned(), 1.0)]),
            call_id: None,
            action: None,
            sampled_at: Some(sampled_at.into()),
        };
        assert_eq!(
            cached_score(&thread_store).as_ref(),
            Some(&fail_closed_score)
        );
    }
}
