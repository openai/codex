use crate::Arg;
use crate::Command;

#[test]
fn propagate_version() {
    let mut cmd = Command::new("test")
        .propagate_version(true)
        .version("1.1")
        .subcommand(Command::new("sub1"));
    cmd._propagate();
    assert_eq!(
        cmd.get_subcommands().next().unwrap().get_version(),
        Some("1.1")
    );
}

#[test]
fn global_setting() {
    let mut cmd = Command::new("test")
        .disable_version_flag(true)
        .subcommand(Command::new("subcmd"));
    cmd._propagate();
    assert!(cmd
        .get_subcommands()
        .find(|s| s.get_name() == "subcmd")
        .unwrap()
        .is_disable_version_flag_set());
}

// This test will *fail to compile* if Command is not Send + Sync
#[test]
fn app_send_sync() {
    fn foo<T: Send + Sync>(_: T) {}
    foo(Command::new("test"));
}

#[test]
fn issue_2090() {
    let mut cmd = Command::new("cmd")
        .disable_version_flag(true)
        .subcommand(Command::new("sub"));
    cmd._build_self(false);

    assert!(cmd
        .get_subcommands()
        .next()
        .unwrap()
        .is_disable_version_flag_set());
}

// This test will *fail to compile* if Arg is not Send + Sync
#[test]
fn arg_send_sync() {
    fn foo<T: Send + Sync>(_: T) {}
    foo(Arg::new("test"));
}
