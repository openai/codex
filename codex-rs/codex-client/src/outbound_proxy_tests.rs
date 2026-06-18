use super::*;
use pretty_assertions::assert_eq;

struct MapEnv {
    values: HashMap<String, String>,
}

impl EnvSource for MapEnv {
    fn var(&self, key: &str) -> Option<String> {
        self.values.get(key).cloned()
    }
}

#[test]
fn proxy_env_value_matches_reqwest_casing_precedence() {
    let env = MapEnv {
        values: HashMap::from([
            ("HTTPS_PROXY".to_string(), "upper".to_string()),
            ("https_proxy".to_string(), "lower".to_string()),
            ("http_proxy".to_string(), "lower-only".to_string()),
            ("ALL_PROXY".to_string(), String::new()),
            ("all_proxy".to_string(), "masked".to_string()),
        ]),
    };

    assert_eq!(
        proxy_env_value(&env, "HTTPS_PROXY"),
        Some("upper".to_string())
    );
    assert_eq!(
        proxy_env_value(&env, "HTTP_PROXY"),
        Some("lower-only".to_string())
    );
    assert_eq!(proxy_env_value(&env, "ALL_PROXY"), Some(String::new()));
}

#[test]
fn environment_fallback_reads_injected_proxy_environment() {
    let env = MapEnv {
        values: HashMap::from([("HTTPS_PROXY".to_string(), "://invalid".to_string())]),
    };
    let origin = RequestOrigin::parse("https://auth.openai.com/oauth/token").expect("valid URL");
    let result = configure_env_proxy_handling(
        &env,
        reqwest::Client::builder(),
        Some(&origin),
        ClientRouteClass::Auth,
    );

    assert!(matches!(
        result,
        Err(BuildRouteAwareHttpClientError::InvalidProxyConfig {
            route_class: ClientRouteClass::Auth,
        })
    ));
}

#[test]
fn unavailable_system_proxy_decision_is_cached() {
    let request_url = "https://unavailable-cache.test/oauth/token";
    let decision = SystemProxyDecision::Unavailable {
        failure: RouteFailureClass::ProxyResolutionUnavailable,
    };

    cache_system_proxy_decision(request_url, decision.clone());

    assert_eq!(cached_system_proxy_decision(request_url), Some(decision));
}
