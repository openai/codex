//! Exercises installed-package and fixed skill location prewarming on real filesystem fixtures.

use pretty_assertions::assert_eq;
use std::path::Path;

use codex_utils_path_uri::PathUri;

use super::CapabilityLocation;
use super::CapabilityLocationRequest;
use super::InstalledPlugin;
use super::prewarm_locations;
use crate::LocalFileSystem;

pub(crate) const MANIFEST: &str = r#"{"name":"demo","version":"1.10.0"}"#;

pub(crate) fn request(root: &Path) -> anyhow::Result<CapabilityLocationRequest> {
    for path in ["codex", "home", "project"] {
        std::fs::create_dir_all(root.join(path))?;
    }
    Ok(CapabilityLocationRequest {
        codex_home: PathUri::from_host_native_path(root.join("codex"))?,
        user_home: Some(PathUri::from_host_native_path(root.join("home"))?),
    })
}

pub(crate) fn write(root: &Path, relative: &str, contents: &str) -> anyhow::Result<()> {
    let path = root.join(relative);
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("test fixture path has no parent"))?;
    std::fs::create_dir_all(parent)?;
    std::fs::write(path, contents)?;
    Ok(())
}

#[tokio::test]
async fn prewarming_resolves_global_locations_without_cwd_or_config() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let request = request(temp.path())?;
    // Prewarming must not read configuration.
    write(temp.path(), "codex/config.toml", "invalid TOML [")?;
    for root in [
        "codex/skills",
        "home/.agents/skills",
        "project/.agents/skills",
    ] {
        write(temp.path(), &format!("{root}/demo/SKILL.md"), "unparsed")?;
    }
    for version in ["1.9.0", "1.10.0"] {
        write(
            temp.path(),
            &format!("codex/plugins/cache/team/demo/{version}/.codex-plugin/plugin.json"),
            MANIFEST,
        )?;
    }
    write(
        temp.path(),
        "codex/plugins/cache/team/demo/.codex-remote-plugin-install.json",
        r#"{"schema_version":1,"remote_plugin_id":"plugins~demo"}"#,
    )?;
    let locations = prewarm_locations(&LocalFileSystem::unsandboxed(), &request).await?;
    let mut actual = locations.locations;
    actual.sort_unstable_by_key(|location| location.root.to_string());
    let mut expected = vec![CapabilityLocation {
        root: request.codex_home.join("plugins/cache/team/demo/1.10.0")?,
        plugin: Some(InstalledPlugin {
            id: "demo@team".to_string(),
            version: "1.10.0".to_string(),
            remote_plugin_id: Some("plugins~demo".to_string()),
        }),
    }];
    for root in ["codex/skills", "home/.agents/skills"] {
        expected.push(CapabilityLocation {
            root: PathUri::from_host_native_path(temp.path().join(root))?,
            plugin: None,
        });
    }
    expected.sort_unstable_by_key(|location| location.root.to_string());
    assert_eq!(actual, expected);
    assert!(locations.warnings.is_empty());

    let metadata = "codex/plugins/cache/team/demo/.codex-remote-plugin-install.json";
    for contents in [
        "invalid JSON".to_string(),
        " ".repeat(super::MAX_REMOTE_INSTALL_METADATA_BYTES as usize + 1),
    ] {
        write(temp.path(), metadata, &contents)?;
        let locations = prewarm_locations(&LocalFileSystem::unsandboxed(), &request).await?;
        assert!(locations.warnings.iter().any(|warning| {
            warning.contains("could not read remote installation metadata for demo@team")
        }));
        assert!(locations.locations.iter().any(|location| {
            location
                .plugin
                .as_ref()
                .is_some_and(|plugin| plugin.id == "demo@team" && plugin.remote_plugin_id.is_none())
        }));
    }
    // Missing remote metadata is valid for locally installed plugins.
    std::fs::remove_file(temp.path().join(metadata))?;
    let locations = prewarm_locations(&LocalFileSystem::unsandboxed(), &request).await?;
    assert!(locations.warnings.is_empty());
    assert!(locations.locations.iter().any(|location| {
        location
            .plugin
            .as_ref()
            .is_some_and(|plugin| plugin.id == "demo@team" && plugin.remote_plugin_id.is_none())
    }));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let directory = temp.path().join("codex/plugins/cache/team");
        let permissions = std::fs::metadata(&directory)?.permissions();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o000))?;
        let result = prewarm_locations(&LocalFileSystem::unsandboxed(), &request).await;
        std::fs::set_permissions(&directory, permissions)?;
        assert!(
            result.is_err(),
            "cache traversal failure must not retain partial locations"
        );
    }
    Ok(())
}
