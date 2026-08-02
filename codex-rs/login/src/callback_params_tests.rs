use super::*;
use pretty_assertions::assert_eq;

#[test]
fn accepts_the_original_oauth_state() {
    assert_eq!(
        login_callback_result_from_state("expected-state", "expected-state"),
        Some(LoginCallbackResult::default())
    );
}

#[test]
fn accepts_the_allowlisted_life_sciences_suffix() {
    assert_eq!(
        login_callback_result_from_state(
            "expected-state.onboarding_entrypoint=life_sciences",
            "expected-state",
        ),
        Some(LoginCallbackResult {
            onboarding_entrypoint: Some(LoginOnboardingEntrypoint::LifeSciences),
        })
    );
}

#[test]
fn rejects_a_suffix_when_the_nonce_does_not_match() {
    assert_eq!(
        login_callback_result_from_state(
            "different-state.onboarding_entrypoint=life_sciences",
            "expected-state",
        ),
        None
    );
}

#[test]
fn rejects_unrecognized_or_repeated_suffixes() {
    for callback_state in [
        "expected-state.onboarding_entrypoint=unknown",
        "expected-state.onboarding_entrypoint=life_sciences.onboarding_entrypoint=life_sciences",
        "expected-state.extra=value.onboarding_entrypoint=life_sciences",
    ] {
        assert_eq!(
            login_callback_result_from_state(callback_state, "expected-state"),
            None,
            "unexpectedly accepted {callback_state}",
        );
    }
}
