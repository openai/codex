mod common;

use dotenvy::*;
use std::{env, error::Error, result::Result};

use crate::common::*;

#[test]
fn test_variable_substitutions() -> Result<(), Box<dyn Error>> {
    std::env::set_var("KEY", "value");
    std::env::set_var("KEY1", "value1");

    let substitutions_to_test = [
        "$ZZZ", "$KEY", "$KEY1", "${KEY}1", "$KEY_U", "${KEY_U}", "\\$KEY",
    ];

    let common_string = substitutions_to_test.join(">>");
    let dir = tempdir_with_dotenv(&format!(
        r#"
KEY1=new_value1
KEY_U=$KEY+valueU

SUBSTITUTION_FOR_STRONG_QUOTES='{}'
SUBSTITUTION_FOR_WEAK_QUOTES="{}"
SUBSTITUTION_WITHOUT_QUOTES={}
"#,
        common_string, common_string, common_string
    ))?;

    assert_eq!(var("KEY")?, "value");
    assert_eq!(var("KEY1")?, "value1");
    assert_eq!(var("KEY_U")?, "value+valueU");
    assert_eq!(var("SUBSTITUTION_FOR_STRONG_QUOTES")?, common_string);
    assert_eq!(
        var("SUBSTITUTION_FOR_WEAK_QUOTES")?,
        [
            "",
            "value",
            "value1",
            "value1",
            "value_U",
            "value+valueU",
            "$KEY"
        ]
        .join(">>")
    );
    assert_eq!(
        var("SUBSTITUTION_WITHOUT_QUOTES")?,
        [
            "",
            "value",
            "value1",
            "value1",
            "value_U",
            "value+valueU",
            "$KEY"
        ]
        .join(">>")
    );

    env::set_current_dir(dir.path().parent().unwrap())?;
    dir.close()?;
    Ok(())
}
