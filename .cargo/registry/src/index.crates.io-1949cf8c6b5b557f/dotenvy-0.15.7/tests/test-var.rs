mod common;

use dotenvy::*;
use std::{env, error::Error, result::Result};

use crate::common::*;

#[test]
fn test_var() -> Result<(), Box<dyn Error>> {
    let dir = make_test_dotenv()?;

    assert_eq!(var("TESTKEY")?, "test_val");

    env::set_current_dir(dir.path().parent().unwrap())?;
    dir.close()?;
    Ok(())
}
