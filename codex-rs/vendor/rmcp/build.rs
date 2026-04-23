// Install git hooks on build
fn main() {
    // Only run in the workspace root (not when building as a dependency)
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let workspace_root = std::path::Path::new(&manifest_dir)
        .parent()
        .and_then(|p| p.parent());

    if let Some(root) = workspace_root {
        let githooks_dir = root.join(".githooks");
        let git_dir = root.join(".git");

        // Only configure if we're in the actual workspace (not a dependency)
        // and git directory exists
        if githooks_dir.exists() && git_dir.exists() {
            // Configure git to use our hooks directory
            let _ = std::process::Command::new("git")
                .args(["config", "core.hooksPath", ".githooks"])
                .current_dir(root)
                .output();
        }
    }
}
