mod common;

use crate::common::*;
use dotenvy::*;
use std::{env, error::Error, result::Result};

#[test]
fn test_from_path() -> Result<(), Box<dyn Error>> {
    let dir = make_test_dotenv()?;

    let mut path = env::current_dir()?;
    path.push(".env");

    from_path(&path)?;

    assert_eq!(env::var("TESTKEY")?, "test_val");
    assert_eq!(env::var("EXISTING")?, "from_env");

    env::set_current_dir(dir.path().parent().unwrap())?;
    dir.close()?;
    Ok(())
}
