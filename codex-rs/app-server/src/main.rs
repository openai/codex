use codex_app_server::run_main;
use codex_arg0::arg0_dispatch_or_else;
use codex_common::CliConfigOverrides;
use codex_core::config_loader::LoaderOverrides;
use std::ffi::OsString;
use std::path::PathBuf;

const MANAGED_CONFIG_PATH_FLAG: &str = "--managed-config-path";
const MANAGED_CONFIG_PATH_FLAG_WITH_EQ: &str = "--managed-config-path=";

fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|codex_linux_sandbox_exe| async move {
        // This is intended to be exclusively used via tests: integration tests
        // need to point the server at a temporary managed config file without
        // writing to /etc.
        //
        // We intentionally do NOT allow this in release builds because the
        // managed config layer is meant to be enterprise-controlled.
        let managed_config_path =
            managed_config_path_from_args(std::env::args_os(), cfg!(debug_assertions))?;
        let loader_overrides = LoaderOverrides {
            managed_config_path,
            ..Default::default()
        };

        run_main(
            codex_linux_sandbox_exe,
            CliConfigOverrides::default(),
            loader_overrides,
        )
        .await?;
        Ok(())
    })
}

fn managed_config_path_from_args(
    args: impl IntoIterator<Item = OsString>,
    allow_override: bool,
) -> anyhow::Result<Option<PathBuf>> {
    let mut args = args.into_iter();
    // Skip argv[0].
    let _ = args.next();

    let mut managed_config_path = None;
    while let Some(arg) = args.next() {
        if arg == MANAGED_CONFIG_PATH_FLAG {
            if !allow_override {
                anyhow::bail!("{MANAGED_CONFIG_PATH_FLAG} is not supported in release builds");
            }
            let value = args.next().ok_or_else(|| {
                anyhow::format_err!("{MANAGED_CONFIG_PATH_FLAG} requires a path value")
            })?;
            managed_config_path = Some(PathBuf::from(value));
            continue;
        }

        if let Some(value) = arg
            .to_str()
            .and_then(|s| s.strip_prefix(MANAGED_CONFIG_PATH_FLAG_WITH_EQ))
        {
            if !allow_override {
                anyhow::bail!("{MANAGED_CONFIG_PATH_FLAG} is not supported in release builds");
            }
            managed_config_path = Some(PathBuf::from(value));
        }
    }

    Ok(managed_config_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_constants_are_in_sync() {
        // Unfortunately, we cannot derive one from the other using concat!,
        // so we hardcode both and test for consistency.
        assert_eq!(
            format!("{MANAGED_CONFIG_PATH_FLAG}="),
            MANAGED_CONFIG_PATH_FLAG_WITH_EQ
        );
    }
}
