mod common;

use std::{collections::HashMap, env, error::Error, result::Result};

use dotenvy::*;

use crate::common::*;

#[test]
fn test_vars() -> Result<(), Box<dyn Error>> {
    let dir = make_test_dotenv()?;

    let vars: HashMap<String, String> = vars().collect();

    assert_eq!(vars["TESTKEY"], "test_val");

    env::set_current_dir(dir.path().parent().unwrap())?;
    dir.close()?;
    Ok(())
}
