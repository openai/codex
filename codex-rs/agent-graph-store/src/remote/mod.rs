mod helpers;

use async_trait::async_trait;
use codex_protocol::ThreadId;
use proto::agent_graph_store_client::AgentGraphStoreClient;

use crate::AgentGraphStore;
use crate::AgentGraphStoreError;
use crate::AgentGraphStoreResult;
use crate::ThreadSpawnEdgeStatus;

#[path = "proto/codex.agent_graph_store.v1.rs"]
mod proto;

/// gRPC-backed [`AgentGraphStore`] implementation for deployments whose durable
/// subagent graph lives outside the app-server process.
#[derive(Clone, Debug)]
pub struct RemoteAgentGraphStore {
    endpoint: String,
}

impl RemoteAgentGraphStore {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
        }
    }

    async fn client(
        &self,
    ) -> AgentGraphStoreResult<AgentGraphStoreClient<tonic::transport::Channel>> {
        AgentGraphStoreClient::connect(self.endpoint.clone())
            .await
            .map_err(|err| AgentGraphStoreError::Internal {
                message: format!("failed to connect to remote agent graph store: {err}"),
            })
    }
}

#[async_trait]
impl AgentGraphStore for RemoteAgentGraphStore {
    async fn upsert_thread_spawn_edge(
        &self,
        parent_thread_id: ThreadId,
        child_thread_id: ThreadId,
        status: ThreadSpawnEdgeStatus,
    ) -> AgentGraphStoreResult<()> {
        let request = proto::UpsertThreadSpawnEdgeRequest {
            parent_thread_id: parent_thread_id.to_string(),
            child_thread_id: child_thread_id.to_string(),
            status: helpers::proto_status(status).into(),
        };
        self.client()
            .await?
            .upsert_thread_spawn_edge(request)
            .await
            .map_err(helpers::remote_status_to_error)?;
        Ok(())
    }

    async fn set_thread_spawn_edge_status(
        &self,
        child_thread_id: ThreadId,
        status: ThreadSpawnEdgeStatus,
    ) -> AgentGraphStoreResult<()> {
        let request = proto::SetThreadSpawnEdgeStatusRequest {
            child_thread_id: child_thread_id.to_string(),
            status: helpers::proto_status(status).into(),
        };
        self.client()
            .await?
            .set_thread_spawn_edge_status(request)
            .await
            .map_err(helpers::remote_status_to_error)?;
        Ok(())
    }

    async fn list_thread_spawn_children(
        &self,
        parent_thread_id: ThreadId,
        status_filter: Option<ThreadSpawnEdgeStatus>,
    ) -> AgentGraphStoreResult<Vec<ThreadId>> {
        let response = self
            .client()
            .await?
            .list_thread_spawn_children(proto::ListThreadSpawnChildrenRequest {
                parent_thread_id: parent_thread_id.to_string(),
                status_filter: helpers::proto_status_filter(status_filter),
            })
            .await
            .map_err(helpers::remote_status_to_error)?
            .into_inner();
        helpers::thread_ids_from_proto(response.thread_ids, "child thread_id")
    }

    async fn list_thread_spawn_descendants(
        &self,
        root_thread_id: ThreadId,
        status_filter: Option<ThreadSpawnEdgeStatus>,
    ) -> AgentGraphStoreResult<Vec<ThreadId>> {
        let response = self
            .client()
            .await?
            .list_thread_spawn_descendants(proto::ListThreadSpawnDescendantsRequest {
                root_thread_id: root_thread_id.to_string(),
                status_filter: helpers::proto_status_filter(status_filter),
            })
            .await
            .map_err(helpers::remote_status_to_error)?
            .into_inner();
        helpers::thread_ids_from_proto(response.thread_ids, "descendant thread_id")
    }
}

#[cfg(test)]
mod tests {
    use super::proto;
    use super::proto::agent_graph_store_server;
    use super::proto::agent_graph_store_server::AgentGraphStoreServer;
    use super::*;
    use pretty_assertions::assert_eq;
    use tonic::Request;
    use tonic::Response;
    use tonic::Status;
    use tonic::transport::Server;

    fn thread_id(suffix: u128) -> ThreadId {
        ThreadId::from_string(&format!("00000000-0000-0000-0000-{suffix:012}"))
            .expect("valid thread id")
    }

    async fn serve_test_server(server: TestServer) -> (RemoteAgentGraphStore, ServerShutdown) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let addr = listener.local_addr().expect("test server addr");
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let handle = tokio::spawn(async move {
            Server::builder()
                .add_service(AgentGraphStoreServer::new(server))
                .serve_with_incoming_shutdown(
                    tokio_stream::wrappers::TcpListenerStream::new(listener),
                    async {
                        let _ = shutdown_rx.await;
                    },
                )
                .await
        });

        (
            RemoteAgentGraphStore::new(format!("http://{addr}")),
            ServerShutdown {
                shutdown_tx,
                handle,
            },
        )
    }

    struct ServerShutdown {
        shutdown_tx: tokio::sync::oneshot::Sender<()>,
        handle: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
    }

    impl ServerShutdown {
        async fn shutdown(self) {
            let _ = self.shutdown_tx.send(());
            self.handle.await.expect("join server").expect("server");
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum TestServer {
        HappyPath,
        InvalidThreadIdResponse,
        InvalidArgumentStatus,
    }

    #[tonic::async_trait]
    impl agent_graph_store_server::AgentGraphStore for TestServer {
        async fn upsert_thread_spawn_edge(
            &self,
            request: Request<proto::UpsertThreadSpawnEdgeRequest>,
        ) -> Result<Response<proto::Empty>, Status> {
            match self {
                TestServer::InvalidArgumentStatus => {
                    Err(Status::invalid_argument("status must be specified"))
                }
                TestServer::HappyPath | TestServer::InvalidThreadIdResponse => {
                    let request = request.into_inner();
                    assert_eq!(
                        request.parent_thread_id,
                        "00000000-0000-0000-0000-000000000001"
                    );
                    assert_eq!(
                        request.child_thread_id,
                        "00000000-0000-0000-0000-000000000002"
                    );
                    assert_eq!(
                        proto::ThreadSpawnEdgeStatus::try_from(request.status),
                        Ok(proto::ThreadSpawnEdgeStatus::Open)
                    );
                    Ok(Response::new(proto::Empty {}))
                }
            }
        }

        async fn set_thread_spawn_edge_status(
            &self,
            request: Request<proto::SetThreadSpawnEdgeStatusRequest>,
        ) -> Result<Response<proto::Empty>, Status> {
            let request = request.into_inner();
            assert_eq!(
                request.child_thread_id,
                "00000000-0000-0000-0000-000000000002"
            );
            assert_eq!(
                proto::ThreadSpawnEdgeStatus::try_from(request.status),
                Ok(proto::ThreadSpawnEdgeStatus::Closed)
            );
            Ok(Response::new(proto::Empty {}))
        }

        async fn list_thread_spawn_children(
            &self,
            request: Request<proto::ListThreadSpawnChildrenRequest>,
        ) -> Result<Response<proto::ListThreadSpawnChildrenResponse>, Status> {
            let request = request.into_inner();
            assert_eq!(
                request.parent_thread_id,
                "00000000-0000-0000-0000-000000000001"
            );
            assert_eq!(
                request
                    .status_filter
                    .map(proto::ThreadSpawnEdgeStatus::try_from),
                Some(Ok(proto::ThreadSpawnEdgeStatus::Open))
            );
            let thread_ids = match self {
                TestServer::InvalidThreadIdResponse => vec!["not-a-thread-id".to_string()],
                TestServer::HappyPath | TestServer::InvalidArgumentStatus => {
                    vec![
                        "00000000-0000-0000-0000-000000000002".to_string(),
                        "00000000-0000-0000-0000-000000000003".to_string(),
                    ]
                }
            };
            Ok(Response::new(proto::ListThreadSpawnChildrenResponse {
                thread_ids,
            }))
        }

        async fn list_thread_spawn_descendants(
            &self,
            request: Request<proto::ListThreadSpawnDescendantsRequest>,
        ) -> Result<Response<proto::ListThreadSpawnDescendantsResponse>, Status> {
            let request = request.into_inner();
            assert_eq!(
                request.root_thread_id,
                "00000000-0000-0000-0000-000000000001"
            );
            assert_eq!(request.status_filter, None);
            Ok(Response::new(proto::ListThreadSpawnDescendantsResponse {
                thread_ids: vec![
                    "00000000-0000-0000-0000-000000000002".to_string(),
                    "00000000-0000-0000-0000-000000000003".to_string(),
                    "00000000-0000-0000-0000-000000000004".to_string(),
                ],
            }))
        }
    }

    #[tokio::test]
    async fn remote_store_calls_agent_graph_service() {
        let (store, shutdown) = serve_test_server(TestServer::HappyPath).await;
        let parent_thread_id = thread_id(/*suffix*/ 1);
        let child_thread_id = thread_id(/*suffix*/ 2);

        store
            .upsert_thread_spawn_edge(
                parent_thread_id,
                child_thread_id,
                ThreadSpawnEdgeStatus::Open,
            )
            .await
            .expect("upsert should succeed");
        store
            .set_thread_spawn_edge_status(child_thread_id, ThreadSpawnEdgeStatus::Closed)
            .await
            .expect("status update should succeed");

        let children = store
            .list_thread_spawn_children(parent_thread_id, Some(ThreadSpawnEdgeStatus::Open))
            .await
            .expect("children should load");
        assert_eq!(
            children,
            vec![thread_id(/*suffix*/ 2), thread_id(/*suffix*/ 3)]
        );

        let descendants = store
            .list_thread_spawn_descendants(parent_thread_id, /*status_filter*/ None)
            .await
            .expect("descendants should load");
        assert_eq!(
            descendants,
            vec![
                thread_id(/*suffix*/ 2),
                thread_id(/*suffix*/ 3),
                thread_id(/*suffix*/ 4),
            ]
        );

        shutdown.shutdown().await;
    }

    #[tokio::test]
    async fn remote_store_maps_invalid_response_thread_id_to_invalid_request() {
        let (store, shutdown) = serve_test_server(TestServer::InvalidThreadIdResponse).await;

        let err = store
            .list_thread_spawn_children(thread_id(/*suffix*/ 1), Some(ThreadSpawnEdgeStatus::Open))
            .await
            .expect_err("invalid response thread id should fail");

        assert!(matches!(
            err,
            AgentGraphStoreError::InvalidRequest { message } if message.contains("invalid child thread_id")
        ));

        shutdown.shutdown().await;
    }

    #[tokio::test]
    async fn remote_store_maps_invalid_argument_status_to_invalid_request() {
        let (store, shutdown) = serve_test_server(TestServer::InvalidArgumentStatus).await;

        let err = store
            .upsert_thread_spawn_edge(
                thread_id(/*suffix*/ 1),
                thread_id(/*suffix*/ 2),
                ThreadSpawnEdgeStatus::Open,
            )
            .await
            .expect_err("invalid argument should fail");

        assert!(matches!(
            err,
            AgentGraphStoreError::InvalidRequest { message } if message == "status must be specified"
        ));

        shutdown.shutdown().await;
    }
}
