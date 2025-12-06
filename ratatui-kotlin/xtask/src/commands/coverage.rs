use color_eyre::Result;

use crate::{Run, run_cargo};

/// Generate code coverage report
#[derive(Clone, Debug, clap::Args)]
pub struct Coverage {
    /// Only generate coverage for unit tests
    #[arg(long)]
    pub lib: bool,
}

impl Run for Coverage {
    fn run(self) -> Result<()> {
        let mut args = vec![
            "llvm-cov",
            "--workspace",
            "--exclude",
            "ratatui-crossterm",
            "--all-features",
            "--no-report",
        ];
        if self.lib {
            args.push("--lib");
        }
        run_cargo(args)?;

        let mut args = vec!["llvm-cov", "--package", "ratatui-crossterm", "--no-report"];
        if self.lib {
            args.push("--lib");
        }
        run_cargo(args)?;

        run_cargo(vec![
            "llvm-cov",
            "report",
            "--lcov",
            "--output-path",
            "target/lcov.info",
        ])
    }
}
