mod common;

use crate::common::*;
use dotenvy::*;
use std::{env, error::Error, result::Result};

#[test]
fn test_multiline() -> Result<(), Box<dyn Error>> {
    let value = "-----BEGIN PRIVATE KEY-----\n-----END PRIVATE KEY-----\\n\\\"QUOTED\\\"";
    let weak = "-----BEGIN PRIVATE KEY-----\n-----END PRIVATE KEY-----\n\"QUOTED\"";
    let dir = tempdir_with_dotenv(&format!(
        r#"
KEY=my\ cool\ value
KEY3="awesome \"stuff\"
more
on other
lines"
KEY4='hello '\''world'"
good ' \'morning"
WEAK="{}"
STRONG='{}'
"#,
        value, value
    ))?;

    dotenv()?;
    assert_eq!(var("KEY")?, r#"my cool value"#);
    assert_eq!(
        var("KEY3")?,
        r#"awesome "stuff"
more
on other
lines"#
    );
    assert_eq!(
        var("KEY4")?,
        r#"hello 'world
good ' 'morning"#
    );
    assert_eq!(var("WEAK")?, weak);
    assert_eq!(var("STRONG")?, value);

    env::set_current_dir(dir.path().parent().unwrap())?;
    dir.close()?;
    Ok(())
}
