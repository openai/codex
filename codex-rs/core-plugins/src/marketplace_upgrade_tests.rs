use super::*;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerSource;
use codex_config::ConfigRequirements;
use codex_config::ConfigRequirementsToml;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn readback_ignores_unrelated_malformed_marketplace() {
    let codex_home = TempDir::new().expect("create Codex home");
    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"
[marketplaces.bad]
source_type = "git"
source = 17

[marketplaces.good]
source_type = "git"
source = "https://github.com/example/good.git"
ref = "main"
sparse_paths = ["plugins"]
last_revision = "abc123"
"#,
    )
    .expect("write config");

    assert_eq!(
        read_configured_git_marketplace(codex_home.path(), "good")
            .expect("read configured marketplace"),
        Some(ConfiguredGitMarketplace {
            name: "good".to_string(),
            source: "https://github.com/example/good.git".to_string(),
            ref_name: Some("main".to_string()),
            sparse_paths: vec!["plugins".to_string()],
            last_revision: Some("abc123".to_string()),
        })
    );
}

#[test]
fn one_upgrade_failure_does_not_block_another_marketplace() {
    let codex_home = TempDir::new().expect("create Codex home");
    let remote_repo = TempDir::new().expect("create remote repository");
    init_marketplace_repo(remote_repo.path(), "good");
    let good_url = url::Url::from_directory_path(remote_repo.path())
        .expect("remote repository URL")
        .to_string();
    let missing_url = url::Url::from_directory_path(codex_home.path().join("missing-repository"))
        .expect("missing repository URL")
        .to_string();
    let config = format!(
        r#"
[marketplaces.bad]
source_type = "git"
source = {missing_url:?}

[marketplaces.good]
source_type = "git"
source = {good_url:?}
"#
    );
    std::fs::write(codex_home.path().join(CONFIG_TOML_FILE), &config).expect("write config");
    let stack = config_layer_stack(codex_home.path(), &config);

    let outcome = upgrade_configured_git_marketplaces(
        codex_home.path(),
        &stack,
        /*marketplace_name*/ None,
    );

    assert_eq!(
        outcome.selected_marketplaces,
        vec!["bad".to_string(), "good".to_string()]
    );
    assert_eq!(outcome.errors.len(), 1);
    assert_eq!(outcome.errors[0].marketplace_name, "bad");
    assert_eq!(
        outcome.upgraded_roots,
        vec![
            AbsolutePathBuf::try_from(marketplace_install_root(codex_home.path()).join("good"))
                .expect("installed marketplace root")
        ]
    );
}

#[test]
fn upgrade_uses_validated_source_for_git_operations() {
    let codex_home = TempDir::new().expect("create Codex home");
    let remote_repo = TempDir::new().expect("create remote repository");
    init_marketplace_repo(remote_repo.path(), "good");
    let normalized_url = url::Url::from_directory_path(remote_repo.path())
        .expect("remote repository URL")
        .to_string();
    let raw_source = codex_home.path().join("missing-raw-source");
    let raw_source = raw_source.to_string_lossy().into_owned();
    let config = format!(
        r#"
[marketplaces.good]
source_type = "git"
source = {raw_source:?}
ref = "missing-ref"
"#
    );
    std::fs::write(codex_home.path().join(CONFIG_TOML_FILE), config).expect("write config");
    let marketplace = ConfiguredGitMarketplace {
        name: "good".to_string(),
        source: raw_source,
        ref_name: Some("missing-ref".to_string()),
        sparse_paths: Vec::new(),
        last_revision: None,
    };
    let normalized_source = MarketplaceSource::Git {
        url: normalized_url,
        ref_name: Some("HEAD".to_string()),
    };
    let install_root = marketplace_install_root(codex_home.path());

    let upgraded_root = upgrade_configured_git_marketplace(
        codex_home.path(),
        &install_root,
        &marketplace,
        Some(&normalized_source),
    )
    .expect("upgrade should use the validated source")
    .expect("marketplace should be upgraded");

    assert_eq!(
        upgraded_root,
        AbsolutePathBuf::try_from(install_root.join("good")).expect("installed marketplace root")
    );
}

#[test]
fn up_to_date_fast_path_validates_marketplace_name() {
    const REVISION: &str = "0123456789abcdef0123456789abcdef01234567";
    let codex_home = TempDir::new().expect("create Codex home");
    let install_root = marketplace_install_root(codex_home.path());
    let destination = install_root.join("good");
    let manifest_dir = destination.join(".agents/plugins");
    std::fs::create_dir_all(&manifest_dir).expect("create marketplace manifest directory");
    std::fs::write(
        manifest_dir.join("marketplace.json"),
        r#"{"name":"wrong","plugins":[]}"#,
    )
    .expect("write mismatched marketplace manifest");
    let missing_source = codex_home.path().join("missing-source");
    let missing_source = missing_source.to_string_lossy().into_owned();
    let marketplace = ConfiguredGitMarketplace {
        name: "good".to_string(),
        source: missing_source.clone(),
        ref_name: Some(REVISION.to_string()),
        sparse_paths: Vec::new(),
        last_revision: Some(REVISION.to_string()),
    };
    super::activation::write_installed_marketplace_metadata(&destination, &marketplace, REVISION)
        .expect("write installed marketplace metadata");
    let normalized_source = MarketplaceSource::Git {
        url: missing_source,
        ref_name: Some(REVISION.to_string()),
    };

    let err = upgrade_configured_git_marketplace(
        codex_home.path(),
        &install_root,
        &marketplace,
        Some(&normalized_source),
    )
    .expect_err("mismatched marketplace name must not use the up-to-date fast path");

    assert!(err.contains("git clone marketplace source failed"));
}

fn config_layer_stack(codex_home: &Path, config: &str) -> ConfigLayerStack {
    let config_file =
        AbsolutePathBuf::try_from(codex_home.join(CONFIG_TOML_FILE)).expect("absolute config path");
    ConfigLayerStack::new(
        vec![ConfigLayerEntry::new(
            ConfigLayerSource::User {
                file: config_file,
                profile: None,
            },
            toml::from_str(config).expect("parse config"),
        )],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("build config layer stack")
}

fn init_marketplace_repo(repo: &Path, marketplace_name: &str) {
    let manifest_dir = repo.join(".agents/plugins");
    std::fs::create_dir_all(&manifest_dir).expect("create marketplace manifest directory");
    std::fs::write(
        manifest_dir.join("marketplace.json"),
        format!(r#"{{"name":"{marketplace_name}","plugins":[]}}"#),
    )
    .expect("write marketplace manifest");
    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "codex-test@example.com"]);
    run_git(repo, &["config", "user.name", "Codex Test"]);
    run_git(repo, &["add", "."]);
    run_git(repo, &["commit", "-m", "initial"]);
}

fn run_git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
