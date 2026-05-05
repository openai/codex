use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OpenTarget {
    BundleId(String),
    AppPath(PathBuf),
}

pub(crate) fn build_open_args(
    target: &OpenTarget,
    workspace: &Path,
    config_overrides: &[String],
) -> Vec<OsString> {
    let mut args = Vec::new();
    match target {
        OpenTarget::BundleId(bundle_id) => {
            args.push(OsString::from("-b"));
            args.push(OsString::from(bundle_id.as_str()));
        }
        OpenTarget::AppPath(app_path) => {
            args.push(OsString::from("-a"));
            args.push(app_path.as_os_str().to_os_string());
        }
    }
    args.push(workspace.as_os_str().to_os_string());
    if !config_overrides.is_empty() {
        args.push(OsString::from("--args"));
        for override_kv in config_overrides {
            args.push(OsString::from("-c"));
            args.push(OsString::from(override_kv));
        }
    }
    args
}

pub(crate) fn display_open_args(args: &[OsString]) -> String {
    args.iter()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::OpenTarget;
    use super::build_open_args;
    use super::display_open_args;
    use std::path::Path;
    use std::path::PathBuf;

    fn args_as_strings(args: Vec<std::ffi::OsString>) -> Vec<String> {
        args.into_iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn builds_open_args_for_bundle_id_with_config_overrides() {
        assert_eq!(
            args_as_strings(build_open_args(
                &OpenTarget::BundleId("com.openai.codex.nightly".to_string()),
                Path::new("/tmp/workspace"),
                &[
                    "hooks.on_event=[\"echo hi\"]".to_string(),
                    "features.experimental_foo=true".to_string(),
                    "features.experimental_bar=false".to_string(),
                ],
            )),
            vec![
                "-b",
                "com.openai.codex.nightly",
                "/tmp/workspace",
                "--args",
                "-c",
                "hooks.on_event=[\"echo hi\"]",
                "-c",
                "features.experimental_foo=true",
                "-c",
                "features.experimental_bar=false",
            ]
        );
    }

    #[test]
    fn builds_open_args_for_app_path_with_config_overrides() {
        assert_eq!(
            args_as_strings(build_open_args(
                &OpenTarget::AppPath(PathBuf::from("/Applications/Codex (Nightly).app")),
                Path::new("/tmp/workspace"),
                &["model=o3".to_string()],
            )),
            vec![
                "-a",
                "/Applications/Codex (Nightly).app",
                "/tmp/workspace",
                "--args",
                "-c",
                "model=o3",
            ]
        );
    }

    #[test]
    fn displays_open_args_for_error_messages() {
        let args = build_open_args(
            &OpenTarget::BundleId("com.openai.codex.nightly".to_string()),
            Path::new("/tmp/workspace"),
            &[],
        );

        assert_eq!(
            display_open_args(&args),
            "-b com.openai.codex.nightly /tmp/workspace"
        );
    }
}
