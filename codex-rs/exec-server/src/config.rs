use std::path::Path;
use std::time::Duration;

use serde::Deserialize;
use serde::Serialize;

use crate::ExecServerError;

pub const EXEC_SERVER_TOML_FILE: &str = "exec-server.toml";

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ExecServerConfig {
    pub listen: Option<String>,
    pub remote: Option<RemoteExecServerConfig>,
    pub runtime: ExecServerRuntimeConfig,
}

impl ExecServerConfig {
    pub fn from_codex_home(codex_home: &Path) -> Result<Self, ExecServerError> {
        let path = codex_home.join(EXEC_SERVER_TOML_FILE);
        if !path.try_exists().map_err(|err| {
            ExecServerError::Protocol(format!(
                "failed to inspect exec-server config `{}`: {err}",
                path.display()
            ))
        })? {
            return Ok(Self::default());
        }

        let contents = std::fs::read_to_string(&path).map_err(|err| {
            ExecServerError::Protocol(format!(
                "failed to read exec-server config `{}`: {err}",
                path.display()
            ))
        })?;
        let config: ExecServerToml = toml::from_str(&contents).map_err(|err| {
            ExecServerError::Protocol(format!(
                "failed to parse exec-server config `{}`: {err}",
                path.display()
            ))
        })?;
        config.try_into()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteExecServerConfig {
    pub url: Option<String>,
    pub environment_id: Option<String>,
    pub name: Option<String>,
    pub use_agent_identity_auth: bool,
    pub reconnect_initial_backoff: Duration,
    pub reconnect_max_backoff: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ExecServerRuntimeConfig {
    pub sessions: SessionRegistryConfig,
    pub processes: LocalProcessConfig,
    pub filesystem: LocalFileSystemConfig,
}

impl ExecServerRuntimeConfig {
    pub(crate) fn validate(&self) -> Result<(), ExecServerError> {
        self.sessions.validate()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionRegistryConfig {
    pub detached_session_ttl: Duration,
}

impl Default for SessionRegistryConfig {
    fn default() -> Self {
        Self {
            detached_session_ttl: default_detached_session_ttl(),
        }
    }
}

impl SessionRegistryConfig {
    fn validate(&self) -> Result<(), ExecServerError> {
        validate_detached_session_ttl(self.detached_session_ttl)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalProcessConfig {
    pub retained_output_bytes_per_process: usize,
    pub exited_process_retention: Duration,
}

impl Default for LocalProcessConfig {
    fn default() -> Self {
        Self {
            retained_output_bytes_per_process: 1024 * 1024,
            exited_process_retention: default_exited_process_retention(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalFileSystemConfig {
    pub max_read_file_bytes: u64,
}

impl Default for LocalFileSystemConfig {
    fn default() -> Self {
        Self {
            max_read_file_bytes: 512 * 1024 * 1024,
        }
    }
}

#[cfg(test)]
fn default_detached_session_ttl() -> Duration {
    Duration::from_millis(200)
}

#[cfg(not(test))]
fn default_detached_session_ttl() -> Duration {
    Duration::from_secs(10)
}

#[cfg(test)]
fn default_exited_process_retention() -> Duration {
    Duration::from_millis(25)
}

#[cfg(not(test))]
fn default_exited_process_retention() -> Duration {
    Duration::from_secs(30)
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct ExecServerToml {
    listen: Option<String>,
    remote: Option<RemoteExecServerToml>,
    sessions: Option<SessionsToml>,
    processes: Option<ProcessesToml>,
    filesystem: Option<FileSystemToml>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoteExecServerToml {
    url: Option<String>,
    environment_id: Option<String>,
    name: Option<String>,
    use_agent_identity_auth: Option<bool>,
    reconnect_initial_backoff_sec: Option<u64>,
    reconnect_max_backoff_sec: Option<u64>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct SessionsToml {
    detached_ttl_sec: Option<u64>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct ProcessesToml {
    retained_output_bytes_per_process: Option<usize>,
    exited_process_retention_sec: Option<u64>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct FileSystemToml {
    max_read_file_bytes: Option<u64>,
}

impl TryFrom<ExecServerToml> for ExecServerConfig {
    type Error = ExecServerError;

    fn try_from(config: ExecServerToml) -> Result<Self, Self::Error> {
        let ExecServerToml {
            listen,
            remote,
            sessions,
            processes,
            filesystem,
        } = config;
        if listen.is_some() && remote.is_some() {
            return Err(ExecServerError::Protocol(
                "exec-server config cannot set both `listen` and `[remote]`".to_string(),
            ));
        }

        let mut runtime = ExecServerRuntimeConfig::default();
        if let Some(sessions) = sessions
            && let Some(detached_ttl_sec) = sessions.detached_ttl_sec
        {
            let detached_session_ttl = Duration::from_secs(detached_ttl_sec);
            validate_detached_session_ttl(detached_session_ttl)?;
            runtime.sessions.detached_session_ttl = detached_session_ttl;
        }
        if let Some(processes) = processes {
            if let Some(retained_output_bytes_per_process) =
                processes.retained_output_bytes_per_process
            {
                runtime.processes.retained_output_bytes_per_process =
                    retained_output_bytes_per_process;
            }
            if let Some(exited_process_retention_sec) = processes.exited_process_retention_sec {
                runtime.processes.exited_process_retention =
                    Duration::from_secs(exited_process_retention_sec);
            }
        }
        if let Some(filesystem) = filesystem
            && let Some(max_read_file_bytes) = filesystem.max_read_file_bytes
        {
            runtime.filesystem.max_read_file_bytes = max_read_file_bytes;
        }

        runtime.validate()?;

        Ok(Self {
            listen,
            remote: remote.map(TryInto::try_into).transpose()?,
            runtime,
        })
    }
}

impl TryFrom<RemoteExecServerToml> for RemoteExecServerConfig {
    type Error = ExecServerError;

    fn try_from(config: RemoteExecServerToml) -> Result<Self, Self::Error> {
        let RemoteExecServerToml {
            url,
            environment_id,
            name,
            use_agent_identity_auth,
            reconnect_initial_backoff_sec,
            reconnect_max_backoff_sec,
        } = config;
        let reconnect_initial_backoff =
            Duration::from_secs(reconnect_initial_backoff_sec.unwrap_or(1));
        let reconnect_max_backoff = Duration::from_secs(reconnect_max_backoff_sec.unwrap_or(30));
        validate_reconnect_backoff(reconnect_initial_backoff, reconnect_max_backoff)?;
        Ok(Self {
            url,
            environment_id,
            name,
            use_agent_identity_auth: use_agent_identity_auth.unwrap_or(false),
            reconnect_initial_backoff,
            reconnect_max_backoff,
        })
    }
}

pub(crate) fn validate_reconnect_backoff(
    reconnect_initial_backoff: Duration,
    reconnect_max_backoff: Duration,
) -> Result<(), ExecServerError> {
    if reconnect_initial_backoff.is_zero() || reconnect_max_backoff.is_zero() {
        return Err(ExecServerError::Protocol(
            "remote reconnect backoff must be positive".to_string(),
        ));
    }
    if reconnect_initial_backoff > reconnect_max_backoff {
        return Err(ExecServerError::Protocol(
            "remote reconnect initial backoff must not exceed max backoff".to_string(),
        ));
    }
    Ok(())
}

fn validate_detached_session_ttl(detached_session_ttl: Duration) -> Result<(), ExecServerError> {
    if std::time::Instant::now()
        .checked_add(detached_session_ttl)
        .is_none()
    {
        return Err(ExecServerError::Protocol(
            "session detached TTL is too large".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn absent_file_uses_current_defaults() {
        let codex_home = tempdir().expect("tempdir");

        let config = ExecServerConfig::from_codex_home(codex_home.path()).expect("config");

        assert_eq!(config, ExecServerConfig::default());
    }

    #[test]
    fn file_loads_remote_and_runtime_settings() {
        let codex_home = tempdir().expect("tempdir");
        std::fs::write(
            codex_home.path().join(EXEC_SERVER_TOML_FILE),
            r#"
[remote]
url = "https://chatgpt.com"
environment_id = "devbox-123"
name = "remote-name"
use_agent_identity_auth = true
reconnect_initial_backoff_sec = 2
reconnect_max_backoff_sec = 10

[sessions]
detached_ttl_sec = 11

[processes]
retained_output_bytes_per_process = 12
exited_process_retention_sec = 13

[filesystem]
max_read_file_bytes = 14
"#,
        )
        .expect("write config");

        let config = ExecServerConfig::from_codex_home(codex_home.path()).expect("config");

        assert_eq!(
            config,
            ExecServerConfig {
                listen: None,
                remote: Some(RemoteExecServerConfig {
                    url: Some("https://chatgpt.com".to_string()),
                    environment_id: Some("devbox-123".to_string()),
                    name: Some("remote-name".to_string()),
                    use_agent_identity_auth: true,
                    reconnect_initial_backoff: Duration::from_secs(2),
                    reconnect_max_backoff: Duration::from_secs(10),
                }),
                runtime: ExecServerRuntimeConfig {
                    sessions: SessionRegistryConfig {
                        detached_session_ttl: Duration::from_secs(11),
                    },
                    processes: LocalProcessConfig {
                        retained_output_bytes_per_process: 12,
                        exited_process_retention: Duration::from_secs(13),
                    },
                    filesystem: LocalFileSystemConfig {
                        max_read_file_bytes: 14,
                    },
                },
            }
        );
    }

    #[test]
    fn file_loads_partial_remote_settings() {
        let codex_home = tempdir().expect("tempdir");
        std::fs::write(
            codex_home.path().join(EXEC_SERVER_TOML_FILE),
            r#"
[remote]
environment_id = "devbox-123"
name = "remote-name"
"#,
        )
        .expect("write config");

        let config = ExecServerConfig::from_codex_home(codex_home.path()).expect("config");

        assert_eq!(
            config.remote,
            Some(RemoteExecServerConfig {
                url: None,
                environment_id: Some("devbox-123".to_string()),
                name: Some("remote-name".to_string()),
                use_agent_identity_auth: false,
                reconnect_initial_backoff: Duration::from_secs(1),
                reconnect_max_backoff: Duration::from_secs(30),
            })
        );
    }

    #[test]
    fn file_rejects_listen_with_remote() {
        let codex_home = tempdir().expect("tempdir");
        std::fs::write(
            codex_home.path().join(EXEC_SERVER_TOML_FILE),
            r#"
listen = "ws://127.0.0.1:0"

[remote]
url = "https://chatgpt.com"
environment_id = "devbox-123"
"#,
        )
        .expect("write config");

        let err = ExecServerConfig::from_codex_home(codex_home.path()).expect_err("invalid config");

        assert_eq!(
            err.to_string(),
            "exec-server protocol error: exec-server config cannot set both `listen` and `[remote]`"
        );
    }

    #[test]
    fn file_rejects_unknown_fields() {
        let codex_home = tempdir().expect("tempdir");
        std::fs::write(
            codex_home.path().join(EXEC_SERVER_TOML_FILE),
            "unknown = true\n",
        )
        .expect("write config");

        let err = ExecServerConfig::from_codex_home(codex_home.path()).expect_err("invalid config");

        assert!(err.to_string().contains("unknown field `unknown`"));
    }

    #[test]
    fn file_rejects_invalid_remote_reconnect_backoff() {
        for (initial, max, expected) in [
            (0, 30, "remote reconnect backoff must be positive"),
            (
                31,
                30,
                "remote reconnect initial backoff must not exceed max backoff",
            ),
        ] {
            let config = ExecServerToml {
                remote: Some(RemoteExecServerToml {
                    url: Some("https://chatgpt.com".to_string()),
                    environment_id: Some("devbox-123".to_string()),
                    name: None,
                    use_agent_identity_auth: None,
                    reconnect_initial_backoff_sec: Some(initial),
                    reconnect_max_backoff_sec: Some(max),
                }),
                ..Default::default()
            };

            let err = ExecServerConfig::try_from(config).expect_err("invalid config");

            assert_eq!(
                err.to_string(),
                format!("exec-server protocol error: {expected}")
            );
        }
    }

    #[test]
    fn file_rejects_invalid_detached_session_ttl() {
        let config = ExecServerToml {
            sessions: Some(SessionsToml {
                detached_ttl_sec: Some(u64::MAX),
            }),
            ..Default::default()
        };

        let err = ExecServerConfig::try_from(config).expect_err("invalid config");

        assert_eq!(
            err.to_string(),
            "exec-server protocol error: session detached TTL is too large"
        );
    }

    #[test]
    fn runtime_rejects_invalid_detached_session_ttl() {
        let config = ExecServerRuntimeConfig {
            sessions: SessionRegistryConfig {
                detached_session_ttl: Duration::MAX,
            },
            ..Default::default()
        };

        let err = config.validate().expect_err("invalid config");

        assert_eq!(
            err.to_string(),
            "exec-server protocol error: session detached TTL is too large"
        );
    }
}
