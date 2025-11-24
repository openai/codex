#![allow(clippy::incompatible_msrv)]
use crate::prelude::*;
use garde::Validate;

const ONE: usize = 1;
const HUNDRED: usize = 10;

#[derive(JsonSchema, Deserialize, Serialize, Validate)]
pub struct GardeAttrStruct {
    #[garde(range(min = 1.0, max = 100.0))]
    min_max: f32,
    #[garde(range(min = ONE as f32, max = HUNDRED as f32))]
    min_max2: f32,
    #[garde(pattern(r"^[Hh]ello"))]
    regex_str: String,
    #[garde(contains(concat!("sub","string...")))]
    contains_str: String,
    #[garde(email)]
    email_address: String,
    #[garde(url)]
    homepage: String,
    #[garde(length(min = ONE, max = HUNDRED))]
    non_empty_str: String,
    #[garde(length(equal = 2), inner(length(min = 1)))]
    pair: Vec<String>,
    #[garde(required)]
    required_option: Option<bool>,
    #[garde(required, dive)]
    #[serde(flatten)]
    required_flattened: Option<GardeAttrInner>,
}

#[derive(JsonSchema, Deserialize, Serialize, Validate)]
pub struct GardeAttrInner {
    #[garde(range(min = -100, max = 100))]
    x: i32,
}

impl Default for GardeAttrStruct {
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
            required_flattened: Some(GardeAttrInner { x: 0 }),
        }
    }
}

impl GardeAttrStruct {
    pub fn invalid_values() -> impl IntoIterator<Item = Self> {
        static MUTATORS: &[fn(&mut GardeAttrStruct)] = &[
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
            |v| v.pair = vec!["".to_owned(), "b".to_owned()],
            |v| v.required_option = None,
            |v| v.required_flattened = None,
            |v| v.required_flattened = Some(GardeAttrInner { x: -101 }),
            |v| v.required_flattened = Some(GardeAttrInner { x: 101 }),
        ];
        MUTATORS.iter().map(|f| {
            let mut result = GardeAttrStruct::default();
            f(&mut result);
            result
        })
    }
}

#[test]
fn garde_attrs() {
    test!(GardeAttrStruct)
        .with_validator(|v| v.validate().is_ok())
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_rejects_invalid(GardeAttrStruct::invalid_values())
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[allow(dead_code)]
#[derive(JsonSchema)]
#[schemars(rename = "GardeAttrStruct")]
pub struct SchemarsAttrStruct {
    #[schemars(range(min = 1.0, max = 100.0))]
    min_max: f32,
    #[schemars(range(min = ONE as f32, max = HUNDRED as f32))]
    min_max2: f32,
    #[schemars(pattern(r"^[Hh]ello"))]
    regex_str: String,
    #[schemars(contains(concat!("sub","string...")))]
    contains_str: String,
    #[schemars(email)]
    email_address: String,
    #[schemars(url)]
    homepage: String,
    #[schemars(length(min = ONE, max = HUNDRED))]
    non_empty_str: String,
    #[schemars(length(equal = 2), inner(length(min = 1)))]
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
    test!(SchemarsAttrStruct).assert_identical::<GardeAttrStruct>();
}

#[derive(JsonSchema, Deserialize, Serialize, Validate)]
pub struct GardeAttrTuple(
    #[garde(range(max = 10))] u8,
    #[garde(required)] Option<bool>,
);

#[test]
fn garde_attrs_tuple() {
    test!(GardeAttrTuple)
        .with_validator(|v| v.validate().is_ok())
        .assert_snapshot()
        .assert_allows_ser_roundtrip([GardeAttrTuple(10, Some(false))])
        .assert_rejects_invalid([GardeAttrTuple(11, Some(false)), GardeAttrTuple(10, None)])
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[derive(JsonSchema, Deserialize, Serialize, Validate)]
pub struct GardeAttrNewType(#[garde(range(max = 10))] u8);

#[test]
fn garde_attrs_newtype() {
    test!(GardeAttrNewType)
        .with_validator(|v| v.validate().is_ok())
        .assert_snapshot()
        .assert_allows_ser_roundtrip([GardeAttrNewType(10)])
        .assert_rejects_invalid([GardeAttrNewType(11)])
        .assert_matches_de_roundtrip(arbitrary_values());
}
