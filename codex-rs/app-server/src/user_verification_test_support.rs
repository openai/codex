//! Dispatcher harness with a controllable native provider for lifecycle tests.
//! Public stdio and WebSocket framing are exercised in the integration suite.

use super::*;
use crate::config_manager::ConfigManager;
use crate::message_processor::ConnectionSessionState;
use crate::message_processor::MessageProcessor;
use crate::message_processor::MessageProcessorArgs;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingEnvelope;
use crate::outgoing_message::OutgoingMessage;
use crate::outgoing_message::OutgoingMessageSender;
use crate::transport::AppServerTransport;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::write_chatgpt_auth;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codex_analytics::AnalyticsEventsClient;
use codex_analytics::AppServerRpcTransport;
use codex_arg0::Arg0DispatchPaths;
use codex_config::CloudConfigBundleLoader;
use codex_config::LoaderOverrides;
use codex_config::types::AuthCredentialsStoreMode;
use codex_core::config::ConfigBuilder;
use codex_exec_server::EnvironmentManager;
use codex_feedback::CodexFeedback;
use codex_protocol::protocol::SessionSource;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::sync::mpsc;
use tokio::time::timeout;

#[derive(Default)]
pub(super) struct BlockingProvider {
    pub(super) entered: Mutex<Option<oneshot::Sender<()>>>,
    pub(super) released: AtomicBool,
    pub(super) calls: AtomicUsize,
    pub(super) ignore_cancellation: AtomicBool,
}

impl native::UserVerificationProvider for BlockingProvider {
    fn status(
        &self,
        guard: &native::UserVerificationRequestGuard,
    ) -> Result<native::UserVerificationStatus, native::UserVerificationError> {
        guard.check()?;
        Ok(native::UserVerificationStatus {
            credential: None,
            unavailable_reason: Some(native::UserVerificationUnavailableReason::CredentialMissing),
            unavailable_message: None,
        })
    }
    fn ensure_key(
        &self,
        guard: &native::UserVerificationRequestGuard,
    ) -> Result<native::UserVerificationKeyCreation, native::UserVerificationError> {
        guard.check()?;
        self.calls.fetch_add(/*val*/ 1, Ordering::SeqCst);
        Ok(native::UserVerificationKeyCreation {
            created: false,
            credential: native::UserVerificationKeyInfo {
                credential_id: "credential".into(),
                algorithm: "ecdsaP256Sha256X962".into(),
                public_key: "public-key".into(),
            },
        })
    }
    fn delete(
        &self,
        _guard: &native::UserVerificationRequestGuard,
    ) -> Result<native::UserVerificationKeyDeletion, native::UserVerificationError> {
        unreachable!("this test must not delete keys")
    }
    fn verify(
        &self,
        _request: &native::UserVerificationRequest,
        guard: &native::UserVerificationRequestGuard,
    ) -> Result<native::UserVerificationProof, native::UserVerificationError> {
        self.calls.fetch_add(/*val*/ 1, Ordering::SeqCst);
        if let Some(entered) = self.entered.lock().unwrap().take() {
            let _ = entered.send(());
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(/*secs*/ 10);
        while !self.released.load(Ordering::Acquire) {
            if !self.ignore_cancellation.load(Ordering::Acquire) {
                guard.check()?;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "test failed to release native worker"
            );
            std::thread::sleep(Duration::from_millis(/*millis*/ 5));
        }
        Ok(native::UserVerificationProof {
            credential_id: "credential".into(),
            signature: "signature".into(),
        })
    }
}

pub(super) struct Harness {
    pub(super) processor: Arc<MessageProcessor>,
    pub(super) session: Arc<ConnectionSessionState>,
    pub(super) auth: Arc<AuthManager>,
    pub(super) service: Arc<Service>,
    pub(super) provider: Arc<BlockingProvider>,
    pub(super) outgoing: Arc<OutgoingMessageSender>,
    pub(super) messages: mpsc::Receiver<OutgoingEnvelope>,
    pub(super) home: tempfile::TempDir,
    transport: AppServerTransport,
}

impl Harness {
    pub(super) async fn new(origin: ConnectionOrigin, supported: fn() -> bool) -> Result<Self> {
        let home = tempfile::tempdir()?;
        write_auth(home.path(), "first")?;
        let config = Arc::new(
            ConfigBuilder::default()
                .codex_home(home.path().into())
                .build()
                .await?,
        );
        let auth = AuthManager::shared_from_config(
            config.as_ref(),
            /*enable_codex_api_key_env*/ false,
        )
        .await?;
        let provider = Arc::new(BlockingProvider::default());
        let factory_provider = Arc::clone(&provider);
        let service = Arc::new(Service {
            auth_manager: Arc::clone(&auth),
            provider: Arc::new(move |_| factory_provider.clone()),
            platform_supported: true,
            device_supported: supported,
            worker: Arc::new(Semaphore::new(/*permits*/ 1)),
        });
        let config_manager = ConfigManager::new(
            home.path().into(),
            Vec::new(),
            LoaderOverrides::default(),
            /*strict_config*/ false,
            CloudConfigBundleLoader::default(),
            Arg0DispatchPaths::default(),
            Arc::new(codex_config::NoopThreadConfigLoader),
        );
        let (sender, messages) = mpsc::channel(/*buffer*/ 16);
        let outgoing = Arc::new(OutgoingMessageSender::new(
            sender,
            AnalyticsEventsClient::disabled(),
        ));
        let processor = Arc::new(MessageProcessor::new(MessageProcessorArgs {
            outgoing: Arc::clone(&outgoing),
            analytics_events_client: AnalyticsEventsClient::disabled(),
            arg0_paths: Arg0DispatchPaths::default(),
            config,
            config_manager,
            environment_manager: Arc::new(EnvironmentManager::default_for_tests()),
            feedback: CodexFeedback::new(),
            log_db: None,
            state_db: None,
            config_warnings: Vec::new(),
            session_source: SessionSource::Cli,
            auth_manager: Arc::clone(&auth),
            user_verification: Arc::clone(&service),
            installation_id: "11111111-1111-4111-8111-111111111111".into(),
            code_mode_session_provider: None,
            rpc_transport: match origin {
                ConnectionOrigin::InProcess => AppServerRpcTransport::InProcess,
                ConnectionOrigin::Stdio | ConnectionOrigin::RemoteControl => {
                    AppServerRpcTransport::Stdio
                }
                ConnectionOrigin::WebSocket => AppServerRpcTransport::Websocket,
            },
            remote_control_handle: None,
            plugin_startup_tasks: None,
        }));
        Ok(Self {
            processor,
            session: Arc::new(ConnectionSessionState::new(origin)),
            auth,
            service,
            provider,
            outgoing,
            messages,
            home,
            transport: match origin {
                ConnectionOrigin::WebSocket => AppServerTransport::WebSocket {
                    bind_address: "127.0.0.1:0".parse()?,
                },
                // Remote control can share a process whose primary listener is stdio.
                // Keeping that case tests that authorization uses the connection's origin.
                ConnectionOrigin::Stdio
                | ConnectionOrigin::InProcess
                | ConnectionOrigin::RemoteControl => AppServerTransport::Stdio,
            },
        })
    }

    pub(super) async fn send(&self, id: i64, method: &str, params: serde_json::Value) {
        self.processor
            .process_request(
                ConnectionId(1),
                rpc::JSONRPCRequest {
                    id: rpc::RequestId::Integer(id),
                    method: method.into(),
                    params: Some(params),
                    trace: None,
                },
                &self.transport,
                Arc::clone(&self.session),
            )
            .await;
    }

    pub(super) async fn response(&mut self) -> OutgoingMessage {
        let envelope = timeout(Duration::from_secs(/*secs*/ 10), self.messages.recv())
            .await
            .expect("response deadline")
            .expect("response channel");
        let OutgoingEnvelope::ToConnection {
            connection_id,
            message,
            ..
        } = envelope
        else {
            panic!("unexpected broadcast")
        };
        assert_eq!(connection_id, ConnectionId(1));
        message
    }

    pub(super) async fn initialize(&mut self, name: &str, opt_in: bool) {
        self.send(/*id*/ 0, "initialize", json!({"clientInfo": {"name": name, "version": "1"}, "capabilities": {"experimentalApi": opt_in}})).await;
        assert!(matches!(
            self.response().await,
            OutgoingMessage::Response(_)
        ));
    }

    pub(super) async fn start_verify(&self) -> Result<()> {
        let (entered, waiting) = oneshot::channel();
        *self.provider.entered.lock().unwrap() = Some(entered);
        self.send(
            /*id*/ 1,
            "userVerification/verify",
            json!({"challenge": "AQID", "title": "Approve", "description": "Action"}),
        )
        .await;
        timeout(Duration::from_secs(/*secs*/ 10), waiting).await??;
        Ok(())
    }

    pub(super) async fn shutdown(self) {
        self.provider
            .released
            .store(/*val*/ true, Ordering::Release);
        self.processor
            .connection_closed(ConnectionId(1), &self.session)
            .await;
        self.processor.clear_runtime_references();
        self.processor.clear_all_thread_listeners().await;
        self.processor.drain_background_tasks().await;
        self.processor.shutdown_threads().await;
    }
}

pub(super) fn write_auth(home: &std::path::Path, token: &str) -> Result<()> {
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&json!({"jti": token, "https://api.openai.com/auth": {"chatgpt_account_id": "account-1", "chatgpt_account_user_id": "membership-1"}}))?);
    write_chatgpt_auth(
        home,
        ChatGptAuthFixture::new(format!("header.{payload}.signature"))
            .chatgpt_user_id("user-1")
            .account_id("account-1")
            .chatgpt_account_id("account-1"),
        AuthCredentialsStoreMode::File,
    )
}
