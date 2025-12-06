use color_eyre::Result;

use crate::{Run, run_cargo_nightly};

/// Check documentation for errors and warnings
#[derive(Clone, Debug, clap::Args)]
pub struct Docs {
    /// Open the documentation in the browser
    #[arg(long)]
    pub open: bool,
}

impl Run for Docs {
    fn run(self) -> Result<()> {
        // cargo +nightly hack --all --ignore-private docs-rs
        let mut args = vec!["hack", "--all", "--ignore-private", "docs-rs"];
        if self.open {
            args.push("--open");
        }
        run_cargo_nightly(args)
    }
}
