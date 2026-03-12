use std::env;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;

fn main() -> Result<()> {
    let mut args = env::args_os();
    let _program = args.next();
    let output_path = PathBuf::from(
        args.next()
            .ok_or_else(|| anyhow!("expected output path as first argument"))?,
    );
    let payload = args
        .next()
        .ok_or_else(|| anyhow!("expected payload as final argument"))?;

    if args.next().is_some() {
        bail!("expected payload as final argument");
    }

    let payload = payload.to_string_lossy();
    let temp_path = PathBuf::from(format!("{}.tmp", output_path.display()));
    let mut file = File::create(&temp_path)
        .with_context(|| format!("failed to create {}", temp_path.display()))?;
    file.write_all(payload.as_bytes())
        .with_context(|| format!("failed to write {}", temp_path.display()))?;
    file.sync_all()
        .with_context(|| format!("failed to sync {}", temp_path.display()))?;
    fs::rename(&temp_path, &output_path).with_context(|| {
        format!(
            "failed to move {} into {}",
            temp_path.display(),
            output_path.display()
        )
    })?;

    Ok(())
}
