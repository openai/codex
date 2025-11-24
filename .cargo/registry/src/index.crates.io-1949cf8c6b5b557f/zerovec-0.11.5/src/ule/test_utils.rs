// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

/// Take a VarULE type and serialize it both in human and machine readable contexts,
/// and ensure it roundtrips correctly
///
/// Note that the concrete type may need to be explicitly specified to prevent issues with
/// https://github.com/rust-lang/rust/issues/130180
#[cfg(feature = "serde")]
pub(crate) fn assert_serde_roundtrips<T>(var: &T)
where
    T: crate::ule::VarULE + ?Sized + serde::Serialize,
    for<'a> Box<T>: serde::Deserialize<'a>,
    for<'a> &'a T: serde::Deserialize<'a>,
    T: core::fmt::Debug + PartialEq,
{
    let bincode = bincode::serialize(var).unwrap();
    let deserialized: &T = bincode::deserialize(&bincode).unwrap();
    let deserialized_box: Box<T> = bincode::deserialize(&bincode).unwrap();
    assert_eq!(var, deserialized, "Single element roundtrips with bincode");
    assert_eq!(
        var, &*deserialized_box,
        "Single element roundtrips with bincode"
    );

    let json = serde_json::to_string(var).unwrap();
    let deserialized: Box<T> = serde_json::from_str(&json).unwrap();
    assert_eq!(var, &*deserialized, "Single element roundtrips with serde");
}
