use codex_core::MCP_SANDBOX_STATE_METHOD;
use codex_core::SandboxState;
use codex_core::protocol::SandboxPolicy;
use codex_utils_cargo_bin::find_resource;
use rmcp::ClientHandler;
use rmcp::ErrorData as McpError;
use rmcp::RoleClient;
use rmcp::Service;
use rmcp::model::ClientCapabilities;
use rmcp::model::ClientInfo;
use rmcp::model::ClientRequest;
use rmcp::model::CreateElicitationRequestParams;
use rmcp::model::CreateElicitationResult;
use rmcp::model::CustomRequest;
use rmcp::model::ElicitationAction;
use rmcp::model::ServerResult;
use rmcp::service::RunningService;
use rmcp::transport::ConfigureCommandExt;
use rmcp::transport::TokioChildProcess;
use serde_json::json;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::process::Command;

pub async fn create_transport<P>(
    codex_home: P,
    dotslash_cache: P,
) -> anyhow::Result<TokioChildProcess>
where
    P: AsRef<Path>,
{
    let mcp_executable = codex_utils_cargo_bin::cargo_bin("codex-exec-mcp-server")?;
    let execve_wrapper = codex_utils_cargo_bin::cargo_bin("codex-execve-wrapper")?;

    // `bash` is a test resource rather than a binary target, so we must use
    // `find_resource!` to locate it instead of `cargo_bin()`.
    let bash = find_resource!("../suite/bash")?;

    // Need to ensure the artifact associated with the bash DotSlash file is
    // available before it is run in a read-only sandbox.
    let status = Command::new("dotslash")
        .arg("--")
        .arg("fetch")
        .arg(bash.clone())
        .env("DOTSLASH_CACHE", dotslash_cache.as_ref())
        .status()
        .await?;
    assert!(status.success(), "dotslash fetch failed: {status:?}");

    let transport = TokioChildProcess::new(Command::new(&mcp_executable).configure(|cmd| {
        cmd.arg("--bash").arg(bash);
        cmd.arg("--execve").arg(&execve_wrapper);
        cmd.env("CODEX_HOME", codex_home.as_ref());
        cmd.env("DOTSLASH_CACHE", dotslash_cache.as_ref());

        // Important: pipe stdio so rmcp can speak JSON-RPC over stdin/stdout
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());

        // Optional but very helpful while debugging:
        cmd.stderr(Stdio::inherit());
    }))?;

    Ok(transport)
}

pub async fn write_default_execpolicy<P>(policy: &str, codex_home: P) -> anyhow::Result<()>
where
    P: AsRef<Path>,
{
    let policy_dir = codex_home.as_ref().join("rules");
    tokio::fs::create_dir_all(&policy_dir).await?;
    tokio::fs::write(policy_dir.join("default.rules"), policy).await?;
    Ok(())
}

pub async fn notify_readable_sandbox<P, S>(
    sandbox_cwd: P,
    codex_linux_sandbox_exe: Option<PathBuf>,
    service: &RunningService<RoleClient, S>,
) -> anyhow::Result<ServerResult>
where
    P: AsRef<Path>,
    S: Service<RoleClient> + ClientHandler,
{
    let sandbox_state = SandboxState {
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        codex_linux_sandbox_exe,
        sandbox_cwd: sandbox_cwd.as_ref().to_path_buf(),
        use_linux_sandbox_bwrap: false,
    };
    send_sandbox_state_update(sandbox_state, service).await
}

pub async fn notify_writable_sandbox_only_one_folder<P, S>(
    writable_folder: P,
    codex_linux_sandbox_exe: Option<PathBuf>,
    service: &RunningService<RoleClient, S>,
) -> anyhow::Result<ServerResult>
where
    P: AsRef<Path>,
    S: Service<RoleClient> + ClientHandler,
{
    let sandbox_state = SandboxState {
        sandbox_policy: SandboxPolicy::WorkspaceWrite {
            // Note that sandbox_cwd will already be included as a writable root
            // when the sandbox policy is expanded.
            writable_roots: vec![],
            read_only_access: Default::default(),
            network_access: false,
            // Disable writes to temp dir because this is a test, so
            // writable_folder is likely also under /tmp and we want to be
            // strict about what is writable.
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        },
        codex_linux_sandbox_exe,
        sandbox_cwd: writable_folder.as_ref().to_path_buf(),
        use_linux_sandbox_bwrap: false,
    };
    send_sandbox_state_update(sandbox_state, service).await
}

async fn send_sandbox_state_update<S>(
    sandbox_state: SandboxState,
    service: &RunningService<RoleClient, S>,
) -> anyhow::Result<ServerResult>
where
    S: Service<RoleClient> + ClientHandler,
{
    let response = service
        .send_request(ClientRequest::CustomRequest(CustomRequest::new(
            MCP_SANDBOX_STATE_METHOD,
            Some(serde_json::to_value(sandbox_state)?),
        )))
        .await?;
    Ok(response)
}

pub struct InteractiveClient {
    pub elicitations_to_accept: HashSet<String>,
    pub elicitation_requests: Arc<Mutex<Vec<CreateElicitationRequestParams>>>,
}

impl ClientHandler for InteractiveClient {
    fn get_info(&self) -> ClientInfo {
        let capabilities = ClientCapabilities::builder().enable_elicitation().build();
        ClientInfo {
            capabilities,
            ..Default::default()
        }
    }

    fn create_elicitation(
        &self,
        request: CreateElicitationRequestParams,
        _context: rmcp::service::RequestContext<RoleClient>,
    ) -> impl std::future::Future<Output = Result<CreateElicitationResult, McpError>> + Send + '_
    {
        self.elicitation_requests
            .lock()
            .unwrap()
            .push(request.clone());

        let message = match &request {
            CreateElicitationRequestParams::FormElicitationParams { message, .. }
            | CreateElicitationRequestParams::UrlElicitationParams { message, .. } => message,
        };
        let accept = self.elicitations_to_accept.contains(message);
        async move {
            if accept {
                Ok(CreateElicitationResult {
                    action: ElicitationAction::Accept,
                    content: Some(json!({ "approve": true })),
                })
            } else {
                Ok(CreateElicitationResult {
                    action: ElicitationAction::Decline,
                    content: None,
                })
            }
        }
    }
}
