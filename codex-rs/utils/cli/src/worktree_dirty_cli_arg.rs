use clap::ValueEnum;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum WorktreeDirtyCliArg {
    #[default]
    Fail,
    Ignore,
    CopyTracked,
    CopyAll,
    MoveTracked,
    MoveAll,
}
