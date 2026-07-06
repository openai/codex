use super::*;
use crate::marketplace_upgrade::upgrade_configured_git_marketplaces;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerSource;
use codex_config::RequirementSource;
use codex_config::RequirementsLayerEntry;
use codex_config::compose_requirements;
use pretty_assertions::assert_eq;
use std::fs;
use tempfile::TempDir;

fn config_layer_stack(requirements_toml: &str) -> ConfigLayerStack {
    config_layer_stack_with_user_config(requirements_toml, /*user_config*/ None)
}

fn config_layer_stack_with_user_config(
    requirements_toml: &str,
    user_config: Option<(&str, AbsolutePathBuf)>,
) -> ConfigLayerStack {
    let with_sources = compose_requirements([RequirementsLayerEntry::from_toml(
        RequirementSource::Unknown,
        requirements_toml,
    )])
    .expect("compose requirements")
    .expect("requirements should be present");
    let requirements_toml = with_sources.clone().into_toml();
    let requirements =
        codex_config::ConfigRequirements::try_from(with_sources).expect("normalize requirements");
    let layers = user_config
        .map(|(contents, file)| {
            vec![ConfigLayerEntry::new(
                ConfigLayerSource::User {
                    file,
                    profile: None,
                },
                toml::from_str(contents).expect("parse user config"),
            )]
        })
        .unwrap_or_default();
    ConfigLayerStack::new(layers, requirements, requirements_toml)
        .expect("build config layer stack")
}

fn parse_source(source: &str, ref_name: Option<&str>) -> MarketplaceSource {
    parse_marketplace_source(source, ref_name.map(str::to_string)).expect("parse source")
}

fn validate_source(stack: &ConfigLayerStack, source: &MarketplaceSource) -> Result<(), String> {
    MarketplacePolicy::from_requirements(stack.requirements()).validate_source(source)
}

#[test]
fn exact_git_rule_matches_url_and_ref() {
    let stack = config_layer_stack(
        r#"
[marketplaces]
restrict_to_allowed_sources = true

[marketplaces.allowed_sources.company]
source = "git"
url = "https://github.com/example/plugins"
ref = "main"
"#,
    );

    assert_eq!(
        validate_source(
            &stack,
            &parse_source("https://github.com/example/plugins.git", Some("main")),
        ),
        Ok(())
    );
    for denied in [
        parse_source("https://github.com/example/plugins.git", Some("release")),
        parse_source("https://github.com/other/plugins.git", Some("main")),
    ] {
        assert!(validate_source(&stack, &denied).is_err());
    }
    let normalized = MarketplacePolicy::from_requirements(stack.requirements())
        .validate_git_source("example/plugins", Some("main".to_string()))
        .expect("allowlisted shorthand should validate")
        .expect("restricted policy should normalize the source");
    assert_eq!(
        normalized,
        MarketplaceSource::Git {
            url: "https://github.com/example/plugins.git".to_string(),
            ref_name: Some("main".to_string()),
        }
    );
}

#[test]
fn git_rule_without_ref_allows_any_ref_for_the_same_repository() {
    let stack = config_layer_stack(
        r#"
[marketplaces]
restrict_to_allowed_sources = true

[marketplaces.allowed_sources.company]
source = "git"
url = "https://github.com/example/plugins"
"#,
    );

    assert_eq!(
        validate_source(
            &stack,
            &parse_source("https://github.com/example/plugins.git", Some("release")),
        ),
        Ok(())
    );
}

#[test]
fn git_host_pattern_matches_https_and_ssh_sources() {
    let stack = config_layer_stack(
        r#"
[marketplaces]
restrict_to_allowed_sources = true

[marketplaces.allowed_sources.internal]
source = "host_pattern"
host_pattern = '^git\.example\.com$'
url = "https://github.com/example/ignored.git"
ref = "ignored"
"#,
    );

    for source in [
        "https://git.example.com/team/plugins.git",
        "ssh://git@git.example.com/team/plugins.git",
        "git@git.example.com:team/plugins.git",
    ] {
        assert_eq!(
            validate_source(&stack, &parse_source(source, /*ref_name*/ None)),
            Ok(())
        );
    }
    assert!(
        validate_source(
            &stack,
            &parse_source(
                "https://github.com/example/plugins.git",
                /*ref_name*/ None,
            ),
        )
        .is_err()
    );
}

#[test]
fn exact_local_rule_rejects_other_directories() {
    let allowed = TempDir::new().expect("create allowed marketplace directory");
    let denied = TempDir::new().expect("create denied marketplace directory");
    let allowed = allowed
        .path()
        .canonicalize()
        .expect("canonical allowed path");
    let denied = denied.path().canonicalize().expect("canonical denied path");
    let stack = config_layer_stack(&format!(
        r#"
[marketplaces]
restrict_to_allowed_sources = true

[marketplaces.allowed_sources.local]
source = "local"
path = {allowed:?}
"#
    ));

    assert_eq!(
        validate_source(
            &stack,
            &parse_source(allowed.to_string_lossy().as_ref(), /*ref_name*/ None),
        ),
        Ok(())
    );
    assert!(
        validate_source(
            &stack,
            &parse_source(denied.to_string_lossy().as_ref(), /*ref_name*/ None),
        )
        .is_err()
    );
}

#[test]
fn restriction_flag_controls_empty_allowlist() {
    for (restricted, expected_allowed) in [(true, false), (false, true)] {
        let stack = config_layer_stack(&format!(
            r#"
[marketplaces]
restrict_to_allowed_sources = {restricted}
"#
        ));
        let result = validate_source(
            &stack,
            &parse_source(
                "https://github.com/example/plugins.git",
                /*ref_name*/ None,
            ),
        );
        assert_eq!(result.is_ok(), expected_allowed);
    }
}

#[test]
fn strict_install_validates_configured_name_source_and_root() {
    let codex_home = TempDir::new().expect("create Codex home");
    let configured_root = TempDir::new().expect("create configured marketplace");
    let other_root = TempDir::new().expect("create other marketplace");
    let configured_root = configured_root
        .path()
        .canonicalize()
        .expect("canonical configured root");
    let other_root = other_root
        .path()
        .canonicalize()
        .expect("canonical other root");
    let config_file = AbsolutePathBuf::try_from(codex_home.path().join("config.toml"))
        .expect("absolute config path");
    let stack = config_layer_stack_with_user_config(
        &format!(
            r#"
[marketplaces]
restrict_to_allowed_sources = true

[marketplaces.allowed_sources.company]
source = "local"
path = {configured_root:?}
"#
        ),
        Some((
            &format!(
                r#"
[marketplaces.company]
source_type = "local"
source = {configured_root:?}
"#
            ),
            config_file,
        )),
    );
    let policy = MarketplacePolicy::from_requirements(stack.requirements());
    let configured_path =
        AbsolutePathBuf::try_from(configured_root.join(".agents/plugins/marketplace.json"))
            .expect("configured marketplace path");
    let other_path = AbsolutePathBuf::try_from(other_root.join(".agents/plugins/marketplace.json"))
        .expect("other marketplace path");

    assert_eq!(
        policy.validate_install(&stack, codex_home.path(), &configured_path, "company"),
        Ok(())
    );
    assert!(
        policy
            .validate_install(&stack, codex_home.path(), &configured_path, "other")
            .expect_err("unconfigured name should fail")
            .contains("must be added to config")
    );
    assert!(
        policy
            .validate_install(&stack, codex_home.path(), &other_path, "company")
            .expect_err("mismatched root should fail")
            .contains("does not match configured marketplace")
    );
}

#[test]
fn blocked_configured_source_is_not_installable() {
    let codex_home = TempDir::new().expect("create Codex home");
    let config_file = AbsolutePathBuf::try_from(codex_home.path().join("config.toml"))
        .expect("absolute config path");
    let stack = config_layer_stack_with_user_config(
        r#"
[marketplaces]
restrict_to_allowed_sources = true

[marketplaces.allowed_sources.company]
source = "git"
url = "https://github.com/example/allowed.git"
"#,
        Some((
            r#"
[marketplaces.debug]
source_type = "git"
source = "https://github.com/example/blocked.git"
"#,
            config_file,
        )),
    );
    let marketplace_path = AbsolutePathBuf::try_from(
        marketplace_install_root(codex_home.path()).join("debug/.agents/plugins/marketplace.json"),
    )
    .expect("absolute marketplace path");

    let err = MarketplacePolicy::from_requirements(stack.requirements())
        .validate_install(&stack, codex_home.path(), &marketplace_path, "debug")
        .expect_err("blocked marketplace install should fail");
    assert!(err.contains("is not allowed by requirements"));
}

#[test]
fn bare_relative_local_config_source_is_not_parsed_as_git_shorthand() {
    let marketplace: toml::Value = toml::from_str(
        r#"
source_type = "local"
source = "marketplaces/company"
"#,
    )
    .expect("parse marketplace config");

    assert_eq!(
        configured_marketplace_source("company", &marketplace),
        Ok(MarketplaceSource::Local {
            path: PathBuf::from("marketplaces/company"),
        })
    );
}

#[test]
fn curated_marketplace_requires_its_expected_name() {
    let codex_home = TempDir::new().expect("create Codex home");
    let stack = config_layer_stack(
        r#"
[marketplaces]
restrict_to_allowed_sources = true
"#,
    );
    let marketplace_path = AbsolutePathBuf::try_from(
        curated_plugins_repo_path(codex_home.path()).join(".agents/plugins/marketplace.json"),
    )
    .expect("absolute marketplace path");
    let policy = MarketplacePolicy::from_requirements(stack.requirements());

    assert_eq!(
        policy.validate_install(
            &stack,
            codex_home.path(),
            &marketplace_path,
            crate::OPENAI_CURATED_MARKETPLACE_NAME,
        ),
        Ok(())
    );
    assert!(
        policy
            .validate_install(
                &stack,
                codex_home.path(),
                &marketplace_path,
                crate::OPENAI_API_CURATED_MARKETPLACE_NAME,
            )
            .is_err()
    );
}

#[test]
fn managed_bundled_source_is_bound_to_its_expected_name() {
    let codex_home = TempDir::new().expect("create Codex home");
    let bundled_root = codex_home
        .path()
        .join(".tmp/bundled-marketplaces")
        .join(crate::OPENAI_BUNDLED_MARKETPLACE_NAME);
    fs::create_dir_all(&bundled_root).expect("create bundled marketplace root");
    let stack = config_layer_stack(
        r#"
[marketplaces]
restrict_to_allowed_sources = true
"#,
    );
    let source = parse_source(
        bundled_root.to_string_lossy().as_ref(),
        /*ref_name*/ None,
    );

    let expected_name =
        validate_marketplace_source_for_add(codex_home.path(), stack.requirements(), &source)
            .expect("managed marketplace source should bypass restrictions");
    assert_eq!(
        validate_marketplace_name_for_add(expected_name, crate::OPENAI_BUNDLED_MARKETPLACE_NAME,),
        Ok(())
    );
    assert!(validate_marketplace_name_for_add(expected_name, "other").is_err());
}

#[test]
fn projected_user_config_removes_blocked_marketplaces_and_plugins() {
    let codex_home = TempDir::new().expect("create Codex home");
    let config_file = AbsolutePathBuf::try_from(codex_home.path().join("config.toml"))
        .expect("absolute config path");
    let stack = config_layer_stack_with_user_config(
        r#"
[marketplaces]
restrict_to_allowed_sources = true

[marketplaces.allowed_sources.company]
source = "git"
url = "https://github.com/example/allowed.git"
"#,
        Some((
            r#"
[marketplaces.allowed]
source_type = "git"
source = "https://github.com/example/allowed.git"

[marketplaces.blocked]
source_type = "git"
source = "https://github.com/example/blocked.git"

[plugins."sample@allowed"]
enabled = true

[plugins."sample@blocked"]
enabled = true
"#,
            config_file,
        )),
    );

    let projected =
        project_effective_user_config(&stack, codex_home.path()).expect("project user config");
    assert_eq!(
        projected["marketplaces"]
            .as_table()
            .expect("projected marketplaces")
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["allowed".to_string()]
    );
    assert_eq!(
        configured_plugins_from_stack(&stack, codex_home.path())
            .into_keys()
            .collect::<Vec<_>>(),
        vec!["sample@allowed".to_string()]
    );

    let raw = stack.effective_user_config().expect("raw user config");
    assert!(raw["marketplaces"]["blocked"].is_table());
    assert!(raw["plugins"]["sample@blocked"].is_table());
}

#[test]
fn managed_bundled_config_is_retained_only_at_its_owned_path() {
    let codex_home = TempDir::new().expect("create Codex home");
    let bundled_root = codex_home
        .path()
        .join(".tmp/bundled-marketplaces")
        .join(crate::OPENAI_BUNDLED_MARKETPLACE_NAME);
    let config_file = AbsolutePathBuf::try_from(codex_home.path().join("config.toml"))
        .expect("absolute config path");
    let stack = config_layer_stack_with_user_config(
        r#"
[marketplaces]
restrict_to_allowed_sources = true
"#,
        Some((
            &format!(
                r#"
[marketplaces.openai-bundled]
source_type = "local"
source = {bundled_root:?}

[marketplaces.openai-bundled-alpha]
source_type = "local"
source = "/tmp/not-managed"

[marketplaces.evil]
source_type = "local"
source = {bundled_root:?}

[plugins."sample@openai-bundled"]
enabled = true

[plugins."sample@openai-bundled-alpha"]
enabled = true

[plugins."sample@evil"]
enabled = true
"#
            ),
            config_file,
        )),
    );

    let projected =
        project_effective_user_config(&stack, codex_home.path()).expect("project user config");

    assert_eq!(
        projected["marketplaces"]
            .as_table()
            .expect("projected marketplaces")
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec![crate::OPENAI_BUNDLED_MARKETPLACE_NAME.to_string()]
    );
    assert_eq!(
        projected["plugins"]
            .as_table()
            .expect("projected plugins")
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec![format!("sample@{}", crate::OPENAI_BUNDLED_MARKETPLACE_NAME)]
    );
}

#[test]
fn allowlisted_config_names_are_not_globally_reserved() {
    let codex_home = TempDir::new().expect("create Codex home");
    let source_root = TempDir::new().expect("create marketplace root");
    let source_root = source_root
        .path()
        .canonicalize()
        .expect("canonical marketplace root");
    let config_file = AbsolutePathBuf::try_from(codex_home.path().join("config.toml"))
        .expect("absolute config path");
    let stack = config_layer_stack_with_user_config(
        &format!(
            r#"
[marketplaces]
restrict_to_allowed_sources = true

[marketplaces.allowed_sources.local]
source = "local"
path = {source_root:?}
"#
        ),
        Some((
            &format!(
                r#"
[marketplaces.openai-bundled]
source_type = "local"
source = {source_root:?}

[marketplaces.openai-curated]
source_type = "local"
source = {source_root:?}

[plugins."sample@openai-bundled"]
enabled = true

[plugins."sample@openai-curated"]
enabled = true
"#
            ),
            config_file,
        )),
    );

    let projected =
        project_effective_user_config(&stack, codex_home.path()).expect("project user config");
    assert_eq!(
        projected["marketplaces"]
            .as_table()
            .expect("projected marketplaces")
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["openai-bundled".to_string(), "openai-curated".to_string()]
    );
    assert_eq!(
        projected["plugins"]
            .as_table()
            .expect("projected plugins")
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec![
            "sample@openai-bundled".to_string(),
            "sample@openai-curated".to_string()
        ]
    );
}

#[test]
fn blocked_upgrade_is_rejected_before_marketplace_installation() {
    let codex_home = TempDir::new().expect("create Codex home");
    let config_file = AbsolutePathBuf::try_from(codex_home.path().join("config.toml"))
        .expect("absolute config path");
    let stack = config_layer_stack_with_user_config(
        r#"
[marketplaces]
restrict_to_allowed_sources = true
"#,
        Some((
            r#"
[marketplaces.debug]
source_type = "git"
source = "https://github.com/example/blocked.git"
"#,
            config_file,
        )),
    );

    let outcome = upgrade_configured_git_marketplaces(codex_home.path(), &stack, Some("debug"));

    assert_eq!(outcome.selected_marketplaces, vec!["debug".to_string()]);
    assert_eq!(outcome.upgraded_roots, Vec::new());
    assert_eq!(outcome.errors.len(), 1);
    assert!(
        outcome.errors[0]
            .message
            .contains("is not allowed by requirements")
    );
    assert!(!marketplace_install_root(codex_home.path()).exists());
}

#[test]
fn invalid_active_rule_fails_closed_even_when_another_rule_matches() {
    let stack = config_layer_stack(
        r#"
[marketplaces]
restrict_to_allowed_sources = true

[marketplaces.allowed_sources.allowed]
source = "git"
url = "https://github.com/example/plugins.git"

[marketplaces.allowed_sources.invalid]
source = "host_pattern"
host_pattern = "("
"#,
    );

    let err = validate_source(
        &stack,
        &parse_source(
            "https://github.com/example/plugins.git",
            /*ref_name*/ None,
        ),
    )
    .expect_err("invalid active rule should fail closed");
    assert!(err.contains("invalid marketplace allowed source `invalid`"));
}

#[test]
fn invalid_allowed_source_shapes_fail_closed() {
    for (rule, expected_error) in [
        (
            r#"
[marketplaces.allowed_sources.invalid]
url = "https://github.com/example/plugins.git"
"#,
            "missing source",
        ),
        (
            r#"
[marketplaces.allowed_sources.invalid]
source = "git"
"#,
            "missing url",
        ),
        (
            r#"
[marketplaces.allowed_sources.invalid]
source = "git"
url = "https://github.com/example/plugins.git"
ref = " "
"#,
            "ref must not be empty",
        ),
        (
            r#"
[marketplaces.allowed_sources.invalid]
source = "local"
path = "../plugins"
"#,
            "local path must be absolute",
        ),
    ] {
        let stack = config_layer_stack(&format!(
            r#"
[marketplaces]
restrict_to_allowed_sources = true
{rule}
"#
        ));

        let err = validate_source(
            &stack,
            &parse_source(
                "https://github.com/example/plugins.git",
                /*ref_name*/ None,
            ),
        )
        .expect_err("invalid rule should fail closed");
        assert!(err.contains(expected_error), "{err}");
    }
}
