mod common;

use crate::common::*;
use dotenvy::*;
use std::{env, error::Error, result::Result};

#[test]
fn test_ignore_bom() -> Result<(), Box<dyn Error>> {
    let bom = "\u{feff}";
    let dir = tempdir_with_dotenv(&format!("{}TESTKEY=test_val", bom))?;

    let mut path = env::current_dir()?;
    path.push(".env");

    from_path(&path)?;

    assert_eq!(env::var("TESTKEY")?, "test_val");

    env::set_current_dir(dir.path().parent().unwrap())?;
    dir.close()?;
    Ok(())
}
