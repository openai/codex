use super::*;

struct MapEnv {
    values: HashMap<String, String>,
}

impl EnvSource for MapEnv {
    fn var(&self, key: &str) -> Option<String> {
        self.values.get(key).cloned()
    }
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
