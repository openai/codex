use serde_derive::Serialize;

#[derive(Serialize)]
struct NewType<T>(T);

#[test]
fn serialize_newtype_i32() {
    let params = &[("field", Some(NewType(11)))];
    assert_eq!(
        serde_urlencoded::to_string(params),
        Ok("field=11".to_owned())
    );
}

#[test]
fn serialize_newtype_u128() {
    let params = &[("field", Some(NewType(u128::MAX)))];
    assert_eq!(
        serde_urlencoded::to_string(params),
        Ok(format!("field={}", u128::MAX))
    );
}

#[test]
fn serialize_newtype_i128() {
    let params = &[("field", Some(NewType(i128::MIN)))];
    assert_eq!(
        serde_urlencoded::to_string(params),
        Ok(format!("field={}", i128::MIN))
    );
}

#[test]
fn serialize_option_map_int() {
    let params = &[("first", Some(23)), ("middle", None), ("last", Some(42))];

    assert_eq!(
        serde_urlencoded::to_string(params),
        Ok("first=23&last=42".to_owned())
    );
}

#[test]
fn serialize_option_map_string() {
    let params = &[
        ("first", Some("hello")),
        ("middle", None),
        ("last", Some("world")),
    ];

    assert_eq!(
        serde_urlencoded::to_string(params),
        Ok("first=hello&last=world".to_owned())
    );
}

#[test]
fn serialize_option_map_bool() {
    let params = &[("one", Some(true)), ("two", Some(false))];

    assert_eq!(
        serde_urlencoded::to_string(params),
        Ok("one=true&two=false".to_owned())
    );
}

#[test]
fn serialize_map_bool() {
    let params = &[("one", true), ("two", false)];

    assert_eq!(
        serde_urlencoded::to_string(params),
        Ok("one=true&two=false".to_owned())
    );
}

#[derive(Serialize)]
enum X {
    A,
    B,
    C,
}

#[test]
fn serialize_unit_enum() {
    let params = &[("one", X::A), ("two", X::B), ("three", X::C)];
    assert_eq!(
        serde_urlencoded::to_string(params),
        Ok("one=A&two=B&three=C".to_owned())
    );
}

#[derive(Serialize)]
struct Unit;

#[test]
fn serialize_unit_struct() {
    assert_eq!(serde_urlencoded::to_string(Unit), Ok("".to_owned()));
}

#[test]
fn serialize_unit_type() {
    assert_eq!(serde_urlencoded::to_string(()), Ok("".to_owned()));
}
