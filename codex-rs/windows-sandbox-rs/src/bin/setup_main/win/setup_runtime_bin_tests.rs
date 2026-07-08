use super::runtime_paths;
use pretty_assertions::assert_eq;
use std::path::PathBuf;

#[test]
fn runtime_paths_include_desktop_and_primary_runtime_roots() {
    let local_app_data = PathBuf::from(r"C:\Users\user\AppData\Local");
    let user_profile = PathBuf::from(r"C:\Users\user");

    assert_eq!(
        runtime_paths(Some(local_app_data), Some(user_profile)),
        vec![
            PathBuf::from(r"C:\Users\user\AppData\Local\OpenAI\Codex\bin"),
            PathBuf::from(r"C:\Users\user\AppData\Local\OpenAI\Codex\runtimes"),
            PathBuf::from(r"C:\Users\user\.cache\codex-runtimes"),
        ]
    );
}

#[test]
fn primary_runtime_path_does_not_depend_on_local_app_data() {
    let user_profile = PathBuf::from(r"C:\Users\user");

    assert_eq!(
        runtime_paths(/*local_app_data*/ None, Some(user_profile)),
        vec![PathBuf::from(r"C:\Users\user\.cache\codex-runtimes")]
    );
}
