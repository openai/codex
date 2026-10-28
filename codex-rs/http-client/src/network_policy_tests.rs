//! Destination matching, bootstrap scope, and account revocation behavior.

use std::collections::BTreeSet;

use pretty_assertions::assert_eq;
use reqwest::Url;

use super::*;

#[test]
fn credential_binding_keeps_both_endpoint_restrictions_and_account_revocation() {
    let controller = NetworkPolicyController::default();
    let policy = controller.policy();
    assert!(controller.publish(policy.revision(), DestinationPolicy::Unrestricted));
    let a = Url::parse("https://example.com/a").unwrap();
    let b = Url::parse("https://example.com/b").unwrap();
    let c = Url::parse("https://example.com/c").unwrap();
    let credentials = policy
        .clone()
        .for_current_account()
        .restrict_to_endpoints(BTreeSet::from([b.clone(), c.clone()]));
    let bound = policy
        .clone()
        .restrict_to_endpoints(BTreeSet::from([a.clone(), b.clone()]))
        .with_account_binding(&credentials)
        .unwrap();
    assert!(matches!(
        bound.acquire(&a),
        Err(NetworkPolicyDenied::Destination)
    ));
    assert!(matches!(
        bound.acquire(&c),
        Err(NetworkPolicyDenied::Destination)
    ));
    let permit = bound.acquire(&b).unwrap();

    policy.invalidate();
    assert!(controller.publish(policy.revision(), DestinationPolicy::Unrestricted));
    assert!(matches!(
        bound.acquire(&b),
        Err(NetworkPolicyDenied::Revoked)
    ));
    assert_eq!(permit.check(), Err(NetworkPolicyDenied::Revoked));
}

#[test]
fn credential_binding_cannot_replace_a_callers_policy_or_revive_its_account() {
    let controller = NetworkPolicyController::default();
    let policy = controller.policy();
    let allowed = Url::parse("https://example.com/allowed").unwrap();
    assert!(controller.publish(policy.revision(), DestinationPolicy::Unrestricted));
    let caller = policy
        .clone()
        .for_current_account()
        .restrict_to_endpoints(BTreeSet::from([allowed.clone()]));
    let bound = caller
        .clone()
        .with_account_binding(&NetworkPolicy::unmanaged())
        .unwrap();
    assert!(bound.acquire(&allowed).is_ok());
    assert!(matches!(
        bound.acquire(&Url::parse("https://example.com/other").unwrap()),
        Err(NetworkPolicyDenied::Destination)
    ));
    assert!(
        caller
            .clone()
            .with_account_binding(&NetworkPolicyController::default().policy())
            .is_none()
    );

    policy.invalidate();
    assert!(controller.publish(policy.revision(), DestinationPolicy::Unrestricted));
    assert!(
        caller
            .with_account_binding(&policy.for_current_account())
            .is_none()
    );
    assert!(matches!(
        bound.acquire(&allowed),
        Err(NetworkPolicyDenied::Revoked)
    ));
}

#[test]
fn restricted_policy_matches_exact_secure_hosts() {
    let controller = NetworkPolicyController::default();
    let policy = controller.policy();
    assert!(controller.publish(
        policy.revision(),
        DestinationPolicy::Restricted {
            allowed_hosts: BTreeSet::from(["example.com".to_string()]),
        }
    ));
    for (url, expected) in [
        ("https://EXAMPLE.com/path", true),
        ("https://example.com.:8443/path", true),
        ("wss://example.com/path", true),
        ("http://example.com/path", false),
        ("ws://example.com/path", false),
        ("https://sub.example.com/path", false),
        ("https://example.com.evil.test/path", false),
        ("https://example.com@evil.test/path", false),
        ("https://127.0.0.1/path", false),
    ] {
        assert_eq!(
            policy.acquire(&Url::parse(url).unwrap()).is_ok(),
            expected,
            "{url}"
        );
    }
}

#[tokio::test]
async fn invalidation_revokes_existing_permits_and_rejects_stale_publication() {
    let controller = NetworkPolicyController::default();
    let policy = controller.policy();
    let url = Url::parse("https://example.com").unwrap();
    assert_eq!(
        policy.acquire(&url).unwrap_err(),
        NetworkPolicyDenied::Unavailable
    );
    let previous = policy.revision();
    assert!(controller.publish(previous, DestinationPolicy::Unrestricted));
    let permit = policy.acquire(&url).unwrap();
    policy.invalidate();
    assert!(!controller.publish(previous, DestinationPolicy::Unrestricted));
    assert!(controller.publish(policy.revision(), DestinationPolicy::Unrestricted));
    assert_eq!(
        permit.run(async { "should not run" }).await,
        Err(NetworkPolicyDenied::Revoked)
    );
    assert!(policy.acquire(&url).is_ok());
}

#[tokio::test]
async fn load_failure_recovers_current_account_but_cannot_revive_previous_account() {
    let controller = NetworkPolicyController::default();
    let policy = controller.policy();
    let account = policy.clone().for_current_account();
    let url = Url::parse("https://example.com").unwrap();
    let mut changes = policy.changes().unwrap();
    controller.publish(policy.revision(), DestinationPolicy::Unrestricted);
    changes.changed().await.unwrap();
    controller.publish(policy.revision(), DestinationPolicy::Unrestricted);
    assert!(!changes.has_changed().unwrap());
    let old_request = account.acquire(&url).unwrap();
    controller.unavailable(policy.revision());
    assert_eq!(
        account.acquire(&url).unwrap_err(),
        NetworkPolicyDenied::Unavailable
    );
    controller.publish(policy.revision(), DestinationPolicy::Unrestricted);
    assert!(account.acquire(&url).is_ok());
    assert_eq!(old_request.check(), Err(NetworkPolicyDenied::Revoked));
    policy.invalidate();
    controller.publish(policy.revision(), DestinationPolicy::Unrestricted);
    assert_eq!(
        account.acquire(&url).unwrap_err(),
        NetworkPolicyDenied::Revoked
    );
    assert!(policy.for_current_account().acquire(&url).is_ok());
}

#[tokio::test]
async fn unsupported_sdk_work_is_denied_and_running_work_is_cancelled() {
    let controller = NetworkPolicyController::default();
    let policy = controller.policy();
    controller.publish(policy.revision(), DestinationPolicy::Unrestricted);
    let permit = policy.acquire_for_unsupported_sdk().unwrap();
    controller.publish(
        policy.revision(),
        DestinationPolicy::Restricted {
            allowed_hosts: BTreeSet::from(["example.com".to_string()]),
        },
    );
    assert_eq!(
        policy.acquire_for_unsupported_sdk().unwrap_err(),
        NetworkPolicyDenied::UnsupportedTransport
    );
    assert_eq!(
        permit.run(std::future::pending::<()>()).await,
        Err(NetworkPolicyDenied::Revoked)
    );
}
