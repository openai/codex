use anyhow::{ensure, Result};
use std::ops::{Deref, Not};

struct Bool(bool);

struct DerefBool(bool);

struct NotBool(bool);

impl Deref for DerefBool {
    type Target = bool;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Not for NotBool {
    type Output = bool;
    fn not(self) -> Self::Output {
        !self.0
    }
}

fn main() -> Result<()> {
    ensure!("...");

    let mut s = Bool(true);
    match &mut s {
        Bool(cond) => ensure!(cond),
    }

    let db = DerefBool(true);
    ensure!(db);
    ensure!(&db);

    let nb = NotBool(true);
    ensure!(nb);

    Ok(())
}
