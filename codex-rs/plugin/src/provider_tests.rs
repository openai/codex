use super::PluginResourceLocator;
use super::ResolvedPlugin;
use super::ResolvedPluginError;
use crate::manifest::PluginManifest;
use crate::manifest::PluginManifestHooks;
use crate::manifest::PluginManifestInterface;
use crate::manifest::PluginManifestMcpServers;
use crate::manifest::PluginManifestPaths;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;

fn absolute(path: impl AsRef<std::path::Path>) -> AbsolutePathBuf {
    AbsolutePathBuf::from_absolute_path_checked(path.as_ref()).expect("absolute test path")
}

fn path_uri(path: &AbsolutePathBuf) -> PathUri {
    PathUri::from_abs_path(path)
}

fn resource(environment_id: &str, path: &AbsolutePathBuf) -> PluginResourceLocator {
    PluginResourceLocator::Environment {
        environment_id: environment_id.to_string(),
        path: path_uri(path),
    }
}

#[test]
fn environment_descriptor_binds_every_manifest_resource() {
    let root = absolute(std::env::current_dir().expect("cwd").join("plugin-root"));
    let root_uri = path_uri(&root);
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
            skills: vec![path_uri(&skills)],
            mcp_servers: Some(PluginManifestMcpServers::Path(path_uri(&mcp_servers))),
            apps: Some(path_uri(&apps)),
            hooks: Some(PluginManifestHooks::Paths(vec![path_uri(&hooks)])),
        },
        interface: Some(PluginManifestInterface {
            composer_icon: Some(path_uri(&composer_icon)),
            logo: Some(path_uri(&logo)),
            screenshots: vec![path_uri(&screenshot)],
            ..PluginManifestInterface::default()
        }),
    };

    let plugin = ResolvedPlugin::from_environment(
        "selected-demo".to_string(),
        "executor-1".to_string(),
        root_uri,
        path_uri(&manifest_path),
        manifest,
    )
    .expect("valid descriptor");

    assert_eq!(
        plugin.manifest_path(),
        &resource("executor-1", &manifest_path)
    );
    assert_eq!(
        plugin.manifest(),
        &PluginManifest {
            name: "demo".to_string(),
            version: None,
            description: None,
            keywords: Vec::new(),
            paths: PluginManifestPaths {
                skills: vec![resource("executor-1", &skills)],
                mcp_servers: Some(PluginManifestMcpServers::Path(resource(
                    "executor-1",
                    &mcp_servers,
                ))),
                apps: Some(resource("executor-1", &apps)),
                hooks: Some(PluginManifestHooks::Paths(vec![resource(
                    "executor-1",
                    &hooks
                )])),
            },
            interface: Some(PluginManifestInterface {
                composer_icon: Some(resource("executor-1", &composer_icon)),
                logo: Some(resource("executor-1", &logo)),
                screenshots: vec![resource("executor-1", &screenshot)],
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
            skills: Vec::new(),
            mcp_servers: Some(PluginManifestMcpServers::Path(path_uri(&outside))),
            apps: None,
            hooks: None,
        },
        interface: None,
    };

    let err = ResolvedPlugin::from_environment(
        "selected-demo".to_string(),
        "executor-1".to_string(),
        path_uri(&root),
        path_uri(&root.join(".codex-plugin/plugin.json")),
        manifest,
    )
    .expect_err("outside resource should fail");

    assert_eq!(
        err,
        ResolvedPluginError::ResourceOutsideRoot {
            root: path_uri(&root),
            path: path_uri(&outside),
        }
    );
}
