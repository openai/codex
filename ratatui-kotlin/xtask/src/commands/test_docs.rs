use crate::{CROSSTERM_COMMON_FEATURES, CROSSTERM_VERSION_FEATURES, Result, run_cargo};

/// Run doc tests for the workspace's default packages
pub fn test_docs() -> Result<()> {
    run_cargo(vec![
        "hack",
        "--workspace",
        "--ignore-private", // exclude packages that are libraries
        "--exclude",
        "ratatui-crossterm",
        "test",
        "--doc",
        "--all-features",
    ])?;
    let common_features = CROSSTERM_COMMON_FEATURES.join(",");
    for crossterm_version in CROSSTERM_VERSION_FEATURES {
        let features = format!("{common_features},{crossterm_version}");
        run_cargo(vec![
            "test",
            "--package",
            "ratatui-crossterm",
            "--doc",
            "--no-default-features",
            "--features",
            features.as_str(),
        ])?;
    }
    Ok(())
}
