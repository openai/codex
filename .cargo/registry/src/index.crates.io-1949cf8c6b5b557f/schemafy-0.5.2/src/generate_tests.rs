//! Generate test cases from the JSON Schema Test Suite.

use inflector::Inflector;
use serde::{Deserialize, Serialize};
use std::{error::Error, ffi::OsStr, fs, path::PathBuf};

// Each test has a description, schema, and a list of tests. Each of
// those tests has a description, some data, and a `valid` field which
// indicates whether that data should validate against the schema.
schemafy::schemafy!(
    root: TestSchema
    "tests/JSON-Schema-Test-Suite/test-schema.json"
);

fn main() -> Result<(), Box<dyn Error>> {
    let test_suite_dir = PathBuf::from("tests/test_suite");
    let schemas_dir = test_suite_dir.join("schemas");
    if test_suite_dir.exists() {
        fs::remove_dir_all("tests/test_suite")?;
    }
    fs::create_dir(&test_suite_dir)?;
    fs::create_dir(&schemas_dir)?;

    let mut test_modules = vec![];
    let mut blacklist_count = 0;

    for path in fs::read_dir("tests/JSON-Schema-Test-Suite/tests/draft4")?
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension() == Some(OsStr::new("json")))
    {
        let buffer = fs::read_to_string(&path)?;
        let test_schema: TestSchema = serde_json::from_str(&buffer)?;
        println!("{} ==> {} tests", path.display(), test_schema.len());

        let module_name = path.file_stem().unwrap().to_str().unwrap().to_snake_case();

        let mut test_file: String = format!(
            r#"//! Automatically generated from {}
"#,
            path.display()
        );

        for (i, test_group) in test_schema.iter().enumerate() {
            if is_blacklisted(&module_name, i) {
                blacklist_count += 1;
                println!(" !! skipping test group: {}", test_group.description);
                continue;
            }

            let schema_name = format!("{}_{}.json", module_name, i);
            let schema = serde_json::to_string(&test_group.schema)?;
            fs::write(schemas_dir.join(&schema_name), schema)?;

            test_file.push_str(&format!(
                r#"
mod _{}_{} {{
    #[allow(unused_imports)]
    use serde::{{Deserialize, Serialize}};

    schemafy::schemafy!(root: Schema "tests/test_suite/schemas/{}");
"#,
                i,
                test_group.description.to_snake_case(),
                schema_name
            ));
            for test in &test_group.tests {
                let test_name = {
                    // Prefix the name with an underscore if it starts
                    // with a number.
                    let root = test.description.to_snake_case();
                    let prefix = if root.chars().next().unwrap().is_numeric() {
                        "_"
                    } else {
                        ""
                    };
                    format!("{}{}", prefix, root)
                };

                // For the positive test cases, unwrapping the result
                // gives better error messages than simply asserting
                // on .is_ok(). For the negative test cases, a simple
                // assert is the best we can do.
                let assertion = if test.valid {
                    "let _: Schema = serde_json::from_str(&data).unwrap();"
                } else {
                    "assert!(serde_json::from_str::<Schema>(&data).is_err());"
                };

                test_file.push_str(&format!(
                    r##"
    #[test]
    fn r#{}() {{
        let data = r#"{}"#;
        {}
    }}
"##,
                    test_name, test.data, assertion
                ));
            }
            test_file.push_str("}\n");
        }

        fs::write(
            test_suite_dir.join(format!("{}.rs", module_name)),
            test_file,
        )?;
        test_modules.push(module_name);
    }

    // Generate a root module that declares all the above files.
    let mut tests: String = r#"//! Automatically generated
"#
    .into();
    for module in &test_modules {
        tests.push_str(&format!("mod r#{};\n", module));
    }
    fs::write(test_suite_dir.join("mod.rs"), tests)?;

    if blacklist_count > 0 {
        println!("\nSkipped {} test schemas\n", blacklist_count);
    }

    Ok(())
}

/// To allow for gradual progress, this function determines whether a
/// test should be skipped.
fn is_blacklisted(test_group: &str, index: usize) -> bool {
    let blacklisted_indices: &[usize] = match test_group {
        "additional_items" => &[0, 2],
        "additional_properties" => &[0, 1, 2, 3, 5],
        "all_of" => &[1, 2, 5, 6],
        "any_of" => &[0, 1, 2, 4],
        "definitions" => &[0, 1],
        "dependencies" => &[0, 1, 2, 3],
        "enum" => &[0, 1, 3, 4, 5, 6, 7],
        "items" => &[0, 1, 2],
        "max_items" => &[0],
        "max_length" => &[0],
        "max_properties" => &[0],
        "maximum" => &[0, 1, 2],
        "min_items" => &[0],
        "min_length" => &[0],
        "min_properties" => &[0],
        "minimum" => &[0, 1, 2, 3],
        "multiple_of" => &[0, 1, 2],
        "not" => &[0, 1, 2, 3],
        "one_of" => &[0, 1, 2, 3, 4],
        "pattern" => &[0],
        "pattern_properties" => &[0, 1, 2],
        "properties" => &[0, 1, 2],
        "ref" => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
        "ref_remote" => &[0, 1, 2, 3, 4, 5, 6],
        "required" => &[0, 2],
        "type" => &[6, 7, 9, 10],
        "unique_items" => &[0, 1, 2],

        _ => &[],
    };

    blacklisted_indices.contains(&index)
}
