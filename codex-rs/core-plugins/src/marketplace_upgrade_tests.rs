use super::*;
use crate::PluginGitMode;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerSource;
use codex_config::ConfigRequirements;
use codex_config::ConfigRequirementsToml;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::path::PathBuf;
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
fn automatic_marketplace_git_ignores_inherited_repository_configuration() {
    const CHILD_HOME: &str = "CODEX_MARKETPLACE_GIT_ISOLATION_CHILD_HOME";
    const CHILD_SOURCE: &str = "CODEX_MARKETPLACE_GIT_ISOLATION_CHILD_SOURCE";

    if let Some(codex_home) = std::env::var_os(CHILD_HOME) {
        let codex_home = PathBuf::from(codex_home);
        let source = std::env::var(CHILD_SOURCE).expect("read configured marketplace source");
        let config =
            format!("[marketplaces.trusted]\nsource_type = \"git\"\nsource = {source:?}\n");
        std::fs::write(codex_home.join(CONFIG_TOML_FILE), &config).expect("write config");
        let stack = config_layer_stack(&codex_home, &config);
        let outcome = upgrade_configured_git_marketplaces_with_mode(
            &codex_home,
            &stack,
            /*marketplace_name*/ None,
            PluginGitMode::Automatic,
        );
        assert_eq!(
            outcome,
            ConfiguredMarketplaceUpgradeOutcome {
                selected_marketplaces: vec!["trusted".to_string()],
                upgraded_roots: vec![
                    AbsolutePathBuf::try_from(
                        marketplace_install_root(&codex_home).join("trusted")
                    )
                    .expect("installed marketplace root"),
                ],
                errors: Vec::new(),
            }
        );
        for (url, mode) in [
            ("global:marketplace", PluginGitMode::Automatic),
            ("manual:marketplace", PluginGitMode::Manual),
        ] {
            let materialized = crate::loader::materialize_marketplace_plugin_source_with_mode(
                &codex_home,
                &crate::marketplace::MarketplacePluginSource::Git {
                    url: url.to_string(),
                    path: None,
                    ref_name: matches!(mode, PluginGitMode::Manual)
                        .then(|| "manual-filter".to_string()),
                    sha: None,
                },
                mode,
            )
            .expect("materialize automatic or manually installed Git plugin");
            if matches!(mode, PluginGitMode::Manual) {
                assert!(
                    std::fs::read_to_string(materialized.path.as_path().join("manual.txt"))
                        .expect("read manually filtered checkout")
                        .starts_with("git version ")
                );
            }
        }
        return;
    }

    let root = TempDir::new().expect("create temporary directory");
    let project = root.path().join("project");
    let codex_home = project.join("codex-home");
    let remote = root.path().join("remote");
    std::fs::create_dir_all(&codex_home).expect("create Codex home");
    std::fs::create_dir_all(&remote).expect("create remote marketplace");
    init_marketplace_repo(&remote, "trusted");
    run_git(&remote, &["switch", "--create", "manual-filter"]);
    std::fs::write(
        remote.join(".gitattributes"),
        "manual.txt filter=codex-required\n",
    )
    .expect("write required manual checkout filter");
    std::fs::write(remote.join("manual.txt"), "manual checkout")
        .expect("write manual checkout fixture");
    run_git(&remote, &["add", "."]);
    run_git(
        &remote,
        &["commit", "-m", "add required manual checkout filter"],
    );
    run_git(&remote, &["switch", "-"]);
    run_git(&project, &["init", "--quiet"]);

    let source = url::Url::from_directory_path(&remote)
        .expect("remote marketplace URL")
        .to_string();
    let untrusted = url::Url::from_directory_path(root.path().join("missing"))
        .expect("malicious replacement URL")
        .to_string();
    let rewrite_key = format!("url.{untrusted}.insteadOf");
    run_git(&project, &["config", &rewrite_key, &source]);
    std::fs::write(
        root.path().join("global.conf"),
        "[url \"../remote\"]\n\tinsteadOf = global:marketplace\n[protocol \"file\"]\n\tallow = always\n",
    )
    .expect("write global Git configuration");
    std::fs::write(
        root.path().join("system.conf"),
        "[protocol \"file\"]\n\tallow = never\n",
    )
    .expect("write system Git configuration");
    let output = Command::new(std::env::current_exe().expect("locate test binary"))
        .args([
            "--exact",
            "marketplace_upgrade::tests::automatic_marketplace_git_ignores_inherited_repository_configuration",
            "--nocapture",
        ])
        .current_dir(&project)
        .env(CHILD_HOME, &codex_home)
        .env(CHILD_SOURCE, &source)
        .env("GIT_CONFIG_GLOBAL", "../global.conf")
        .env("GIT_CONFIG_SYSTEM", "../system.conf")
        .env("GIT_DIR", project.join(".git"))
        .env("GIT_CONFIG_COUNT", "3")
        .env("GIT_CONFIG_KEY_0", &rewrite_key)
        .env("GIT_CONFIG_VALUE_0", &source)
        .env("GIT_CONFIG_KEY_1", "url.../remote.insteadOf")
        .env("GIT_CONFIG_VALUE_1", "manual:marketplace")
        .env("GIT_CONFIG_KEY_2", "filter.codex-required.smudge")
        .env("GIT_CONFIG_VALUE_2", "git version")
        .output()
        .expect("run marketplace Git isolation regression");
    assert!(
        output.status.success(),
        "marketplace Git isolation failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
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
        PluginGitMode::Manual,
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
        PluginGitMode::Manual,
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
