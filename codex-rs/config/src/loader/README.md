# `codex-config` loader

This module is the canonical place to **load and describe Codex configuration layers** (user config, CLI/session overrides, cloud-managed config, managed config, and MDM-managed preferences) and to produce:

- An **effective merged** TOML config.
- **Per-key origins** metadata (which layer “wins” for a given key).
- **Per-layer versions** (stable fingerprints) used for optimistic concurrency / conflict detection.

## Public surface

Exported from `codex_config::loader`:

- `load_config_layers_state(fs, codex_home, cwd_opt, cli_overrides, options, thread_config_loader) -> ConfigLayerStack`
- `ConfigLayerStack`
  - `effective_config() -> toml::Value`
  - `origins() -> HashMap<String, ConfigLayerMetadata>`
  - `layers_high_to_low() -> Vec<ConfigLayer>`
  - `with_user_config(user_config) -> ConfigLayerStack`
- `ConfigLayerEntry` (one layer’s `{name, config, version, disabled_reason}`; `name` carries source metadata)
- `ConfigLoadOptions` (user-facing load behavior such as strict config validation)
- `LoaderOverrides` (test/override hooks for managed config sources)
- `merge_toml_values(base, overlay)` (public helper used elsewhere)

## Layering model

Precedence is **top overrides bottom**:

1. `LegacyManagedConfigTomlFromMdm` (MDM-delivered `managed_config.toml`, while it is being phased out)
2. `LegacyManagedConfigTomlFromFile` (`managed_config.toml`, while it is being phased out)
3. `SessionFlags` (CLI overrides, applied as dotted-path TOML writes)
4. `Project` config (`.codex/config.toml`)
5. `User` profile config, when present
6. `User` config (`config.toml`)
7. `EnterpriseManaged` cloud-managed config bundle layers
8. `System` config (`/etc/codex/config.toml` or the Windows system config path)

`ConfigLayerStack` stores layers in the opposite order internally: lowest
precedence first, highest precedence last, so later layers override earlier
layers when folded. Thread config entries supplied by `thread_config_loader` are
inserted according to their translated `ConfigLayerSource` precedence.

Layers with a `disabled_reason` are still surfaced for UI, but are ignored when
computing the effective config and origins metadata. This is what
`ConfigLayerStack::effective_config()` implements.

## Typical usage

Most callers want the effective config plus metadata:

```rust
use codex_config::LoaderOverrides;
use codex_config::NoopThreadConfigLoader;
use codex_config::loader::load_config_layers_state;
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
    &NoopThreadConfigLoader,
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
