#![cfg(test)]

use super::cmd_script_command_line;

#[test]
fn preserves_quotes_in_the_hash_command() {
    let argv = [
        r"C:\Windows\System32\CMD.EXE",
        "/C",
        r#"certutil -hashfile "C:\New folder\long name.pftrace" SHA256"#,
    ]
    .map(str::to_owned);
    assert_eq!(
        cmd_script_command_line(&argv).as_deref(),
        Some(
            r#""C:\Windows\System32\CMD.EXE" /s /c "certutil -hashfile "C:\New folder\long name.pftrace" SHA256""#
        )
    );
}
