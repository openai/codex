# `codex-core` config loader

This module is the canonical place to **load and describe Codex configuration layers** (user config, CLI/session overrides, managed config, and MDM-managed preferences) and to produce:

- An **effective merged** TOML config.
- **Per-key origins** metadata (which layer “wins” for a given key).
- **Per-layer versions** (stable fingerprints) used for optimistic concurrency / conflict detection.

## Public surface

Exported from `codex_core::config_loader`:

- `load_config_layers_state(codex_home, cwd_opt, cli_overrides, overrides) -> ConfigLayerStack`
- `ConfigLayerStack`
  - `effective_config() -> toml::Value`
  - `origins() -> HashMap<String, ConfigLayerMetadata>`
  - `layers_high_to_low() -> Vec<ConfigLayer>`
  - `with_user_config(user_config) -> ConfigLayerStack`
- `ConfigLayerEntry` (one layer’s `{name, config, version}`; `name` carries source metadata)
- `LoaderOverrides` (test/override hooks for managed config sources)
- `merge_toml_values(base, overlay)` (public helper used elsewhere)

## Layering model

Precedence is **top overrides bottom**:

1. **MDM** managed preferences (macOS only)
2. **System** managed config (e.g. `managed_config.toml`)
3. **Session flags** (CLI overrides, applied as dotted-path TOML writes)
4. **User** config (`config.toml`)

This is what `ConfigLayerStack::effective_config()` implements.

## Typical usage

Most callers want the effective config plus metadata:

```rust
use codex_core::config_loader::{load_config_layers_state, LoaderOverrides};
use codex_utils_absolute_path::AbsolutePathBuf;
use toml::Value as TomlValue;

let cli_overrides: Vec<(String, TomlValue)> = Vec::new();
let cwd = AbsolutePathBuf::current_dir()?;
let layers = load_config_layers_state(
    &codex_home,
    Some(cwd),
    &cli_overrides,
    LoaderOverrides::default(),
).await?;

let effective = layers.effective_config();
let origins = layers.origins();
let layers_for_ui = layers.layers_high_to_low();
```

## Internal layout

Implementation is split by concern:

- `state.rs`: public types (`ConfigLayerEntry`, `ConfigLayerStack`) + merge/origins convenience methods.
- `layer_io.rs`: reading `config.toml`, managed config, and managed preferences inputs.
- `overrides.rs`: CLI dotted-path overrides → TOML “session flags” layer.
- `merge.rs`: recursive TOML merge.
- `fingerprint.rs`: stable per-layer hashing and per-key origins traversal.
- `macos.rs`: managed preferences integration (macOS only).
