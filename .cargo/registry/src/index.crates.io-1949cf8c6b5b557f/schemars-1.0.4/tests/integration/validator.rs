use crate::prelude::*;
use regex::Regex;
use validator::Validate;

const ONE: f32 = 1.0;
const HUNDRED: f32 = 10.0;

// In real code, this would use something like a LazyLock
fn hello_regex() -> Regex {
    Regex::new(r"^[Hh]ello").unwrap()
}

#[derive(JsonSchema, Deserialize, Serialize, Validate)]
pub struct ValidateAttrStruct {
    #[validate(range(min = 1.0, max = 100.0))]
    min_max: f32,
    #[validate(range(min = ONE, max = HUNDRED))]
    min_max2: f32,
    #[validate(regex(path = hello_regex()))]
    regex_str: String,
    #[validate(contains(pattern = "substring..."))]
    contains_str: String,
    #[validate(email)]
    email_address: String,
    #[validate(url)]
    homepage: String,
    #[validate(length(min = 1, max = 100))]
    non_empty_str: String,
    #[validate(length(equal = 2))]
    pair: Vec<String>,
    #[validate(required)]
    required_option: Option<bool>,
    #[validate(required, nested)]
    #[serde(flatten)]
    required_flattened: Option<ValidateAttrInner>,
}

#[derive(JsonSchema, Deserialize, Serialize, Validate)]
pub struct ValidateAttrInner {
    #[validate(range(min = -100, max = 100))]
    x: i32,
}

impl Default for ValidateAttrStruct {
    fn default() -> Self {
        Self {
            min_max: 1.0,
            min_max2: 1.0,
            regex_str: "Hello world".to_owned(),
            contains_str: "Contains substring...".to_owned(),
            email_address: "test@test.test".to_owned(),
            homepage: "http://test.test".to_owned(),
            non_empty_str: "test".to_owned(),
            pair: vec!["a".to_owned(), "b".to_owned()],
            required_option: Some(true),
            required_flattened: Some(ValidateAttrInner { x: 0 }),
        }
    }
}

impl ValidateAttrStruct {
    pub fn invalid_values() -> impl IntoIterator<Item = Self> {
        static MUTATORS: &[fn(&mut ValidateAttrStruct)] = &[
            |v| v.min_max = 0.9,
            |v| v.min_max = 100.1,
            |v| v.min_max2 = 0.9,
            |v| v.min_max2 = 100.1,
            |v| v.regex_str = "fail".to_owned(),
            |v| v.contains_str = "fail".to_owned(),
            |v| v.email_address = "fail".to_owned(),
            |v| v.homepage = "fail".to_owned(),
            |v| v.non_empty_str = String::new(),
            |v| v.pair = Vec::new(),
            |v| v.pair = vec!["a".to_owned(), "b".to_owned(), "c".to_owned()],
            |v| v.required_option = None,
            |v| v.required_flattened = None,
            |v| v.required_flattened = Some(ValidateAttrInner { x: -101 }),
            |v| v.required_flattened = Some(ValidateAttrInner { x: 101 }),
        ];
        MUTATORS.iter().map(|f| {
            let mut result = ValidateAttrStruct::default();
            f(&mut result);
            result
        })
    }
}

#[test]
fn validate_attrs() {
    test!(ValidateAttrStruct)
        .with_validator(|v| v.validate().is_ok())
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_rejects_invalid(ValidateAttrStruct::invalid_values())
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[allow(dead_code)]
#[derive(JsonSchema)]
#[schemars(rename = "ValidateAttrStruct")]
pub struct SchemarsAttrStruct {
    #[schemars(range(min = 1.0, max = 100.0))]
    min_max: f32,
    #[schemars(range(min = ONE, max = HUNDRED))]
    min_max2: f32,
    #[schemars(regex(pattern = hello_regex()))]
    regex_str: String,
    #[schemars(contains(pattern = "substring..."))]
    contains_str: String,
    #[schemars(email)]
    email_address: String,
    #[schemars(url)]
    homepage: String,
    #[schemars(length(min = 1, max = 100))]
    non_empty_str: String,
    #[schemars(length(equal = 2))]
    pair: Vec<String>,
    #[schemars(required)]
    required_option: Option<bool>,
    #[schemars(required)]
    #[serde(flatten)]
    required_flattened: Option<SchemarsAttrInner>,
}

#[allow(dead_code)]
#[derive(JsonSchema)]
pub struct SchemarsAttrInner {
    #[schemars(range(min = -100, max = 100))]
    x: i32,
}

#[test]
fn schemars_attrs() {
    test!(SchemarsAttrStruct).assert_identical::<ValidateAttrStruct>();
}
