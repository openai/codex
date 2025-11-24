use crate::{
    error::ErrorData as McpError,
    model::*,
    service::{NotificationContext, RequestContext, RoleServer, Service, ServiceRole},
};

pub mod common;
pub mod prompt;
mod resource;
pub mod router;
pub mod tool;
pub mod wrapper;
impl<H: ServerHandler> Service<RoleServer> for H {
    async fn handle_request(
        &self,
        request: <RoleServer as ServiceRole>::PeerReq,
        context: RequestContext<RoleServer>,
    ) -> Result<<RoleServer as ServiceRole>::Resp, McpError> {
        match request {
            ClientRequest::InitializeRequest(request) => self
                .initialize(request.params, context)
                .await
                .map(ServerResult::InitializeResult),
            ClientRequest::PingRequest(_request) => {
                self.ping(context).await.map(ServerResult::empty)
            }
            ClientRequest::CompleteRequest(request) => self
                .complete(request.params, context)
                .await
                .map(ServerResult::CompleteResult),
            ClientRequest::SetLevelRequest(request) => self
                .set_level(request.params, context)
                .await
                .map(ServerResult::empty),
            ClientRequest::GetPromptRequest(request) => self
                .get_prompt(request.params, context)
                .await
                .map(ServerResult::GetPromptResult),
            ClientRequest::ListPromptsRequest(request) => self
                .list_prompts(request.params, context)
                .await
                .map(ServerResult::ListPromptsResult),
            ClientRequest::ListResourcesRequest(request) => self
                .list_resources(request.params, context)
                .await
                .map(ServerResult::ListResourcesResult),
            ClientRequest::ListResourceTemplatesRequest(request) => self
                .list_resource_templates(request.params, context)
                .await
                .map(ServerResult::ListResourceTemplatesResult),
            ClientRequest::ReadResourceRequest(request) => self
                .read_resource(request.params, context)
                .await
                .map(ServerResult::ReadResourceResult),
            ClientRequest::SubscribeRequest(request) => self
                .subscribe(request.params, context)
                .await
                .map(ServerResult::empty),
            ClientRequest::UnsubscribeRequest(request) => self
                .unsubscribe(request.params, context)
                .await
                .map(ServerResult::empty),
            ClientRequest::CallToolRequest(request) => self
                .call_tool(request.params, context)
                .await
                .map(ServerResult::CallToolResult),
            ClientRequest::ListToolsRequest(request) => self
                .list_tools(request.params, context)
                .await
                .map(ServerResult::ListToolsResult),
        }
    }

    async fn handle_notification(
        &self,
        notification: <RoleServer as ServiceRole>::PeerNot,
        context: NotificationContext<RoleServer>,
    ) -> Result<(), McpError> {
        match notification {
            ClientNotification::CancelledNotification(notification) => {
                self.on_cancelled(notification.params, context).await
            }
            ClientNotification::ProgressNotification(notification) => {
                self.on_progress(notification.params, context).await
            }
            ClientNotification::InitializedNotification(_notification) => {
                self.on_initialized(context).await
            }
            ClientNotification::RootsListChangedNotification(_notification) => {
                self.on_roots_list_changed(context).await
            }
        };
        Ok(())
    }

    fn get_info(&self) -> <RoleServer as ServiceRole>::Info {
        self.get_info()
    }
}

#[allow(unused_variables)]
pub trait ServerHandler: Sized + Send + Sync + 'static {
    fn ping(
        &self,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<(), McpError>> + Send + '_ {
        std::future::ready(Ok(()))
    }
    // handle requests
    fn initialize(
        &self,
        request: InitializeRequestParam,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<InitializeResult, McpError>> + Send + '_ {
        if context.peer.peer_info().is_none() {
            context.peer.set_peer_info(request);
        }
        std::future::ready(Ok(self.get_info()))
    }
    fn complete(
        &self,
        request: CompleteRequestParam,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CompleteResult, McpError>> + Send + '_ {
        std::future::ready(Ok(CompleteResult::default()))
    }
    fn set_level(
        &self,
        request: SetLevelRequestParam,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<(), McpError>> + Send + '_ {
        std::future::ready(Err(McpError::method_not_found::<SetLevelRequestMethod>()))
    }
    fn get_prompt(
        &self,
        request: GetPromptRequestParam,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<GetPromptResult, McpError>> + Send + '_ {
        std::future::ready(Err(McpError::method_not_found::<GetPromptRequestMethod>()))
    }
    fn list_prompts(
        &self,
        request: Option<PaginatedRequestParam>,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListPromptsResult, McpError>> + Send + '_ {
        std::future::ready(Ok(ListPromptsResult::default()))
    }
    fn list_resources(
        &self,
        request: Option<PaginatedRequestParam>,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, McpError>> + Send + '_ {
        std::future::ready(Ok(ListResourcesResult::default()))
    }
    fn list_resource_templates(
        &self,
        request: Option<PaginatedRequestParam>,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourceTemplatesResult, McpError>> + Send + '_ {
        std::future::ready(Ok(ListResourceTemplatesResult::default()))
    }
    fn read_resource(
        &self,
        request: ReadResourceRequestParam,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ReadResourceResult, McpError>> + Send + '_ {
        std::future::ready(Err(
            McpError::method_not_found::<ReadResourceRequestMethod>(),
        ))
    }
    fn subscribe(
        &self,
        request: SubscribeRequestParam,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<(), McpError>> + Send + '_ {
        std::future::ready(Err(McpError::method_not_found::<SubscribeRequestMethod>()))
    }
    fn unsubscribe(
        &self,
        request: UnsubscribeRequestParam,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<(), McpError>> + Send + '_ {
        std::future::ready(Err(McpError::method_not_found::<UnsubscribeRequestMethod>()))
    }
    fn call_tool(
        &self,
        request: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        std::future::ready(Err(McpError::method_not_found::<CallToolRequestMethod>()))
    }
    fn list_tools(
        &self,
        request: Option<PaginatedRequestParam>,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        std::future::ready(Ok(ListToolsResult::default()))
    }

    fn on_cancelled(
        &self,
        notification: CancelledNotificationParam,
        context: NotificationContext<RoleServer>,
    ) -> impl Future<Output = ()> + Send + '_ {
        std::future::ready(())
    }
    fn on_progress(
        &self,
        notification: ProgressNotificationParam,
        context: NotificationContext<RoleServer>,
    ) -> impl Future<Output = ()> + Send + '_ {
        std::future::ready(())
    }
    fn on_initialized(
        &self,
        context: NotificationContext<RoleServer>,
    ) -> impl Future<Output = ()> + Send + '_ {
        tracing::info!("client initialized");
        std::future::ready(())
    }
    fn on_roots_list_changed(
        &self,
        context: NotificationContext<RoleServer>,
    ) -> impl Future<Output = ()> + Send + '_ {
        std::future::ready(())
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo::default()
    }
}
