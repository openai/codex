use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use async_trait::async_trait;
use serde::Deserialize;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

use crate::DefaultEnvironmentProvider;
use crate::Environment;
use crate::EnvironmentProvider;
use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::client_api::ExecServerTransportParams;
use crate::client_api::StdioExecServerCommand;
use crate::environment::LOCAL_ENVIRONMENT_ID;
use crate::environment_provider::EnvironmentDefault;
use crate::environment_provider::EnvironmentProviderSnapshot;

const ENVIRONMENTS_TOML_FILE: &str = "environments.toml";
const MAX_ENVIRONMENT_ID_LEN: usize = 64;

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
struct EnvironmentsToml {
    default: Option<String>,

    #[serde(default)]
    environments: Vec<EnvironmentToml>,
}

#[derive(Deserialize, Debug, Default, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct EnvironmentToml {
    id: String,
    url: Option<String>,
    program: Option<String>,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
    cwd: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TomlEnvironmentProvider {
    default: EnvironmentDefault,
    environments: Vec<(String, ExecServerTransportParams)>,
}

impl TomlEnvironmentProvider {
    #[cfg(test)]
    fn new(config: EnvironmentsToml) -> Result<Self, ExecServerError> {
        Self::new_with_config_dir(config, /*config_dir*/ None)
    }

    fn new_with_config_dir(
        config: EnvironmentsToml,
        config_dir: Option<&Path>,
    ) -> Result<Self, ExecServerError> {
        let mut ids = HashSet::from([LOCAL_ENVIRONMENT_ID.to_string()]);
        let mut environments = Vec::with_capacity(config.environments.len());
        for item in config.environments {
            let (id, transport) = parse_environment_toml(item, config_dir)?;
            if !ids.insert(id.clone()) {
                return Err(ExecServerError::Protocol(format!(
                    "environment id `{id}` is duplicated"
                )));
            }
            environments.push((id, transport));
        }
        let default = normalize_default_environment_id(config.default.as_deref(), &ids)?;
        Ok(Self {
            default,
            environments,
        })
    }
}

#[async_trait]
impl EnvironmentProvider for TomlEnvironmentProvider {
    async fn snapshot(
        &self,
        local_runtime_paths: &ExecServerRuntimePaths,
    ) -> Result<EnvironmentProviderSnapshot, ExecServerError> {
        let mut environments = Vec::with_capacity(self.environments.len() + 1);
        environments.push((
            LOCAL_ENVIRONMENT_ID.to_string(),
            Environment::local(local_runtime_paths.clone()),
        ));
        for (id, transport_params) in &self.environments {
            environments.push((
                id.clone(),
                Environment::remote_with_transport(
                    transport_params.clone(),
                    Some(local_runtime_paths.clone()),
                ),
            ));
        }

        Ok(EnvironmentProviderSnapshot {
            environments,
            default: self.default.clone(),
        })
    }
}

fn parse_environment_toml(
    item: EnvironmentToml,
    config_dir: Option<&Path>,
) -> Result<(String, ExecServerTransportParams), ExecServerError> {
    let EnvironmentToml {
        id,
        url,
        program,
        args,
        env,
        cwd,
    } = item;
    validate_environment_id(&id)?;
    if program.is_none() && (args.is_some() || env.is_some() || cwd.is_some()) {
        return Err(ExecServerError::Protocol(format!(
            "environment `{id}` args, env, and cwd require program"
        )));
    }

    let transport_params = match (url, program) {
        (Some(url), None) => {
            let url = validate_websocket_url(url)?;
            ExecServerTransportParams::WebSocketUrl(url)
        }
        (None, Some(program)) => {
            let program = program.trim().to_string();
            if program.is_empty() {
                return Err(ExecServerError::Protocol(format!(
                    "environment `{id}` program cannot be empty"
                )));
            }
            let cwd = normalize_stdio_cwd(&id, cwd, config_dir)?;
            ExecServerTransportParams::StdioCommand(StdioExecServerCommand {
                program,
                args: args.unwrap_or_default(),
                env: env.unwrap_or_default(),
                cwd,
            })
        }
        (None, None) | (Some(_), Some(_)) => {
            return Err(ExecServerError::Protocol(format!(
                "environment `{id}` must set exactly one of url or program"
            )));
        }
    };

    Ok((id, transport_params))
}

fn normalize_stdio_cwd(
    id: &str,
    cwd: Option<PathBuf>,
    config_dir: Option<&Path>,
) -> Result<Option<PathBuf>, ExecServerError> {
    let Some(cwd) = cwd else {
        return Ok(None);
    };
    if cwd.is_absolute() {
        return Ok(Some(cwd));
    }
    let Some(config_dir) = config_dir else {
        return Err(ExecServerError::Protocol(format!(
            "environment `{id}` cwd must be absolute"
        )));
    };
    Ok(Some(config_dir.join(cwd)))
}

pub(crate) fn environment_provider_from_codex_home(
    codex_home: &Path,
) -> Result<Box<dyn EnvironmentProvider>, ExecServerError> {
    let path = codex_home.join(ENVIRONMENTS_TOML_FILE);
    if !path.try_exists().map_err(|err| {
        ExecServerError::Protocol(format!(
            "failed to inspect environment config `{}`: {err}",
            path.display()
        ))
    })? {
        return Ok(Box::new(DefaultEnvironmentProvider::from_env()));
    }

    let environments = load_environments_toml(&path)?;
    Ok(Box::new(TomlEnvironmentProvider::new_with_config_dir(
        environments,
        Some(codex_home),
    )?))
}

fn normalize_default_environment_id(
    default: Option<&str>,
    ids: &HashSet<String>,
) -> Result<EnvironmentDefault, ExecServerError> {
    let Some(default) = default.map(str::trim) else {
        return Ok(EnvironmentDefault::EnvironmentId(
            LOCAL_ENVIRONMENT_ID.to_string(),
        ));
    };
    if default.is_empty() {
        return Err(ExecServerError::Protocol(
            "default environment id cannot be empty".to_string(),
        ));
    }
    if !default.eq_ignore_ascii_case("none") && !ids.contains(default) {
        return Err(ExecServerError::Protocol(format!(
            "default environment `{default}` is not configured"
        )));
    }
    if default.eq_ignore_ascii_case("none") {
        Ok(EnvironmentDefault::Disabled)
    } else {
        Ok(EnvironmentDefault::EnvironmentId(default.to_string()))
    }
}

fn validate_environment_id(id: &str) -> Result<(), ExecServerError> {
    let trimmed_id = id.trim();
    if trimmed_id.is_empty() {
        return Err(ExecServerError::Protocol(
            "environment id cannot be empty".to_string(),
        ));
    }
    if trimmed_id != id {
        return Err(ExecServerError::Protocol(format!(
            "environment id `{id}` must not contain surrounding whitespace"
        )));
    }
    if id == LOCAL_ENVIRONMENT_ID || id.eq_ignore_ascii_case("none") {
        return Err(ExecServerError::Protocol(format!(
            "environment id `{id}` is reserved"
        )));
    }
    if id.len() > MAX_ENVIRONMENT_ID_LEN {
        return Err(ExecServerError::Protocol(format!(
            "environment id `{id}` cannot be longer than {MAX_ENVIRONMENT_ID_LEN} characters"
        )));
    }
    if !id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(ExecServerError::Protocol(format!(
            "environment id `{id}` must contain only ASCII letters, numbers, '-' or '_'"
        )));
    }
    Ok(())
}

fn validate_websocket_url(url: String) -> Result<String, ExecServerError> {
    let url = url.trim();
    if url.is_empty() {
        return Err(ExecServerError::Protocol(
            "environment url cannot be empty".to_string(),
        ));
    }
    if !url.starts_with("ws://") && !url.starts_with("wss://") {
        return Err(ExecServerError::Protocol(format!(
            "environment url `{url}` must use ws:// or wss://"
        )));
    }
    url.into_client_request().map_err(|err| {
        ExecServerError::Protocol(format!("environment url `{url}` is invalid: {err}"))
    })?;
    Ok(url.to_string())
}

fn load_environments_toml(path: &Path) -> Result<EnvironmentsToml, ExecServerError> {
    let contents = std::fs::read_to_string(path).map_err(|err| {
        ExecServerError::Protocol(format!(
            "failed to read environment config `{}`: {err}",
            path.display()
        ))
    })?;

    toml::from_str(&contents).map_err(|err| {
        ExecServerError::Protocol(format!(
            "failed to parse environment config `{}`: {err}",
            path.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    use super::*;

    fn test_runtime_paths() -> ExecServerRuntimePaths {
        ExecServerRuntimePaths::new(
            std::env::current_exe().expect("current exe"),
            /*codex_linux_sandbox_exe*/ None,
        )
        .expect("runtime paths")
    }

    #[tokio::test]
    async fn toml_provider_adds_implicit_local_and_configured_environments() {
        let provider = TomlEnvironmentProvider::new(EnvironmentsToml {
            default: Some("ssh-dev".to_string()),
            environments: vec![
                EnvironmentToml {
                    id: "devbox".to_string(),
                    url: Some(" ws://127.0.0.1:8765 ".to_string()),
                    ..Default::default()
                },
                EnvironmentToml {
                    id: "ssh-dev".to_string(),
                    program: Some(" ssh ".to_string()),
                    args: Some(vec![
                        "dev".to_string(),
                        "codex exec-server --listen stdio".to_string(),
                    ]),
                    env: Some(HashMap::from([(
                        "CODEX_LOG".to_string(),
                        "debug".to_string(),
                    )])),
                    ..Default::default()
                },
            ],
        })
        .expect("provider");
        let runtime_paths = test_runtime_paths();

        let snapshot = provider
            .snapshot(&runtime_paths)
            .await
            .expect("environments");
        let EnvironmentProviderSnapshot {
            environments,
            default,
        } = snapshot;
        let environment_ids: Vec<_> = environments
            .iter()
            .map(|(id, _environment)| id.as_str())
            .collect();
        assert_eq!(
            environment_ids,
            vec![LOCAL_ENVIRONMENT_ID, "devbox", "ssh-dev"]
        );
        let environments: HashMap<_, _> = environments.into_iter().collect();

        assert!(!environments[LOCAL_ENVIRONMENT_ID].is_remote());
        assert_eq!(
            environments["devbox"].exec_server_url(),
            Some("ws://127.0.0.1:8765")
        );
        assert!(environments["ssh-dev"].is_remote());
        assert_eq!(environments["ssh-dev"].exec_server_url(), None);
        assert_eq!(
            default,
            EnvironmentDefault::EnvironmentId("ssh-dev".to_string())
        );
    }

    #[tokio::test]
    async fn toml_provider_default_omitted_selects_local() {
        let provider = TomlEnvironmentProvider::new(EnvironmentsToml::default()).expect("provider");
        let snapshot = provider
            .snapshot(&test_runtime_paths())
            .await
            .expect("environments");

        assert_eq!(
            snapshot.default,
            EnvironmentDefault::EnvironmentId(LOCAL_ENVIRONMENT_ID.to_string())
        );
    }

    #[tokio::test]
    async fn toml_provider_default_none_disables_default() {
        let provider = TomlEnvironmentProvider::new(EnvironmentsToml {
            default: Some("none".to_string()),
            environments: Vec::new(),
        })
        .expect("provider");
        let snapshot = provider
            .snapshot(&test_runtime_paths())
            .await
            .expect("environments");

        assert_eq!(snapshot.default, EnvironmentDefault::Disabled);
    }

    #[test]
    fn toml_provider_rejects_invalid_environments() {
        let cases = [
            (
                EnvironmentToml {
                    id: "local".to_string(),
                    url: Some("ws://127.0.0.1:8765".to_string()),
                    ..Default::default()
                },
                "environment id `local` is reserved",
            ),
            (
                EnvironmentToml {
                    id: " devbox ".to_string(),
                    url: Some("ws://127.0.0.1:8765".to_string()),
                    ..Default::default()
                },
                "environment id ` devbox ` must not contain surrounding whitespace",
            ),
            (
                EnvironmentToml {
                    id: "dev box".to_string(),
                    url: Some("ws://127.0.0.1:8765".to_string()),
                    ..Default::default()
                },
                "environment id `dev box` must contain only ASCII letters, numbers, '-' or '_'",
            ),
            (
                EnvironmentToml {
                    id: "devbox".to_string(),
                    url: Some("http://127.0.0.1:8765".to_string()),
                    ..Default::default()
                },
                "environment url `http://127.0.0.1:8765` must use ws:// or wss://",
            ),
            (
                EnvironmentToml {
                    id: "devbox".to_string(),
                    url: Some("ws://127.0.0.1:8765".to_string()),
                    program: Some("codex".to_string()),
                    ..Default::default()
                },
                "environment `devbox` must set exactly one of url or program",
            ),
            (
                EnvironmentToml {
                    id: "devbox".to_string(),
                    program: Some(" ".to_string()),
                    ..Default::default()
                },
                "environment `devbox` program cannot be empty",
            ),
            (
                EnvironmentToml {
                    id: "devbox".to_string(),
                    args: Some(Vec::new()),
                    ..Default::default()
                },
                "environment `devbox` args, env, and cwd require program",
            ),
        ];

        for (item, expected) in cases {
            let err = TomlEnvironmentProvider::new(EnvironmentsToml {
                default: None,
                environments: vec![item],
            })
            .expect_err("invalid item should fail");

            assert_eq!(
                err.to_string(),
                format!("exec-server protocol error: {expected}")
            );
        }
    }

    #[test]
    fn toml_provider_resolves_relative_stdio_cwd_from_config_dir() {
        let config_dir = tempdir().expect("tempdir");
        let provider = TomlEnvironmentProvider::new_with_config_dir(
            EnvironmentsToml {
                default: None,
                environments: vec![EnvironmentToml {
                    id: "ssh-dev".to_string(),
                    program: Some("ssh".to_string()),
                    cwd: Some(PathBuf::from("workspace")),
                    ..Default::default()
                }],
            },
            Some(config_dir.path()),
        )
        .expect("provider");

        assert_eq!(
            provider.environments[0].1,
            ExecServerTransportParams::StdioCommand(StdioExecServerCommand {
                program: "ssh".to_string(),
                args: Vec::new(),
                env: HashMap::new(),
                cwd: Some(config_dir.path().join("workspace")),
            })
        );
    }

    #[test]
    fn toml_provider_rejects_relative_stdio_cwd_without_config_dir() {
        let err = TomlEnvironmentProvider::new(EnvironmentsToml {
            default: None,
            environments: vec![EnvironmentToml {
                id: "ssh-dev".to_string(),
                program: Some("ssh".to_string()),
                cwd: Some(PathBuf::from("workspace")),
                ..Default::default()
            }],
        })
        .expect_err("relative cwd without config dir should fail");

        assert_eq!(
            err.to_string(),
            "exec-server protocol error: environment `ssh-dev` cwd must be absolute"
        );
    }

    #[test]
    fn toml_provider_rejects_duplicate_ids() {
        let err = TomlEnvironmentProvider::new(EnvironmentsToml {
            default: None,
            environments: vec![
                EnvironmentToml {
                    id: "devbox".to_string(),
                    url: Some("ws://127.0.0.1:8765".to_string()),
                    ..Default::default()
                },
                EnvironmentToml {
                    id: "devbox".to_string(),
                    program: Some("codex".to_string()),
                    ..Default::default()
                },
            ],
        })
        .expect_err("duplicate id should fail");

        assert_eq!(
            err.to_string(),
            "exec-server protocol error: environment id `devbox` is duplicated"
        );
    }

    #[test]
    fn toml_provider_rejects_overlong_id() {
        let id = "a".repeat(MAX_ENVIRONMENT_ID_LEN + 1);
        let err = TomlEnvironmentProvider::new(EnvironmentsToml {
            default: None,
            environments: vec![EnvironmentToml {
                id: id.clone(),
                url: Some("ws://127.0.0.1:8765".to_string()),
                ..Default::default()
            }],
        })
        .expect_err("overlong id should fail");

        assert_eq!(
            err.to_string(),
            format!(
                "exec-server protocol error: environment id `{id}` cannot be longer than {MAX_ENVIRONMENT_ID_LEN} characters"
            )
        );
    }

    #[test]
    fn toml_provider_rejects_unknown_default() {
        let err = TomlEnvironmentProvider::new(EnvironmentsToml {
            default: Some("missing".to_string()),
            environments: Vec::new(),
        })
        .expect_err("unknown default should fail");

        assert_eq!(
            err.to_string(),
            "exec-server protocol error: default environment `missing` is not configured"
        );
    }

    #[test]
    fn load_environments_toml_reads_root_environment_list() {
        let codex_home = tempdir().expect("tempdir");
        let path = codex_home.path().join(ENVIRONMENTS_TOML_FILE);
        std::fs::write(
            &path,
            r#"
default = "ssh-dev"

[[environments]]
id = "devbox"
url = "ws://127.0.0.1:4512"

[[environments]]
id = "ssh-dev"
program = "ssh"
args = ["dev", "codex exec-server --listen stdio"]
cwd = "/tmp"
[environments.env]
CODEX_LOG = "debug"
"#,
        )
        .expect("write environments.toml");

        let environments = load_environments_toml(&path).expect("environments.toml");

        assert_eq!(environments.default.as_deref(), Some("ssh-dev"));
        assert_eq!(environments.environments.len(), 2);
        assert_eq!(environments.environments[0].id, "devbox");
        assert_eq!(
            environments.environments[1],
            EnvironmentToml {
                id: "ssh-dev".to_string(),
                program: Some("ssh".to_string()),
                args: Some(vec![
                    "dev".to_string(),
                    "codex exec-server --listen stdio".to_string(),
                ]),
                env: Some(HashMap::from([(
                    "CODEX_LOG".to_string(),
                    "debug".to_string(),
                )])),
                cwd: Some(PathBuf::from("/tmp")),
                ..Default::default()
            }
        );
    }

    #[test]
    fn load_environments_toml_rejects_unknown_fields() {
        let codex_home = tempdir().expect("tempdir");
        let cases = [
            ("unknown = true\n", "unknown field `unknown`"),
            (
                r#"
[[environments]]
id = "devbox"
url = "ws://127.0.0.1:4512"
unknown = true
"#,
                "unknown field `unknown`",
            ),
        ];

        for (index, (contents, expected)) in cases.into_iter().enumerate() {
            let path = codex_home.path().join(format!("environments-{index}.toml"));
            std::fs::write(&path, contents).expect("write environments.toml");

            let err = load_environments_toml(&path).expect_err("unknown field should fail");

            assert!(
                err.to_string().contains(expected),
                "expected `{err}` to contain `{expected}`"
            );
        }
    }

    #[test]
    fn toml_provider_rejects_malformed_websocket_url() {
        let err = TomlEnvironmentProvider::new(EnvironmentsToml {
            default: None,
            environments: vec![EnvironmentToml {
                id: "devbox".to_string(),
                url: Some("ws://".to_string()),
                ..Default::default()
            }],
        })
        .expect_err("malformed websocket url should fail");

        assert!(
            err.to_string()
                .contains("environment url `ws://` is invalid"),
            "expected malformed URL error, got `{err}`"
        );
    }

    #[tokio::test]
    async fn environment_provider_from_codex_home_uses_present_environments_file() {
        let codex_home = tempdir().expect("tempdir");
        std::fs::write(
            codex_home.path().join(ENVIRONMENTS_TOML_FILE),
            r#"
default = "none"
"#,
        )
        .expect("write environments.toml");

        let provider =
            environment_provider_from_codex_home(codex_home.path()).expect("environment provider");

        let snapshot = provider
            .snapshot(&test_runtime_paths())
            .await
            .expect("environments");
        let environment_ids: Vec<_> = snapshot
            .environments
            .into_iter()
            .map(|(id, _environment)| id)
            .collect();

        assert!(environment_ids.contains(&LOCAL_ENVIRONMENT_ID.to_string()));
        assert_eq!(snapshot.default, EnvironmentDefault::Disabled);
    }

    #[tokio::test]
    async fn environment_provider_from_codex_home_falls_back_when_file_is_missing() {
        let codex_home = tempdir().expect("tempdir");

        let provider =
            environment_provider_from_codex_home(codex_home.path()).expect("environment provider");

        let snapshot = provider
            .snapshot(&test_runtime_paths())
            .await
            .expect("environments");
        let environment_ids: Vec<_> = snapshot
            .environments
            .into_iter()
            .map(|(id, _environment)| id)
            .collect();

        assert!(environment_ids.contains(&LOCAL_ENVIRONMENT_ID.to_string()));
    }
}
