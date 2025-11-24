#[cfg(feature = "bigdecimal-impl")]
#[test]
fn impl_primitive_bigdecimal() {
    assert_eq!(
        <bigdecimal::BigDecimal as ts_rs::TS>::name(),
        <String as ts_rs::TS>::name()
    );
    assert_eq!(
        <bigdecimal::BigDecimal as ts_rs::TS>::inline(),
        <String as ts_rs::TS>::inline()
    )
}

#[cfg(feature = "smol_str-impl")]
#[test]
fn impl_primitive_smolstr() {
    assert_eq!(
        <smol_str::SmolStr as ts_rs::TS>::name(),
        <String as ts_rs::TS>::name()
    );
    assert_eq!(
        <smol_str::SmolStr as ts_rs::TS>::inline(),
        <String as ts_rs::TS>::inline()
    )
}

#[cfg(feature = "uuid-impl")]
#[test]
fn impl_primitive_uuid() {
    assert_eq!(
        <uuid::Uuid as ts_rs::TS>::name(),
        <String as ts_rs::TS>::name()
    );
    assert_eq!(
        <uuid::Uuid as ts_rs::TS>::inline(),
        <String as ts_rs::TS>::inline()
    )
}

#[cfg(feature = "url-impl")]
#[test]
fn impl_primitive_url() {
    assert_eq!(
        <url::Url as ts_rs::TS>::name(),
        <String as ts_rs::TS>::name()
    );
    assert_eq!(
        <url::Url as ts_rs::TS>::inline(),
        <String as ts_rs::TS>::inline()
    )
}

#[cfg(feature = "ordered-float-impl")]
#[test]
fn impl_primitive_order_float() {
    assert_eq!(
        <ordered_float::OrderedFloat<f64> as ts_rs::TS>::name(),
        <f64 as ts_rs::TS>::name()
    );
    assert_eq!(
        <ordered_float::OrderedFloat<f64> as ts_rs::TS>::inline(),
        <f64 as ts_rs::TS>::inline()
    );
    assert_eq!(
        <ordered_float::OrderedFloat<f32> as ts_rs::TS>::name(),
        <f32 as ts_rs::TS>::name()
    );
    assert_eq!(
        <ordered_float::OrderedFloat<f32> as ts_rs::TS>::inline(),
        <f32 as ts_rs::TS>::inline()
    )
}

#[cfg(feature = "bson-uuid-impl")]
#[test]
fn impl_primitive_bson_uuid() {
    assert_eq!(
        <bson::oid::ObjectId as ts_rs::TS>::name(),
        <String as ts_rs::TS>::name()
    );
    assert_eq!(
        <bson::oid::ObjectId as ts_rs::TS>::inline(),
        <String as ts_rs::TS>::inline()
    );
    assert_eq!(
        <bson::Uuid as ts_rs::TS>::name(),
        <String as ts_rs::TS>::name()
    );
    assert_eq!(
        <bson::Uuid as ts_rs::TS>::inline(),
        <String as ts_rs::TS>::inline()
    )
}

#[cfg(feature = "semver-impl")]
#[test]
fn impl_primitive_semver() {
    assert_eq!(
        <semver::Version as ts_rs::TS>::name(),
        <String as ts_rs::TS>::name()
    );
    assert_eq!(
        <semver::Version as ts_rs::TS>::inline(),
        <String as ts_rs::TS>::inline()
    )
}
