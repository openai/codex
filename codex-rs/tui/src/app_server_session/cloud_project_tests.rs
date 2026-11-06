#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn github_origins_are_normalized_without_credentials_or_query_strings() {
    for origin in [
        "git@github.com:Org/Repo.git",
        "ssh://git@github.com/Org/Repo.git",
        "https://user:password@github.com/Org/Repo.git?token=secret#fragment",
    ] {
        let project = github_project(origin).unwrap();
        assert_eq!(
            (project.name, project.origin_urls),
            (
                "repo".to_owned(),
                vec!["https://github.com/org/repo".to_owned()]
            ),
        );
    }
}

#[tokio::test]
async fn cloud_starts_use_each_requests_host_project_without_caching() {
    use super::super::AppServerSession;
    use super::super::ThreadParamsMode;
    use crate::legacy_core::config::ConfigBuilder;
    use crate::local_settings::LocalSettings;
    use codex_app_server_client::AppServerClient;
    use codex_app_server_client::CloudClient;
    use codex_app_server_client::CloudOperation;
    use codex_app_server_protocol::ClientRequest;
    use codex_app_server_protocol::JSONRPCErrorError;

    let home = tempfile::tempdir().unwrap();
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await
        .unwrap();
    let local_settings = LocalSettings::from(&config);
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let misleading = tempfile::tempdir().unwrap();
    for (directory, origin) in [
        (first.path(), "git@github.com:org/first.git"),
        (second.path(), "ssh://git@ssh.github.com:443/Org/Second.git"),
        (
            misleading.path(),
            "https://example.com?target=@github.com/org/wrong",
        ),
    ] {
        for args in [
            vec!["init", "--quiet"],
            vec!["remote", "add", "origin", origin],
        ] {
            assert!(
                tokio::process::Command::new("git")
                    .args(args)
                    .current_dir(directory)
                    .status()
                    .await
                    .unwrap()
                    .success()
            );
        }
    }
    let nested = first.path().join("src");
    std::fs::create_dir(&nested).unwrap();
    let projectless = tempfile::tempdir().unwrap();
    let (client, mut commands, _events) = CloudClient::channel(/*capacity*/ 1);
    let mut session = AppServerSession::new(
        crate::backend_id::BackendId::new(),
        AppServerClient::Cloud(client),
        ThreadParamsMode::Cloud,
    );
    let worker = tokio::spawn(async move {
        for expected in [Some("first"), Some("second"), None, None, None, None] {
            let command = commands.recv().await.unwrap();
            match (expected, command.operation) {
                (
                    Some(expected),
                    CloudOperation::StartProjectThread {
                        params, project, ..
                    },
                ) => {
                    assert_eq!(project.name, expected);
                    assert_eq!(project.cwd, Some(format!("/workspace/org/{expected}")));
                    assert_eq!(params.cwd, None);
                    assert_eq!(params.runtime_workspace_roots, None);
                    assert_eq!(
                        project.origin_urls,
                        vec![format!("https://github.com/org/{expected}")]
                    );
                }
                (None, CloudOperation::Request(request)) => {
                    assert!(matches!(*request, ClientRequest::ThreadStart { .. }))
                }
                _ => panic!("incorrect project routing"),
            }
            command
                .response
                .send(Ok(Err(JSONRPCErrorError {
                    code: -32000,
                    message: "checkout failed".into(),
                    data: None,
                })))
                .unwrap();
        }
    });
    for (mode, host) in [
        (ThreadParamsMode::Cloud, Some(nested.as_path())),
        (ThreadParamsMode::Cloud, Some(second.path())),
        (ThreadParamsMode::Cloud, Some(misleading.path())),
        (ThreadParamsMode::Cloud, Some(projectless.path())),
        (ThreadParamsMode::Cloud, None),
        (ThreadParamsMode::Remote, Some(first.path())),
    ] {
        session.thread_params_mode = mode;
        let result = session
            .start_thread_with_session_start_source(
                &local_settings,
                &config,
                /*session_start_source*/ None,
                /*remote_cwd_override*/ None,
                /*selected_profile*/ None,
                host,
            )
            .await;
        assert!(result.unwrap_err().to_string().contains("checkout failed"));
    }
    worker.await.unwrap();
}

#[test]
fn unsupported_or_non_repository_origins_leave_the_thread_blank() {
    for origin in [
        "/local/repository",
        "github.com/org/repo",
        "https://git.example.com/org/repo",
        "http://ssh.github.com:443/org/repo.git",
        "ssh://git@ssh.github.com:8443/org/repo.git",
        "https://github.com/org",
        "https://github.com/org/repo/tree/main",
        "https://github.com/org/repo%2fother",
    ] {
        assert!(
            github_project(origin).is_none(),
            "unexpected origin: {origin}"
        );
    }
}
