use super::*;

#[test]
fn cases() {
    case(TestFlags::empty(), "TestFlags(0x0)", "0", "0", "0", "0");
    case(TestFlags::A, "TestFlags(A)", "1", "1", "1", "1");
    case(
        TestFlags::all(),
        "TestFlags(A | B | C)",
        "7",
        "7",
        "7",
        "111",
    );
    case(
        TestFlags::from_bits_retain(1 << 3),
        "TestFlags(0x8)",
        "8",
        "8",
        "10",
        "1000",
    );
    case(
        TestFlags::A | TestFlags::from_bits_retain(1 << 3),
        "TestFlags(A | 0x8)",
        "9",
        "9",
        "11",
        "1001",
    );

    case(TestZero::ZERO, "TestZero(0x0)", "0", "0", "0", "0");
    case(
        TestZero::ZERO | TestZero::from_bits_retain(1),
        "TestZero(0x1)",
        "1",
        "1",
        "1",
        "1",
    );

    case(TestZeroOne::ONE, "TestZeroOne(ONE)", "1", "1", "1", "1");

    case(
        TestOverlapping::from_bits_retain(1 << 1),
        "TestOverlapping(0x2)",
        "2",
        "2",
        "2",
        "10",
    );

    case(
        TestExternal::from_bits_retain(1 | 1 << 1 | 1 << 3),
        "TestExternal(A | B | 0x8)",
        "B",
        "b",
        "13",
        "1011",
    );

    case(
        TestExternal::all(),
        "TestExternal(A | B | C | 0xf8)",
        "FF",
        "ff",
        "377",
        "11111111",
    );

    case(
        TestExternalFull::all(),
        "TestExternalFull(0xff)",
        "FF",
        "ff",
        "377",
        "11111111",
    );
}

#[track_caller]
fn case<
    T: std::fmt::Debug + std::fmt::UpperHex + std::fmt::LowerHex + std::fmt::Octal + std::fmt::Binary,
>(
    value: T,
    debug: &str,
    uhex: &str,
    lhex: &str,
    oct: &str,
    bin: &str,
) {
    assert_eq!(debug, format!("{:?}", value));
    assert_eq!(uhex, format!("{:X}", value));
    assert_eq!(lhex, format!("{:x}", value));
    assert_eq!(oct, format!("{:o}", value));
    assert_eq!(bin, format!("{:b}", value));
}
