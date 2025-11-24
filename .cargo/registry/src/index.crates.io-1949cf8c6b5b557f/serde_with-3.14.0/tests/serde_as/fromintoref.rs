use super::*;
use serde_with::{FromIntoRef, TryFromIntoRef};

#[derive(Debug, PartialEq)]
enum IntoSerializable {
    A,
    B,
    C,
}

impl<'a> From<&'a IntoSerializable> for String {
    fn from(value: &'a IntoSerializable) -> Self {
        match value {
            IntoSerializable::A => "String A",
            IntoSerializable::B => "Some other value",
            IntoSerializable::C => "Looks like 123",
        }
        .to_string()
    }
}

#[derive(Debug, PartialEq)]
enum FromDeserializable {
    Zero,
    Odd(u32),
    Even(u32),
}

impl From<u32> for FromDeserializable {
    fn from(value: u32) -> Self {
        match value {
            0 => FromDeserializable::Zero,
            e if e % 2 == 0 => FromDeserializable::Even(e),
            o => FromDeserializable::Odd(o),
        }
    }
}

#[derive(Debug, PartialEq)]
enum LikeBool {
    Trueish,
    Falseisch,
}

impl From<bool> for LikeBool {
    fn from(b: bool) -> Self {
        if b {
            LikeBool::Trueish
        } else {
            LikeBool::Falseisch
        }
    }
}

impl<'a> From<&'a LikeBool> for bool {
    fn from(lb: &'a LikeBool) -> Self {
        match lb {
            LikeBool::Trueish => true,
            LikeBool::Falseisch => false,
        }
    }
}

#[test]
fn test_frominto_ser() {
    #[serde_as]
    #[derive(Debug, PartialEq, Serialize)]
    struct S(#[serde_as(serialize_as = "FromIntoRef<String>")] IntoSerializable);

    check_serialization(S(IntoSerializable::A), expect![[r#""String A""#]]);
    check_serialization(S(IntoSerializable::B), expect![[r#""Some other value""#]]);
    check_serialization(S(IntoSerializable::C), expect![[r#""Looks like 123""#]]);
}

#[test]
fn test_tryfrominto_ser() {
    #[serde_as]
    #[derive(Debug, PartialEq, Serialize)]
    struct S(#[serde_as(serialize_as = "TryFromIntoRef<String>")] IntoSerializable);

    check_serialization(S(IntoSerializable::A), expect![[r#""String A""#]]);
    check_serialization(S(IntoSerializable::B), expect![[r#""Some other value""#]]);
    check_serialization(S(IntoSerializable::C), expect![[r#""Looks like 123""#]]);
}

#[test]
fn test_frominto_de() {
    #[serde_as]
    #[derive(Debug, PartialEq, Deserialize)]
    struct S(#[serde_as(deserialize_as = "FromIntoRef<u32>")] FromDeserializable);

    check_deserialization(S(FromDeserializable::Zero), "0");
    check_deserialization(S(FromDeserializable::Odd(1)), "1");
    check_deserialization(S(FromDeserializable::Odd(101)), "101");
    check_deserialization(S(FromDeserializable::Even(2)), "2");
    check_deserialization(S(FromDeserializable::Even(202)), "202");
}

#[test]
fn test_tryfrominto_de() {
    #[serde_as]
    #[derive(Debug, PartialEq, Deserialize)]
    struct S(#[serde_as(deserialize_as = "TryFromIntoRef<u32>")] FromDeserializable);

    check_deserialization(S(FromDeserializable::Zero), "0");
    check_deserialization(S(FromDeserializable::Odd(1)), "1");
    check_deserialization(S(FromDeserializable::Odd(101)), "101");
    check_deserialization(S(FromDeserializable::Even(2)), "2");
    check_deserialization(S(FromDeserializable::Even(202)), "202");
}

#[test]
fn test_frominto_de_and_ser() {
    #[serde_as]
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct S(#[serde_as(as = "FromIntoRef<bool>")] LikeBool);

    is_equal(S(LikeBool::Trueish), expect![[r#"true"#]]);
    is_equal(S(LikeBool::Falseisch), expect![[r#"false"#]]);
}

#[test]
fn test_tryfrominto_de_and_ser() {
    #[serde_as]
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct S(#[serde_as(as = "TryFromIntoRef<bool>")] LikeBool);

    is_equal(S(LikeBool::Trueish), expect![[r#"true"#]]);
    is_equal(S(LikeBool::Falseisch), expect![[r#"false"#]]);
}

#[derive(Debug, PartialEq)]
enum TryIntoSerializable {
    Works,
    Fails,
}

impl<'a> TryFrom<&'a TryIntoSerializable> for String {
    type Error = &'static str;

    fn try_from(value: &'a TryIntoSerializable) -> Result<Self, Self::Error> {
        match value {
            TryIntoSerializable::Works => Ok("Works".to_string()),
            TryIntoSerializable::Fails => Err("Fails cannot be turned into String"),
        }
    }
}

#[derive(Debug, PartialEq)]
enum TryFromDeserializable {
    Zero,
}

impl TryFrom<u32> for TryFromDeserializable {
    type Error = &'static str;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(TryFromDeserializable::Zero),
            _ => Err("Number is not zero"),
        }
    }
}

#[test]
fn test_tryfrominto_ser_with_error() {
    #[serde_as]
    #[derive(Debug, PartialEq, Serialize)]
    struct S(#[serde_as(serialize_as = "TryFromIntoRef<String>")] TryIntoSerializable);

    check_serialization(S(TryIntoSerializable::Works), expect![[r#""Works""#]]);
    check_error_serialization(
        S(TryIntoSerializable::Fails),
        expect![[r#"Fails cannot be turned into String"#]],
    );
}

#[test]
fn test_tryfrominto_de_with_error() {
    #[serde_as]
    #[derive(Debug, PartialEq, Deserialize)]
    struct S(#[serde_as(deserialize_as = "TryFromIntoRef<u32>")] TryFromDeserializable);

    check_deserialization(S(TryFromDeserializable::Zero), "0");
    check_error_deserialization::<S>("1", expect![[r#"Number is not zero"#]]);
}
