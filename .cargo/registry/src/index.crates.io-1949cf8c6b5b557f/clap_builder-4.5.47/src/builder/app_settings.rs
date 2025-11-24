#[allow(unused)]
use crate::Arg;
#[allow(unused)]
use crate::Command;

#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct AppFlags(u32);

impl AppFlags {
    pub(crate) fn set(&mut self, setting: AppSettings) {
        self.0 |= setting.bit();
    }

    pub(crate) fn unset(&mut self, setting: AppSettings) {
        self.0 &= !setting.bit();
    }

    pub(crate) fn is_set(&self, setting: AppSettings) -> bool {
        self.0 & setting.bit() != 0
    }

    pub(crate) fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

impl std::ops::BitOr for AppFlags {
    type Output = Self;

    fn bitor(mut self, rhs: Self) -> Self::Output {
        self.insert(rhs);
        self
    }
}

/// Application level settings, which affect how [`Command`] operates
///
/// <div class="warning">
///
/// **NOTE:** When these settings are used, they apply only to current command, and are *not*
/// propagated down or up through child or parent subcommands
///
/// </div>
///
/// [`Command`]: crate::Command
#[derive(Debug, PartialEq, Copy, Clone)]
#[repr(u8)]
pub(crate) enum AppSettings {
    IgnoreErrors,
    AllowHyphenValues,
    AllowNegativeNumbers,
    AllArgsOverrideSelf,
    AllowMissingPositional,
    TrailingVarArg,
    DontDelimitTrailingValues,
    InferLongArgs,
    InferSubcommands,
    SubcommandRequired,
    AllowExternalSubcommands,
    Multicall,
    SubcommandsNegateReqs,
    ArgsNegateSubcommands,
    SubcommandPrecedenceOverArg,
    FlattenHelp,
    ArgRequiredElseHelp,
    NextLineHelp,
    DisableColoredHelp,
    DisableHelpFlag,
    DisableHelpSubcommand,
    DisableVersionFlag,
    PropagateVersion,
    Hidden,
    HidePossibleValues,
    HelpExpected,
    NoBinaryName,
    #[allow(dead_code)]
    ColorAuto,
    ColorAlways,
    ColorNever,
    Built,
    BinNameBuilt,
}

impl AppSettings {
    fn bit(self) -> u32 {
        1 << (self as u8)
    }
}
