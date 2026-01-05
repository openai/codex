use codex_app_server::run_main;
use codex_arg0::arg0_dispatch_or_else;
use codex_common::CliConfigOverrides;
use codex_core::config_loader::LoaderOverrides;
use std::ffi::OsString;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|codex_linux_sandbox_exe| async move {
        let managed_config_path = managed_config_path_from_args(std::env::args_os())?;
        let mut loader_overrides = LoaderOverrides::default();
        loader_overrides.managed_config_path = managed_config_path;

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
) -> anyhow::Result<Option<PathBuf>> {
    let mut args = args.into_iter();
    // Skip argv[0].
    let _ = args.next();

    let mut managed_config_path = None;
    while let Some(arg) = args.next() {
        if arg == "--managed-config-path" {
            let value = args.next().ok_or_else(|| {
                anyhow::format_err!("--managed-config-path requires a path value")
            })?;
            managed_config_path = Some(PathBuf::from(value));
            continue;
        }

        if let Some(value) = arg
            .to_str()
            .and_then(|s| s.strip_prefix("--managed-config-path="))
        {
            managed_config_path = Some(PathBuf::from(value));
        }
    }

    Ok(managed_config_path)
}
