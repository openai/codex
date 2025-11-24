mod common;

use dotenvy::*;

use std::{env, error::Error, result::Result};

use crate::common::*;

#[test]
fn test_default_location() -> Result<(), Box<dyn Error>> {
    let dir = make_test_dotenv()?;

    dotenv()?;

    assert_eq!(env::var("TESTKEY")?, "test_val");
    assert_eq!(env::var("EXISTING")?, "from_env");

    env::set_current_dir(dir.path().parent().unwrap())?;
    dir.close()?;
    Ok(())
}
