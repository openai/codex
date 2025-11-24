//! Lists contents of a directory.
//!
//! Run with:
//!
//! ```
//! cargo run --example ls .
//! ```

use std::{env, fs, io};

use blocking::Unblock;
use futures_lite::{future, prelude::*};

fn main() -> io::Result<()> {
    let path = env::args().nth(1).unwrap_or_else(|| ".".into());

    future::block_on(async {
        let mut dir = Unblock::new(fs::read_dir(path)?);

        while let Some(item) = dir.next().await {
            println!("{}", item?.file_name().to_string_lossy());
        }

        Ok(())
    })
}
