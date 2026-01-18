//! Standard type for the `--lsp` CLI option.

use clap::ValueEnum;

#[derive(Clone, Copy, Debug, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum LspModeCliArg {
    /// Disable language server integration.
    Off,
    /// Enable language server integration when a matching project is detected.
    Auto,
    /// Force language server integration on supported file types.
    On,
}

impl LspModeCliArg {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Auto => "auto",
            Self::On => "on",
        }
    }
}
