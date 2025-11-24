use std::fs::File;
use std::io::prelude::*;
use std::{env, io};
use tempfile::{tempdir, TempDir};

pub fn tempdir_with_dotenv(dotenv_text: &str) -> io::Result<TempDir> {
    env::set_var("EXISTING", "from_env");
    let dir = tempdir()?;
    env::set_current_dir(dir.path())?;
    let dotenv_path = dir.path().join(".env");
    let mut dotenv_file = File::create(dotenv_path)?;
    dotenv_file.write_all(dotenv_text.as_bytes())?;
    dotenv_file.sync_all()?;
    Ok(dir)
}

#[allow(dead_code)]
pub fn make_test_dotenv() -> io::Result<TempDir> {
    tempdir_with_dotenv("TESTKEY=test_val\nTESTKEY=test_val_overridden\nEXISTING=from_file")
}
