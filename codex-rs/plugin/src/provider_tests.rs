use super::PluginResourceLocator;
use super::ResolvedPlugin;
use super::ResolvedPluginError;
use crate::manifest::PluginManifest;
use crate::manifest::PluginManifestHooks;
use crate::manifest::PluginManifestInterface;
use crate::manifest::PluginManifestPaths;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

fn absolute(path: impl AsRef<std::path::Path>) -> AbsolutePathBuf {
    AbsolutePathBuf::from_absolute_path_checked(path.as_ref()).expect("absolute test path")
}

fn resource(environment_id: &str, path: AbsolutePathBuf) -> PluginResourceLocator {
    PluginResourceLocator::Environment {
        environment_id: environment_id.to_string(),
        path,
    }
}

#[test]
fn environment_descriptor_binds_every_manifest_resource() {
    let root = absolute(std::env::current_dir().expect("cwd").join("plugin-root"));
    let manifest_path = root.join(".codex-plugin/plugin.json");
    let skills = root.join("skills");
    let mcp_servers = root.join(".mcp.json");
    let apps = root.join(".app.json");
    let hooks = root.join("hooks/hooks.json");
    let composer_icon = root.join("assets/composer.svg");
    let logo = root.join("assets/logo.svg");
    let screenshot = root.join("assets/screenshot.png");
    let manifest = PluginManifest {
        name: "demo".to_string(),
        version: None,
        description: None,
        keywords: Vec::new(),
        paths: PluginManifestPaths {
            skills: Some(skills.clone()),
            mcp_servers: Some(mcp_servers.clone()),
            apps: Some(apps.clone()),
            hooks: Some(PluginManifestHooks::Paths(vec![hooks.clone()])),
        },
        interface: Some(PluginManifestInterface {
            composer_icon: Some(composer_icon.clone()),
            logo: Some(logo.clone()),
            screenshots: vec![screenshot.clone()],
            ..PluginManifestInterface::default()
        }),
    };

    let plugin = ResolvedPlugin::from_environment(
        "selected-demo".to_string(),
        "executor-1".to_string(),
        root,
        manifest_path.clone(),
        manifest,
    )
    .expect("valid descriptor");

    assert_eq!(
        plugin.manifest_path(),
        &resource("executor-1", manifest_path)
    );
    assert_eq!(
        plugin.manifest(),
        &PluginManifest {
            name: "demo".to_string(),
            version: None,
            description: None,
            keywords: Vec::new(),
            paths: PluginManifestPaths {
                skills: Some(resource("executor-1", skills)),
                mcp_servers: Some(resource("executor-1", mcp_servers)),
                apps: Some(resource("executor-1", apps)),
                hooks: Some(PluginManifestHooks::Paths(vec![resource(
                    "executor-1",
                    hooks,
                )])),
            },
            interface: Some(PluginManifestInterface {
                composer_icon: Some(resource("executor-1", composer_icon)),
                logo: Some(resource("executor-1", logo)),
                screenshots: vec![resource("executor-1", screenshot)],
                ..PluginManifestInterface::default()
            }),
        }
    );
}

#[test]
fn environment_descriptor_rejects_resources_outside_package_root() {
    let cwd = std::env::current_dir().expect("cwd");
    let root = absolute(cwd.join("plugin-root"));
    let outside = absolute(cwd.join("outside/.mcp.json"));
    let manifest = PluginManifest {
        name: "demo".to_string(),
        version: None,
        description: None,
        keywords: Vec::new(),
        paths: PluginManifestPaths {
            skills: None,
            mcp_servers: Some(outside.clone()),
            apps: None,
            hooks: None,
        },
        interface: None,
    };

    let err = ResolvedPlugin::from_environment(
        "selected-demo".to_string(),
        "executor-1".to_string(),
        root.clone(),
        root.join(".codex-plugin/plugin.json"),
        manifest,
    )
    .expect_err("outside resource should fail");

    assert_eq!(
        err,
        ResolvedPluginError::ResourceOutsideRoot {
            root,
            path: outside,
        }
    );
}
