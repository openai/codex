//! Prepare an ordinary cloud coding thread with server-owned managed environment defaults.

use crate::Client;
use crate::Error;
use bytes::Bytes;
use bytes::BytesMut;
use prost::Message;

impl Client {
    /// Prewarm an ordinary coding thread in the default managed environment.
    /// The service makes it discoverable when it admits the first user message.
    /// This mutation is never retried. A timeout or disconnect can leave a thread
    /// prepared even if its identity and settings were not received. The caller
    /// must not use this ordinary-thread API for an ephemeral session.
    pub async fn prewarm_default_thread(
        &self,
        model: Option<String>,
        model_provider: Option<String>,
    ) -> Result<crate::ThreadActivation, Error> {
        let response = self
            .raw_unary(
                wire::PrewarmRequest {
                    model,
                    model_provider,
                    environments: Some(wire::EnvironmentList {
                        values: vec![wire::EnvironmentSelection {
                            managed: Some(wire::ManagedEnvironment {}),
                        }],
                    }),
                },
                "/oaiproto.codex_app_server_backend.ThreadService/Prewarm",
            )
            .await?;
        let warm = wire::PrewarmResponse::decode(response).map_err(|_| Error::InvalidResponse)?;
        // PrewarmThread omits turns, but the native Thread requires an empty
        // ThreadTurns message. Prefix it so the original wire bytes, including
        // unknown fields, are preserved and malformed trailing data stays invalid.
        let mut thread = BytesMut::new();
        prost::encoding::bytes::encode(/* tag */ 31, &Bytes::new(), &mut thread);
        thread.extend_from_slice(&warm.thread.ok_or(Error::InvalidResponse)?);

        crate::ThreadActivation::from_settings(crate::activation_wire::UpdateConfigResponse {
            active_permission_profile: warm.active_permission_profile,
            approval_policy: warm.approval_policy,
            approvals_reviewer: warm.approvals_reviewer,
            cwd: warm.cwd,
            model: warm.model,
            model_provider: warm.model_provider,
            reasoning_effort: warm.reasoning_effort,
            runtime_workspace_roots: warm.runtime_workspace_roots,
            sandbox: warm.sandbox,
            service_tier: warm.service_tier,
            thread: Some(thread.freeze()),
            ..Default::default()
        })
    }
}

mod wire {
    use super::*;

    #[derive(Clone, Message)]
    pub(super) struct PrewarmRequest {
        #[prost(message, optional, tag = "10")]
        pub environments: Option<EnvironmentList>,
        #[prost(string, optional, tag = "15")]
        pub model: Option<String>,
        #[prost(string, optional, tag = "16")]
        pub model_provider: Option<String>,
    }
    #[derive(Clone, Message)]
    pub(super) struct EnvironmentList {
        #[prost(message, repeated, tag = "1")]
        pub values: Vec<EnvironmentSelection>,
    }
    #[derive(Clone, Message)]
    pub(super) struct EnvironmentSelection {
        #[prost(message, optional, tag = "1")]
        pub managed: Option<ManagedEnvironment>,
    }
    #[derive(Clone, Message)]
    pub(super) struct ManagedEnvironment {}
    #[derive(Clone, Message)]
    pub(super) struct PrewarmResponse {
        #[prost(message, optional, tag = "1")]
        pub active_permission_profile: Option<crate::activation_wire::ActivePermissionProfile>,
        #[prost(message, optional, tag = "2")]
        pub approval_policy: Option<crate::activation_wire::ApprovalPolicy>,
        #[prost(int32, optional, tag = "3")]
        pub approvals_reviewer: Option<i32>,
        #[prost(string, optional, tag = "4")]
        pub cwd: Option<String>,
        #[prost(string, optional, tag = "7")]
        pub model: Option<String>,
        #[prost(string, optional, tag = "8")]
        pub model_provider: Option<String>,
        #[prost(string, optional, tag = "10")]
        pub reasoning_effort: Option<String>,
        #[prost(message, optional, tag = "11")]
        pub runtime_workspace_roots: Option<crate::activation_wire::StringList>,
        #[prost(message, optional, tag = "12")]
        pub sandbox: Option<crate::activation_wire::SandboxPolicy>,
        #[prost(message, optional, tag = "13")]
        pub service_tier: Option<crate::activation_wire::NullableString>,
        #[prost(bytes = "bytes", optional, tag = "14")]
        pub thread: Option<Bytes>,
    }
}
