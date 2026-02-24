//! Unix shell-escalation protocol implementation.
//!
//! A patched shell invokes an exec wrapper on every `exec()` attempt. The wrapper sends an
//! `EscalateRequest` over the inherited `CODEX_ESCALATE_SOCKET`, and the server decides whether to
//! run the command directly (`Run`) or execute it on the server side (`Escalate`).
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
pub mod socket;
pub mod core_shell_escalation;
pub mod stopwatch;
