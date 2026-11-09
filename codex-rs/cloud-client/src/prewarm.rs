//! Prepare an ordinary cloud coding thread with server-owned managed environment defaults.

use crate::Client;
use crate::Error;
use bytes::Bytes;
use bytes::BytesMut;
use prost::Message;

/// A local project's GitHub origins to provision in the default cloud environment.
/// Callers must supply credential-free origin URLs; this client does not inspect local Git state.
#[derive(Clone)]
pub struct ProjectEnvironment {
    pub name: String,
    pub origin_urls: Vec<String>,
    /// Working directory inside the managed environment, when a project selects one.
    pub cwd: Option<String>,
}

impl std::fmt::Debug for ProjectEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProjectEnvironment([redacted])")
    }
}

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
        self.prewarm_thread(model, model_provider, /*project*/ None)
            .await
    }

    /// Prewarm a coding thread, deferring executor readiness for a selected project checkout.
    pub async fn prewarm_thread(
        &self,
        model: Option<String>,
        model_provider: Option<String>,
        project: Option<ProjectEnvironment>,
    ) -> Result<crate::ThreadActivation, Error> {
        let deferred_environment = project.is_some().then_some(true);
        let managed = match project {
            Some(ProjectEnvironment {
                name,
                origin_urls,
                cwd,
            }) => wire::ManagedEnvironment {
                environment_config_id: Some("asenvcfg_base".to_owned()),
                cwd,
                overrides: Some(wire::EnvironmentOverrides {
                    repositories: Some(wire::RepositoryOverrides {
                        values: origin_urls
                            .into_iter()
                            .map(|origin_url| wire::RepositoryOverride {
                                origin_url: Some(origin_url),
                            })
                            .collect(),
                    }),
                    name: Some(name),
                }),
            },
            None => wire::ManagedEnvironment::default(),
        };
        let response = self
            .raw_unary(
                wire::PrewarmRequest {
                    model,
                    model_provider,
                    plugins_mcp: Some(wire::PluginsMcp {
                        product_sku: Some("codex".to_owned()),
                    }),
                    deferred_environment,
                    environments: Some(wire::EnvironmentList {
                        values: vec![wire::EnvironmentSelection {
                            managed: Some(managed),
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
        #[prost(message, optional, tag = "28")]
        pub plugins_mcp: Option<PluginsMcp>,
        #[prost(bool, optional, tag = "29")]
        pub deferred_environment: Option<bool>,
    }
    #[derive(Clone, Message)]
    pub(super) struct PluginsMcp {
        #[prost(string, optional, tag = "1")]
        pub product_sku: Option<String>,
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
    pub(super) struct ManagedEnvironment {
        #[prost(string, optional, tag = "1")]
        pub environment_config_id: Option<String>,
        #[prost(string, optional, tag = "2")]
        pub cwd: Option<String>,
        #[prost(message, optional, tag = "3")]
        pub overrides: Option<EnvironmentOverrides>,
    }
    #[derive(Clone, Message)]
    pub(super) struct EnvironmentOverrides {
        #[prost(message, optional, tag = "1")]
        pub repositories: Option<RepositoryOverrides>,
        #[prost(string, optional, tag = "2")]
        pub name: Option<String>,
    }
    #[derive(Clone, Message)]
    pub(super) struct RepositoryOverrides {
        #[prost(message, repeated, tag = "1")]
        pub values: Vec<RepositoryOverride>,
    }
    #[derive(Clone, Message)]
    pub(super) struct RepositoryOverride {
        #[prost(string, optional, tag = "5")]
        pub origin_url: Option<String>,
    }
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
