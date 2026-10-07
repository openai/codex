mod common;

use codex_config::CONFIG_TOML_FILE;
use codex_config::ConfigLayerSource;
use codex_config::format_config_layer_source;
use codex_config::loader::project_trust_key;
use codex_exec_server::Environment;
use codex_exec_server::EnvironmentConfigLayer;
use codex_exec_server::EnvironmentConfigLayerStack;
use codex_exec_server::EnvironmentConfigReadParams;
use codex_exec_server::EnvironmentConfigReadResponse;
use codex_exec_server::ExecServerError;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use common::exec_server::exec_server;
use common::exec_server::exec_server_with_env;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_environment_reads_projected_executor_config() -> anyhow::Result<()> {
    let mut server =
        exec_server_with_env([("CODEX_EXEC_SERVER_TEST_PREFER_MXC", "true")], &[]).await?;
    let codex_home =
        AbsolutePathBuf::from_absolute_path(std::fs::canonicalize(server.codex_home())?)?;
    let config_file = codex_home.join(CONFIG_TOML_FILE);
    let project = codex_home.join("project");
    let nested = project.join("nested");
    let dot_codex = project.join(".codex");
    tokio::fs::create_dir_all(dot_codex.as_path()).await?;
    tokio::fs::write(project.join(".project-root").as_path(), "").await?;
    let project_key = toml::Value::String(project_trust_key(project.as_path())).to_string();
    let nested_key = toml::Value::String(project_trust_key(nested.as_path())).to_string();
    let projects_toml =
        format!("[projects.{project_key}]\ntrust_level = \"trusted\"\n[projects.{nested_key}]\n");
    tokio::fs::write(
        &config_file,
        format!("project_root_markers = [\".project-root\"]\n{projects_toml}"),
    )
    .await?;
    tokio::fs::write(
        dot_codex.join(CONFIG_TOML_FILE).as_path(),
        r#"
[future_environment]
relative_path = "./executor-relative"
unselected = "do not return"
[features]
prefer_mxc = false
private_startup_value = "unselected-private-value"
"#,
    )
    .await?;

    let environment = Environment::create_for_tests(Some(server.websocket_url().to_string()))?;
    let environment_info = environment.info().await?;
    assert!(environment_info.capabilities.environment_config_read);
    assert_eq!(
        environment_info.user_home_dir,
        dirs::home_dir().and_then(|home_dir| PathUri::from_host_native_path(home_dir).ok()),
    );

    let response = environment
        .read_environment_config(EnvironmentConfigReadParams {
            cwd: PathUri::from_abs_path(&project),
            config_paths: vec![vec![
                "future_environment".to_string(),
                "relative_path".to_string(),
            ]],
            requirements_paths: Vec::new(),
        })
        .await?;

    let projected_toml = toml::toml! {
        [future_environment]
        relative_path = "./executor-relative"
    };
    assert_eq!(
        response,
        EnvironmentConfigReadResponse {
            user_home_dir: dirs::home_dir()
                .and_then(|home_dir| PathUri::from_host_native_path(home_dir).ok()),
            codex_home_dir: PathUri::from_abs_path(&codex_home),
            hostname: codex_config::host_name(),
            config: EnvironmentConfigLayerStack {
                layers: vec![EnvironmentConfigLayer {
                    source: format_config_layer_source(
                        &ConfigLayerSource::Project {
                            dot_codex_folder: dot_codex.clone(),
                        },
                        CONFIG_TOML_FILE,
                    ),
                    base_dir: PathUri::from_abs_path(&dot_codex),
                    toml: toml::to_string(&projected_toml)?,
                }],
                cloud_insertion_index: 0,
            },
            requirements: EnvironmentConfigLayerStack {
                layers: Vec::new(),
                cloud_insertion_index: 0,
            },
        }
    );

    let mut expected_projects = response;
    expected_projects.config.layers[0] = EnvironmentConfigLayer {
        source: format_config_layer_source(
            &ConfigLayerSource::User {
                file: config_file,
                profile: None,
            },
            CONFIG_TOML_FILE,
        ),
        base_dir: PathUri::from_abs_path(&codex_home),
        toml: toml::to_string(&toml::from_str::<toml::Value>(&projects_toml)?)?,
    };
    let projects = environment
        .read_environment_config(EnvironmentConfigReadParams {
            cwd: PathUri::from_abs_path(&project),
            config_paths: vec![vec!["projects".to_string()]],
            requirements_paths: Vec::new(),
        })
        .await?;
    assert_eq!(projects, expected_projects);

    let preference = environment
        .read_environment_config(EnvironmentConfigReadParams {
            cwd: PathUri::from_abs_path(&project),
            config_paths: vec![vec!["features".to_string(), "prefer_mxc".to_string()]],
            requirements_paths: Vec::new(),
        })
        .await?;
    assert_eq!(preference.config.cloud_insertion_index, 0);
    assert_eq!(preference.config.layers.len(), 2);
    let configured = &preference.config.layers[0];
    let startup = &preference.config.layers[1];
    assert_eq!(configured.base_dir, PathUri::from_abs_path(&dot_codex));
    assert_eq!(startup.source, "session-flags");
    assert_eq!(startup.base_dir, PathUri::from_abs_path(&project));
    assert_eq!(
        toml::from_str::<toml::Value>(&configured.toml)?,
        toml::Value::Table(toml::toml! { [features] prefer_mxc = false }),
    );
    assert_eq!(
        toml::from_str::<toml::Value>(&startup.toml)?,
        toml::Value::Table(toml::toml! { [features] prefer_mxc = true }),
    );
    server.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn environment_config_read_rejects_empty_selectors() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    let codex_home =
        AbsolutePathBuf::from_absolute_path(std::fs::canonicalize(server.codex_home())?)?;
    let environment = Environment::create_for_tests(Some(server.websocket_url().to_string()))?;

    for (config_paths, expected_message) in [
        (
            Vec::new(),
            "at least one config or requirements path is required",
        ),
        (
            vec![Vec::new()],
            "TOML paths must contain at least one key segment",
        ),
    ] {
        let error = environment
            .read_environment_config(EnvironmentConfigReadParams {
                cwd: PathUri::from_abs_path(&codex_home),
                config_paths,
                requirements_paths: Vec::new(),
            })
            .await
            .expect_err("invalid selectors should fail");
        assert!(
            matches!(
                error,
                ExecServerError::Server { code: -32602, ref message }
                    if message == expected_message
            ),
            "unexpected error: {error:?}"
        );
    }

    server.shutdown().await?;
    Ok(())
}
