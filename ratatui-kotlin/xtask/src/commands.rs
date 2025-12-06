use std::fmt::Debug;

use backend::{Backend, TestBackend};
use clap::Subcommand;
use color_eyre::Result;
use coverage::Coverage;
use duct::cmd;
use rdme::Readme;

use self::backend::CheckBackend;
use self::check::Check;
use self::clippy::Clippy;
use self::docs::Docs;
use self::format::Format;
use self::typos::Typos;
use crate::{CROSSTERM_COMMON_FEATURES, ExpressionExt, Run, run_cargo};

mod backend;
mod check;
mod clippy;
mod coverage;
mod docs;
mod format;
mod rdme;
mod test_docs;
mod typos;

#[derive(Clone, Debug, Subcommand)]
pub enum Command {
    /// Run CI checks (lint, build, test)
    CI,

    /// Lint formatting, typos, clippy, and docs
    #[command(visible_alias = "l")]
    Lint,

    /// Build the project
    #[command(visible_alias = "b")]
    Build,

    #[command(visible_alias = "c")]
    Check(Check),

    /// Run tests
    #[command(visible_alias = "t")]
    Test,

    /// Check backend
    #[command(visible_alias = "cb")]
    CheckBackend(CheckBackend),

    /// Check if README.md is up-to-date (using cargo-rdme)
    #[command(visible_alias = "cr", alias = "rdme")]
    Readme(Readme),

    /// Generate code coverage report
    #[command(visible_alias = "cov")]
    Coverage(Coverage),

    /// Run clippy on the project
    #[command(visible_alias = "cl")]
    Clippy(Clippy),

    /// Check documentation for errors and warnings
    #[command(name = "docs", visible_alias = "d")]
    Docs(Docs),

    /// Check for formatting issues in the project
    #[command(visible_aliases = ["fmt", "f"])]
    Format(Format),

    /// Lint markdown files
    #[command(visible_alias = "md")]
    LintMarkdown,

    /// Check for typos in the project
    #[command(visible_alias = "ty")]
    Typos(Typos),

    /// Test backend
    #[command(visible_alias = "tb")]
    TestBackend(TestBackend),

    /// Run doc tests
    #[command(visible_alias = "td")]
    TestDocs,

    /// Run lib tests
    #[command(visible_alias = "tl")]
    TestLibs,

    /// Run cargo hack to test each feature in isolation
    #[command(visible_alias = "h")]
    Hack,
}

impl Run for Command {
    fn run(self) -> crate::Result<()> {
        match self {
            Command::CI => ci(),
            Command::Build => build(),
            Command::Check(command) => command.run(),
            Command::CheckBackend(command) => command.run(),
            Command::Readme(command) => command.run(),
            Command::Coverage(command) => command.run(),
            Command::Lint => lint(),
            Command::Clippy(command) => command.run(),
            Command::Docs(command) => command.run(),
            Command::Format(command) => command.run(),
            Command::Typos(command) => command.run(),
            Command::LintMarkdown => lint_markdown(),
            Command::Test => test(),
            Command::TestBackend(command) => command.run(),
            Command::TestDocs => test_docs::test_docs(),
            Command::TestLibs => test_libs(),
            Command::Hack => hack(),
        }
    }
}

/// Run CI checks (lint, build, test)
fn ci() -> Result<()> {
    lint()?;
    build()?;
    test()?;
    Ok(())
}

/// Build the project
fn build() -> Result<()> {
    run_cargo(vec!["build", "--all-targets", "--all-features"])
}

/// Lint formatting, typos, clippy, and docs (and a soft fail on markdown)
fn lint() -> Result<()> {
    Clippy { fix: false }.run()?;
    Docs { open: false }.run()?;
    Format { check: true }.run()?;
    Typos { fix: false }.run()?;
    if let Err(err) = lint_markdown() {
        tracing::warn!("known issue: markdownlint is currently noisy and can be ignored: {err}");
    }
    Ok(())
}

/// Lint markdown files using [markdownlint-cli2](https://github.com/DavidAnson/markdownlint-cli2)
fn lint_markdown() -> Result<()> {
    cmd!("markdownlint-cli2", "**/*.md", "!target").run_with_trace()?;
    Ok(())
}

/// Run tests for libs, backends, and docs
fn test() -> Result<()> {
    test_libs()?;
    for backend in [Backend::Crossterm, Backend::Termion, Backend::Termwiz] {
        TestBackend { backend }.run()?;
    }
    test_docs::test_docs()?; // run last because it's slow
    Ok(())
}

/// Run lib tests for the workspace's default packages
fn test_libs() -> Result<()> {
    run_cargo(vec![
        "hack",
        "--ignore-private", // exclude packages that are libraries
        "--exclude",
        "ratatui-crossterm",
        "test",
        "--lib",
        "--all-targets",
        "--all-features",
    ])?;
    let crossterm_feature = CROSSTERM_COMMON_FEATURES.join(",");
    for crossterm_version in crate::CROSSTERM_VERSION_FEATURES {
        let features = format!("{crossterm_feature},{crossterm_version}");
        run_cargo(vec![
            "test",
            "--package",
            "ratatui-crossterm",
            "--lib",
            "--no-default-features",
            "--features",
            features.as_str(),
        ])?;
    }
    Ok(())
}

/// Run cargo hack to test each feature in isolation
fn hack() -> Result<()> {
    run_cargo(vec![
        "hack",
        "test",
        "--lib",
        "--each-feature",
        "--workspace",
    ])
}
