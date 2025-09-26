extern crate codex_execpolicy;

use codex_execpolicy::ArgMatcher;
use codex_execpolicy::ArgType;
use codex_execpolicy::Error;
use codex_execpolicy::ExecCall;
use codex_execpolicy::MatchedArg;
use codex_execpolicy::MatchedExec;
use codex_execpolicy::Policy;
use codex_execpolicy::Result;
use codex_execpolicy::ValidExec;
use codex_execpolicy::get_default_policy;

#[expect(clippy::expect_used)]
fn setup() -> Policy {
    get_default_policy().expect("failed to load default policy")
}

#[test]
fn test_cp_no_args() {
    let policy = setup();
    let cp = ExecCall::new("cp", &[]);
    assert_eq!(
        Err(Error::NotEnoughArgs {
            program: "cp".to_string(),
            args: vec![],
            arg_patterns: vec![ArgMatcher::ReadableFiles, ArgMatcher::WriteableFile]
        }),
        policy.check(&cp)
    )
}

#[test]
fn test_cp_one_arg() {
    let policy = setup();
    let cp = ExecCall::new("cp", &["foo/bar"]);

    assert_eq!(
        Err(Error::VarargMatcherDidNotMatchAnything {
            program: "cp".to_string(),
            matcher: ArgMatcher::ReadableFiles,
        }),
        policy.check(&cp)
    );
}

#[test]
fn test_cp_one_file() -> Result<()> {
    let policy = setup();
    let cp = ExecCall::new("cp", &["foo/bar", "../baz"]);
    assert_eq!(
        Ok(MatchedExec::Match {
            exec: ValidExec::new(
                "cp",
                vec![
                    MatchedArg::new(0, ArgType::ReadableFile, "foo/bar")?,
                    MatchedArg::new(1, ArgType::WriteableFile, "../baz")?,
                ],
                &["/bin/cp", "/usr/bin/cp"]
            )
        }),
        policy.check(&cp)
    );
    Ok(())
}

#[test]
fn test_cp_multiple_files() -> Result<()> {
    let policy = setup();
    let cp = ExecCall::new("cp", &["foo", "bar", "baz"]);
    assert_eq!(
        Ok(MatchedExec::Match {
            exec: ValidExec::new(
                "cp",
                vec![
                    MatchedArg::new(0, ArgType::ReadableFile, "foo")?,
                    MatchedArg::new(1, ArgType::ReadableFile, "bar")?,
                    MatchedArg::new(2, ArgType::WriteableFile, "baz")?,
                ],
                &["/bin/cp", "/usr/bin/cp"]
            )
        }),
        policy.check(&cp)
    );
    Ok(())
}
