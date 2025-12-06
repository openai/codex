use color_eyre::Result;

use crate::{CROSSTERM_VERSION_FEATURES, Run, run_cargo};

/// Run clippy on the project
#[derive(Clone, Debug, clap::Args)]
pub struct Clippy {
    /// Fix clippy warnings
    #[arg(long)]
    pub fix: bool,
}

impl Run for Clippy {
    fn run(self) -> Result<()> {
        // Arguments that go before feature flags (e.g., "clippy", "--fix")
        let mut clippy_command = vec!["clippy"];
        if self.fix {
            clippy_command.push("--fix");
        }

        // Define common non-version-specific features for ratatui-crossterm. These will be enabled
        // for both crossterm 0.28 and 0.29 runs. `underline-color`` is part of default, but with
        // `--no-default-features``, we must add it.
        let common_features = [
            "serde",
            "underline-color",
            "scrolling-regions",
            "unstable",
            "unstable-backend-writer",
        ]
        .join(",");

        let clippy_options = ["--", "-D", "warnings"];

        // Run Clippy on `ratatui-crossterm` with `crossterm_0_28` and `crossterm_0_29`
        for crossterm_feature in CROSSTERM_VERSION_FEATURES {
            let mut command = clippy_command.clone();
            let features = format!("{common_features},{crossterm_feature}");
            // Note that adding --tests or --benches causes clippy to pick up the default features.
            // I'm not sure why this is the case (JM 2025-05-10).
            command.extend(vec![
                "--package",
                "ratatui-crossterm",
                "--no-default-features",
                "--features",
                features.as_str(),
            ]);
            command.extend(clippy_options);
            run_cargo(command)?;
        }

        // Run Clippy on all other workspace packages with --all-features
        let mut command = clippy_command.clone();
        command.extend(vec![
            "--all-features",
            "--all-targets",
            "--tests",
            "--benches",
            "--workspace",
            "--exclude",
            "ratatui-crossterm",
        ]);
        command.extend(clippy_options);
        run_cargo(command)?;

        Ok(())
    }
}
