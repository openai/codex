use color_eyre::Result;
use duct::cmd;

use crate::{ExpressionExt, Run, run_cargo_nightly};

/// Check for formatting issues in the project
#[derive(Clone, Debug, clap::Args)]
pub struct Format {
    /// Check formatting issues
    #[arg(long)]
    pub check: bool,
}

impl Run for Format {
    fn run(self) -> Result<()> {
        self.run_rustfmt()?;
        self.run_taplo()?;
        Ok(())
    }
}

impl Format {
    fn run_rustfmt(&self) -> Result<(), color_eyre::eyre::Error> {
        let mut args = vec!["fmt", "--all"];
        if self.check {
            args.push("--check");
        }
        run_cargo_nightly(args)?;
        Ok(())
    }

    fn run_taplo(&self) -> Result<(), color_eyre::eyre::Error> {
        let mut args = vec!["format", "--colors", "always"];
        if self.check {
            args.push("--check");
        }
        cmd("taplo", args).run_with_trace()?;
        Ok(())
    }
}
