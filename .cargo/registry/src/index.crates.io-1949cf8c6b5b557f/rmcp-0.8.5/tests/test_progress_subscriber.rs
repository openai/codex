use futures::StreamExt;
use rmcp::{
    ClientHandler, Peer, RoleServer, ServerHandler, ServiceExt,
    handler::{client::progress::ProgressDispatcher, server::tool::ToolRouter},
    model::{CallToolRequestParam, ClientRequest, Meta, ProgressNotificationParam, Request},
    service::PeerRequestOptions,
    tool, tool_handler, tool_router,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub struct MyClient {
    progress_handler: ProgressDispatcher,
}

impl MyClient {
    pub fn new() -> Self {
        Self {
            progress_handler: ProgressDispatcher::new(),
        }
    }
}

impl Default for MyClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientHandler for MyClient {
    async fn on_progress(
        &self,
        params: rmcp::model::ProgressNotificationParam,
        _context: rmcp::service::NotificationContext<rmcp::RoleClient>,
    ) {
        tracing::info!("Received progress notification: {:?}", params);
        self.progress_handler.handle_notification(params).await;
    }
}

pub struct MyServer {
    tool_router: ToolRouter<Self>,
}

impl MyServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

impl Default for MyServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl MyServer {
    #[tool]
    pub async fn some_progress(
        meta: Meta,
        client: Peer<RoleServer>,
    ) -> Result<(), rmcp::ErrorData> {
        let progress_token = meta
            .get_progress_token()
            .ok_or(rmcp::ErrorData::invalid_params(
                "Progress token is required for this tool",
                None,
            ))?;
        for step in 0..10 {
            let _ = client
                .notify_progress(ProgressNotificationParam {
                    progress_token: progress_token.clone(),
                    progress: (step as f64),
                    total: Some(10.0),
                    message: Some("Some message".into()),
                })
                .await;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        Ok(())
    }
}

#[tool_handler]
impl ServerHandler for MyServer {}

#[tokio::test]
async fn test_progress_subscriber() -> anyhow::Result<()> {
    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init();
    let client = MyClient::new();

    let server = MyServer::new();
    let (transport_server, transport_client) = tokio::io::duplex(4096);
    tokio::spawn(async move {
        let service = server.serve(transport_server).await?;
        service.waiting().await?;
        anyhow::Ok(())
    });
    let client_service = client.serve(transport_client).await?;
    let handle = client_service
        .send_cancellable_request(
            ClientRequest::CallToolRequest(Request::new(CallToolRequestParam {
                name: "some_progress".into(),
                arguments: None,
            })),
            PeerRequestOptions::no_options(),
        )
        .await?;
    let mut progress_subscriber = client_service
        .service()
        .progress_handler
        .subscribe(handle.progress_token.clone())
        .await;
    tokio::spawn(async move {
        while let Some(notification) = progress_subscriber.next().await {
            tracing::info!("Progress notification: {:?}", notification);
        }
    });
    let _response = handle.await_response().await?;

    // Simulate some delay to allow the async task to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    Ok(())
}
