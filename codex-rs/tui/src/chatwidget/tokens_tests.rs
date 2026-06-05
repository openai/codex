use super::*;
use codex_app_server_protocol::AccountTokenUsageSummary;
use pretty_assertions::assert_eq;

#[test]
fn loaded_state_freezes_chart_anchor_date_at_completion() {
    let state = Arc::new(RwLock::new(TokenActivityState::Loading));
    let handle = TokenActivityHandle {
        state: Arc::clone(&state),
    };
    let today =
        NaiveDate::from_ymd_opt(/*year*/ 2026, /*month*/ 5, /*day*/ 29).expect("valid date");

    handle.finish_with_today(
        Ok(GetAccountTokenUsageResponse {
            summary: AccountTokenUsageSummary {
                lifetime_tokens: None,
                peak_daily_tokens: None,
                longest_running_turn_sec: None,
                current_streak_days: None,
                longest_streak_days: None,
            },
            daily_usage_buckets: None,
        }),
        today,
    );

    let state = state.read().expect("token activity state poisoned");
    match &*state {
        TokenActivityState::Loaded {
            today: loaded_today,
            ..
        } => {
            assert_eq!(*loaded_today, today);
        }
        other => panic!("expected loaded state, got {other:?}"),
    }
}
