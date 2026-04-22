use super::RemoteThreadStore;
use super::helpers::proto_session_source;
use super::helpers::proto_sort_key;
use super::helpers::remote_status_to_error;
use super::helpers::stored_thread_from_proto;
use super::proto;
use crate::ListThreadsParams;
use crate::ThreadPage;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

pub(super) async fn list_threads(
    store: &RemoteThreadStore,
    params: ListThreadsParams,
) -> ThreadStoreResult<ThreadPage> {
    let request = proto::ListThreadsRequest {
        page_size: params
            .page_size
            .try_into()
            .map_err(|_| ThreadStoreError::InvalidRequest {
                message: format!("page_size is too large: {}", params.page_size),
            })?,
        cursor: params.cursor,
        sort_key: proto_sort_key(params.sort_key).into(),
        allowed_sources: params
            .allowed_sources
            .iter()
            .map(proto_session_source)
            .collect(),
        model_provider_filter: params
            .model_providers
            .map(|values| proto::ModelProviderFilter { values }),
        archived: params.archived,
        search_term: params.search_term,
    };

    let response = store
        .client()
        .await?
        .list_threads(request)
        .await
        .map_err(remote_status_to_error)?
        .into_inner();

    let items = response
        .threads
        .into_iter()
        .map(stored_thread_from_proto)
        .collect::<ThreadStoreResult<Vec<_>>>()?;

    Ok(ThreadPage {
        items,
        next_cursor: response.next_cursor,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::Mutex;

    use codex_protocol::openai_models::ReasoningEffort;
    use codex_protocol::protocol::SessionSource;
    use pretty_assertions::assert_eq;
    use tonic::Request;
    use tonic::Response;
    use tonic::Status;
    use tonic::transport::Server;

    use super::super::helpers::stored_thread_to_proto;
    use super::super::proto::thread_store_server;
    use super::super::proto::thread_store_server::ThreadStoreServer;
    use super::*;
    use crate::IDENTITY_KEY_HEADER;
    use crate::ThreadSortKey;
    use crate::ThreadStore;

    #[derive(Default)]
    struct TestServer {
        captured_identity_key: Option<Arc<Mutex<Option<Vec<u8>>>>>,
    }

    #[tonic::async_trait]
    impl thread_store_server::ThreadStore for TestServer {
        async fn list_threads(
            &self,
            request: Request<proto::ListThreadsRequest>,
        ) -> Result<Response<proto::ListThreadsResponse>, Status> {
            if let Some(captured_identity_key) = &self.captured_identity_key {
                let identity_key = request
                    .metadata()
                    .get_bin(IDENTITY_KEY_HEADER)
                    .and_then(|value| value.to_bytes().ok())
                    .map(|value| value.to_vec());
                *captured_identity_key
                    .lock()
                    .expect("captured identity key mutex poisoned") = identity_key;
            }
            let request = request.into_inner();
            assert_eq!(request.page_size, 2);
            assert_eq!(request.cursor.as_deref(), Some("cursor-1"));
            assert_eq!(
                proto::ThreadSortKey::try_from(request.sort_key),
                Ok(proto::ThreadSortKey::UpdatedAt)
            );
            assert_eq!(request.archived, true);
            assert_eq!(request.search_term.as_deref(), Some("needle"));
            assert_eq!(
                request.model_provider_filter,
                Some(proto::ModelProviderFilter {
                    values: vec!["openai".to_string()],
                })
            );
            assert_eq!(request.allowed_sources.len(), 1);
            assert_eq!(
                proto::SessionSourceKind::try_from(request.allowed_sources[0].kind),
                Ok(proto::SessionSourceKind::Cli)
            );

            Ok(Response::new(proto::ListThreadsResponse {
                threads: vec![proto::StoredThread {
                    thread_id: "11111111-1111-1111-1111-111111111111".to_string(),
                    forked_from_id: None,
                    preview: "hello".to_string(),
                    name: Some("named thread".to_string()),
                    model_provider: "openai".to_string(),
                    model: Some("gpt-5".to_string()),
                    created_at: 100,
                    updated_at: 200,
                    archived_at: Some(300),
                    cwd: "/workspace".to_string(),
                    cli_version: "1.2.3".to_string(),
                    source: Some(proto::SessionSource {
                        kind: proto::SessionSourceKind::Cli.into(),
                        ..Default::default()
                    }),
                    git_info: Some(proto::GitInfo {
                        sha: Some("abc123".to_string()),
                        branch: Some("main".to_string()),
                        origin_url: Some("https://example.test/repo.git".to_string()),
                    }),
                    agent_nickname: None,
                    agent_role: None,
                    agent_path: None,
                    reasoning_effort: Some("medium".to_string()),
                    first_user_message: Some("hello".to_string()),
                }],
                next_cursor: Some("cursor-2".to_string()),
            }))
        }
    }

    #[tokio::test]
    async fn list_threads_calls_remote_service() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let addr = listener.local_addr().expect("test server addr");
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            Server::builder()
                .add_service(ThreadStoreServer::new(TestServer::default()))
                .serve_with_incoming_shutdown(
                    tokio_stream::wrappers::TcpListenerStream::new(listener),
                    async {
                        let _ = shutdown_rx.await;
                    },
                )
                .await
        });

        let store = RemoteThreadStore::new(format!("http://{addr}"));
        let page = store
            .list_threads(ListThreadsParams {
                page_size: 2,
                cursor: Some("cursor-1".to_string()),
                sort_key: ThreadSortKey::UpdatedAt,
                allowed_sources: vec![SessionSource::Cli],
                model_providers: Some(vec!["openai".to_string()]),
                archived: true,
                search_term: Some("needle".to_string()),
            })
            .await
            .expect("list threads");

        assert_eq!(page.next_cursor.as_deref(), Some("cursor-2"));
        assert_eq!(page.items.len(), 1);
        let item = &page.items[0];
        assert_eq!(
            item.thread_id.to_string(),
            "11111111-1111-1111-1111-111111111111"
        );
        assert_eq!(item.name.as_deref(), Some("named thread"));
        assert_eq!(item.preview, "hello");
        assert_eq!(item.first_user_message.as_deref(), Some("hello"));
        assert_eq!(item.model_provider, "openai");
        assert_eq!(item.model.as_deref(), Some("gpt-5"));
        assert_eq!(item.created_at.timestamp(), 100);
        assert_eq!(item.updated_at.timestamp(), 200);
        assert_eq!(item.archived_at.map(|ts| ts.timestamp()), Some(300));
        assert_eq!(item.cwd, PathBuf::from("/workspace"));
        assert_eq!(item.cli_version, "1.2.3");
        assert_eq!(item.source, SessionSource::Cli);
        assert_eq!(item.reasoning_effort, Some(ReasoningEffort::Medium));
        assert_eq!(
            item.git_info.as_ref().and_then(|git| git.branch.as_deref()),
            Some("main")
        );

        let _ = shutdown_tx.send(());
        server.await.expect("join server").expect("server");
    }

    #[tokio::test]
    async fn list_threads_forwards_identity_key_as_metadata() {
        let captured_identity_key = Arc::new(Mutex::new(None));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let addr = listener.local_addr().expect("test server addr");
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let server_identity_key = captured_identity_key.clone();
        let server = tokio::spawn(async move {
            Server::builder()
                .add_service(ThreadStoreServer::new(TestServer {
                    captured_identity_key: Some(server_identity_key),
                }))
                .serve_with_incoming_shutdown(
                    tokio_stream::wrappers::TcpListenerStream::new(listener),
                    async {
                        let _ = shutdown_rx.await;
                    },
                )
                .await
        });

        let store = RemoteThreadStore::new_with_identity_key(
            format!("http://{addr}"),
            Some(b"tenant-key-\x00\xff".to_vec()),
        );
        store
            .list_threads(ListThreadsParams {
                page_size: 2,
                cursor: Some("cursor-1".to_string()),
                sort_key: ThreadSortKey::UpdatedAt,
                allowed_sources: vec![SessionSource::Cli],
                model_providers: Some(vec!["openai".to_string()]),
                archived: true,
                search_term: Some("needle".to_string()),
            })
            .await
            .expect("list threads");

        assert_eq!(
            captured_identity_key
                .lock()
                .expect("captured identity key mutex poisoned")
                .as_deref(),
            Some(&b"tenant-key-\x00\xff"[..])
        );

        let _ = shutdown_tx.send(());
        server.await.expect("join server").expect("server");
    }

    #[test]
    fn stored_thread_proto_roundtrips_through_domain_type() {
        let thread = proto::StoredThread {
            thread_id: "11111111-1111-1111-1111-111111111111".to_string(),
            forked_from_id: Some("22222222-2222-2222-2222-222222222222".to_string()),
            preview: "preview text".to_string(),
            name: Some("named thread".to_string()),
            model_provider: "openai".to_string(),
            model: Some("gpt-5".to_string()),
            created_at: 100,
            updated_at: 200,
            archived_at: Some(300),
            cwd: "/workspace/project".to_string(),
            cli_version: "1.2.3".to_string(),
            source: Some(proto::SessionSource {
                kind: proto::SessionSourceKind::SubAgentThreadSpawn.into(),
                sub_agent_parent_thread_id: Some(
                    "33333333-3333-3333-3333-333333333333".to_string(),
                ),
                sub_agent_depth: Some(2),
                sub_agent_path: Some("/root/review/backend".to_string()),
                sub_agent_nickname: Some("Navigator".to_string()),
                sub_agent_role: Some("explorer".to_string()),
                ..Default::default()
            }),
            git_info: Some(proto::GitInfo {
                sha: Some("abc123".to_string()),
                branch: Some("main".to_string()),
                origin_url: Some("https://example.test/repo.git".to_string()),
            }),
            agent_nickname: Some("Navigator".to_string()),
            agent_role: Some("explorer".to_string()),
            agent_path: Some("/root/review/backend".to_string()),
            reasoning_effort: Some("high".to_string()),
            first_user_message: Some("first message".to_string()),
        };

        let stored = stored_thread_from_proto(thread.clone()).expect("proto to stored thread");

        assert_eq!(stored.rollout_path, None);
        assert!(stored.history.is_none());
        assert_eq!(stored_thread_to_proto(stored), thread);
    }
}
