use crate::BrowserUseRequirementsToml;
use crate::ConfigRequirementsToml;
use pretty_assertions::assert_eq;

#[test]
fn webmcp_requirements_preserve_explicit_values_and_omission() {
    for (contents, expected_browser_use, expected_empty) in [
        ("", None, true),
        ("[browser_use]", Some(None), true),
        (
            "[browser_use]\nallow_webmcp = true",
            Some(Some(true)),
            false,
        ),
        (
            "[browser_use]\nallow_webmcp = false",
            Some(Some(false)),
            false,
        ),
    ] {
        let requirements: ConfigRequirementsToml =
            toml::from_str(contents).expect("parse managed WebMCP policy");
        assert_eq!(
            requirements,
            ConfigRequirementsToml {
                browser_use: expected_browser_use.map(|allow_webmcp| BrowserUseRequirementsToml {
                    allow_webmcp,
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        assert_eq!(requirements.is_empty(), expected_empty, "{contents}");
    }
}

#[test]
fn webmcp_requirements_reject_non_booleans() {
    for value in ["\"true\"", "1", "[]"] {
        let contents = format!("[browser_use]\nallow_webmcp = {value}");
        let error = toml::from_str::<ConfigRequirementsToml>(&contents)
            .expect_err("WebMCP policy must be a boolean");
        assert!(error.to_string().contains("allow_webmcp"));
    }
}

#[test]
fn extension_headers_preserve_templates_and_explicit_disablement() {
    for (value, expected) in [
        ("[]", vec![]),
        (
            r#"[{name = "x-browser-agent", value = "ChatGPT/{{session_id}}"}]"#,
            vec![crate::RequestHeaderToml {
                name: "x-browser-agent".into(),
                value: "ChatGPT/{{session_id}}".into(),
            }],
        ),
    ] {
        let requirements: ConfigRequirementsToml = toml::from_str(&format!(
            "[browser_use.extension]\nrequest_headers = {value}"
        ))
        .unwrap();
        assert!(!requirements.is_empty());
        assert_eq!(
            requirements
                .browser_use
                .unwrap()
                .extension
                .unwrap()
                .request_headers,
            Some(expected)
        );
    }
}

#[test]
fn extension_headers_reject_malformed_entries() {
    for value in [
        "true",
        r#"[{name = "x-test"}]"#,
        r#"[{name = "x-test", value = 1}]"#,
        r#"[{name = "x-test", value = "ok", typo = true}]"#,
    ] {
        assert!(
            toml::from_str::<ConfigRequirementsToml>(&format!(
                "[browser_use.extension]\nrequest_headers = {value}"
            ))
            .is_err()
        );
    }
}

#[test]
fn extension_headers_remain_scoped_to_extension() {
    let extension = r#"[browser_use.extension]
request_headers = [{ name = "x-extension", value = "{{session_id}}" }]"#;
    let other = r#"[browser_use.iab]
request_headers = [{ name = "x-iab", value = "future" }]"#;
    let expected: ConfigRequirementsToml = toml::from_str(extension).unwrap();
    let combined: ConfigRequirementsToml =
        toml::from_str(&format!("{extension}\n{other}")).unwrap();
    assert_eq!(combined, expected);
    for contents in [other, "[browser_use.extension]"] {
        let requirements: ConfigRequirementsToml = toml::from_str(contents).unwrap();
        assert!(requirements.is_empty());
    }
}

#[test]
fn request_header_debug_redacts_values_in_parent_requirements() {
    let requirements: ConfigRequirementsToml = toml::from_str(
        r#"[browser_use.extension]
request_headers = [{ name = "x-company-token", value = "private-header-token" }]"#,
    )
    .unwrap();
    let header = &requirements
        .browser_use
        .as_ref()
        .unwrap()
        .extension
        .as_ref()
        .unwrap()
        .request_headers
        .as_ref()
        .unwrap()[0];
    assert_eq!(header.value, "private-header-token");
    assert_eq!(
        format!("{header:?}"),
        r#"RequestHeaderToml { name: "x-company-token", value: "[REDACTED]" }"#
    );
    for debug in [format!("{requirements:?}"), format!("{requirements:#?}")] {
        assert!(!debug.contains("private-header-token"));
        assert!(debug.contains("x-company-token"));
        assert!(debug.contains("[REDACTED]"));
    }
}
