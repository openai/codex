//! Verify DNS authorization ordering and mandatory checks after approval.

use super::*;
use crate::config::NetworkProxyConfig;
use crate::runtime::network_proxy_state_for_policy;
use pretty_assertions::assert_eq;
use std::cell::Cell;

#[tokio::test]
async fn unapproved_hosts_do_not_trigger_dns() {
    for allowed in [vec![], vec!["allowed.example".to_string()]] {
        let mut config = NetworkProxyConfig::default();
        config.set_allowed_domains(allowed);
        let state = network_proxy_state_for_policy(config);
        let lookups = Cell::new(0);
        let decision = state
            .host_blocked_with_lookup(
                "payload.unapproved.example",
                /*port*/ 443,
                /*allow_local_binding*/ None,
                HostAuthorization::RequireAllowlist,
                |_host, _port| {
                    lookups.set(lookups.get() + 1);
                    async { Ok(vec!["93.184.215.14:443".parse().unwrap()]) }
                },
            )
            .await
            .unwrap();
        assert_eq!(
            (decision, lookups.get()),
            (HostBlockDecision::Blocked(HostBlockReason::NotAllowed), 0),
            "rejecting an unapproved hostname must not disclose it to DNS",
        );
    }
    // Approval permits DNS, but must still reject private resolution.
    let state = network_proxy_state_for_policy(NetworkProxyConfig::default());
    let decision = state
        .host_blocked_with_lookup(
            "approved.example",
            /*port*/ 443,
            /*allow_local_binding*/ None,
            HostAuthorization::Approved,
            |_host, _port| async { Ok(vec!["10.0.0.1:443".parse().unwrap()]) },
        )
        .await
        .unwrap();
    assert_eq!(
        decision,
        HostBlockDecision::Blocked(HostBlockReason::NotAllowedLocal)
    );
}

#[tokio::test]
async fn local_and_remote_approvals_preserve_review_precedence_and_recheck_dns() -> Result<()> {
    use crate::NetworkDecision;
    use crate::NetworkDecisionSource;
    use crate::NetworkPolicyDecider;
    use crate::NetworkPolicyRequest;
    use crate::NetworkPolicyRequestArgs;
    use crate::NetworkProtocol;
    use crate::NetworkProxy;
    use crate::network_policy::evaluate_host_policy;
    use std::sync::Arc;
    use std::sync::Mutex;

    for remote in [false, true] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let decider_calls = Arc::clone(&calls);
        let decider: Arc<dyn NetworkPolicyDecider> =
            Arc::new(move |request: NetworkPolicyRequest| {
                decider_calls.lock().unwrap().push(request.host);
                async { NetworkDecision::Allow }
            });
        // Explicit controller false must survive an executor allowing local binding.
        let mut config = NetworkProxyConfig {
            allow_local_binding: Some(false),
            review_domains: vec![
                "8.8.8.8".to_string(),
                "9.9.9.9".to_string(),
                "host.review.invalid".to_string(),
            ],
            ..NetworkProxyConfig::default()
        };
        config.set_allowed_domains(vec![
            "8.8.8.8".to_string(),
            "1.1.1.1".to_string(),
            "9.9.9.9".to_string(),
            "**.review.invalid".to_string(),
        ]);
        config.set_denied_domains(vec!["9.9.9.9".to_string()]);
        let state = Arc::new(network_proxy_state_for_policy(config));
        let proxy = NetworkProxy::builder()
            .state(Arc::clone(&state))
            .managed_by_codex(/*managed_by_codex*/ false)
            .policy_decider_arc(Arc::clone(&decider))
            .build()
            .await?;
        let scoped = proxy.for_execution(
            "remote",
            "execution",
            "token".to_string(),
            /*environment_policy*/ None,
            /*fallback_policy_decider*/ None,
        )?;
        let remote_decider = scoped
            .remote_policy_decider(/*allow_local_binding*/ true)
            .unwrap();
        for host in [
            "does-not-resolve.invalid",
            "8.8.8.8",
            "host.review.invalid",
            "ordinary.review.invalid",
            "1.1.1.1",
            "9.9.9.9",
        ] {
            let request = NetworkPolicyRequest::new(NetworkPolicyRequestArgs {
                protocol: NetworkProtocol::HttpsConnect,
                host: host.to_string(),
                port: 443,
                environment_id: None,
                client_addr: None,
                method: None,
                command: None,
                exec_policy_hint: None,
            });
            let decision = if remote {
                remote_decider.decide(request).await
            } else {
                evaluate_host_policy(&state, Some(&decider), &request).await?
            };
            let expected = if host.ends_with(".invalid") {
                NetworkDecision::deny_with_source(
                    HostBlockReason::NotAllowedLocal.as_str(),
                    NetworkDecisionSource::BaselinePolicy,
                )
            } else if host == "9.9.9.9" {
                NetworkDecision::deny_with_source(
                    HostBlockReason::Denied.as_str(),
                    NetworkDecisionSource::BaselinePolicy,
                )
            } else {
                NetworkDecision::Allow
            };
            assert_eq!(decision, expected);
        }
        assert_eq!(
            *calls.lock().unwrap(),
            vec!["does-not-resolve.invalid", "8.8.8.8", "host.review.invalid"],
            "review selectors precede exact and wildcard allows; ordinary allows and denies skip the decider"
        );
    }
    Ok(())
}
