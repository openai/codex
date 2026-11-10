use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let schema_root = match std::env::args_os().nth(1) {
        Some(path) => PathBuf::from(path),
        None => default_schema_root()?,
    };
    codex_hooks::write_schema_fixtures(&schema_root)
}

fn default_schema_root() -> anyhow::Result<PathBuf> {
    // Preserve Cargo's default without retaining its compile-time source path.
    if let Some(manifest_dir) = std::env::var_os("CARGO_MANIFEST_DIR") {
        return Ok(PathBuf::from(manifest_dir).join("schema"));
    }

    let cwd = std::env::current_dir()?;
    for directory in cwd.ancestors() {
        for workspace in [directory.to_path_buf(), directory.join("codex-rs")] {
            if workspace.join("hooks/Cargo.toml").is_file()
                && workspace.join("core/Cargo.toml").is_file()
            {
                return Ok(workspace.join("hooks/schema"));
            }
        }
    }

    anyhow::bail!("could not locate the Codex checkout; specify the schema output directory")
}
