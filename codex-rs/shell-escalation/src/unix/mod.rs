//! Unix shell-escalation protocol implementation.
//!
//! A patched shell invokes an exec wrapper on every `exec()` attempt. The wrapper sends an
//! `EscalateRequest` over the inherited `CODEX_ESCALATE_SOCKET`, and the server decides whether to
//! run the command directly (`Run`) or execute it on the server side (`Escalate`).
//!
//! Of key importance is the `EscalateRequest` includes a file descriptor for a socket
//! that the server can use to send the response to the execve wrapper. In this
//! way, all descendents of the Server process can use the file descriptor
//! specified by the `CODEX_ESCALATE_SOCKET` environment variable to _send_ escalation requests,
//! but responses are read from a separate socket that is created for each request, which
//! allows the server to handle multiple concurrent escalation requests.
//!
//! ### Escalation flow
//!
//! Command  Server  Shell  Execve Wrapper
//!          |
//!          o----->o
//!          |      |
//!          |      o--(exec)-->o
//!          |      |           |
//!          |o<-(EscalateReq)--o
//!          ||     |           |
//!          |o--(Escalate)---->o
//!          ||     |           |
//!          |o<---------(fds)--o
//!          ||     |           |
//!   o<------o     |           |
//!   |      ||     |           |
//!   x------>o     |           |
//!          ||     |           |
//!          |x--(exit code)--->o
//!          |      |           |
//!          |      o<--(exit)--x
//!          |      |
//!          o<-----x
//!
//! ### Non-escalation flow
//!
//! Server  Shell  Execve Wrapper  Command
//!   |
//!   o----->o
//!   |      |
//!   |      o--(exec)-->o
//!   |      |           |
//!   |o<-(EscalateReq)--o
//!   ||     |           |
//!   |o-(Run)---------->o
//!   |      |           |
//!   |      |           x--(exec)-->o
//!   |      |                       |
//!   |      o<--------------(exit)--x
//!   |      |
//!   o<-----x
//!
pub mod escalate_client;
pub mod escalate_protocol;
pub mod escalate_server;
pub mod escalation_policy;
pub mod execve_wrapper;
pub mod socket;
pub mod stopwatch;

pub use self::escalate_client::run_shell_escalation_execve_wrapper;
pub use self::escalate_protocol::EscalateAction;
pub use self::escalate_protocol::EscalationDecision;
pub use self::escalate_protocol::EscalationExecution;
pub use self::escalate_server::EscalateServer;
pub use self::escalate_server::ExecParams;
pub use self::escalate_server::ExecResult;
pub use self::escalate_server::PreparedExec;
pub use self::escalate_server::ShellCommandExecutor;
pub use self::escalation_policy::EscalationPolicy;
pub use self::execve_wrapper::main_execve_wrapper;
pub use self::stopwatch::Stopwatch;
pub use codex_protocol::approvals::EscalationPermissions;
pub use codex_protocol::approvals::Permissions;
