use crate::prelude::*;
use std::ffi::{CStr, CString, OsStr, OsString};
use std::marker::PhantomData;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::num::{NonZeroI64, NonZeroU64};
use std::ops::{Bound, Range, RangeInclusive};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[test]
fn option() {
    test!(Option<bool>)
        .assert_allows_ser_roundtrip([Some(true), None])
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[test]
fn result() {
    test!(Result<bool, String>)
        .assert_allows_ser_roundtrip([Ok(true), Err("oh no!".to_owned())])
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[test]
fn nonzero() {
    test!(NonZeroI64)
        .assert_allows_ser_roundtrip([NonZeroI64::MIN, NonZeroI64::MAX])
        .assert_rejects_de([Value::from(0)])
        .assert_matches_de_roundtrip(arbitrary_values_except(
            |v| v.as_u64().is_some_and(|u| u > i64::MAX as u64),
            "FIXME schema allows out-of-range positive integers",
        ));

    test!(NonZeroU64)
        .assert_allows_ser_roundtrip([NonZeroU64::MIN, NonZeroU64::MAX])
        .assert_rejects_de([Value::from(0)])
        .assert_matches_de_roundtrip(arbitrary_values());
}

const IPV4_SAMPLES: [Ipv4Addr; 3] = [
    // Commented-out until https://github.com/Stranger6667/jsonschema-rs/issues/512 is fixed
    // Ipv4Addr::UNSPECIFIED,
    Ipv4Addr::LOCALHOST,
    Ipv4Addr::BROADCAST,
    Ipv4Addr::new(1, 2, 3, 4),
];
const IPV6_SAMPLES: [Ipv6Addr; 4] = [
    Ipv6Addr::UNSPECIFIED,
    Ipv6Addr::LOCALHOST,
    Ipv4Addr::LOCALHOST.to_ipv6_mapped(),
    Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8),
];

#[test]
fn ip_addr() {
    test!(Ipv4Addr)
        .assert_allows_ser_roundtrip(IPV4_SAMPLES)
        .assert_matches_de_roundtrip(arbitrary_values());

    test!(Ipv6Addr)
        .assert_allows_ser_roundtrip(IPV6_SAMPLES)
        .assert_matches_de_roundtrip(arbitrary_values());

    test!(IpAddr)
        .assert_allows_ser_roundtrip(IPV4_SAMPLES.map(Into::into))
        .assert_allows_ser_roundtrip(IPV6_SAMPLES.map(Into::into))
        .assert_matches_de_roundtrip(arbitrary_values_except(
            Value::is_string,
            "Custom format 'ip', so arbitrary strings technically allowed by schema",
        ));
}

#[test]
fn socket_addr() {
    let port = 12345;

    test!(SocketAddrV4)
        .assert_allows_ser_roundtrip(IPV4_SAMPLES.map(|ip| SocketAddrV4::new(ip, port)))
        .assert_matches_de_roundtrip(arbitrary_values_except(
            Value::is_string,
            "Arbitrary strings allowed by schema",
        ));

    test!(SocketAddrV6)
        .assert_allows_ser_roundtrip(IPV6_SAMPLES.map(|ip| SocketAddrV6::new(ip, port, 0, 0)))
        .assert_matches_de_roundtrip(arbitrary_values_except(
            Value::is_string,
            "Arbitrary strings allowed by schema",
        ));

    test!(SocketAddr)
        .assert_allows_ser_roundtrip(IPV4_SAMPLES.map(|ip| (ip, port).into()))
        .assert_allows_ser_roundtrip(IPV6_SAMPLES.map(|ip| (ip, port).into()))
        .assert_matches_de_roundtrip(arbitrary_values_except(
            Value::is_string,
            "Arbitrary strings allowed by schema",
        ));
}

#[test]
fn time() {
    test!(SystemTime)
        .assert_allows_ser_roundtrip([SystemTime::UNIX_EPOCH, SystemTime::now()])
        .assert_matches_de_roundtrip(arbitrary_values());

    test!(Duration)
        .assert_allows_ser_roundtrip([Duration::from_secs(0), Duration::from_secs_f64(123.456)])
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[test]
fn c_strings() {
    let strings = [
        CString::default(),
        CString::new("test").unwrap(),
        CString::new([255]).unwrap(),
    ];

    test!(CString)
        .assert_allows_ser_roundtrip(strings.clone())
        .assert_matches_de_roundtrip(arbitrary_values_except(
            |v| v.as_str().is_some_and(|s| s.contains('\0')),
            "CString cannot contain null bytes, but schema does not enforce this",
        ));

    test!(Box<CStr>)
        .assert_identical::<CString>()
        .assert_allows_ser_roundtrip(strings.into_iter().map(Into::into))
        .assert_matches_de_roundtrip(arbitrary_values_except(
            |v| v.as_str().is_some_and(|s| s.contains('\0')),
            "CString cannot contain null bytes, but schema does not enforce this",
        ));
}

#[test]
fn os_strings() {
    let strings = [OsString::new(), OsString::from("test")];

    test!(OsString)
        .assert_allows_ser_roundtrip(strings.clone())
        .assert_matches_de_roundtrip(arbitrary_values());

    test!(Box<OsStr>)
        .assert_identical::<OsString>()
        .assert_allows_ser_roundtrip(strings.into_iter().map(Into::into))
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[test]
fn paths() {
    let strings = [PathBuf::new(), PathBuf::from("test")];

    test!(PathBuf)
        .assert_identical::<String>()
        .assert_allows_ser_roundtrip(strings.clone())
        .assert_matches_de_roundtrip(arbitrary_values());

    test!(Box<Path>)
        .assert_identical::<String>()
        .assert_identical::<PathBuf>()
        .assert_allows_ser_roundtrip(strings.into_iter().map(Into::into))
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[test]
fn bound() {
    test!(Bound<i32>).assert_allows_ser_roundtrip([
        Bound::Included(123),
        Bound::Excluded(456),
        Bound::Unbounded,
    ]);
}

#[test]
fn ranges() {
    test!(Range<i32>)
        .assert_allows_ser_roundtrip([0..0, 123..456])
        .assert_matches_de_roundtrip(arbitrary_values());

    test!(RangeInclusive<i32>)
        .assert_allows_ser_roundtrip([0..=0, 123..=456])
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[test]
fn phantom_data() {
    struct DoesNotImplementJsonSchema;

    test!(PhantomData<DoesNotImplementJsonSchema>)
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());
}
