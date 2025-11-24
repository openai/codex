#![deny(unused_must_use)]

use anyhow::anyhow;

fn main() -> anyhow::Result<()> {
    if true {
        // meant to write bail!
        anyhow!("it failed");
    }
    Ok(())
}
