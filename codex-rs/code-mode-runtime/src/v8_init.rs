use std::sync::OnceLock;

/// Controls whether V8 may generate executable code at runtime.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum V8JitMode {
    #[default]
    Enabled,
    Disabled,
}

struct V8Initialization {
    _platform: v8::SharedRef<v8::Platform>,
    jit_mode: V8JitMode,
}

static V8_INITIALIZATION: OnceLock<Result<V8Initialization, String>> = OnceLock::new();

/// Initializes the process-wide V8 platform with the requested JIT mode.
///
/// Call this before executing any code-mode cells when JIT must be disabled.
/// V8 cannot change JIT mode after initialization, so a later call requesting
/// a different mode returns an error. Code mode initializes V8 with JIT enabled
/// by default when this function has not been called explicitly.
pub fn initialize_v8(jit_mode: V8JitMode) -> Result<(), String> {
    match V8_INITIALIZATION.get_or_init(|| initialize_v8_with_mode(jit_mode)) {
        Ok(initialization) if initialization.jit_mode == jit_mode => Ok(()),
        Ok(initialization) => Err(format!(
            "V8 was already initialized with JIT {}",
            initialization.jit_mode.description()
        )),
        Err(error_text) => Err(error_text.clone()),
    }
}

pub(crate) fn ensure_v8_initialized() -> Result<(), String> {
    match V8_INITIALIZATION.get_or_init(|| initialize_v8_with_mode(V8JitMode::Enabled)) {
        Ok(_) => Ok(()),
        Err(error_text) => Err(error_text.clone()),
    }
}

fn initialize_v8_with_mode(jit_mode: V8JitMode) -> Result<V8Initialization, String> {
    v8::icu::set_common_data_77(deno_core_icudata::ICU_DATA)
        .map_err(|error_code| format!("failed to initialize ICU data: {error_code}"))?;
    // The pinned V8 can inline Array.prototype.sort with incompatible element kinds.
    // Disable the affected paths in TurboFan and the Maglev/Turbolev frontend until
    // our V8 artifacts include the upstream fix for mixed-element sorting:
    // https://github.com/v8/v8/commit/e0562d87ad9c17042b581582c99237d798572e67
    v8::V8::set_flags_from_string("--no-maglev --no-turbolev --no-turbo-inline-array-builtins");
    match jit_mode {
        V8JitMode::Enabled => {}
        V8JitMode::Disabled => v8::V8::set_flags_from_string("--jitless"),
    }
    let platform = v8::new_default_platform(0, false).make_shared();
    v8::V8::initialize_platform(platform.clone());
    v8::V8::initialize();
    Ok(V8Initialization {
        _platform: platform,
        jit_mode,
    })
}

impl V8JitMode {
    fn description(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
        }
    }
}
