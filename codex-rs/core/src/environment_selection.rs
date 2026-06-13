use std::collections::HashSet;
use std::sync::Arc;

use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecutorFileSystem;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;

use crate::session::turn_context::TurnEnvironment;
use crate::shell::Shell;

pub(crate) fn default_thread_environment_selections(
    environment_manager: &EnvironmentManager,
    cwd: &AbsolutePathBuf,
) -> Vec<TurnEnvironmentSelection> {
    environment_manager
        .default_environment_ids()
        .into_iter()
        .map(|environment_id| TurnEnvironmentSelection {
            environment_id,
            cwd: PathUri::from_abs_path(cwd),
        })
        .collect()
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ResolvedTurnEnvironments {
    pub(crate) turn_environments: Vec<TurnEnvironment>,
}

impl ResolvedTurnEnvironments {
    pub(crate) fn to_selections(&self) -> Vec<TurnEnvironmentSelection> {
        self.turn_environments
            .iter()
            .map(TurnEnvironment::selection)
            .collect()
    }

    pub(crate) fn primary(&self) -> Option<&TurnEnvironment> {
        self.turn_environments.first()
    }

    #[cfg(test)]
    pub(crate) fn primary_environment(&self) -> Option<Arc<codex_exec_server::Environment>> {
        self.primary()
            .map(|environment| Arc::clone(&environment.environment))
    }

    pub(crate) fn primary_filesystem(&self) -> Option<Arc<dyn ExecutorFileSystem>> {
        self.primary()
            .map(|environment| environment.environment.get_filesystem())
    }

    pub(crate) fn single_local_environment_cwd(&self) -> Option<AbsolutePathBuf> {
        let [environment] = self.turn_environments.as_slice() else {
            return None;
        };

        (!environment.environment.is_remote())
            .then(|| environment.compatible_cwd())
            .flatten()
    }
}

pub(crate) async fn resolve_environment_selections(
    environment_manager: &EnvironmentManager,
    environments: &[TurnEnvironmentSelection],
) -> CodexResult<ResolvedTurnEnvironments> {
    let mut seen_environment_ids = HashSet::with_capacity(environments.len());
    let mut turn_environments = Vec::with_capacity(environments.len());
    for selected_environment in environments {
        if !seen_environment_ids.insert(selected_environment.environment_id.as_str()) {
            return Err(CodexErr::InvalidRequest(format!(
                "duplicate turn environment id `{}`",
                selected_environment.environment_id
            )));
        }
        let environment_id = selected_environment.environment_id.clone();
        let environment = environment_manager
            .get_environment(&environment_id)
            .ok_or_else(|| {
                CodexErr::InvalidRequest(format!("unknown turn environment id `{environment_id}`"))
            })?;
        let info = environment.info().await.map_err(|err| {
            CodexErr::InvalidRequest(format!(
                "failed to get info for environment `{environment_id}`: {err}"
            ))
        })?;
        let path_convention = info.path_convention;
        let shell = Shell::from_environment_shell_info(info.shell).map_err(|err| {
            CodexErr::InvalidRequest(format!(
                "failed to resolve shell for environment `{environment_id}`: {err}"
            ))
        })?;
        turn_environments.push(TurnEnvironment::new_with_uri(
            environment_id,
            environment,
            selected_environment.cwd.clone(),
            path_convention,
            shell,
        )?);
    }
    Ok(ResolvedTurnEnvironments { turn_environments })
}

#[cfg(test)]
mod tests {
    use codex_app_server_protocol::JSONRPCMessage;
    use codex_app_server_protocol::JSONRPCResponse;
    use codex_exec_server::Environment;
    use codex_exec_server::EnvironmentInfo;
    use codex_exec_server::ExecServerRuntimePaths;
    use codex_exec_server::InitializeResponse;
    use codex_exec_server::LOCAL_ENVIRONMENT_ID;
    use codex_exec_server::REMOTE_ENVIRONMENT_ID;
    use codex_exec_server::ShellInfo;
    use codex_protocol::protocol::TurnEnvironmentSelection;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use codex_utils_path_uri::PathConvention;
    use futures::SinkExt;
    use futures::StreamExt;
    use pretty_assertions::assert_eq;
    use tokio::net::TcpListener;
    use tokio::net::TcpStream;
    use tokio::task::JoinHandle;
    use tokio::time::Duration;
    use tokio::time::timeout;
    use tokio_tungstenite::WebSocketStream;
    use tokio_tungstenite::accept_async;
    use tokio_tungstenite::tungstenite::Message;

    use super::*;

    async fn spawn_environment_info_server(info: EnvironmentInfo) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("environment info server should bind");
        let websocket_url = format!(
            "ws://{}",
            listener.local_addr().expect("environment info address")
        );
        let server = tokio::spawn(async move {
            let (stream, _) = timeout(Duration::from_secs(1), listener.accept())
                .await
                .expect("environment info connection should not time out")
                .expect("environment info connection should succeed");
            let mut websocket = accept_async(stream)
                .await
                .expect("environment info websocket handshake should succeed");

            let initialize = match read_jsonrpc_websocket(&mut websocket).await {
                JSONRPCMessage::Request(request) if request.method == "initialize" => request,
                other => panic!("expected initialize request, got {other:?}"),
            };
            write_jsonrpc_websocket(
                &mut websocket,
                JSONRPCMessage::Response(JSONRPCResponse {
                    id: initialize.id,
                    result: serde_json::to_value(InitializeResponse {
                        session_id: "session-1".to_string(),
                    })
                    .expect("initialize response should serialize"),
                }),
            )
            .await;

            match read_jsonrpc_websocket(&mut websocket).await {
                JSONRPCMessage::Notification(notification)
                    if notification.method == "initialized" => {}
                other => panic!("expected initialized notification, got {other:?}"),
            }
            let environment_info = match read_jsonrpc_websocket(&mut websocket).await {
                JSONRPCMessage::Request(request) if request.method == "environment/info" => request,
                other => panic!("expected environment/info request, got {other:?}"),
            };
            write_jsonrpc_websocket(
                &mut websocket,
                JSONRPCMessage::Response(JSONRPCResponse {
                    id: environment_info.id,
                    result: serde_json::to_value(info)
                        .expect("environment info response should serialize"),
                }),
            )
            .await;
        });

        (websocket_url, server)
    }

    async fn read_jsonrpc_websocket(websocket: &mut WebSocketStream<TcpStream>) -> JSONRPCMessage {
        loop {
            match timeout(Duration::from_secs(1), websocket.next())
                .await
                .expect("environment info read should not time out")
                .expect("environment info websocket should stay open")
                .expect("environment info frame should read")
            {
                Message::Text(text) => {
                    return serde_json::from_str(text.as_ref())
                        .expect("environment info text frame should parse");
                }
                Message::Binary(bytes) => {
                    return serde_json::from_slice(bytes.as_ref())
                        .expect("environment info binary frame should parse");
                }
                Message::Ping(_) | Message::Pong(_) => {}
                other => panic!("expected environment info JSON-RPC frame, got {other:?}"),
            }
        }
    }

    async fn write_jsonrpc_websocket(
        websocket: &mut WebSocketStream<TcpStream>,
        message: JSONRPCMessage,
    ) {
        let encoded = serde_json::to_string(&message).expect("JSON-RPC response should serialize");
        websocket
            .send(Message::Text(encoded.into()))
            .await
            .expect("environment info response should write");
    }

    fn test_runtime_paths() -> ExecServerRuntimePaths {
        ExecServerRuntimePaths::new(
            std::env::current_exe().expect("current exe"),
            /*codex_linux_sandbox_exe*/ None,
        )
        .expect("runtime paths")
    }

    #[tokio::test]
    async fn default_thread_environment_selections_use_manager_default_id() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let manager = EnvironmentManager::create_for_tests(
            Some("ws://127.0.0.1:8765".to_string()),
            Some(test_runtime_paths()),
        )
        .await;

        assert_eq!(
            default_thread_environment_selections(&manager, &cwd),
            vec![TurnEnvironmentSelection {
                environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                cwd: PathUri::from_abs_path(&cwd),
            }]
        );
    }

    #[tokio::test]
    async fn toml_default_thread_environment_selections_include_local_and_remote() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp_dir.path().join("environments.toml"),
            r#"
[[environments]]
id = "remote"
url = "ws://127.0.0.1:8765"
"#,
        )
        .expect("write environments.toml");
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let manager =
            EnvironmentManager::from_codex_home(temp_dir.path(), Some(test_runtime_paths()))
                .await
                .expect("environment manager");

        assert_eq!(
            default_thread_environment_selections(&manager, &cwd),
            vec![
                TurnEnvironmentSelection {
                    environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                    cwd: PathUri::from_abs_path(&cwd),
                },
                TurnEnvironmentSelection {
                    environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                    cwd: PathUri::from_abs_path(&cwd),
                },
            ]
        );
    }

    #[tokio::test]
    async fn default_thread_environment_selections_empty_when_default_disabled() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let manager = EnvironmentManager::without_environments();

        assert_eq!(
            default_thread_environment_selections(&manager, &cwd),
            Vec::<TurnEnvironmentSelection>::new()
        );
    }

    #[tokio::test]
    async fn resolve_environment_selections_rejects_duplicate_ids() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let manager = EnvironmentManager::default_for_tests();

        let err = resolve_environment_selections(
            &manager,
            &[
                TurnEnvironmentSelection {
                    environment_id: "local".to_string(),
                    cwd: PathUri::from_abs_path(&cwd),
                },
                TurnEnvironmentSelection {
                    environment_id: "local".to_string(),
                    cwd: PathUri::from_abs_path(&cwd.join("other")),
                },
            ],
        )
        .await
        .expect_err("duplicate environment id should fail");

        assert!(err.to_string().contains("duplicate"));
    }

    #[tokio::test]
    async fn resolve_environment_selections_rejects_unknown_shell_metadata() {
        let (websocket_url, server) = spawn_environment_info_server(EnvironmentInfo {
            shell: ShellInfo {
                name: "fish".to_string(),
                path: "/usr/bin/fish".to_string(),
            },
            path_convention: PathConvention::native(),
        })
        .await;
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let manager = EnvironmentManager::default_for_tests();
        manager
            .upsert_environment("malformed".to_string(), websocket_url)
            .expect("malformed metadata environment should register");

        let error = resolve_environment_selections(
            &manager,
            &[TurnEnvironmentSelection {
                environment_id: "malformed".to_string(),
                cwd: PathUri::from_abs_path(&cwd),
            }],
        )
        .await
        .expect_err("unknown shell metadata must reject the selected environment");

        assert_eq!(
            error.to_string(),
            "failed to resolve shell for environment `malformed`: unknown environment shell `fish`"
        );
        server.await.expect("environment info server should finish");
    }

    #[cfg(unix)]
    #[test]
    fn foreign_remote_environment_requires_explicit_native_cwd() {
        let environment = Arc::new(
            Environment::create_for_tests(Some("ws://127.0.0.1:8765".to_string()))
                .expect("remote environment"),
        );
        let error = TurnEnvironment::new_with_uri(
            REMOTE_ENVIRONMENT_ID.to_string(),
            environment,
            PathUri::parse("file:///workspace").expect("POSIX cwd URI"),
            codex_utils_path_uri::PathConvention::Windows,
            crate::shell::default_user_shell(),
        )
        .expect_err("host cwd must not be projected into a foreign environment");

        assert_eq!(
            error.to_string(),
            "explicit environment cwd required for foreign environment `remote` using Windows path syntax"
        );
    }

    #[tokio::test]
    async fn resolved_environment_selections_use_first_selection_as_primary() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let selected_cwd = cwd.join("selected");
        let manager = EnvironmentManager::default_for_tests();

        let resolved = resolve_environment_selections(
            &manager,
            &[TurnEnvironmentSelection {
                environment_id: "local".to_string(),
                cwd: PathUri::from_abs_path(&selected_cwd),
            }],
        )
        .await
        .expect("environment selections should resolve");

        assert_eq!(
            resolved
                .primary()
                .expect("primary environment")
                .environment_id,
            "local"
        );
        assert_eq!(
            resolved.primary().expect("primary environment").shell,
            Shell::from_environment_shell_info(
                manager
                    .get_environment("local")
                    .expect("local environment")
                    .info()
                    .await
                    .expect("local environment info")
                    .shell
            )
            .expect("resolved shell")
        );
    }

    #[tokio::test]
    async fn single_local_environment_cwd_requires_exactly_one_local_environment() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let local_manager = EnvironmentManager::default_for_tests();
        let local = resolve_environment_selections(
            &local_manager,
            &[TurnEnvironmentSelection {
                environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                cwd: PathUri::from_abs_path(&cwd),
            }],
        )
        .await
        .expect("local environment should resolve");
        let remote_environment = Arc::new(
            Environment::create_for_tests(Some("ws://127.0.0.1:8765".to_string()))
                .expect("remote environment"),
        );
        let remote = ResolvedTurnEnvironments {
            turn_environments: vec![TurnEnvironment::new(
                REMOTE_ENVIRONMENT_ID.to_string(),
                remote_environment.clone(),
                cwd.clone(),
                crate::shell::default_user_shell(),
            )],
        };
        let multiple = ResolvedTurnEnvironments {
            turn_environments: vec![
                local.primary().expect("local environment").clone(),
                TurnEnvironment::new(
                    REMOTE_ENVIRONMENT_ID.to_string(),
                    remote_environment,
                    cwd.clone(),
                    crate::shell::default_user_shell(),
                ),
            ],
        };

        assert_eq!(local.single_local_environment_cwd(), Some(cwd.clone()));
        assert_eq!(remote.single_local_environment_cwd(), None);
        assert_eq!(multiple.single_local_environment_cwd(), None);
    }
}
