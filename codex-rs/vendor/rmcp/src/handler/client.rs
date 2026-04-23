pub mod progress;
use std::sync::Arc;

use crate::{
    error::ErrorData as McpError,
    model::*,
    service::{NotificationContext, RequestContext, RoleClient, Service, ServiceRole},
};

impl<H: ClientHandler> Service<RoleClient> for H {
    async fn handle_request(
        &self,
        request: <RoleClient as ServiceRole>::PeerReq,
        context: RequestContext<RoleClient>,
    ) -> Result<<RoleClient as ServiceRole>::Resp, McpError> {
        match request {
            ServerRequest::PingRequest(_) => self.ping(context).await.map(ClientResult::empty),
            ServerRequest::CreateMessageRequest(request) => self
                .create_message(request.params, context)
                .await
                .map(Box::new)
                .map(ClientResult::CreateMessageResult),
            ServerRequest::ListRootsRequest(_) => self
                .list_roots(context)
                .await
                .map(ClientResult::ListRootsResult),
            ServerRequest::CreateElicitationRequest(request) => self
                .create_elicitation(request.params, context)
                .await
                .map(ClientResult::CreateElicitationResult),
            ServerRequest::CustomRequest(request) => self
                .on_custom_request(request, context)
                .await
                .map(ClientResult::CustomResult),
        }
    }

    async fn handle_notification(
        &self,
        notification: <RoleClient as ServiceRole>::PeerNot,
        context: NotificationContext<RoleClient>,
    ) -> Result<(), McpError> {
        match notification {
            ServerNotification::CancelledNotification(notification) => {
                self.on_cancelled(notification.params, context).await
            }
            ServerNotification::ProgressNotification(notification) => {
                self.on_progress(notification.params, context).await
            }
            ServerNotification::LoggingMessageNotification(notification) => {
                self.on_logging_message(notification.params, context).await
            }
            ServerNotification::ResourceUpdatedNotification(notification) => {
                self.on_resource_updated(notification.params, context).await
            }
            ServerNotification::ResourceListChangedNotification(_notification_no_param) => {
                self.on_resource_list_changed(context).await
            }
            ServerNotification::ToolListChangedNotification(_notification_no_param) => {
                self.on_tool_list_changed(context).await
            }
            ServerNotification::PromptListChangedNotification(_notification_no_param) => {
                self.on_prompt_list_changed(context).await
            }
            ServerNotification::ElicitationCompletionNotification(notification) => {
                self.on_url_elicitation_notification_complete(notification.params, context)
                    .await
            }
            ServerNotification::CustomNotification(notification) => {
                self.on_custom_notification(notification, context).await
            }
        };
        Ok(())
    }

    fn get_info(&self) -> <RoleClient as ServiceRole>::Info {
        self.get_info()
    }
}

#[allow(unused_variables)]
pub trait ClientHandler: Sized + Send + Sync + 'static {
    fn ping(
        &self,
        context: RequestContext<RoleClient>,
    ) -> impl Future<Output = Result<(), McpError>> + Send + '_ {
        std::future::ready(Ok(()))
    }

    fn create_message(
        &self,
        params: CreateMessageRequestParams,
        context: RequestContext<RoleClient>,
    ) -> impl Future<Output = Result<CreateMessageResult, McpError>> + Send + '_ {
        std::future::ready(Err(
            McpError::method_not_found::<CreateMessageRequestMethod>(),
        ))
    }

    fn list_roots(
        &self,
        context: RequestContext<RoleClient>,
    ) -> impl Future<Output = Result<ListRootsResult, McpError>> + Send + '_ {
        std::future::ready(Ok(ListRootsResult::default()))
    }

    /// Handle an elicitation request from a server asking for user input.
    ///
    /// This method is called when a server needs interactive input from the user
    /// during tool execution. Implementations should present the message to the user,
    /// collect their input according to the requested schema, and return the result.
    ///
    /// # Arguments
    /// * `request` - The elicitation request with message and schema
    /// * `context` - The request context
    ///
    /// # Returns
    /// The user's response including action (accept/decline/cancel) and optional data
    ///
    /// # Default Behavior
    /// The default implementation automatically declines all elicitation requests.
    /// Real clients should override this to provide user interaction.
    ///
    /// # Example
    /// ```rust,ignore
    /// use rmcp::model::CreateElicitationRequestParam;
    /// use rmcp::{
    ///     model::ErrorData as McpError,
    ///     model::*,
    ///     service::{NotificationContext, RequestContext, RoleClient, Service, ServiceRole},
    /// };
    /// use rmcp::ClientHandler;
    ///
    /// impl ClientHandler for MyClient {
    ///  async fn create_elicitation(
    ///     &self,
    ///     request: CreateElicitationRequestParam,
    ///     context: RequestContext<RoleClient>,
    ///  ) -> Result<CreateElicitationResult, McpError> {
    ///     match request {
    ///         CreateElicitationRequestParam::FormElicitationParam {meta, message, requested_schema,} => {
    ///            // Display message to user and collect input according to requested_schema
    ///           let user_input = get_user_input(message, requested_schema).await?;
    ///          Ok(CreateElicitationResult {
    ///             action: ElicitationAction::Accept,
    ///              content: Some(user_input),
    ///          })
    ///         }
    ///         CreateElicitationRequestParam::UrlElicitationParam {meta, message, url, elicitation_id,} => {
    ///           // Open URL in browser for user to complete elicitation
    ///           open_url_in_browser(url).await?;
    ///          Ok(CreateElicitationResult {
    ///              action: ElicitationAction::Accept,
    ///             content: None,
    ///             })
    ///         }
    ///     }
    ///  }
    /// }
    /// ```
    fn create_elicitation(
        &self,
        request: CreateElicitationRequestParams,
        context: RequestContext<RoleClient>,
    ) -> impl Future<Output = Result<CreateElicitationResult, McpError>> + Send + '_ {
        // Default implementation declines all requests - real clients should override this
        let _ = (request, context);
        std::future::ready(Ok(CreateElicitationResult {
            action: ElicitationAction::Decline,
            content: None,
        }))
    }

    fn on_custom_request(
        &self,
        request: CustomRequest,
        context: RequestContext<RoleClient>,
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
        params: CancelledNotificationParam,
        context: NotificationContext<RoleClient>,
    ) -> impl Future<Output = ()> + Send + '_ {
        std::future::ready(())
    }
    fn on_progress(
        &self,
        params: ProgressNotificationParam,
        context: NotificationContext<RoleClient>,
    ) -> impl Future<Output = ()> + Send + '_ {
        std::future::ready(())
    }
    fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        context: NotificationContext<RoleClient>,
    ) -> impl Future<Output = ()> + Send + '_ {
        std::future::ready(())
    }
    fn on_resource_updated(
        &self,
        params: ResourceUpdatedNotificationParam,
        context: NotificationContext<RoleClient>,
    ) -> impl Future<Output = ()> + Send + '_ {
        std::future::ready(())
    }
    fn on_resource_list_changed(
        &self,
        context: NotificationContext<RoleClient>,
    ) -> impl Future<Output = ()> + Send + '_ {
        std::future::ready(())
    }
    fn on_tool_list_changed(
        &self,
        context: NotificationContext<RoleClient>,
    ) -> impl Future<Output = ()> + Send + '_ {
        std::future::ready(())
    }
    fn on_prompt_list_changed(
        &self,
        context: NotificationContext<RoleClient>,
    ) -> impl Future<Output = ()> + Send + '_ {
        std::future::ready(())
    }

    fn on_url_elicitation_notification_complete(
        &self,
        params: ElicitationResponseNotificationParam,
        context: NotificationContext<RoleClient>,
    ) -> impl Future<Output = ()> + Send + '_ {
        std::future::ready(())
    }
    fn on_custom_notification(
        &self,
        notification: CustomNotification,
        context: NotificationContext<RoleClient>,
    ) -> impl Future<Output = ()> + Send + '_ {
        let _ = (notification, context);
        std::future::ready(())
    }

    fn get_info(&self) -> ClientInfo {
        ClientInfo::default()
    }
}

/// Do nothing, with default client info.
impl ClientHandler for () {}

/// Do nothing, with a specific client info.
impl ClientHandler for ClientInfo {
    fn get_info(&self) -> ClientInfo {
        self.clone()
    }
}

macro_rules! impl_client_handler_for_wrapper {
    ($wrapper:ident) => {
        impl<T: ClientHandler> ClientHandler for $wrapper<T> {
            fn ping(
                &self,
                context: RequestContext<RoleClient>,
            ) -> impl Future<Output = Result<(), McpError>> + Send + '_ {
                (**self).ping(context)
            }

            fn create_message(
                &self,
                params: CreateMessageRequestParams,
                context: RequestContext<RoleClient>,
            ) -> impl Future<Output = Result<CreateMessageResult, McpError>> + Send + '_ {
                (**self).create_message(params, context)
            }

            fn list_roots(
                &self,
                context: RequestContext<RoleClient>,
            ) -> impl Future<Output = Result<ListRootsResult, McpError>> + Send + '_ {
                (**self).list_roots(context)
            }

            fn create_elicitation(
                &self,
                request: CreateElicitationRequestParams,
                context: RequestContext<RoleClient>,
            ) -> impl Future<Output = Result<CreateElicitationResult, McpError>> + Send + '_ {
                (**self).create_elicitation(request, context)
            }

            fn on_custom_request(
                &self,
                request: CustomRequest,
                context: RequestContext<RoleClient>,
            ) -> impl Future<Output = Result<CustomResult, McpError>> + Send + '_ {
                (**self).on_custom_request(request, context)
            }

            fn on_cancelled(
                &self,
                params: CancelledNotificationParam,
                context: NotificationContext<RoleClient>,
            ) -> impl Future<Output = ()> + Send + '_ {
                (**self).on_cancelled(params, context)
            }

            fn on_progress(
                &self,
                params: ProgressNotificationParam,
                context: NotificationContext<RoleClient>,
            ) -> impl Future<Output = ()> + Send + '_ {
                (**self).on_progress(params, context)
            }

            fn on_logging_message(
                &self,
                params: LoggingMessageNotificationParam,
                context: NotificationContext<RoleClient>,
            ) -> impl Future<Output = ()> + Send + '_ {
                (**self).on_logging_message(params, context)
            }

            fn on_resource_updated(
                &self,
                params: ResourceUpdatedNotificationParam,
                context: NotificationContext<RoleClient>,
            ) -> impl Future<Output = ()> + Send + '_ {
                (**self).on_resource_updated(params, context)
            }

            fn on_resource_list_changed(
                &self,
                context: NotificationContext<RoleClient>,
            ) -> impl Future<Output = ()> + Send + '_ {
                (**self).on_resource_list_changed(context)
            }

            fn on_tool_list_changed(
                &self,
                context: NotificationContext<RoleClient>,
            ) -> impl Future<Output = ()> + Send + '_ {
                (**self).on_tool_list_changed(context)
            }

            fn on_prompt_list_changed(
                &self,
                context: NotificationContext<RoleClient>,
            ) -> impl Future<Output = ()> + Send + '_ {
                (**self).on_prompt_list_changed(context)
            }

            fn on_custom_notification(
                &self,
                notification: CustomNotification,
                context: NotificationContext<RoleClient>,
            ) -> impl Future<Output = ()> + Send + '_ {
                (**self).on_custom_notification(notification, context)
            }

            fn get_info(&self) -> ClientInfo {
                (**self).get_info()
            }
        }
    };
}

impl_client_handler_for_wrapper!(Box);
impl_client_handler_for_wrapper!(Arc);
