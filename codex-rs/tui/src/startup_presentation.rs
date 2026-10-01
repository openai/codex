//! Resolve first-frame screen ownership from the existing client configuration loader.
//!
//! The caller supplies its resolved profile overrides and configuration cwd, so this path does
//! not introduce separate profile or project precedence. Loading excludes cloud fetches and
//! returns the bootstrap result for reuse by startup. Final cloud or selected-project settings
//! may update screen policy before handing the terminal to the conversation.

use std::io;
use std::path::Path;

use codex_config::CloudConfigBundleLoader;
use codex_config::ConfigLoadOptions;
use codex_config::LoaderOverrides;
use codex_config::TomlValue;
use codex_utils_absolute_path::AbsolutePathBuf;

use crate::Cli;
use crate::keymap::RuntimeKeymap;
use crate::legacy_core::config::ConfigTomlLoadResult;
use crate::legacy_core::config::load_config_toml_with_layer_stack;
use crate::startup_draft::StartupScreen;

/// Screen policy and the already-loaded bootstrap configuration for this exact loading context.
pub(super) struct StartupPresentation {
    pub(super) bootstrap_config: ConfigTomlLoadResult,
    pub(super) config_cwd: Option<AbsolutePathBuf>,
    pub(super) screen: StartupScreen,
    pub(super) local_settings: crate::local_settings::LocalSettings,
}

/// Load client presentation settings without session setup or remote app-server connections.
///
/// `loader_overrides` must already contain the selected profile and remote login-policy choice.
/// `config_cwd` must come from the existing target/environment-aware cwd resolver. Reuse the
/// returned bootstrap only when these loading inputs still match the later startup context.
pub(super) async fn load(
    cli: &Cli,
    codex_home: &Path,
    loader_overrides: LoaderOverrides,
    cli_kv_overrides: Vec<(String, TomlValue)>,
    config_cwd: Option<AbsolutePathBuf>,
) -> io::Result<StartupPresentation> {
    let bootstrap_config = load_config_toml_with_layer_stack(
        codex_home,
        config_cwd.as_ref(),
        cli_kv_overrides,
        ConfigLoadOptions {
            loader_overrides,
            strict_config: cli.strict_config,
            cloud_config_bundle: CloudConfigBundleLoader::default(),
        },
    )
    .await?;
    let local_settings = crate::local_settings::LocalSettings::from_bootstrap(
        &bootstrap_config,
        AbsolutePathBuf::from_absolute_path(codex_home)?,
    )
    .map_err(io::Error::other)?;
    let settings = &local_settings.tui;
    // Terminal probing happens after configuration loads; startup finalizes this before painting.
    let use_alt_screen = crate::determine_alt_screen_mode(
        cli.no_alt_screen,
        settings.alternate_screen,
        /*terminal_app_over_ssh*/ false,
    );
    let transcript_mode = crate::transcript_mode::TranscriptMode::resolve(
        settings.fullscreen_transcript,
        use_alt_screen,
    );
    let status_line_enabled = settings
        .status_line
        .as_ref()
        .is_none_or(|items| !items.is_empty());
    let keymap = RuntimeKeymap::from_config(&settings.keymap).map_err(io::Error::other)?;
    let disable_paste_burst = settings.disable_paste_burst.unwrap_or(false);
    let welcome_motion = crate::motion::MotionMode::from_animations_enabled(
        settings.animations && settings.effects.welcome,
    );
    Ok(StartupPresentation {
        bootstrap_config,
        config_cwd,
        local_settings,
        screen: StartupScreen {
            use_alt_screen,
            transcript_mode,
            status_line_enabled,
            welcome_motion,
            keymap,
            disable_paste_burst,
        },
    })
}

#[cfg(test)]
#[path = "startup_presentation_tests.rs"]
mod tests;
