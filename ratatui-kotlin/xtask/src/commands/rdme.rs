use color_eyre::Result;

use crate::{Run, run_cargo};

/// Check if README.md is up-to-date (using cargo-rdme)
#[derive(Clone, Debug, clap::Args)]
pub struct Readme {
    /// Check if README.md is up-to-date
    #[arg(long)]
    check: bool,
}

/// The projects that should have their README.md generated from the source code.
///
/// Notably, we removed `ratatui` from this list as we have a more specifically crafted README for
/// the main crate.
const PROJECTS: &[&str] = &[
    "ratatui-core",
    "ratatui-crossterm",
    "ratatui-macros",
    "ratatui-termion",
    "ratatui-termwiz",
    "ratatui-widgets",
];

impl Run for Readme {
    fn run(self) -> Result<()> {
        // This would be simpler perhaps with cargo-hack, however cargo-rdme does not support the
        // `--manifest-path` option that is required for this to work, so it's easiest to hard code
        // the package names here. See https://github.com/orium/cargo-rdme/issues/261
        for package in PROJECTS {
            let mut args = vec!["rdme", "--workspace-project", package];
            if self.check {
                args.push("--check");
            }
            run_cargo(args)?;
        }
        Ok(())
    }
}
