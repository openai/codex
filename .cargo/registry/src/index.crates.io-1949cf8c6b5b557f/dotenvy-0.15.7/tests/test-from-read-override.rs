mod common;

use dotenvy::*;
use std::{env, error::Error, fs::File, result::Result};

use crate::common::*;

#[test]
fn test_from_read_override() -> Result<(), Box<dyn Error>> {
    let dir = make_test_dotenv()?;

    from_read_override(File::open(".env")?)?;

    assert_eq!(env::var("TESTKEY")?, "test_val_overridden");
    assert_eq!(env::var("EXISTING")?, "from_file");

    env::set_current_dir(dir.path().parent().unwrap())?;
    dir.close()?;
    Ok(())
}
