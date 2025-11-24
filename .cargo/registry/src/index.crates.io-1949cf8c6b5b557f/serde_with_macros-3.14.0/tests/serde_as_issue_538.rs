//! Check for the correct processing of `ExprGroup`s in types
//!
//! They occur when the types is passed in as a `macro_rules` argument like here.
//! <https://github.com/jonasbb/serde_with/issues/538>

macro_rules! t {
    ($($param:ident : $ty:ty),*) => {
        #[derive(Default)]
        #[serde_with_macros::apply(
            crate = "serde_with_macros",
            Option => #[serde(skip_serializing_if = "Option::is_none")],
        )]
        #[derive(serde::Serialize)]
        struct Data {
            a: Option<String>,
            b: Option<u64>,
            c: Option<String>,
            $(pub $param: $ty),*
        }
    }
}

t!(d: Option<bool>);

#[test]
fn t() {
    let expected = r#"{}"#;
    let data = Data::default();
    assert_eq!(expected, serde_json::to_string(&data).unwrap());
}
