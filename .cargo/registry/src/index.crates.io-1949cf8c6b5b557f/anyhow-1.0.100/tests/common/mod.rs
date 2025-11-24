use anyhow::{bail, Result};
use std::io;

pub fn bail_literal() -> Result<()> {
    bail!("oh no!");
}

pub fn bail_fmt() -> Result<()> {
    bail!("{} {}!", "oh", "no");
}

pub fn bail_error() -> Result<()> {
    bail!(io::Error::new(io::ErrorKind::Other, "oh no!"));
}
