use codex_execpolicy::Error;
use codex_execpolicy::parse_sed_command;

#[test]
fn parses_simple_print_command() {
    assert_eq!(parse_sed_command("122,202p"), Ok(()));
}

#[test]
fn rejects_malformed_print_command() {
    assert_eq!(
        parse_sed_command("122,202"),
        Err(Error::SedCommandNotProvablySafe {
            command: "122,202".to_string(),
        })
    );
    assert_eq!(
        parse_sed_command("122202"),
        Err(Error::SedCommandNotProvablySafe {
            command: "122202".to_string(),
        })
    );
}
