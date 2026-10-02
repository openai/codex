use super::Code;
use super::Diagnostics;
use super::Phase;
use anyhow::Context;
use anyhow::anyhow;
use http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn fatal_diagnostic_drops_untrusted_error_chain_and_flushes_one_record() {
    #[derive(Default)]
    struct Output {
        bytes: Vec<u8>,
        flushes: usize,
    }
    impl std::io::Write for Output {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.flushes += 1;
            Ok(())
        }
    }
    let mut output = Output::default();
    let error = Diagnostics::Json
        .finish(
            Err(anyhow!(
                "Bearer secret; x-route: private; https://private.example/path"
            ))
            .context(Code::Timeout),
            Phase::Startup,
            &mut output,
        )
        .unwrap_err();
    assert_eq!(format!("{error:#}"), "TCP tunnel failed");
    assert_eq!(output.flushes, 1);
    assert!(output.bytes.len() < 256);
    assert_eq!(
        output.bytes.iter().filter(|&&byte| byte == b'\n').count(),
        1
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.bytes).unwrap(),
        json!({
            "v": 1, "phase": "startup", "code": "timeout", "terminal": true,
        })
    );
}

#[test]
fn control_io_failure_and_invalid_input_have_distinct_safe_codes() {
    for (error, code) in [
        (anyhow!("invalid private bearer"), "invalid_input"),
        (
            anyhow!(std::io::Error::other("private input failure")),
            "failed",
        ),
    ] {
        let mut output = Vec::new();
        Diagnostics::Json
            .finish(
                Err(error).context(Phase::Control),
                Phase::Startup,
                &mut output,
            )
            .unwrap_err();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output).unwrap(),
            json!({
                "v": 1, "phase": "control", "code": code, "terminal": true,
            })
        );
    }
}

#[test]
fn connect_rejection_is_nonterminal_and_contains_only_valid_status() {
    for status in [403, 407, 503, 999] {
        let mut output = Vec::new();
        let error = anyhow!("private proxy response body")
            .context(Code::Rejected(StatusCode::from_u16(status).unwrap()));
        Diagnostics::write(&mut output, Phase::Connect, &error, /*terminal*/ false);
        let mut expected =
            json!({"v": 1, "phase": "connect", "code": "rejected", "terminal": false});
        if status < 600 {
            expected["http_status"] = status.into();
        }
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output).unwrap(),
            expected
        );
    }
}

#[test]
fn legacy_errors_and_successful_shutdown_produce_no_json() {
    let mut output = Vec::new();
    let result = Diagnostics::Human.with_phase(Err(anyhow!("legacy error")), Phase::Transport);
    let error = Diagnostics::Human
        .finish(result, Phase::Startup, &mut output)
        .unwrap_err();
    assert_eq!(format!("{error:#}"), "legacy error");
    Diagnostics::Json
        .finish(Ok(()), Phase::Control, &mut output)
        .unwrap();
    assert!(output.is_empty());
}

#[test]
fn handshake_timeout_preserves_legacy_debug_output() {
    let error = anyhow!(Code::HandshakeTimeout);
    assert_eq!(
        format!("{error:?}"),
        "QUIC handshake failed: QUIC handshake timed out"
    );
    let mut output = Vec::new();
    Diagnostics::write(
        &mut output,
        Phase::Transport,
        &error,
        /*terminal*/ false,
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output).unwrap(),
        json!({
            "v": 1, "phase": "transport", "code": "timeout", "terminal": false,
        })
    );
}
