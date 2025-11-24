mod common;

use dotenvy::*;
use std::{env, error::Error, fs, result::Result};

use crate::common::*;

#[test]
fn test_child_dir() -> Result<(), Box<dyn Error>> {
    let dir = make_test_dotenv()?;

    fs::create_dir("child")?;

    env::set_current_dir("child")?;

    dotenv()?;
    assert_eq!(env::var("TESTKEY")?, "test_val");

    env::set_current_dir(dir.path().parent().unwrap())?;
    dir.close()?;
    Ok(())
}
