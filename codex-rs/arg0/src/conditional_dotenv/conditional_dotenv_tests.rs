use super::*;
use pretty_assertions::assert_eq;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::time::Duration;

#[derive(Debug, Default, PartialEq, Eq)]
struct TestEnvironment(BTreeMap<String, String>);

impl StartupEnvironment for TestEnvironment {
    fn set(&mut self, key: &str, value: &str) {
        self.0.insert(key.to_string(), value.to_string());
    }

    fn remove(&mut self, key: &str) {
        self.0.remove(key);
    }
}

fn write_overlay(codex_home: &Path, name: &str, contents: &str) -> std::io::Result<()> {
    fs::write(codex_home.join(name), contents)
}

fn test_environment<const N: usize>(values: [(&str, &str); N]) -> TestEnvironment {
    TestEnvironment(BTreeMap::from(
        values.map(|(key, value)| (key.to_string(), value.to_string())),
    ))
}

#[test]
fn proxy_overlays_set_or_unset_environment_for_network() -> anyhow::Result<()> {
    let codex_home = tempfile::tempdir()?;
    write_overlay(
        codex_home.path(),
        ".env.10-proxy-on",
        r#"﻿# codex-env-if: {"type":"tcp_connect","from":"HTTPS_PROXY","timeout_ms":500}
HTTPS_PROXY=http://user:password@proxy.example.com:8080
HTTP_PROXY=http://proxy.example.com:8080
ALL_PROXY=http://proxy.example.com:8080
NO_PROXY=localhost,127.0.0.1,.example.com
"#,
    )?;
    write_overlay(
        codex_home.path(),
        ".env.20-proxy-off",
        r#"# codex-env-if: {"not":{"type":"tcp_connect","host":"proxy.example.com","port":8080,"timeout_ms":500}}
# codex-env-unset: ["HTTPS_PROXY","HTTP_PROXY","ALL_PROXY","NO_PROXY"]
"#,
    )?;

    let on_network_calls = Cell::new(0);
    let on_network_connector = |host: &str, port: u16, timeout: Duration| {
        on_network_calls.set(on_network_calls.get() + 1);
        assert_eq!(host, "proxy.example.com");
        assert_eq!(port, 8080);
        assert_eq!(timeout, Duration::from_millis(500));
        true
    };
    let mut on_network_environment = test_environment([
        ("HTTPS_PROXY", "http://stale.example.com:8080"),
        ("UNRELATED", "preserved"),
    ]);

    let warnings = load_conditional_dotenv_overlays(
        codex_home.path(),
        &mut on_network_environment,
        &on_network_connector,
    );

    assert_eq!(on_network_calls.get(), 2);
    assert_eq!(warnings, Vec::<String>::new());
    assert_eq!(
        on_network_environment,
        test_environment([
            ("ALL_PROXY", "http://proxy.example.com:8080"),
            ("HTTPS_PROXY", "http://user:password@proxy.example.com:8080"),
            ("HTTP_PROXY", "http://proxy.example.com:8080"),
            ("NO_PROXY", "localhost,127.0.0.1,.example.com"),
            ("UNRELATED", "preserved"),
        ])
    );

    let off_network_calls = Cell::new(0);
    let off_network_connector = |host: &str, port: u16, timeout: Duration| {
        off_network_calls.set(off_network_calls.get() + 1);
        assert_eq!(host, "proxy.example.com");
        assert_eq!(port, 8080);
        assert_eq!(timeout, Duration::from_millis(500));
        false
    };
    let mut off_network_environment = test_environment([
        ("ALL_PROXY", "http://inherited.example.com:8080"),
        ("HTTPS_PROXY", "http://inherited.example.com:8080"),
        ("HTTP_PROXY", "http://inherited.example.com:8080"),
        ("NO_PROXY", "localhost"),
        ("UNRELATED", "preserved"),
    ]);

    let warnings = load_conditional_dotenv_overlays(
        codex_home.path(),
        &mut off_network_environment,
        &off_network_connector,
    );

    assert_eq!(off_network_calls.get(), 2);
    assert_eq!(warnings, Vec::<String>::new());
    assert_eq!(
        off_network_environment,
        test_environment([("UNRELATED", "preserved")])
    );
    Ok(())
}

#[test]
fn overlay_filters_reserved_names_for_set_and_unset() -> anyhow::Result<()> {
    let codex_home = tempfile::tempdir()?;
    write_overlay(
        codex_home.path(),
        ".env.proxy",
        r#"# codex-env-if: {"type":"tcp_connect","host":"proxy.example.com","port":8080}
# codex-env-unset: ["ALL_PROXY","codex_internal"]
HTTP_PROXY=http://proxy.example.com:8080
codex_internal=changed
"#,
    )?;
    let mut environment = test_environment([
        ("ALL_PROXY", "http://old.example.com:8080"),
        ("codex_internal", "preserved"),
    ]);

    let warnings =
        load_conditional_dotenv_overlays(codex_home.path(), &mut environment, &|_, _, _| true);

    assert_eq!(warnings, Vec::<String>::new());
    assert_eq!(
        environment,
        test_environment([
            ("HTTP_PROXY", "http://proxy.example.com:8080"),
            ("codex_internal", "preserved"),
        ])
    );
    Ok(())
}

#[test]
fn invalid_timeout_fails_closed_without_blocking_valid_overlays() -> anyhow::Result<()> {
    let codex_home = tempfile::tempdir()?;
    write_overlay(codex_home.path(), ".env.example", "SHOULD_NOT_LOAD=1\n")?;
    write_overlay(
        codex_home.path(),
        ".env.10-timeout",
        r#"# codex-env-if: {"type":"tcp_connect","host":"proxy.example.com","port":8080,"timeout_ms":5001}
SHOULD_NOT_LOAD=2
"#,
    )?;
    write_overlay(
        codex_home.path(),
        ".env.20-maximum-timeout",
        r#"# codex-env-if: {"type":"tcp_connect","host":"proxy.example.com","port":8080,"timeout_ms":5000}
MAXIMUM_TIMEOUT=loaded
"#,
    )?;
    write_overlay(
        codex_home.path(),
        ".env.30-default-timeout",
        r#"# codex-env-if: {"type":"tcp_connect","host":"proxy.example.com","port":8080}
DEFAULT_TIMEOUT=loaded
"#,
    )?;
    let observed_timeouts = RefCell::new(Vec::new());
    let connector = |_: &str, _: u16, timeout: Duration| {
        observed_timeouts.borrow_mut().push(timeout);
        true
    };
    let mut environment = TestEnvironment::default();

    let warnings =
        load_conditional_dotenv_overlays(codex_home.path(), &mut environment, &connector);

    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains(".env.10-timeout"));
    assert_eq!(
        observed_timeouts.into_inner(),
        vec![Duration::from_millis(5000), Duration::from_millis(500)]
    );
    assert_eq!(
        environment,
        test_environment([("DEFAULT_TIMEOUT", "loaded"), ("MAXIMUM_TIMEOUT", "loaded"),])
    );
    Ok(())
}

#[test]
fn invalid_from_sources_fail_closed_without_connecting() -> anyhow::Result<()> {
    let codex_home = tempfile::tempdir()?;
    write_overlay(
        codex_home.path(),
        ".env.10-missing-source",
        r#"# codex-env-if: {"type":"tcp_connect","from":"HTTPS_PROXY"}
SHOULD_NOT_LOAD=1
"#,
    )?;
    write_overlay(
        codex_home.path(),
        ".env.20-invalid-source",
        r#"# codex-env-if: {"type":"tcp_connect","from":"HTTPS_PROXY"}
HTTPS_PROXY="not a url"
"#,
    )?;
    let connector_called = Cell::new(false);
    let connector = |_: &str, _: u16, _: Duration| {
        connector_called.set(true);
        true
    };
    let mut environment = TestEnvironment::default();

    let warnings =
        load_conditional_dotenv_overlays(codex_home.path(), &mut environment, &connector);

    assert!(!connector_called.get());
    assert_eq!(warnings.len(), 2);
    assert_eq!(environment, TestEnvironment::default());
    Ok(())
}

#[test]
fn directives_inside_multiline_values_are_not_scanned() -> anyhow::Result<()> {
    let codex_home = tempfile::tempdir()?;
    write_overlay(
        codex_home.path(),
        ".env.multiline",
        r#"# codex-env-if: {"type":"tcp_connect","host":"proxy.example.com","port":8080}
# codex-env-unset: ["HTTP_PROXY"]
MESSAGE='before
# codex-env-unset: ["ALL_PROXY"]
# codex-env-if: {"not":{"type":"tcp_connect","host":"other.example.com","port":8080}}
after'
"#,
    )?;
    let mut environment = test_environment([
        ("ALL_PROXY", "http://proxy.example.com:8080"),
        ("HTTP_PROXY", "http://proxy.example.com:8080"),
    ]);

    let warnings =
        load_conditional_dotenv_overlays(codex_home.path(), &mut environment, &|_, _, _| true);

    assert_eq!(warnings, Vec::<String>::new());
    assert_eq!(
        environment,
        test_environment([
            ("ALL_PROXY", "http://proxy.example.com:8080"),
            (
                "MESSAGE",
                "before\n# codex-env-unset: [\"ALL_PROXY\"]\n# codex-env-if: {\"not\":{\"type\":\"tcp_connect\",\"host\":\"other.example.com\",\"port\":8080}}\nafter",
            ),
        ])
    );
    Ok(())
}

#[test]
fn endpoint_parser_supports_urls_bare_authorities_and_known_default_ports() {
    assert_eq!(
        [
            "http://user:password@proxy.example.com:8080",
            "proxy.example.com:8080",
            "https://proxy.example.com",
            "http://[::1]:8080",
        ]
        .map(parse_endpoint),
        [
            Some(("proxy.example.com".to_string(), 8080)),
            Some(("proxy.example.com".to_string(), 8080)),
            Some(("proxy.example.com".to_string(), 443)),
            Some(("::1".to_string(), 8080)),
        ]
    );
}
