extern crate codex_execpolicy;

use codex_execpolicy::{
    ArgMatcher, ArgType, Error, ExecCall, MatchedArg, MatchedExec, Policy, Result, ValidExec,
    get_default_policy,
};

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
