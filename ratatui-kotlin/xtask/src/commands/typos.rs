use color_eyre::Result;
use duct::cmd;

use crate::{ExpressionExt, Run};

/// Check for typos in the project
#[derive(Clone, Debug, clap::Args)]
pub struct Typos {
    /// Fix typos
    #[arg(long)]
    pub fix: bool,
}

impl Run for Typos {
    fn run(self) -> Result<()> {
        if self.fix {
            cmd!("typos", "--write-changes").run_with_trace()?;
        } else {
            cmd!("typos").run_with_trace()?;
        }
        Ok(())
    }
}
