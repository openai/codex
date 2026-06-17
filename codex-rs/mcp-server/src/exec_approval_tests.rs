use super::*;
use pretty_assertions::assert_eq;

#[test]
fn message_names_execution_environment() {
    let command = vec!["echo".to_string(), "hello world".to_string()];

    assert_eq!(
        exec_approval_message(&command, Path::new("/workspace"), Some("remote")),
        "Allow Codex to run `echo 'hello world'` in environment `remote` with working directory `/workspace`?"
    );
    assert_eq!(
        exec_approval_message(
            &command,
            Path::new("/workspace"),
            /*environment_id*/ None,
        ),
        "Allow Codex to run `echo 'hello world'` in `/workspace`?"
    );
}
