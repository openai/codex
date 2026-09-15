//! Covers complete-package staging, conservative selection, and legacy preservation.
#![cfg(unix)]
use super::InstallMode;
use super::prepare_from_package;
use super::validate_package;
use crate::settings::DaemonSettings;
use pretty_assertions::assert_eq;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;

fn daemon(home: &std::path::Path) -> crate::Daemon {
    let state = home.join("app-server-daemon");
    crate::Daemon {
        socket_path: state.join("app-server.sock"),
        pid_file: state.join("app-server.pid"),
        update_pid_file: state.join("app-server-updater.pid"),
        operation_lock_file: state.join("daemon.lock"),
        settings_file: state.join("settings.json"),
        managed_codex_bin: crate::managed_install::managed_codex_bin(home),
    }
}

fn package(root: &Path, version: &str) -> PathBuf {
    let target = super::platform_target().expect("target");
    for dir in ["bin", "codex-path", "codex-resources/nested"] {
        std::fs::create_dir_all(root.join(dir)).expect("package directory");
    }
    let bin = root.join("bin/codex");
    std::fs::write(&bin, format!("#!/bin/sh\necho 'codex {version}'\n")).expect("codex executable");
    for file in [
        "bin/codex-code-mode-host",
        "codex-path/rg",
        "codex-resources/nested/runtime",
    ] {
        std::fs::write(root.join(file), b"runtime").expect("package file");
        if file != "codex-resources/nested/runtime" {
            std::fs::set_permissions(root.join(file), std::fs::Permissions::from_mode(0o755))
                .expect("executable helper");
        }
    }
    if cfg!(target_os = "linux") {
        std::fs::write(root.join("codex-resources/bwrap"), b"runtime").expect("bwrap");
        std::fs::set_permissions(
            root.join("codex-resources/bwrap"),
            std::fs::Permissions::from_mode(0o755),
        )
        .expect("executable bwrap");
    }
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755))
        .expect("executable permission");
    std::fs::write(
        root.join("codex-package.json"),
        serde_json::json!({
            "version": version, "target": target, "entrypoint": "bin/codex"
        })
        .to_string(),
    )
    .expect("manifest");
    bin
}

#[tokio::test]
async fn seeds_full_package() {
    let temp = tempfile::TempDir::new().expect("temp");
    let home = temp.path().join("home");
    let daemon = daemon(&home);
    let settings = DaemonSettings::default();
    let old = temp.path().join("old");
    let old_bin = package(&old, "0.152.0");
    prepare_from_package(
        &daemon,
        &settings,
        InstallMode::Missing,
        Some(&old),
        &old_bin,
        |_| Ok(true),
    )
    .await
    .expect("seed");

    let standalone = home.join("packages/app-server-daemon");
    let selected = std::fs::canonicalize(standalone.join("current")).expect("selected");
    assert_eq!(
        std::fs::read(selected.join("codex-resources/nested/runtime")).expect("runtime"),
        b"runtime"
    );
    assert_eq!(
        std::fs::read_to_string(standalone.join("auto-update-version")).expect("marker"),
        selected.file_name().expect("name").to_string_lossy()
    );
    assert!(validate_package(&selected).is_ok());
}

#[tokio::test]
async fn incomplete_source_fails_without_selecting_it() {
    let temp = tempfile::TempDir::new().expect("temp");
    let source = temp.path().join("package");
    let bin = package(&source, "0.152.0");
    std::fs::remove_file(source.join("bin/codex-code-mode-host")).expect("remove helper");
    let home = temp.path().join("home");
    let error = prepare_from_package(
        &daemon(&home),
        &DaemonSettings::default(),
        InstallMode::Missing,
        Some(&source),
        &bin,
        |_| Ok(true),
    )
    .await
    .expect_err("incomplete package");
    assert!(error.to_string().contains("bin/codex-code-mode-host"));
    assert!(!home.join("packages/app-server-daemon/current").exists());
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn provisioned_macos_bundle_seeds_from_its_running_executable() {
    let temp = tempfile::TempDir::new().expect("temp");
    let source = temp.path().join("package");
    let launcher = package(&source, "0.1.0-internal-test.202609091200.1");
    std::fs::write(&launcher, b"#!/bin/sh\necho codex 0.0.0\n").expect("launcher");
    let bundle = source.join("CodexCLI.app/Contents/MacOS/codex");
    std::fs::create_dir_all(bundle.parent().expect("bundle parent")).expect("bundle dir");
    std::fs::write(&bundle, b"provisioned executable").expect("bundle executable");
    let home = temp.path().join("home");
    prepare_from_package(
        &daemon(&home),
        &DaemonSettings::default(),
        InstallMode::Missing,
        Some(&source),
        &bundle,
        |_| Ok(true),
    )
    .await
    .expect("seed provisioned bundle");
    let selected = std::fs::canonicalize(home.join("packages/app-server-daemon/current"))
        .expect("selected release");
    assert_eq!(
        std::fs::read(selected.join("CodexCLI.app/Contents/MacOS/codex"))
            .expect("bundled executable"),
        b"provisioned executable"
    );
    assert!(
        !home
            .join("packages/app-server-daemon/auto-update-version")
            .exists()
    );
}

#[tokio::test]
async fn legacy_selection_is_not_migrated_or_refreshed() {
    let temp = tempfile::TempDir::new().unwrap();
    let home = temp.path().join("home");
    let legacy = home.join("packages/standalone");
    let bin = package(&legacy.join("releases/old"), "0.150.0");
    std::os::unix::fs::symlink("releases/old", legacy.join("current")).unwrap();
    let state = home.join("app-server-daemon");
    std::fs::create_dir(&state).unwrap();
    std::fs::write(state.join("app-server.stderr.log"), b"").unwrap();
    let source = temp.path().join("new");
    let new_bin = package(&source, "0.160.0");
    prepare_from_package(
        &daemon(&home),
        &DaemonSettings::default(),
        InstallMode::Missing,
        Some(&source),
        &new_bin,
        |_| Ok(true),
    )
    .await
    .unwrap();
    assert_eq!(
        crate::managed_install::managed_codex_bin(&home)
            .canonicalize()
            .unwrap(),
        bin.canonicalize().unwrap()
    );
    assert!(!home.join("packages/app-server-daemon").exists());
}

#[tokio::test]
async fn standalone_seed_preserves_explicit_pin_or_latest_channel() {
    for follows_latest in [false, true] {
        let temp = tempfile::TempDir::new().unwrap();
        let home = temp.path().join("home");
        let standalone = home.join("packages/standalone");
        let source = standalone.join("releases/0.152.0-local-target");
        let bin = package(&source, "0.152.0");
        std::os::unix::fs::symlink(&source, standalone.join("current")).unwrap();
        if follows_latest {
            std::fs::write(
                standalone.join("auto-update-version"),
                "0.152.0-local-target",
            )
            .unwrap();
        }
        prepare_from_package(
            &daemon(&home),
            &DaemonSettings::default(),
            InstallMode::Missing,
            Some(&source),
            &bin,
            |_| Ok(true),
        )
        .await
        .unwrap();
        let root = home.join("packages/app-server-daemon");
        let selected = root.join("current").canonicalize().unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("auto-update-version")).ok(),
            follows_latest.then(|| selected.file_name().unwrap().to_string_lossy().into_owned())
        );
    }
}

#[tokio::test]
async fn explicit_selection_requires_unchanged_cli_and_pins_all_versions() {
    for legacy in [false, true] {
        let temp = tempfile::TempDir::new().unwrap();
        let home = temp.path().join("home");
        let source = temp.path().join("source");
        let bin = package(&source, "0.152.0");
        let settings = DaemonSettings::default();
        let initial = daemon(&home);
        prepare_from_package(
            &initial,
            &settings,
            InstallMode::Missing,
            Some(&source),
            &bin,
            |_| Ok(true),
        )
        .await
        .unwrap();
        if legacy {
            std::fs::rename(
                home.join("packages/app-server-daemon"),
                home.join("packages/standalone"),
            )
            .unwrap();
            let root = home.join("packages/standalone");
            let name = std::fs::read_to_string(root.join("auto-update-version")).unwrap();
            std::fs::remove_file(root.join("current")).unwrap();
            std::os::unix::fs::symlink(root.join("releases").join(name), root.join("current"))
                .unwrap();
            let state = home.join("app-server-daemon");
            std::fs::create_dir_all(&state).unwrap();
            std::fs::write(state.join("app-server.stderr.log"), b"").unwrap();
        }
        let initial_root = crate::managed_install::package_root(&home);
        let legacy_release = initial_root.join("current").canonicalize().unwrap();
        let legacy_marker = std::fs::read(initial_root.join("auto-update-version")).unwrap();
        let error = prepare_from_package(
            &daemon(&home),
            &settings,
            InstallMode::Replace,
            Some(&source),
            &bin,
            |_| {
                let mut rebuilt = std::fs::read(&bin)?;
                rebuilt.extend_from_slice(b"# rebuilt during confirmation\n");
                std::fs::write(&bin, rebuilt)?;
                Ok(true)
            },
        )
        .await
        .expect_err("reject a CLI rebuilt during confirmation");
        assert!(
            error
                .to_string()
                .contains("differs from the running executable")
        );
        assert_eq!(
            initial_root.join("current").canonicalize().unwrap(),
            legacy_release
        );
        assert_eq!(
            std::fs::read(initial_root.join("auto-update-version")).unwrap(),
            legacy_marker
        );
        let root = home.join("packages/app-server-daemon");
        let mut previous = legacy_release.clone();
        for version in [
            "0.153.0",
            "0.151.0",
            "0.151.0",
            "0.154.0-alpha.1",
            "0.0.0",
            "0.0.0",
        ] {
            let daemon = daemon(&home);
            let previous_root = crate::managed_install::package_root(&home);
            package(&source, version);
            std::fs::write(
                source.join("codex-resources/nested/runtime"),
                previous.to_string_lossy().as_bytes(),
            )
            .unwrap();
            let before = std::fs::read(previous.join("bin/codex")).unwrap();
            assert!(
                !prepare_from_package(
                    &daemon,
                    &settings,
                    InstallMode::Replace,
                    Some(&source),
                    &bin,
                    |request| {
                        assert_eq!(request.destination, root);
                        Ok(false)
                    }
                )
                .await
                .unwrap()
            );
            assert_eq!(crate::managed_install::package_root(&home), previous_root);
            assert_eq!(
                previous_root.join("current").canonicalize().unwrap(),
                previous
            );
            prepare_from_package(
                &daemon,
                &settings,
                InstallMode::Replace,
                Some(&source),
                &bin,
                |_| Ok(true),
            )
            .await
            .unwrap();
            let selected = root.join("current").canonicalize().unwrap();
            assert_eq!(crate::managed_install::package_root(&home), root);
            if legacy {
                assert_eq!(
                    initial_root.join("current").canonicalize().unwrap(),
                    legacy_release
                );
                assert_eq!(
                    std::fs::read(initial_root.join("auto-update-version")).unwrap(),
                    legacy_marker
                );
            }
            assert_ne!(selected, previous);
            assert_eq!(
                std::fs::read(selected.join("bin/codex")).unwrap(),
                std::fs::read(&bin).unwrap()
            );
            assert_eq!(std::fs::read(previous.join("bin/codex")).unwrap(), before);
            assert!(!root.join("auto-update-version").exists());
            assert!(daemon.running_backend(&settings).await.unwrap().is_none());
            previous = selected;
        }
    }
}

#[tokio::test]
async fn broken_selection_is_not_a_missing_installation() {
    let temp = tempfile::TempDir::new().unwrap();
    let home = temp.path().join("home");
    let source = temp.path().join("source");
    let bin = package(&source, "0.152.0");
    let daemon = daemon(&home);
    let current = home.join("packages/app-server-daemon/current");
    std::fs::create_dir_all(current.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink("missing-release", &current).unwrap();
    let error = prepare_from_package(
        &daemon,
        &DaemonSettings::default(),
        InstallMode::Missing,
        Some(&source),
        &bin,
        |_| panic!("broken installation must not request replacement"),
    )
    .await
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("repair the existing installation")
    );
    assert_eq!(
        std::fs::read_link(current).unwrap(),
        PathBuf::from("missing-release")
    );
}
