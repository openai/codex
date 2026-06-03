/// Configuration for one durable code-mode session.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CodeModeSessionConfig {
    /// Controls whether cells can persist JSON values for later cells.
    pub store_load: StoreLoadMode,
}

/// Controls whether the `store` and `load` globals are available in a session.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StoreLoadMode {
    /// Expose `store` and `load` and retain their JSON values between cells.
    #[default]
    Enabled,
    /// Do not expose `store` or `load`, and do not retain JSON values.
    Disabled,
}
