use clap::Parser;
use codex_arg0::Arg0DispatchPaths;
use codex_arg0::arg0_dispatch_or_else;
use codex_tui::AppExitInfo;
use codex_tui::Cli;
use codex_tui::ExitReason;
use codex_tui::run_main;
use codex_tui::update_action::UpdateAction;
use codex_utils_cli::CliConfigOverrides;

#[derive(Parser, Debug)]
struct TopCli {
    #[clap(flatten)]
    config_overrides: CliConfigOverrides,

    #[clap(flatten)]
    inner: Cli,
}

fn into_app_server_cli(cli: Cli) -> codex_tui_app_server::Cli {
    codex_tui_app_server::Cli {
        prompt: cli.prompt,
        images: cli.images,
        resume_picker: cli.resume_picker,
        resume_last: cli.resume_last,
        resume_session_id: cli.resume_session_id,
        resume_show_all: cli.resume_show_all,
        resume_show_all_providers: cli.resume_show_all_providers,
        fork_picker: cli.fork_picker,
        fork_last: cli.fork_last,
        fork_session_id: cli.fork_session_id,
        fork_show_all: cli.fork_show_all,
        fork_show_all_providers: cli.fork_show_all_providers,
        model: cli.model,
        oss: cli.oss,
        oss_provider: cli.oss_provider,
        config_profile: cli.config_profile,
        sandbox_mode: cli.sandbox_mode,
        approval_policy: cli.approval_policy,
        full_auto: cli.full_auto,
        dangerously_bypass_approvals_and_sandbox: cli.dangerously_bypass_approvals_and_sandbox,
        cwd: cli.cwd,
        web_search: cli.web_search,
        add_dir: cli.add_dir,
        no_alt_screen: cli.no_alt_screen,
        config_overrides: cli.config_overrides,
    }
}

fn into_legacy_update_action(
    action: codex_tui_app_server::update_action::UpdateAction,
) -> UpdateAction {
    match action {
        codex_tui_app_server::update_action::UpdateAction::NpmGlobalLatest => {
            UpdateAction::NpmGlobalLatest
        }
        codex_tui_app_server::update_action::UpdateAction::BunGlobalLatest => {
            UpdateAction::BunGlobalLatest
        }
        codex_tui_app_server::update_action::UpdateAction::BrewUpgrade => UpdateAction::BrewUpgrade,
    }
}

fn into_legacy_exit_reason(reason: codex_tui_app_server::ExitReason) -> ExitReason {
    match reason {
        codex_tui_app_server::ExitReason::UserRequested => ExitReason::UserRequested,
        codex_tui_app_server::ExitReason::Fatal(message) => ExitReason::Fatal(message),
    }
}

fn into_legacy_exit_info(exit_info: codex_tui_app_server::AppExitInfo) -> AppExitInfo {
    AppExitInfo {
        token_usage: exit_info.token_usage,
        thread_id: exit_info.thread_id,
        thread_name: exit_info.thread_name,
        update_action: exit_info.update_action.map(into_legacy_update_action),
        exit_reason: into_legacy_exit_reason(exit_info.exit_reason),
    }
}

fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|arg0_paths: Arg0DispatchPaths| async move {
        let top_cli = TopCli::parse();
        let mut inner = top_cli.inner;
        inner
            .config_overrides
            .raw_overrides
            .splice(0..0, top_cli.config_overrides.raw_overrides);
        let use_app_server_tui = codex_tui::should_use_app_server_tui(&inner).await?;
        let exit_info = if use_app_server_tui {
            into_legacy_exit_info(
                codex_tui_app_server::run_main(
                    into_app_server_cli(inner),
                    arg0_paths,
                    codex_core::config_loader::LoaderOverrides::default(),
                    /*remote*/ None,
                )
                .await?,
            )
        } else {
            run_main(
                inner,
                arg0_paths,
                codex_core::config_loader::LoaderOverrides::default(),
            )
            .await?
        };
        let token_usage = exit_info.token_usage;
        if !token_usage.is_zero() {
            println!(
                "{}",
                codex_protocol::protocol::FinalOutput::from(token_usage),
            );
        }
        Ok(())
    })
}
