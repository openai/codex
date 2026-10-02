//! Opt-in, bounded diagnostics contain only classifications, never error-chain or wire values.
//! Nonterminal records describe one stream or a recoverable shared-transport event.
use std::fmt;
use std::io::Write;

use anyhow::Result;
use anyhow::anyhow;
use http::StatusCode;

#[derive(Clone, Copy)]
pub(super) enum Diagnostics {
    Human,
    Json,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Phase {
    Startup,
    Connect,
    Transport,
    Control,
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Startup => "startup",
            Self::Connect => "connect",
            Self::Transport => "transport",
            Self::Control => "control",
        })
    }
}

#[derive(Debug)]
pub(super) enum Code {
    Failed,
    Timeout,
    HandshakeTimeout,
    Rejected(StatusCode),
    Closed,
    Draining,
    InvalidInput,
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed => f.write_str("failed"),
            Self::HandshakeTimeout => {
                f.write_str("QUIC handshake failed: QUIC handshake timed out")
            }
            Self::Timeout => f.write_str("connecting to MASQUE proxy timed out"),
            Self::Rejected(status) => {
                write!(f, "MASQUE CONNECT rejected with status {}", status.as_u16())
            }
            Self::Closed => f.write_str("closed"),
            Self::Draining => f.write_str("draining"),
            Self::InvalidInput => f.write_str("invalid control input"),
        }
    }
}

impl std::error::Error for Code {}

impl Diagnostics {
    pub(super) fn with_phase<T>(self, result: Result<T>, phase: Phase) -> Result<T> {
        match self {
            Self::Human => result,
            Self::Json => result.map_err(|error| error.context(phase)),
        }
    }

    pub(super) fn finish(
        self,
        result: Result<()>,
        phase: Phase,
        output: &mut impl Write,
    ) -> Result<()> {
        match (self, result) {
            (Self::Json, Err(error)) => {
                Self::write(output, phase, &error, /*terminal*/ true);
                // The CLI formats returned errors. Do not let its top-level handler print the raw chain.
                Err(anyhow!("TCP tunnel failed"))
            }
            (_, result) => result,
        }
    }

    pub(super) fn report(self, phase: Phase, error: &anyhow::Error, human: fmt::Arguments<'_>) {
        match self {
            Self::Human => eprintln!("{human}"),
            Self::Json => Self::write(
                &mut std::io::stderr().lock(),
                phase,
                error,
                /*terminal*/ false,
            ),
        }
    }

    fn write(output: &mut impl Write, phase: Phase, error: &anyhow::Error, terminal: bool) {
        let phase = error.downcast_ref::<Phase>().copied().unwrap_or(phase);
        let fallback = if matches!(phase, Phase::Control)
            && error.downcast_ref::<std::io::Error>().is_none()
        {
            Code::InvalidInput
        } else {
            Code::Failed
        };
        let code = error.downcast_ref::<Code>().unwrap_or(&fallback);
        let mut record = serde_json::json!({
            "v": 1,
            "phase": phase.to_string(),
            "code": match code {
                Code::Failed => "failed",
                Code::Timeout | Code::HandshakeTimeout => "timeout",
                Code::Rejected(_) => "rejected",
                Code::Closed => "closed",
                Code::Draining => "draining",
                Code::InvalidInput => "invalid_input",
            },
            "terminal": terminal,
        });
        if let (Phase::Connect, Code::Rejected(status)) = (phase, code)
            && (100..600).contains(&status.as_u16())
            && !status.is_success()
        {
            record["http_status"] = status.as_u16().into();
        }
        // All fields are bounded enums or numbers; one locked, flushed line precedes process exit.
        let _ = writeln!(output, "{record}");
        let _ = output.flush();
    }
}

#[cfg(test)]
#[path = "diagnostics_tests.rs"]
mod tests;
