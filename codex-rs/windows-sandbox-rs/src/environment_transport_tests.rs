//! Shared transport roundtrips, validation, and environment cleanup.

use std::collections::HashMap;

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn unicode_payload_roundtrips_and_stale_chunks_are_replaced() {
    let payload = "a🧊".repeat(500_000);
    let expected = HashMap::from([("CUSTOM".to_owned(), "value".to_owned())]);
    let mut env = expected.clone();
    env.insert(
        format!("{}999", PREFIX.to_ascii_lowercase()),
        "spoofed".to_owned(),
    );
    encode(&payload, &mut env).unwrap();
    assert!(!env.contains_key(&format!("{}999", PREFIX.to_ascii_lowercase())));
    assert_eq!(decode_and_scrub(&mut env).unwrap(), payload);
    assert_eq!(env, expected);
}

#[test]
fn malformed_transport_is_rejected_and_scrubbed() {
    let expected = HashMap::from([("CUSTOM".to_owned(), "value".to_owned())]);
    let mut valid = expected.clone();
    encode("payload", &mut valid).unwrap();
    for (key, value) in [
        (COUNT.to_owned(), (MAX_CHUNKS + 1).to_string()),
        (LENGTH.to_owned(), "0".to_owned()),
        (LENGTH.to_owned(), (MAX_BYTES + 1).to_string()),
        (format!("{PREFIX}0"), "x".repeat(CHUNK_BYTES + 1)),
        (COUNT.to_ascii_lowercase(), "1".to_owned()),
        (format!("{PREFIX}unexpected"), "x".to_owned()),
    ] {
        let mut env = valid.clone();
        env.insert(key, value);
        assert!(decode_and_scrub(&mut env).is_err());
        assert_eq!(env, expected);
    }
    valid.remove(&format!("{PREFIX}0"));
    assert!(decode_and_scrub(&mut valid).is_err());
    assert_eq!(valid, expected);
    assert!(decode(HashMap::<String, String>::new()).is_err());
}

#[test]
fn rejected_payload_preserves_environment() {
    let expected = HashMap::from([("CUSTOM".to_owned(), "value".to_owned())]);
    for payload in [
        String::new(),
        "x".repeat(MAX_BYTES + 1),
        "bad\0payload".into(),
    ] {
        let mut env = expected.clone();
        assert!(encode(&payload, &mut env).is_err());
        assert_eq!(env, expected);
    }
}

#[cfg(any(windows, unix))]
#[test]
fn native_environment_ignores_unrelated_non_unicode_and_rejects_invalid_transport() {
    use std::ffi::OsString;
    #[cfg(unix)]
    use std::os::unix::ffi::OsStringExt;
    #[cfg(windows)]
    use std::os::windows::ffi::OsStringExt;
    #[cfg(windows)]
    let invalid = OsString::from_wide(&[0xd800]);
    #[cfg(unix)]
    let invalid = OsString::from_vec(vec![0xff]);
    let mut env = vec![
        (OsString::from(COUNT), OsString::from("1")),
        (OsString::from(LENGTH), OsString::from("2")),
        (OsString::from(format!("{PREFIX}0")), OsString::from("ok")),
        (OsString::from("unrelated"), invalid.clone()),
        (invalid.clone(), OsString::from("unrelated")),
    ];
    assert_eq!(decode(env.clone()).unwrap(), "ok");
    env.push((OsString::from(format!("{PREFIX}0")), invalid.clone()));
    assert!(decode(env).is_err());
    let mut invalid_key = OsString::from(PREFIX);
    invalid_key.push(invalid);
    assert!(decode([(invalid_key, OsString::from("bad"))]).is_err());
}

#[cfg(windows)]
#[test]
fn windows_equivalent_names_are_scrubbed_decoded_and_deduplicated() {
    let alias_prefix = PREFIX.to_ascii_lowercase();
    let alias_count = format!("{alias_prefix}count");
    let alias_length = format!("{alias_prefix}bytes");
    let mut env = HashMap::from([
        (alias_count.clone(), "1".to_owned()),
        (alias_length.clone(), "2".to_owned()),
        (format!("{alias_prefix}0"), "ok".to_owned()),
    ]);
    assert_eq!(decode(env.clone()).unwrap(), "ok");
    env.insert(COUNT.to_owned(), "1".to_owned());
    assert!(decode(env).is_err());

    let mut stale = HashMap::from([
        (alias_count, "1".to_owned()),
        (alias_length, "2".to_owned()),
        (format!("{alias_prefix}999"), "stale".to_owned()),
    ]);
    encode("ok", &mut stale).unwrap();
    assert_eq!(stale.len(), 3);
}

// Exercise a real Unicode Windows environment block, without provisioning or ACL changes.
#[cfg(windows)]
#[test]
fn large_payload_survives_windows_process_creation() {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    let payload = r#"C:\fixture\秘密\file-😀.txt\""#.repeat(150_000);
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .arg("--exact")
        .arg("environment_transport::tests::environment_child");
    let mut env = HashMap::new();
    encode(&payload, &mut env).unwrap();
    // Command preserves the spelling of a Windows-equivalent inherited key.
    command.env(LENGTH.replace('S', "s"), "stale");
    command.env("unrelated", OsString::from_wide(&[0xd800]));
    let output = command.envs(env).arg("--nocapture").output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("transport-child-ok"));
}

#[cfg(windows)]
#[test]
fn environment_child() {
    if std::env::var_os(LENGTH).is_none() {
        return;
    }
    let payload = r#"C:\fixture\秘密\file-😀.txt\""#.repeat(150_000);
    assert_eq!(decode(std::env::vars_os()).unwrap(), payload);
    println!("transport-child-ok");
}
