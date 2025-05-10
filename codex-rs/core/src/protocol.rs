//! Defines the protocol for a Codex session between a client and an agent.
//!
//! Uses a SQ (Submission Queue) / EQ (Event Queue) pattern to asynchronously communicate
//! between user and agent.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use mcp_types::CallToolResult;
use serde::Deserialize;
use serde::Serialize;

use crate::model_provider_info::ModelProviderInfo;

/// Submission Queue Entry - requests from user
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Submission {
    /// Unique id for this Submission to correlate with Events
    pub id: String,
    /// Payload
    pub op: Op,
}

/// Submission operation
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
#[non_exhaustive]
pub enum Op {
    /// Configure the model session.
    ConfigureSession {
        /// Provider identifier ("openai", "openrouter", ...).
        provider: ModelProviderInfo,

        /// If not specified, server will use its default model.
        model: String,
        /// Model instructions
        instructions: Option<String>,
        /// When to escalate for approval for execution
        approval_policy: AskForApproval,
        /// How to sandbox commands executed in the system
        sandbox_policy: SandboxPolicy,
        /// Disable server-side response storage (send full context each request)
        #[serde(default)]
        disable_response_storage: bool,

        /// Optional external notifier command tokens. Present only when the
        /// client wants the agent to spawn a program after each completed
        /// turn.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(default)]
        notify: Option<Vec<String>>,

        /// Working directory that should be treated as the *root* of the
        /// session. All relative paths supplied by the model as well as the
        /// execution sandbox are resolved against this directory **instead**
        /// of the process-wide current working directory. CLI front-ends are
        /// expected to expand this to an absolute path before sending the
        /// `ConfigureSession` operation so that the business-logic layer can
        /// operate deterministically.
        cwd: std::path::PathBuf,
    },

    /// Abort current task.
    /// This server sends no corresponding Event
    Interrupt,

    /// Input from the user
    UserInput {
        /// User input items, see `InputItem`
        items: Vec<InputItem>,
    },

    /// Approve a command execution
    ExecApproval {
        /// The id of the submission we are approving
        id: String,
        /// The user's decision in response to the request.
        decision: ReviewDecision,
    },

    /// Approve a code patch
    PatchApproval {
        /// The id of the submission we are approving
        id: String,
        /// The user's decision in response to the request.
        decision: ReviewDecision,
    },
}

/// Determines how liberally commands are auto‑approved by the system.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AskForApproval {
    /// Under this policy, only “known safe” commands—as determined by
    /// `is_safe_command()`—that **only read files** are auto‑approved.
    /// Everything else will ask the user to approve.
    #[default]
    UnlessAllowListed,

    /// In addition to everything allowed by **`Suggest`**, commands that
    /// *write* to files **within the user’s approved list of writable paths**
    /// are also auto‑approved.
    /// TODO(ragona): fix
    AutoEdit,

    /// *All* commands are auto‑approved, but they are expected to run inside a
    /// sandbox where network access is disabled and writes are confined to a
    /// specific set of paths. If the command fails, it will be escalated to
    /// the user to approve execution without a sandbox.
    OnFailure,

    /// Never ask the user to approve commands. Failures are immediately returned
    /// to the model, and never escalated to the user for approval.
    Never,
}

/// Determines execution restrictions for model shell commands
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct SandboxPolicy {
    permissions: Vec<SandboxPermission>,
}

impl From<Vec<SandboxPermission>> for SandboxPolicy {
    fn from(permissions: Vec<SandboxPermission>) -> Self {
        Self { permissions }
    }
}

impl SandboxPolicy {
    pub fn new_read_only_policy() -> Self {
        Self {
            permissions: vec![SandboxPermission::DiskFullReadAccess],
        }
    }

    pub fn new_read_only_policy_with_writable_roots(writable_roots: &[PathBuf]) -> Self {
        let mut permissions = Self::new_read_only_policy().permissions;
        permissions.extend(writable_roots.iter().map(|folder| {
            SandboxPermission::DiskWriteFolder {
                folder: folder.clone(),
            }
        }));
        Self { permissions }
    }

    pub fn new_full_auto_policy() -> Self {
        Self {
            permissions: vec![
                SandboxPermission::DiskFullReadAccess,
                SandboxPermission::DiskWritePlatformUserTempFolder,
                SandboxPermission::DiskWriteCwd,
            ],
        }
    }

    pub fn has_full_disk_read_access(&self) -> bool {
        self.permissions
            .iter()
            .any(|perm| matches!(perm, SandboxPermission::DiskFullReadAccess))
    }

    pub fn has_full_disk_write_access(&self) -> bool {
        self.permissions
            .iter()
            .any(|perm| matches!(perm, SandboxPermission::DiskFullWriteAccess))
    }

    pub fn has_full_network_access(&self) -> bool {
        self.permissions
            .iter()
            .any(|perm| matches!(perm, SandboxPermission::NetworkFullAccess))
    }

    pub fn get_writable_roots_with_cwd(&self, cwd: &Path) -> Vec<PathBuf> {
        let mut writable_roots = Vec::<PathBuf>::new();
        for perm in &self.permissions {
            use SandboxPermission::*;
            match perm {
                DiskWritePlatformUserTempFolder => {
                    if cfg!(target_os = "macos") {
                        if let Some(tempdir) = std::env::var_os("TMPDIR") {
                            // Likely something that starts with /var/folders/...
                            let tmpdir_path = PathBuf::from(&tempdir);
                            if tmpdir_path.is_absolute() {
                                writable_roots.push(tmpdir_path.clone());
                                match tmpdir_path.canonicalize() {
                                    Ok(canonicalized) => {
                                        // Likely something that starts with /private/var/folders/...
                                        if canonicalized != tmpdir_path {
                                            writable_roots.push(canonicalized);
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to canonicalize TMPDIR: {e}");
                                    }
                                }
                            } else {
                                tracing::error!("TMPDIR is not an absolute path: {tempdir:?}");
                            }
                        }
                    }

                    // For Linux, should this be XDG_RUNTIME_DIR, /run/user/<uid>, or something else?
                }
                DiskWritePlatformGlobalTempFolder => {
                    if cfg!(unix) {
                        writable_roots.push(PathBuf::from("/tmp"));
                    }
                }
                DiskWriteCwd => {
                    writable_roots.push(cwd.to_path_buf());
                }
                DiskWriteFolder { folder } => {
                    writable_roots.push(folder.clone());
                }
                DiskFullReadAccess | NetworkFullAccess => {}
                DiskFullWriteAccess => {
                    // Currently, we expect callers to only invoke this method
                    // after verifying has_full_disk_write_access() is false.
                }
            }
        }
        writable_roots
    }

    pub fn is_unrestricted(&self) -> bool {
        self.has_full_disk_read_access()
            && self.has_full_disk_write_access()
            && self.has_full_network_access()
    }
}

/// Permissions that should be granted to the sandbox in which the agent
/// operates.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SandboxPermission {
    /// Is allowed to read all files on disk.
    DiskFullReadAccess,

    /// Is allowed to write to the operating system's temp dir that
    /// is restricted to the user the agent is running as. For
    /// example, on macOS, this is generally something under
    /// `/var/folders` as opposed to `/tmp`.
    DiskWritePlatformUserTempFolder,

    /// Is allowed to write to the operating system's shared temp
    /// dir. On UNIX, this is generally `/tmp`.
    DiskWritePlatformGlobalTempFolder,

    /// Is allowed to write to the current working directory (in practice, this
    /// is the `cwd` where `codex` was spawned).
    DiskWriteCwd,

    /// Is allowed to the specified folder. `PathBuf` must be an
    /// absolute path, though it is up to the caller to canonicalize
    /// it if the path contains symlinks.
    DiskWriteFolder { folder: PathBuf },

    /// Is allowed to write to any file on disk.
    DiskFullWriteAccess,

    /// Can make arbitrary network requests.
    NetworkFullAccess,
}

/// User input
#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputItem {
    Text {
        text: String,
    },
    /// Pre‑encoded data: URI image.
    Image {
        image_url: String,
    },

    /// Local image path provided by the user.  This will be converted to an
    /// `Image` variant (base64 data URL) during request serialization.
    LocalImage {
        path: std::path::PathBuf,
    },
}

/// Event Queue Entry - events from agent
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Event {
    /// Submission `id` that this event is correlated with.
    pub id: String,
    /// Payload
    pub msg: EventMsg,
}

/// Response event from the agent
#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventMsg {
    /// Error while executing a submission
    Error {
        message: String,
    },

    /// Agent has started a task
    TaskStarted,

    /// Agent has completed all actions
    TaskComplete,

    /// Agent text output message
    AgentMessage {
        message: String,
    },

    /// Ack the client's configure message.
    SessionConfigured {
        /// Tell the client what model is being queried.
        model: String,
    },

    McpToolCallBegin {
        /// Identifier so this can be paired with the McpToolCallEnd event.
        call_id: String,

        /// Name of the MCP server as defined in the config.
        server: String,

        /// Name of the tool as given by the MCP server.
        tool: String,

        /// Arguments to the tool call.
        arguments: Option<serde_json::Value>,
    },

    McpToolCallEnd {
        /// Identifier for the McpToolCallBegin that finished.
        call_id: String,

        /// Whether the tool call was successful. If `false`, `result` might
        /// not be present.
        success: bool,

        /// Result of the tool call. Note this could be an error.
        result: Option<CallToolResult>,
    },

    /// Notification that the server is about to execute a command.
    ExecCommandBegin {
        /// Identifier so this can be paired with the ExecCommandEnd event.
        call_id: String,
        /// The command to be executed.
        command: Vec<String>,
        /// The command's working directory if not the default cwd for the
        /// agent.
        cwd: PathBuf,
    },

    ExecCommandEnd {
        /// Identifier for the ExecCommandBegin that finished.
        call_id: String,
        /// Captured stdout
        stdout: String,
        /// Captured stderr
        stderr: String,
        /// The command's exit code.
        exit_code: i32,
    },

    ExecApprovalRequest {
        /// The command to be executed.
        command: Vec<String>,
        /// The command's working directory.
        cwd: PathBuf,
        /// Optional human‑readable reason for the approval (e.g. retry without
        /// sandbox).
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },

    ApplyPatchApprovalRequest {
        changes: HashMap<PathBuf, FileChange>,
        /// Optional explanatory reason (e.g. request for extra write access).
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,

        /// When set, the agent is asking the user to allow writes under this
        /// root for the remainder of the session.
        #[serde(skip_serializing_if = "Option::is_none")]
        grant_root: Option<PathBuf>,
    },

    BackgroundEvent {
        message: String,
    },

    /// Notification that the agent is about to apply a code patch. Mirrors
    /// `ExecCommandBegin` so front‑ends can show progress indicators.
    PatchApplyBegin {
        /// Identifier so this can be paired with the PatchApplyEnd event.
        call_id: String,

        /// If true, there was no ApplyPatchApprovalRequest for this patch.
        auto_approved: bool,

        /// The changes to be applied.
        changes: HashMap<PathBuf, FileChange>,
    },

    /// Notification that a patch application has finished.
    PatchApplyEnd {
        /// Identifier for the PatchApplyBegin that finished.
        call_id: String,
        /// Captured stdout (summary printed by apply_patch).
        stdout: String,
        /// Captured stderr (parser errors, IO failures, etc.).
        stderr: String,
        /// Whether the patch was applied successfully.
        success: bool,
    },
}

/// User's decision in response to an ExecApprovalRequest.
#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecision {
    /// User has approved this command and the agent should execute it.
    Approved,

    /// User has approved this command and wants to automatically approve any
    /// future identical instances (`command` and `cwd` match exactly) for the
    /// remainder of the session.
    ApprovedForSession,

    /// User has denied this command and the agent should not execute it, but
    /// it should continue the session and try something else.
    #[default]
    Denied,

    /// User has denied this command and the agent should not do anything until
    /// the user's next command.
    Abort,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChange {
    Add {
        content: String,
    },
    Delete,
    Update {
        unified_diff: String,
        move_path: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Chunk {
    /// 1-based line index of the first line in the original file
    pub orig_index: u32,
    pub deleted_lines: Vec<String>,
    pub inserted_lines: Vec<String>,
}
