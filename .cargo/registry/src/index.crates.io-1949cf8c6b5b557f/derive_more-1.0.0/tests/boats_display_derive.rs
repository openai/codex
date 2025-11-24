// The following code is from https://github.com/withoutboats/display_derive/blob/232a32ee19e262aacbd2c93be5b4ce9e89a5fc30/tests/tests.rs
// Written by without boats originally

use derive_more::Display;

#[derive(Display)]
#[display("An error has occurred.")]
struct UnitError;

#[test]
fn unit_struct() {
    let s = UnitError.to_string();
    assert_eq!(s, "An error has occurred.");
}

#[derive(Display)]
#[display("Error code: {}", code)]
struct RecordError {
    code: u32,
}

#[test]
fn record_struct() {
    let s = RecordError { code: 0 }.to_string();
    assert_eq!(s, "Error code: 0");
}

#[derive(Display)]
#[display("Error code: {}", _0)]
struct TupleError(i32);

#[test]
fn tuple_struct() {
    let s = TupleError(2).to_string();
    assert_eq!(s, "Error code: 2");
}

#[allow(clippy::enum_variant_names)] // because of the original source
#[derive(Display)]
enum EnumError {
    #[display("Error code: {}", code)]
    StructVariant { code: i32 },
    #[display("Error: {}", _0)]
    TupleVariant(&'static str),
    #[display("An error has occurred.")]
    UnitVariant,
}

#[test]
fn enum_error() {
    let s = EnumError::StructVariant { code: 2 }.to_string();
    assert_eq!(s, "Error code: 2");
    let s = EnumError::TupleVariant("foobar").to_string();
    assert_eq!(s, "Error: foobar");
    let s = EnumError::UnitVariant.to_string();
    assert_eq!(s, "An error has occurred.");
}
