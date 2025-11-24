use std::sync::Arc;

use rmcp::{
    ClientHandler, ServerHandler, ServiceExt,
    model::{
        ResourceUpdatedNotificationParam, ServerCapabilities, ServerInfo, SubscribeRequestParam,
    },
};
use tokio::sync::Notify;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub struct Server {}

impl ServerHandler for Server {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder()
                .enable_resources()
                .enable_resources_subscribe()
                .enable_resources_list_changed()
                .build(),
            ..Default::default()
        }
    }

    async fn subscribe(
        &self,
        request: rmcp::model::SubscribeRequestParam,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<(), rmcp::ErrorData> {
        let uri = request.uri;
        let peer = context.peer;

        tokio::spawn(async move {
            let span = tracing::info_span!("subscribe", uri = %uri);
            let _enter = span.enter();

            if let Err(e) = peer
                .notify_resource_updated(ResourceUpdatedNotificationParam { uri: uri.clone() })
                .await
            {
                panic!("Failed to send notification: {}", e);
            }
        });

        Ok(())
    }
}

pub struct Client {
    receive_signal: Arc<Notify>,
}

impl ClientHandler for Client {
    async fn on_resource_updated(
        &self,
        params: rmcp::model::ResourceUpdatedNotificationParam,
        _context: rmcp::service::NotificationContext<rmcp::RoleClient>,
    ) {
        let uri = params.uri;
        tracing::info!("Resource updated: {}", uri);
        self.receive_signal.notify_one();
    }
}

#[tokio::test]
async fn test_server_notification() -> anyhow::Result<()> {
    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init();
    let (server_transport, client_transport) = tokio::io::duplex(4096);
    tokio::spawn(async move {
        let server = Server {}.serve(server_transport).await?;
        server.waiting().await?;
        anyhow::Ok(())
    });
    let receive_signal = Arc::new(Notify::new());
    let client = Client {
        receive_signal: receive_signal.clone(),
    }
    .serve(client_transport)
    .await?;
    client
        .subscribe(SubscribeRequestParam {
            uri: "test://test-resource".to_owned(),
        })
        .await?;
    receive_signal.notified().await;
    client.cancel().await?;
    Ok(())
}
