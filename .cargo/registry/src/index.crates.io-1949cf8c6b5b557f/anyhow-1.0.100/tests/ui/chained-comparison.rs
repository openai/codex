use anyhow::{ensure, Result};

fn main() -> Result<()> {
    // `ensure!` must not partition this into `(false) == (false == true)`
    // because Rust doesn't ordinarily allow this form of expression.
    ensure!(false == false == true);
    Ok(())
}
