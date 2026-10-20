use super::*;
use crate::PluginGitMode;
use codex_config::CONFIG_TOML_FILE;
use codex_config::LoaderOverrides;
use codex_config::loader::load_config_layers_state;
use codex_exec_server::LOCAL_FS;
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
        read_configured_git_marketplace(&config_reloader(codex_home.path()), "good")
            .expect("read configured marketplace"),
        Some(ConfiguredGitMarketplace {
            name: "good".to_string(),
            source: "https://github.com/example/good.git".to_string(),
            ref_name: Some("main".to_string()),
            sparse_paths: vec!["plugins".to_string()],
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
    let reload_config = config_reloader(codex_home.path());
    let stack = reload_config().expect("load config");

    let outcome = upgrade_configured_git_marketplaces(
        codex_home.path(),
        &stack,
        /*marketplace_name*/ None,
        &reload_config,
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
    assert_eq!(
        std::fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE)).unwrap(),
        config
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
        let reload_config = config_reloader(&codex_home);
        let stack = reload_config().expect("load config");
        let outcome = upgrade_configured_git_marketplaces_with_mode(
            &codex_home,
            &stack,
            /*marketplace_name*/ None,
            PluginGitMode::Automatic,
            &reload_config,
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
        &config_reloader(codex_home.path()),
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
        &config_reloader(codex_home.path()),
        Some(&normalized_source),
        PluginGitMode::Manual,
    )
    .expect_err("mismatched marketplace name must not use the up-to-date fast path");

    assert!(err.contains("git clone marketplace source failed"));
    assert_eq!(
        std::fs::read_dir(install_root.join(".staging"))
            .expect("read marketplace staging directory")
            .count(),
        0
    );
}

#[test]
fn stale_activation_restores_newer_concurrently_installed_marketplace() {
    const INITIAL_REVISION: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const STALE_REVISION: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const NEWER_REVISION: &str = "cccccccccccccccccccccccccccccccccccccccc";

    let codex_home = TempDir::new().expect("create Codex home");
    let install_root = marketplace_install_root(codex_home.path());
    let destination = install_root.join("good");
    std::fs::create_dir_all(&destination).expect("create installed marketplace root");
    let marketplace = ConfiguredGitMarketplace {
        name: "good".to_string(),
        source: "https://github.com/example/good.git".to_string(),
        ref_name: Some("main".to_string()),
        sparse_paths: Vec::new(),
    };
    super::activation::write_installed_marketplace_metadata(
        &destination,
        &marketplace,
        INITIAL_REVISION,
    )
    .expect("write initial installed marketplace metadata");
    let previous_snapshot =
        super::activation::read_installed_marketplace_snapshot(&destination, &marketplace.name);

    std::fs::write(destination.join("marker.txt"), "newer snapshot")
        .expect("write newer installed marketplace snapshot");
    super::activation::write_installed_marketplace_metadata(
        &destination,
        &marketplace,
        NEWER_REVISION,
    )
    .expect("write newer installed marketplace metadata");

    let staged_dir =
        MarketplaceStagingDir::new(&install_root).expect("create stale upgrade staging directory");
    std::fs::create_dir_all(staged_dir.path()).expect("create stale upgrade checkout");
    std::fs::write(staged_dir.path().join("marker.txt"), "stale snapshot")
        .expect("write stale staged marketplace snapshot");
    super::activation::write_installed_marketplace_metadata(
        staged_dir.path(),
        &marketplace,
        STALE_REVISION,
    )
    .expect("write stale staged marketplace metadata");

    let err = super::activation::activate_marketplace_root(
        &destination,
        staged_dir.path(),
        &previous_snapshot,
        || Ok(()),
    )
    .expect_err("stale upgrade must not replace a newer installed snapshot");

    assert_eq!(
        err,
        "installed marketplace `good` changed while auto-upgrade was in flight"
    );
    assert_eq!(
        std::fs::read_to_string(destination.join("marker.txt"))
            .expect("read restored marketplace snapshot"),
        "newer snapshot"
    );
    let restored_snapshot =
        super::activation::read_installed_marketplace_snapshot(&destination, &marketplace.name);
    assert!(super::activation::installed_marketplace_metadata_matches(
        &restored_snapshot,
        &marketplace,
        NEWER_REVISION
    ));
    drop(staged_dir);
    assert_eq!(
        std::fs::read_dir(install_root.join(".staging"))
            .expect("read marketplace staging directory")
            .count(),
        0
    );
}

fn config_reloader(codex_home: &Path) -> ConfigLayerReload {
    let codex_home = codex_home.to_path_buf();
    let options = LoaderOverrides {
        system_config_path: Some(codex_home.join("system.toml")),
        managed_config_path: Some(codex_home.join("managed.toml")),
        system_requirements_path: Some(codex_home.join("requirements.toml")),
        ..LoaderOverrides::without_managed_config_for_tests()
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("create config loading runtime");
    Arc::new(move || {
        runtime.block_on(load_config_layers_state(
            LOCAL_FS.as_ref(),
            &codex_home,
            Some(AbsolutePathBuf::try_from(codex_home.join("project"))?),
            &[],
            options.clone(),
            &codex_config::NoopThreadConfigLoader,
        ))
    })
}

fn system_marketplace_stack(codex_home: &Path, system: &str, user: &str) -> ConfigLayerStack {
    let system_file = AbsolutePathBuf::try_from(codex_home.join("system.toml")).unwrap();
    std::fs::write(&system_file, system).unwrap();
    if !user.is_empty() {
        std::fs::write(codex_home.join(CONFIG_TOML_FILE), user).unwrap();
    }
    config_reloader(codex_home)().expect("load system and user config")
}

#[test]
fn system_marketplace_downloads_and_updates_without_writing_user_config() {
    for user_config in ["", "[marketplaces.good]\nsparse_paths = []\n"] {
        let codex_home = TempDir::new().unwrap();
        let remote = TempDir::new().unwrap();
        init_marketplace_repo(remote.path(), "good");
        run_git(remote.path(), &["checkout", "-b", "release"]);
        std::fs::write(remote.path().join("marker"), "release").unwrap();
        run_git(remote.path(), &["add", "."]);
        run_git(remote.path(), &["commit", "-m", "release"]);
        run_git(remote.path(), &["checkout", "-b", "other"]);
        std::fs::write(remote.path().join("marker"), "wrong branch").unwrap();
        run_git(remote.path(), &["commit", "-am", "other"]);
        let source = url::Url::from_directory_path(remote.path())
            .unwrap()
            .to_string();
        let system = format!(
            "[marketplaces.good]\nsource_type = \"git\"\nsource = {source:?}\nref = \"release\"\n"
        );
        let stack = system_marketplace_stack(codex_home.path(), &system, user_config);
        let reload_config = config_reloader(codex_home.path());
        let root =
            AbsolutePathBuf::try_from(marketplace_install_root(codex_home.path()).join("good"))
                .unwrap();
        for marker in ["release", "updated"] {
            if marker == "updated" {
                run_git(remote.path(), &["checkout", "release"]);
                std::fs::write(remote.path().join("marker"), marker).unwrap();
                run_git(remote.path(), &["commit", "-am", "update release"]);
            }
            assert_eq!(
                upgrade_configured_git_marketplaces_with_mode(
                    codex_home.path(),
                    &stack,
                    Some("good"),
                    PluginGitMode::Automatic,
                    &reload_config,
                ),
                ConfiguredMarketplaceUpgradeOutcome {
                    selected_marketplaces: vec!["good".to_string()],
                    upgraded_roots: vec![root.clone()],
                    errors: Vec::new(),
                },
            );
            assert_eq!(
                std::fs::read_to_string(root.join("marker")).unwrap(),
                marker
            );
        }
        assert_eq!(
            std::fs::read_to_string(codex_home.path().join("system.toml")).unwrap(),
            system
        );
        let user_file = codex_home.path().join(CONFIG_TOML_FILE);
        if user_config.is_empty() {
            assert!(!user_file.exists());
        } else {
            assert_eq!(std::fs::read_to_string(user_file).unwrap(), user_config);
        }
    }
}

#[test]
fn changed_config_rolls_back_marketplace_activation() {
    let codex_home = TempDir::new().unwrap();
    let remote = TempDir::new().unwrap();
    init_marketplace_repo(remote.path(), "good");
    let source = url::Url::from_directory_path(remote.path())
        .unwrap()
        .to_string();
    let system = format!("[marketplaces.good]\nsource_type = \"git\"\nsource = {source:?}\n");
    let stack = system_marketplace_stack(codex_home.path(), &system, "");
    let reload_config = config_reloader(codex_home.path());
    let initial = upgrade_configured_git_marketplaces(
        codex_home.path(),
        &stack,
        Some("good"),
        &reload_config,
    );
    assert!(initial.all_succeeded());
    let root = &initial.upgraded_roots[0];
    let original_metadata = std::fs::read(root.join(".codex-marketplace-install.json")).unwrap();
    std::fs::write(remote.path().join("new-file"), "new revision").unwrap();
    run_git(remote.path(), &["add", "."]);
    run_git(remote.path(), &["commit", "-m", "update"]);
    for (file, changed_config) in [
        ("system.toml", format!("{system}ref = \"different\"\n")),
        ("system.toml", String::new()),
        (
            CONFIG_TOML_FILE,
            "[marketplaces.good]\nref = \"user-override\"\n".to_string(),
        ),
        (CONFIG_TOML_FILE, "invalid = [".to_string()),
    ] {
        std::fs::write(codex_home.path().join("system.toml"), &system).unwrap();
        std::fs::write(codex_home.path().join(file), changed_config).unwrap();
        let outcome = upgrade_configured_git_marketplaces(
            codex_home.path(),
            &stack,
            Some("good"),
            &reload_config,
        );
        assert_eq!(outcome.upgraded_roots, Vec::new());
        assert_eq!(outcome.errors.len(), 1);
        assert!(!root.join("new-file").exists());
        assert_eq!(
            std::fs::read(root.join(".codex-marketplace-install.json")).unwrap(),
            original_metadata
        );
    }
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
