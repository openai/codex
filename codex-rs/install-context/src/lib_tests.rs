#![cfg(test)]

//! Unit tests for the crate and its related test suites.

use super::*;
use pretty_assertions::assert_eq;
use std::fs;

const TEST_RESOURCE_NAME: &str = "codex-test-helper";

#[test]
fn code_mode_host_program_prefers_package_resource_over_legacy_binary() -> std::io::Result<()> {
    let package_dir = tempfile::tempdir()?;
    let bin_dir = package_dir.path().join(BIN_DIRNAME);
    let resources_dir = package_dir.path().join(RESOURCES_DIRNAME);
    fs::create_dir_all(&bin_dir)?;
    fs::create_dir_all(&resources_dir)?;
    fs::write(package_dir.path().join(PACKAGE_METADATA_FILENAME), "{}")?;
    let exe_path = bin_dir.join(if cfg!(windows) { "codex.exe" } else { "codex" });
    let resource_host = resources_dir.join(CODE_MODE_HOST_EXECUTABLE_NAME);
    fs::write(&exe_path, "")?;
    fs::write(bin_dir.join(CODE_MODE_HOST_EXECUTABLE_NAME), "legacy host")?;
    fs::write(&resource_host, "managed host")?;

    let context = InstallContext::from_exe(
        /*is_macos*/ false,
        /*current_exe*/ Some(&exe_path),
        /*method_override*/ None,
    );

    assert_eq!(
        context.code_mode_host_program(),
        AbsolutePathBuf::from_absolute_path(resource_host.canonicalize()?)?.into_path_buf()
    );
    Ok(())
}

#[test]
fn code_mode_host_program_uses_package_resource_without_legacy_binary() -> std::io::Result<()> {
    let package_dir = tempfile::tempdir()?;
    let bin_dir = package_dir.path().join(BIN_DIRNAME);
    let resources_dir = package_dir.path().join(RESOURCES_DIRNAME);
    fs::create_dir_all(&bin_dir)?;
    fs::create_dir_all(&resources_dir)?;
    fs::write(package_dir.path().join(PACKAGE_METADATA_FILENAME), "{}")?;
    let exe_path = bin_dir.join(if cfg!(windows) { "codex.exe" } else { "codex" });
    let resource_host = resources_dir.join(CODE_MODE_HOST_EXECUTABLE_NAME);
    fs::write(&exe_path, "")?;
    fs::write(&resource_host, "managed host")?;

    let context = InstallContext::from_exe(
        /*is_macos*/ false,
        /*current_exe*/ Some(&exe_path),
        /*method_override*/ None,
    );

    assert_eq!(
        context.code_mode_host_program(),
        AbsolutePathBuf::from_absolute_path(resource_host.canonicalize()?)?.into_path_buf()
    );
    Ok(())
}

#[test]
fn code_mode_host_program_ignores_resource_directory_and_uses_legacy_binary() -> std::io::Result<()>
{
    let package_dir = tempfile::tempdir()?;
    let bin_dir = package_dir.path().join(BIN_DIRNAME);
    let resources_dir = package_dir.path().join(RESOURCES_DIRNAME);
    fs::create_dir_all(&bin_dir)?;
    fs::create_dir_all(resources_dir.join(CODE_MODE_HOST_EXECUTABLE_NAME))?;
    fs::write(package_dir.path().join(PACKAGE_METADATA_FILENAME), "{}")?;
    let exe_path = bin_dir.join(if cfg!(windows) { "codex.exe" } else { "codex" });
    let legacy_host = bin_dir.join(CODE_MODE_HOST_EXECUTABLE_NAME);
    fs::write(&exe_path, "")?;
    fs::write(&legacy_host, "legacy host")?;

    let context = InstallContext::from_exe(
        /*is_macos*/ false,
        /*current_exe*/ Some(&exe_path),
        /*method_override*/ None,
    );

    assert_eq!(
        context.code_mode_host_program(),
        AbsolutePathBuf::from_absolute_path(legacy_host.canonicalize()?)?.into_path_buf()
    );
    Ok(())
}

#[test]
fn detects_standalone_install_from_release_layout() -> std::io::Result<()> {
    let codex_home = tempfile::tempdir()?;
    let release_dir = codex_home
        .path()
        .join("packages/standalone/releases/1.2.3-x86_64-unknown-linux-musl");
    let resources_dir = release_dir.join(RESOURCES_DIRNAME);
    fs::create_dir_all(&resources_dir)?;
    let exe_path = release_dir.join(if cfg!(windows) { "codex.exe" } else { "codex" });
    fs::write(&exe_path, "")?;
    fs::write(release_dir.join(CODE_MODE_HOST_EXECUTABLE_NAME), "")?;
    fs::write(
        resources_dir.join(CODE_MODE_HOST_EXECUTABLE_NAME),
        "managed host",
    )?;
    fs::write(resources_dir.join(default_rg_command()), "")?;
    fs::write(resources_dir.join(TEST_RESOURCE_NAME), "")?;
    let canonical_release_dir = AbsolutePathBuf::from_absolute_path(release_dir.canonicalize()?)?;
    let canonical_resources_dir =
        AbsolutePathBuf::from_absolute_path(resources_dir.canonicalize()?)?;

    let context = InstallContext::from_exe_with_codex_home(
        /*is_macos*/ false,
        /*current_exe*/ Some(&exe_path),
        /*method_override*/ None,
        /*codex_home*/ Some(codex_home.path()),
    );
    assert_eq!(
        context,
        InstallContext {
            method: InstallMethod::Standalone {
                release_dir: canonical_release_dir.clone(),
                resources_dir: Some(canonical_resources_dir.clone()),
                platform: standalone_platform(),
            },
            package_layout: None,
        }
    );
    assert_eq!(
        context.code_mode_host_program_from_exe(Some(&exe_path)),
        canonical_release_dir
            .join(CODE_MODE_HOST_EXECUTABLE_NAME)
            .into_path_buf()
    );
    assert_eq!(
        context.code_mode_host_program(),
        canonical_resources_dir
            .join(CODE_MODE_HOST_EXECUTABLE_NAME)
            .into_path_buf()
    );
    assert_eq!(
        context.bundled_resource(TEST_RESOURCE_NAME),
        Some(canonical_resources_dir.join(TEST_RESOURCE_NAME))
    );
    Ok(())
}

#[test]
fn standalone_rg_falls_back_when_resources_are_missing() -> std::io::Result<()> {
    let codex_home = tempfile::tempdir()?;
    let release_dir = codex_home
        .path()
        .join("packages/standalone/releases/1.2.3-x86_64-unknown-linux-musl");
    fs::create_dir_all(&release_dir)?;
    let exe_path = release_dir.join(if cfg!(windows) { "codex.exe" } else { "codex" });
    fs::write(&exe_path, "")?;

    let context = InstallContext::from_exe_with_codex_home(
        /*is_macos*/ false,
        /*current_exe*/ Some(&exe_path),
        /*method_override*/ None,
        /*codex_home*/ Some(codex_home.path()),
    );
    assert_eq!(context.rg_command(), default_rg_command());
    Ok(())
}

#[test]
fn detects_package_layout_independently_from_install_method() -> std::io::Result<()> {
    let package_dir = tempfile::tempdir()?;
    let bin_dir = package_dir.path().join(BIN_DIRNAME);
    let resources_dir = package_dir.path().join(RESOURCES_DIRNAME);
    let path_dir = package_dir.path().join(PATH_DIRNAME);
    fs::create_dir_all(&bin_dir)?;
    fs::create_dir_all(&resources_dir)?;
    fs::create_dir_all(&path_dir)?;
    fs::write(
        package_dir.path().join(PACKAGE_METADATA_FILENAME),
        r#"{
  "layoutVersion": 1,
  "version": "1.2.3",
  "target": "x86_64-unknown-linux-musl",
  "variant": "codex",
  "entrypoint": "bin/codex",
  "resourcesDir": "codex-resources",
  "pathDir": "codex-path"
}
"#,
    )?;
    let exe_path = bin_dir.join(if cfg!(windows) { "codex.exe" } else { "codex" });
    fs::write(&exe_path, "")?;
    fs::write(bin_dir.join(CODE_MODE_HOST_EXECUTABLE_NAME), "")?;
    fs::write(resources_dir.join(TEST_RESOURCE_NAME), "")?;
    fs::write(path_dir.join(default_rg_command()), "")?;
    let canonical_package_dir =
        AbsolutePathBuf::from_absolute_path(package_dir.path().canonicalize()?)?;
    let canonical_bin_dir = AbsolutePathBuf::from_absolute_path(bin_dir.canonicalize()?)?;
    let canonical_resources_dir =
        AbsolutePathBuf::from_absolute_path(resources_dir.canonicalize()?)?;
    let canonical_path_dir = AbsolutePathBuf::from_absolute_path(path_dir.canonicalize()?)?;
    let package_layout = CodexPackageLayout {
        package_dir: canonical_package_dir,
        bin_dir: canonical_bin_dir.clone(),
        resources_dir: Some(canonical_resources_dir.clone()),
        path_dir: Some(canonical_path_dir.clone()),
    };

    let context = InstallContext::from_exe_with_codex_home(
        /*is_macos*/ false,
        /*current_exe*/ Some(&exe_path),
        /*method_override*/ None,
        /*codex_home*/ None,
    );
    assert_eq!(
        context,
        InstallContext {
            method: InstallMethod::Other,
            package_layout: Some(package_layout),
        }
    );
    assert_eq!(
        context.package_manifest(),
        Some(CodexPackageManifest {
            version: Version::new(1, 2, 3),
        })
    );
    assert_eq!(
        context.code_mode_host_program_from_exe(Some(&exe_path)),
        canonical_bin_dir
            .join(CODE_MODE_HOST_EXECUTABLE_NAME)
            .into_path_buf()
    );
    assert_eq!(
        context.code_mode_host_program(),
        canonical_bin_dir
            .join(CODE_MODE_HOST_EXECUTABLE_NAME)
            .into_path_buf()
    );
    assert_eq!(
        context.rg_command(),
        canonical_path_dir
            .join(default_rg_command())
            .into_path_buf()
    );
    assert_eq!(
        context.bundled_resource(TEST_RESOURCE_NAME),
        Some(canonical_resources_dir.join(TEST_RESOURCE_NAME))
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn code_mode_host_program_accepts_symlinks_to_files() -> std::io::Result<()> {
    let package_dir = tempfile::tempdir()?;
    let bin_dir = package_dir.path().join(BIN_DIRNAME);
    fs::create_dir_all(&bin_dir)?;
    fs::write(package_dir.path().join(PACKAGE_METADATA_FILENAME), "{}")?;
    let exe_path = bin_dir.join("codex");
    let executable_target = package_dir.path().join("host-target");
    let executable_path = bin_dir.join(CODE_MODE_HOST_EXECUTABLE_NAME);
    fs::write(&exe_path, "")?;
    fs::write(&executable_target, "")?;
    std::os::unix::fs::symlink(&executable_target, &executable_path)?;
    let canonical_bin_dir = AbsolutePathBuf::from_absolute_path(bin_dir.canonicalize()?)?;

    let context = InstallContext::from_exe(
        /*is_macos*/ false,
        /*current_exe*/ Some(&exe_path),
        /*method_override*/ None,
    );

    assert_eq!(
        context.code_mode_host_program_from_exe(Some(&exe_path)),
        canonical_bin_dir
            .join(CODE_MODE_HOST_EXECUTABLE_NAME)
            .into_path_buf()
    );
    Ok(())
}

#[test]
fn standalone_package_layout_keeps_standalone_install_method() -> std::io::Result<()> {
    let codex_home = tempfile::tempdir()?;
    let package_dir = codex_home
        .path()
        .join("packages/standalone/releases/1.2.3-x86_64-unknown-linux-musl");
    let bin_dir = package_dir.join(BIN_DIRNAME);
    let resources_dir = package_dir.join(RESOURCES_DIRNAME);
    let path_dir = package_dir.join(PATH_DIRNAME);
    fs::create_dir_all(&bin_dir)?;
    fs::create_dir_all(&resources_dir)?;
    fs::create_dir_all(&path_dir)?;
    fs::write(package_dir.join(PACKAGE_METADATA_FILENAME), "{}")?;
    let exe_path = bin_dir.join(if cfg!(windows) { "codex.exe" } else { "codex" });
    fs::write(&exe_path, "")?;
    fs::write(resources_dir.join(TEST_RESOURCE_NAME), "")?;
    fs::write(path_dir.join(default_rg_command()), "")?;
    let canonical_package_dir = AbsolutePathBuf::from_absolute_path(package_dir.canonicalize()?)?;
    let canonical_bin_dir = AbsolutePathBuf::from_absolute_path(bin_dir.canonicalize()?)?;
    let canonical_resources_dir =
        AbsolutePathBuf::from_absolute_path(resources_dir.canonicalize()?)?;
    let canonical_path_dir = AbsolutePathBuf::from_absolute_path(path_dir.canonicalize()?)?;

    let context = InstallContext::from_exe_with_codex_home(
        /*is_macos*/ false,
        /*current_exe*/ Some(&exe_path),
        /*method_override*/ None,
        /*codex_home*/ Some(codex_home.path()),
    );
    assert_eq!(
        context,
        InstallContext {
            method: InstallMethod::Standalone {
                release_dir: canonical_package_dir.clone(),
                resources_dir: Some(canonical_resources_dir.clone()),
                platform: standalone_platform(),
            },
            package_layout: Some(CodexPackageLayout {
                package_dir: canonical_package_dir,
                bin_dir: canonical_bin_dir,
                resources_dir: Some(canonical_resources_dir.clone()),
                path_dir: Some(canonical_path_dir.clone()),
            }),
        }
    );
    assert_eq!(
        context.rg_command(),
        canonical_path_dir
            .join(default_rg_command())
            .into_path_buf()
    );
    assert_eq!(
        context.bundled_resource(TEST_RESOURCE_NAME),
        Some(canonical_resources_dir.join(TEST_RESOURCE_NAME))
    );
    Ok(())
}

#[test]
fn npm_managed_package_keeps_package_layout() -> std::io::Result<()> {
    let package_dir = tempfile::tempdir()?;
    let bin_dir = package_dir.path().join(BIN_DIRNAME);
    let path_dir = package_dir.path().join(PATH_DIRNAME);
    fs::create_dir_all(&bin_dir)?;
    fs::create_dir_all(&path_dir)?;
    fs::write(package_dir.path().join(PACKAGE_METADATA_FILENAME), "{}")?;
    let exe_path = bin_dir.join(if cfg!(windows) { "codex.exe" } else { "codex" });
    fs::write(&exe_path, "")?;
    fs::write(path_dir.join(default_rg_command()), "")?;
    let canonical_path_dir = AbsolutePathBuf::from_absolute_path(path_dir.canonicalize()?)?;

    let context = InstallContext::from_exe(
        /*is_macos*/ false,
        /*current_exe*/ Some(&exe_path),
        /*method_override*/ Some(InstallMethod::Npm),
    );
    assert_eq!(context.method, InstallMethod::Npm);
    assert!(context.package_layout.is_some());
    assert_eq!(
        context.rg_command(),
        canonical_path_dir
            .join(default_rg_command())
            .into_path_buf()
    );
    Ok(())
}

#[test]
fn standalone_package_rg_falls_back_when_codex_path_is_missing() -> std::io::Result<()> {
    let package_dir = tempfile::tempdir()?;
    let bin_dir = package_dir.path().join(BIN_DIRNAME);
    fs::create_dir_all(&bin_dir)?;
    fs::write(package_dir.path().join(PACKAGE_METADATA_FILENAME), "{}")?;
    let exe_path = bin_dir.join(if cfg!(windows) { "codex.exe" } else { "codex" });
    fs::write(&exe_path, "")?;

    let context = InstallContext::from_exe_with_codex_home(
        /*is_macos*/ false,
        /*current_exe*/ Some(&exe_path),
        /*method_override*/ None,
        /*codex_home*/ None,
    );
    assert_eq!(context.rg_command(), default_rg_command());
    Ok(())
}

#[test]
fn bundled_file_lookups_ignore_directories() -> std::io::Result<()> {
    let package_dir = tempfile::tempdir()?;
    let bin_dir = package_dir.path().join(BIN_DIRNAME);
    let resources_dir = package_dir.path().join(RESOURCES_DIRNAME);
    let path_dir = package_dir.path().join(PATH_DIRNAME);
    fs::create_dir_all(&bin_dir)?;
    fs::create_dir_all(bin_dir.join(CODE_MODE_HOST_EXECUTABLE_NAME))?;
    fs::create_dir_all(resources_dir.join(TEST_RESOURCE_NAME))?;
    fs::create_dir_all(path_dir.join(default_rg_command()))?;
    fs::write(package_dir.path().join(PACKAGE_METADATA_FILENAME), "{}")?;
    let exe_path = bin_dir.join(if cfg!(windows) { "codex.exe" } else { "codex" });
    fs::write(&exe_path, "")?;
    let fallback_exe_path = package_dir.path().join("fallback-codex");
    fs::write(&fallback_exe_path, "")?;

    let context = InstallContext::from_exe_with_codex_home(
        /*is_macos*/ false,
        /*current_exe*/ Some(&exe_path),
        /*method_override*/ None,
        /*codex_home*/ None,
    );
    assert_eq!(
        context.code_mode_host_program_from_exe(Some(&fallback_exe_path)),
        package_dir.path().join(CODE_MODE_HOST_EXECUTABLE_NAME)
    );
    assert_eq!(context.rg_command(), default_rg_command());
    assert_eq!(context.bundled_resource(TEST_RESOURCE_NAME), None);
    Ok(())
}

#[test]
fn code_mode_host_program_is_next_to_the_executable_even_when_missing() {
    let context = InstallContext {
        method: InstallMethod::Other,
        package_layout: None,
    };

    assert_eq!(
        context.code_mode_host_program_from_exe(Some(Path::new("/opt/codex/bin/codex"))),
        PathBuf::from("/opt/codex/bin").join(CODE_MODE_HOST_EXECUTABLE_NAME)
    );
}

#[test]
fn code_mode_host_program_falls_back_to_its_name_when_executable_is_unknown() {
    let context = InstallContext {
        method: InstallMethod::Other,
        package_layout: None,
    };

    assert_eq!(
        context.code_mode_host_program_from_exe(/*current_exe*/ None),
        PathBuf::from(CODE_MODE_HOST_EXECUTABLE_NAME)
    );
}

#[test]
fn package_manager_method_overrides_take_precedence() {
    let vite_plus_context = InstallContext::from_exe(
        /*is_macos*/ false,
        /*current_exe*/ Some(Path::new("/tmp/codex")),
        /*method_override*/ Some(InstallMethod::VitePlus),
    );
    assert_eq!(
        vite_plus_context,
        InstallContext {
            method: InstallMethod::VitePlus,
            package_layout: None,
        }
    );

    let pnpm_context = InstallContext::from_exe(
        /*is_macos*/ false,
        /*current_exe*/ Some(Path::new("/tmp/codex")),
        /*method_override*/ Some(InstallMethod::Pnpm),
    );
    assert_eq!(
        pnpm_context,
        InstallContext {
            method: InstallMethod::Pnpm,
            package_layout: None,
        }
    );

    let npm_context = InstallContext::from_exe(
        /*is_macos*/ false,
        /*current_exe*/ Some(Path::new("/tmp/codex")),
        /*method_override*/ Some(InstallMethod::Npm),
    );
    assert_eq!(
        npm_context,
        InstallContext {
            method: InstallMethod::Npm,
            package_layout: None,
        }
    );

    let bun_context = InstallContext::from_exe(
        /*is_macos*/ false,
        /*current_exe*/ Some(Path::new("/tmp/codex")),
        /*method_override*/ Some(InstallMethod::Bun),
    );
    assert_eq!(
        bun_context,
        InstallContext {
            method: InstallMethod::Bun,
            package_layout: None,
        }
    );
}

#[test]
fn brew_is_detected_on_macos_prefixes() {
    let context = InstallContext::from_exe_with_codex_home(
        /*is_macos*/ true,
        /*current_exe*/ Some(Path::new("/opt/homebrew/bin/codex")),
        /*method_override*/ None,
        /*codex_home*/ None,
    );
    assert_eq!(
        context,
        InstallContext {
            method: InstallMethod::Brew,
            package_layout: None,
        }
    );
}

#[path = "bundle_tests.rs"]
mod bundle_tests;
