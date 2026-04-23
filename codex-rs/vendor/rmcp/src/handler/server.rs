use std::sync::Arc;

use crate::{
    error::ErrorData as McpError,
    model::{TaskSupport, *},
    service::{NotificationContext, RequestContext, RoleServer, Service, ServiceRole},
};

pub mod common;
pub mod prompt;
mod resource;
pub mod router;
pub mod tool;
pub mod tool_name_validation;
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
            ClientRequest::CallToolRequest(request) => {
                let is_task = request.params.task.is_some();

                // Validate task support mode per MCP specification
                if let Some(tool) = self.get_tool(&request.params.name) {
                    match (tool.task_support(), is_task) {
                        // If taskSupport is "required", clients MUST invoke the tool as a task.
                        // Servers MUST return a -32601 (Method not found) error if they don't.
                        (TaskSupport::Required, false) => {
                            return Err(McpError::new(
                                ErrorCode::METHOD_NOT_FOUND,
                                "Tool requires task-based invocation",
                                None,
                            ));
                        }
                        // If taskSupport is "forbidden" (default), clients MUST NOT invoke as a task.
                        (TaskSupport::Forbidden, true) => {
                            return Err(McpError::invalid_params(
                                "Tool does not support task-based invocation",
                                None,
                            ));
                        }
                        _ => {}
                    }
                }

                if is_task {
                    tracing::info!("Enqueueing task for tool call: {}", request.params.name);
                    self.enqueue_task(request.params, context.clone())
                        .await
                        .map(ServerResult::CreateTaskResult)
                } else {
                    self.call_tool(request.params, context)
                        .await
                        .map(ServerResult::CallToolResult)
                }
            }
            ClientRequest::ListToolsRequest(request) => self
                .list_tools(request.params, context)
                .await
                .map(ServerResult::ListToolsResult),
            ClientRequest::CustomRequest(request) => self
                .on_custom_request(request, context)
                .await
                .map(ServerResult::CustomResult),
            ClientRequest::ListTasksRequest(request) => self
                .list_tasks(request.params, context)
                .await
                .map(ServerResult::ListTasksResult),
            ClientRequest::GetTaskInfoRequest(request) => self
                .get_task_info(request.params, context)
                .await
                .map(ServerResult::GetTaskInfoResult),
            ClientRequest::GetTaskResultRequest(request) => self
                .get_task_result(request.params, context)
                .await
                .map(ServerResult::TaskResult),
            ClientRequest::CancelTaskRequest(request) => self
                .cancel_task(request.params, context)
                .await
                .map(ServerResult::empty),
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
            ClientNotification::CustomNotification(notification) => {
                self.on_custom_notification(notification, context).await
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
    fn enqueue_task(
        &self,
        _request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CreateTaskResult, McpError>> + Send + '_ {
        std::future::ready(Err(McpError::internal_error(
            "Task processing not implemented".to_string(),
            None,
        )))
    }
    fn ping(
        &self,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<(), McpError>> + Send + '_ {
        std::future::ready(Ok(()))
    }
    // handle requests
    fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<InitializeResult, McpError>> + Send + '_ {
        if context.peer.peer_info().is_none() {
            context.peer.set_peer_info(request);
        }
        std::future::ready(Ok(self.get_info()))
    }
    fn complete(
        &self,
        request: CompleteRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CompleteResult, McpError>> + Send + '_ {
        std::future::ready(Ok(CompleteResult::default()))
    }
    fn set_level(
        &self,
        request: SetLevelRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<(), McpError>> + Send + '_ {
        std::future::ready(Err(McpError::method_not_found::<SetLevelRequestMethod>()))
    }
    fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<GetPromptResult, McpError>> + Send + '_ {
        std::future::ready(Err(McpError::method_not_found::<GetPromptRequestMethod>()))
    }
    fn list_prompts(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListPromptsResult, McpError>> + Send + '_ {
        std::future::ready(Ok(ListPromptsResult::default()))
    }
    fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, McpError>> + Send + '_ {
        std::future::ready(Ok(ListResourcesResult::default()))
    }
    fn list_resource_templates(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourceTemplatesResult, McpError>> + Send + '_ {
        std::future::ready(Ok(ListResourceTemplatesResult::default()))
    }
    fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ReadResourceResult, McpError>> + Send + '_ {
        std::future::ready(Err(
            McpError::method_not_found::<ReadResourceRequestMethod>(),
        ))
    }
    fn subscribe(
        &self,
        request: SubscribeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<(), McpError>> + Send + '_ {
        std::future::ready(Err(McpError::method_not_found::<SubscribeRequestMethod>()))
    }
    fn unsubscribe(
        &self,
        request: UnsubscribeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<(), McpError>> + Send + '_ {
        std::future::ready(Err(McpError::method_not_found::<UnsubscribeRequestMethod>()))
    }
    fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        std::future::ready(Err(McpError::method_not_found::<CallToolRequestMethod>()))
    }
    fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        std::future::ready(Ok(ListToolsResult::default()))
    }
    /// Get a tool definition by name.
    ///
    /// The default implementation returns `None`, which bypasses validation.
    /// When using `#[tool_handler]`, this method is automatically implemented.
    fn get_tool(&self, _name: &str) -> Option<Tool> {
        None
    }
    fn on_custom_request(
        &self,
        request: CustomRequest,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CustomResult, McpError>> + Send + '_ {
        let CustomRequest { method, .. } = request;
        let _ = context;
        std::future::ready(Err(McpError::new(
            ErrorCode::METHOD_NOT_FOUND,
            method,
            None,
        )))
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
    fn on_custom_notification(
        &self,
        notification: CustomNotification,
        context: NotificationContext<RoleServer>,
    ) -> impl Future<Output = ()> + Send + '_ {
        let _ = (notification, context);
        std::future::ready(())
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo::default()
    }

    fn list_tasks(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListTasksResult, McpError>> + Send + '_ {
        std::future::ready(Err(McpError::method_not_found::<ListTasksMethod>()))
    }

    fn get_task_info(
        &self,
        request: GetTaskInfoParams,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<GetTaskInfoResult, McpError>> + Send + '_ {
        std::future::ready(Err(McpError::method_not_found::<GetTaskInfoMethod>()))
    }

    fn get_task_result(
        &self,
        request: GetTaskResultParams,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<TaskResult, McpError>> + Send + '_ {
        let _ = (request, context);
        std::future::ready(Err(McpError::method_not_found::<GetTaskResultMethod>()))
    }

    fn cancel_task(
        &self,
        request: CancelTaskParams,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<(), McpError>> + Send + '_ {
        let _ = (request, context);
        std::future::ready(Err(McpError::method_not_found::<CancelTaskMethod>()))
    }
}

macro_rules! impl_server_handler_for_wrapper {
    ($wrapper:ident) => {
        impl<T: ServerHandler> ServerHandler for $wrapper<T> {
            fn enqueue_task(
                &self,
                request: CallToolRequestParams,
                context: RequestContext<RoleServer>,
            ) -> impl Future<Output = Result<CreateTaskResult, McpError>> + Send + '_ {
                (**self).enqueue_task(request, context)
            }

            fn ping(
                &self,
                context: RequestContext<RoleServer>,
            ) -> impl Future<Output = Result<(), McpError>> + Send + '_ {
                (**self).ping(context)
            }

            fn initialize(
                &self,
                request: InitializeRequestParams,
                context: RequestContext<RoleServer>,
            ) -> impl Future<Output = Result<InitializeResult, McpError>> + Send + '_ {
                (**self).initialize(request, context)
            }

            fn complete(
                &self,
                request: CompleteRequestParams,
                context: RequestContext<RoleServer>,
            ) -> impl Future<Output = Result<CompleteResult, McpError>> + Send + '_ {
                (**self).complete(request, context)
            }

            fn set_level(
                &self,
                request: SetLevelRequestParams,
                context: RequestContext<RoleServer>,
            ) -> impl Future<Output = Result<(), McpError>> + Send + '_ {
                (**self).set_level(request, context)
            }

            fn get_prompt(
                &self,
                request: GetPromptRequestParams,
                context: RequestContext<RoleServer>,
            ) -> impl Future<Output = Result<GetPromptResult, McpError>> + Send + '_ {
                (**self).get_prompt(request, context)
            }

            fn list_prompts(
                &self,
                request: Option<PaginatedRequestParams>,
                context: RequestContext<RoleServer>,
            ) -> impl Future<Output = Result<ListPromptsResult, McpError>> + Send + '_ {
                (**self).list_prompts(request, context)
            }

            fn list_resources(
                &self,
                request: Option<PaginatedRequestParams>,
                context: RequestContext<RoleServer>,
            ) -> impl Future<Output = Result<ListResourcesResult, McpError>> + Send + '_ {
                (**self).list_resources(request, context)
            }

            fn list_resource_templates(
                &self,
                request: Option<PaginatedRequestParams>,
                context: RequestContext<RoleServer>,
            ) -> impl Future<Output = Result<ListResourceTemplatesResult, McpError>> + Send + '_
            {
                (**self).list_resource_templates(request, context)
            }

            fn read_resource(
                &self,
                request: ReadResourceRequestParams,
                context: RequestContext<RoleServer>,
            ) -> impl Future<Output = Result<ReadResourceResult, McpError>> + Send + '_ {
                (**self).read_resource(request, context)
            }

            fn subscribe(
                &self,
                request: SubscribeRequestParams,
                context: RequestContext<RoleServer>,
            ) -> impl Future<Output = Result<(), McpError>> + Send + '_ {
                (**self).subscribe(request, context)
            }

            fn unsubscribe(
                &self,
                request: UnsubscribeRequestParams,
                context: RequestContext<RoleServer>,
            ) -> impl Future<Output = Result<(), McpError>> + Send + '_ {
                (**self).unsubscribe(request, context)
            }

            fn call_tool(
                &self,
                request: CallToolRequestParams,
                context: RequestContext<RoleServer>,
            ) -> impl Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
                (**self).call_tool(request, context)
            }

            fn list_tools(
                &self,
                request: Option<PaginatedRequestParams>,
                context: RequestContext<RoleServer>,
            ) -> impl Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
                (**self).list_tools(request, context)
            }

            fn get_tool(&self, name: &str) -> Option<Tool> {
                (**self).get_tool(name)
            }

            fn on_custom_request(
                &self,
                request: CustomRequest,
                context: RequestContext<RoleServer>,
            ) -> impl Future<Output = Result<CustomResult, McpError>> + Send + '_ {
                (**self).on_custom_request(request, context)
            }

            fn on_cancelled(
                &self,
                notification: CancelledNotificationParam,
                context: NotificationContext<RoleServer>,
            ) -> impl Future<Output = ()> + Send + '_ {
                (**self).on_cancelled(notification, context)
            }

            fn on_progress(
                &self,
                notification: ProgressNotificationParam,
                context: NotificationContext<RoleServer>,
            ) -> impl Future<Output = ()> + Send + '_ {
                (**self).on_progress(notification, context)
            }

            fn on_initialized(
                &self,
                context: NotificationContext<RoleServer>,
            ) -> impl Future<Output = ()> + Send + '_ {
                (**self).on_initialized(context)
            }

            fn on_roots_list_changed(
                &self,
                context: NotificationContext<RoleServer>,
            ) -> impl Future<Output = ()> + Send + '_ {
                (**self).on_roots_list_changed(context)
            }

            fn on_custom_notification(
                &self,
                notification: CustomNotification,
                context: NotificationContext<RoleServer>,
            ) -> impl Future<Output = ()> + Send + '_ {
                (**self).on_custom_notification(notification, context)
            }

            fn get_info(&self) -> ServerInfo {
                (**self).get_info()
            }

            fn list_tasks(
                &self,
                request: Option<PaginatedRequestParams>,
                context: RequestContext<RoleServer>,
            ) -> impl Future<Output = Result<ListTasksResult, McpError>> + Send + '_ {
                (**self).list_tasks(request, context)
            }

            fn get_task_info(
                &self,
                request: GetTaskInfoParams,
                context: RequestContext<RoleServer>,
            ) -> impl Future<Output = Result<GetTaskInfoResult, McpError>> + Send + '_ {
                (**self).get_task_info(request, context)
            }

            fn get_task_result(
                &self,
                request: GetTaskResultParams,
                context: RequestContext<RoleServer>,
            ) -> impl Future<Output = Result<TaskResult, McpError>> + Send + '_ {
                (**self).get_task_result(request, context)
            }

            fn cancel_task(
                &self,
                request: CancelTaskParams,
                context: RequestContext<RoleServer>,
            ) -> impl Future<Output = Result<(), McpError>> + Send + '_ {
                (**self).cancel_task(request, context)
            }
        }
    };
}

impl_server_handler_for_wrapper!(Box);
impl_server_handler_for_wrapper!(Arc);
