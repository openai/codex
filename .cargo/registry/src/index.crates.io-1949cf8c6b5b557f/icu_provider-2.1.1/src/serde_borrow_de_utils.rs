// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use alloc::borrow::Cow;
use serde::de::Deserializer;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(transparent)]
// Cows fail to borrow in some situations (array, option), but structs of Cows don't.
#[allow(clippy::exhaustive_structs)] // newtype
#[derive(Debug)]
pub struct CowWrap<'data>(#[serde(borrow)] pub Cow<'data, str>);

#[derive(Deserialize)]
#[serde(transparent)]
// Cows fail to borrow in some situations (array, option), but structs of Cows don't.
#[allow(clippy::exhaustive_structs)] // newtype
#[derive(Debug)]
pub struct CowBytesWrap<'data>(#[serde(borrow)] pub Cow<'data, [u8]>);

pub fn array_of_cow<'de, D, const N: usize>(deserializer: D) -> Result<[Cow<'de, str>; N], D::Error>
where
    D: Deserializer<'de>,
    [CowWrap<'de>; N]: Deserialize<'de>,
{
    <[CowWrap<'de>; N]>::deserialize(deserializer).map(|array| array.map(|wrap| wrap.0))
}

pub fn option_of_cow<'de, D>(deserializer: D) -> Result<Option<Cow<'de, str>>, D::Error>
where
    D: Deserializer<'de>,
{
    <Option<CowWrap<'de>>>::deserialize(deserializer).map(|opt| opt.map(|wrap| wrap.0))
}

pub fn tuple_of_cow<'de, D>(deserializer: D) -> Result<(Cow<'de, str>, Cow<'de, str>), D::Error>
where
    D: Deserializer<'de>,
    (CowWrap<'de>, CowWrap<'de>): Deserialize<'de>,
{
    <(CowWrap<'de>, CowWrap<'de>)>::deserialize(deserializer).map(|x| (x.0 .0, x.1 .0))
}

#[test]
fn test_option() {
    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Demo<'s>(#[serde(borrow, deserialize_with = "option_of_cow")] Option<Cow<'s, str>>);

    let data_orig = Demo(Some("Hello world".into()));
    let json = serde_json::to_string(&data_orig).expect("serialize");
    let data_new = serde_json::from_str::<Demo>(&json).expect("deserialize");
    assert_eq!(data_orig, data_new);
    assert!(matches!(data_new.0, Some(Cow::Borrowed(_))));
}

#[test]
fn test_tuple() {
    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Demo<'s>(
        #[serde(borrow, deserialize_with = "tuple_of_cow")] (Cow<'s, str>, Cow<'s, str>),
    );

    let data_orig = Demo(("Hello world".into(), "Hello earth".into()));
    let json = serde_json::to_string(&data_orig).expect("serialize");
    let data_new = serde_json::from_str::<Demo>(&json).expect("deserialize");
    assert_eq!(data_orig, data_new);
    assert!(matches!(data_new.0, (Cow::Borrowed(_), Cow::Borrowed(_))));
}

#[test]
fn test_array() {
    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Demo<'s>(#[serde(borrow, deserialize_with = "array_of_cow")] [Cow<'s, str>; 1]);

    let data_orig = Demo(["Hello world".into()]);
    let json = serde_json::to_string(&data_orig).expect("serialize");
    let data_new = serde_json::from_str::<Demo>(&json).expect("deserialize");
    assert_eq!(data_orig, data_new);
    assert!(matches!(data_new.0, [Cow::Borrowed(_)]));
}
