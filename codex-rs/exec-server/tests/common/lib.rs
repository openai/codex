use codex_core::MCP_SANDBOX_STATE_NOTIFICATION;
use codex_core::SandboxState;
use codex_core::protocol::SandboxPolicy;
use rmcp::ClientHandler;
use rmcp::ErrorData as McpError;
use rmcp::RoleClient;
use rmcp::Service;
use rmcp::model::ClientCapabilities;
use rmcp::model::ClientInfo;
use rmcp::model::CreateElicitationRequestParam;
use rmcp::model::CreateElicitationResult;
use rmcp::model::CustomClientNotification;
use rmcp::model::ElicitationAction;
use rmcp::service::RunningService;
use rmcp::transport::ConfigureCommandExt;
use rmcp::transport::TokioChildProcess;
use serde_json::json;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::process::Command;

const CODEX_SHELL_TOOL_MCP_TGZ_URL: &str = "https://github.com/openai/codex/releases/download/rust-v0.65.0/codex-shell-tool-mcp-npm-0.65.0.tgz";

pub fn create_transport<P>(codex_home: P) -> anyhow::Result<TokioChildProcess>
where
    P: AsRef<Path>,
{
    let mcp_executable = assert_cmd::Command::cargo_bin("codex-exec-mcp-server")?;
    let execve_wrapper = assert_cmd::Command::cargo_bin("codex-execve-wrapper")?;
    let mcp_program = Path::new(mcp_executable.get_program());
    let bash = ensure_patched_bash(mcp_program)?;

    let transport = TokioChildProcess::new(Command::new(mcp_program).configure(|cmd| {
        cmd.arg("--bash").arg(bash);
        cmd.arg("--execve").arg(execve_wrapper.get_program());
        cmd.env("CODEX_HOME", codex_home.as_ref());

        // Important: pipe stdio so rmcp can speak JSON-RPC over stdin/stdout
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());

        // Optional but very helpful while debugging:
        cmd.stderr(Stdio::inherit());
    }))?;

    Ok(transport)
}

fn ensure_patched_bash(mcp_executable: &Path) -> anyhow::Result<PathBuf> {
    let bin_dir = mcp_executable
        .parent()
        .ok_or_else(|| anyhow::anyhow!("failed to determine bin dir for {mcp_executable:?}"))?;
    let out_path = bin_dir.join("codex-test-bash");
    if out_path.exists() {
        return Ok(out_path);
    }

    let platform_path = if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "package/vendor/aarch64-apple-darwin/bash/macos-15/bash"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        "package/vendor/x86_64-unknown-linux-musl/bash/ubuntu-24.04/bash"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") {
        "package/vendor/aarch64-unknown-linux-musl/bash/ubuntu-24.04/bash"
    } else {
        anyhow::bail!("unsupported platform for exec-server test bash");
    };

    let staging_dir = bin_dir.join("codex-test-bash.staging");
    let _ = fs::remove_dir_all(&staging_dir);
    fs::create_dir_all(&staging_dir)?;

    let tgz_path = staging_dir.join("codex-shell-tool-mcp.tgz");
    let curl_status = std::process::Command::new("curl")
        .args(["-fsSL", CODEX_SHELL_TOOL_MCP_TGZ_URL, "-o"])
        .arg(&tgz_path)
        .status()?;
    if !curl_status.success() {
        anyhow::bail!("failed to download {CODEX_SHELL_TOOL_MCP_TGZ_URL}");
    }

    let tar_status = std::process::Command::new("tar")
        .arg("-xzf")
        .arg(&tgz_path)
        .arg("-C")
        .arg(&staging_dir)
        .arg(platform_path)
        .status()?;
    if !tar_status.success() {
        anyhow::bail!("failed to extract {platform_path} from {CODEX_SHELL_TOOL_MCP_TGZ_URL}");
    }

    let extracted_path = staging_dir.join(platform_path);
    fs::copy(&extracted_path, &out_path)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mut perms = fs::metadata(&out_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&out_path, perms)?;
    }

    let _ = fs::remove_dir_all(&staging_dir);
    Ok(out_path)
}

pub async fn write_default_execpolicy<P>(policy: &str, codex_home: P) -> anyhow::Result<()>
where
    P: AsRef<Path>,
{
    let policy_dir = codex_home.as_ref().join("policy");
    tokio::fs::create_dir_all(&policy_dir).await?;
    tokio::fs::write(policy_dir.join("default.codexpolicy"), policy).await?;
    Ok(())
}

pub async fn notify_readable_sandbox<P, S>(
    sandbox_cwd: P,
    codex_linux_sandbox_exe: Option<PathBuf>,
    service: &RunningService<RoleClient, S>,
) -> anyhow::Result<()>
where
    P: AsRef<Path>,
    S: Service<RoleClient> + ClientHandler,
{
    let sandbox_state = SandboxState {
        sandbox_policy: SandboxPolicy::ReadOnly,
        codex_linux_sandbox_exe,
        sandbox_cwd: sandbox_cwd.as_ref().to_path_buf(),
    };
    send_sandbox_notification(sandbox_state, service).await
}

pub async fn notify_writable_sandbox_only_one_folder<P, S>(
    writable_folder: P,
    codex_linux_sandbox_exe: Option<PathBuf>,
    service: &RunningService<RoleClient, S>,
) -> anyhow::Result<()>
where
    P: AsRef<Path>,
    S: Service<RoleClient> + ClientHandler,
{
    let sandbox_state = SandboxState {
        sandbox_policy: SandboxPolicy::WorkspaceWrite {
            // Note that sandbox_cwd will already be included as a writable root
            // when the sandbox policy is expanded.
            writable_roots: vec![],
            network_access: false,
            // Disable writes to temp dir because this is a test, so
            // writable_folder is likely also under /tmp and we want to be
            // strict about what is writable.
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        },
        codex_linux_sandbox_exe,
        sandbox_cwd: writable_folder.as_ref().to_path_buf(),
    };
    send_sandbox_notification(sandbox_state, service).await
}

async fn send_sandbox_notification<S>(
    sandbox_state: SandboxState,
    service: &RunningService<RoleClient, S>,
) -> anyhow::Result<()>
where
    S: Service<RoleClient> + ClientHandler,
{
    let sandbox_state_notification = CustomClientNotification::new(
        MCP_SANDBOX_STATE_NOTIFICATION,
        Some(serde_json::to_value(sandbox_state)?),
    );
    service
        .send_notification(sandbox_state_notification.into())
        .await?;
    Ok(())
}

pub struct InteractiveClient {
    pub elicitations_to_accept: HashSet<String>,
    pub elicitation_requests: Arc<Mutex<Vec<CreateElicitationRequestParam>>>,
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
        request: CreateElicitationRequestParam,
        _context: rmcp::service::RequestContext<RoleClient>,
    ) -> impl std::future::Future<Output = Result<CreateElicitationResult, McpError>> + Send + '_
    {
        self.elicitation_requests
            .lock()
            .unwrap()
            .push(request.clone());

        let accept = self.elicitations_to_accept.contains(&request.message);
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
