use crate::prelude::*;

#[test]
fn decimal_types() {
    #[cfg(feature = "rust_decimal1")]
    test!(rust_decimal1::Decimal)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());

    #[cfg(feature = "bigdecimal04")]
    test!(bigdecimal04::BigDecimal)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());

    #[cfg(all(feature = "rust_decimal1", feature = "bigdecimal04"))]
    test!(bigdecimal04::BigDecimal).assert_identical::<rust_decimal1::Decimal>();
}
