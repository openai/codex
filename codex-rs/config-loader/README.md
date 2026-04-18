# `codex-config-loader`

This crate loads and describes Codex configuration layers (user config,
CLI/session overrides, managed config, requirements, and MDM-managed
preferences) and produces:

- An effective merged TOML config.
- Per-key origins metadata.
- Per-layer versions used for optimistic concurrency and conflict detection.

The canonical implementation lives here instead of `codex-core` so callers that
only need config loading do not force the loader implementation into core. The
`codex_core::config_loader` module is a compatibility re-export for existing
callers.

## Public Surface

Exported from `codex_config_loader` and re-exported from
`codex_core::config_loader`:

- `load_config_layers_state(fs, codex_home, cwd_opt, cli_overrides, overrides, cloud_requirements) -> ConfigLayerStack`
- `ConfigLayerStack`
  - `effective_config() -> toml::Value`
  - `origins() -> HashMap<String, ConfigLayerMetadata>`
  - `layers_high_to_low() -> Vec<ConfigLayer>`
  - `with_user_config(user_config) -> ConfigLayerStack`
- `ConfigLayerEntry` for one layer's source, config, version, and optional disabled reason.
- `LoaderOverrides` for test and override hooks for managed config sources.
- `merge_toml_values(base, overlay)` for recursive TOML merge.

## Layering Model

Precedence is top overrides bottom:

1. MDM managed preferences on macOS.
2. Legacy managed config.
3. Session flags.
4. Project config layers.
5. User config.
6. System config.

Layers with a `disabled_reason` are still surfaced for UI, but are ignored when
computing the effective config and origins metadata.

## Typical Usage

```rust
use codex_config_loader::{
    CloudRequirementsLoader, LoaderOverrides, load_config_layers_state,
};
use codex_exec_server::LOCAL_FS;
use codex_utils_absolute_path::AbsolutePathBuf;
use toml::Value as TomlValue;

let cli_overrides: Vec<(String, TomlValue)> = Vec::new();
let cwd = AbsolutePathBuf::current_dir()?;
let layers = load_config_layers_state(
    LOCAL_FS.as_ref(),
    &codex_home,
    Some(cwd),
    &cli_overrides,
    LoaderOverrides::default(),
    CloudRequirementsLoader::default(),
).await?;

let effective = layers.effective_config();
let origins = layers.origins();
let layers_for_ui = layers.layers_high_to_low();
```

## Internal Layout

- `src/lib.rs`: layer assembly, trust decisions, project config discovery, and path resolution.
- `src/layer_io.rs`: config and managed config reads.
- `src/macos.rs`: managed preferences integration on macOS.
- `codex-config`: owns layer state, requirements, merging, overrides, diagnostics, fingerprints, and config TOML types.
