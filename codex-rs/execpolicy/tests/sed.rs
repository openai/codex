extern crate codex_execpolicy;

use codex_execpolicy::ArgType;
use codex_execpolicy::Error;
use codex_execpolicy::ExecCall;
use codex_execpolicy::MatchedArg;
use codex_execpolicy::MatchedExec;
use codex_execpolicy::MatchedFlag;
use codex_execpolicy::MatchedOpt;
use codex_execpolicy::Policy;
use codex_execpolicy::Result;
use codex_execpolicy::ValidExec;
use codex_execpolicy::get_default_policy;

fn setup() -> Policy {
    get_default_policy().expect("failed to load default policy")
}

#[test]
fn test_sed_print_specific_lines() -> Result<()> {
    let policy = setup();
    let sed = ExecCall::new("sed", &["-n", "122,202p", "hello.txt"]);
    assert_eq!(
        Ok(MatchedExec::Match {
            exec: ValidExec {
                program: "sed".to_string(),
                flags: vec![MatchedFlag::new("-n")],
                args: vec![
                    MatchedArg::new(1, ArgType::SedCommand, "122,202p")?,
                    MatchedArg::new(2, ArgType::ReadableFile, "hello.txt")?,
                ],
                system_path: vec!["/usr/bin/sed".to_string()],
                ..Default::default()
            }
        }),
        policy.check(&sed)
    );
    Ok(())
}

#[test]
fn test_sed_print_specific_lines_with_e_flag() -> Result<()> {
    let policy = setup();
    let sed = ExecCall::new("sed", &["-n", "-e", "122,202p", "hello.txt"]);
    assert_eq!(
        Ok(MatchedExec::Match {
            exec: ValidExec {
                program: "sed".to_string(),
                flags: vec![MatchedFlag::new("-n")],
                opts: vec![MatchedOpt::new("-e", "122,202p", ArgType::SedCommand).unwrap()],
                args: vec![MatchedArg::new(3, ArgType::ReadableFile, "hello.txt")?],
                system_path: vec!["/usr/bin/sed".to_string()],
            }
        }),
        policy.check(&sed)
    );
    Ok(())
}

#[test]
fn test_sed_reject_dangerous_command() {
    let policy = setup();
    let sed = ExecCall::new("sed", &["-e", "s/y/echo hi/e", "hello.txt"]);
    assert_eq!(
        Err(Error::SedCommandNotProvablySafe {
            command: "s/y/echo hi/e".to_string(),
        }),
        policy.check(&sed)
    );
}

#[test]
fn test_sed_verify_e_or_pattern_is_required() {
    let policy = setup();
    let sed = ExecCall::new("sed", &["122,202p"]);
    assert_eq!(
        Err(Error::MissingRequiredOptions {
            program: "sed".to_string(),
            options: vec!["-e".to_string()],
        }),
        policy.check(&sed)
    );
}
