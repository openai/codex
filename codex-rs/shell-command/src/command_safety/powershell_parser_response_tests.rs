use super::*;
use pretty_assertions::assert_eq;

fn encode_commands(commands: &[&[&str]]) -> String {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&u32::try_from(commands.len()).unwrap().to_le_bytes());
    for command in commands {
        bytes.extend_from_slice(&u32::try_from(command.len()).unwrap().to_le_bytes());
        for word in *command {
            bytes.extend_from_slice(&u32::try_from(word.len()).unwrap().to_le_bytes());
            bytes.extend_from_slice(word.as_bytes());
        }
    }
    BASE64_STANDARD.encode(bytes)
}

#[test]
fn framed_protocol_preserves_ids_and_command_words() {
    for (id, payload, expected) in [
        (
            42,
            "RwBlAHQALQBDAG8AbgB0AGUAbgB0AA==",
            "42\tRwBlAHQALQBDAG8AbgB0AGUAbgB0AA==",
        ),
        (43, "", "43\t"),
    ] {
        assert_eq!(
            serialize_request(&PowershellParserRequest {
                id,
                payload: payload.into()
            })
            .unwrap(),
            expected,
        );
    }

    let payload = encode_commands(&[&["Get-Content", "fóó.txt"], &["Measure-Object"]]);
    assert_eq!(
        deserialize_response(&format!("42\tok\t{payload}\r\n")).unwrap(),
        PowershellParserResponse {
            id: 42,
            status: "ok".into(),
            commands: Some(vec![
                ["Get-Content", "fóó.txt"].map(str::to_string).to_vec(),
                vec!["Measure-Object".into()],
            ]),
        },
    );
}

#[test]
fn framed_protocol_rejects_malformed_command_payloads() {
    let encode = |bytes| format!("1\tok\t{}", BASE64_STANDARD.encode(bytes));
    for response in [
        "missing-fields".into(),
        "1\tok".into(),
        "x\tunsupported\t".into(),
        "1\tunsupported\tnot-empty".into(),
        "1\tunsupported\t\textra".into(),
        "1\tok\tnot-base64!".into(),
        encode(vec![1, 0, 0]),
        encode(vec![u8::MAX; 4]),
        encode(vec![1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, u8::MAX]),
        encode(vec![1, 0, 0, 0, 1, 0, 0, 0, 255, 255, 255, 255]),
        encode(vec![0, 0, 0, 0, u8::MAX]),
    ] {
        assert!(
            deserialize_response(&response).is_err(),
            "accepted {response:?}"
        );
    }
    assert!(
        serialize_request(&PowershellParserRequest {
            id: 1,
            payload: "bad\tpayload".into(),
        })
        .is_err()
    );
}

#[test]
fn framed_protocol_bounds_lines_and_distinguishes_outcomes() {
    let mut complete = std::io::Cursor::new(b"1\tunsupported\t\n".as_slice());
    assert_eq!(
        read_bounded_response_line(&mut complete, /*max_bytes*/ 32).unwrap(),
        "1\tunsupported\t\n"
    );
    for bytes in [b"123456789\n".as_slice(), b"unterminated".as_slice()] {
        assert!(
            read_bounded_response_line(&mut std::io::Cursor::new(bytes), /*max_bytes*/ 8,).is_err()
        );
    }

    for (status, commands, expected) in [
        ("unsupported", None, PowershellParseOutcome::Unsupported),
        ("ok", Some(Vec::new()), PowershellParseOutcome::Unsupported),
        (
            "ok",
            Some(vec![Vec::new()]),
            PowershellParseOutcome::Unsupported,
        ),
        (
            "ok",
            Some(vec![vec![String::new()]]),
            PowershellParseOutcome::Unsupported,
        ),
        ("parse_failed", None, PowershellParseOutcome::Failed),
        ("parse_errors", None, PowershellParseOutcome::Failed),
        ("unknown", None, PowershellParseOutcome::Failed),
    ] {
        assert_eq!(
            PowershellParserResponse {
                id: 0,
                status: status.into(),
                commands
            }
            .into_outcome(),
            expected,
        );
    }
}
