use rustls_pki_types::ServerName;

fn compile_time_assert_hash<T: core::hash::Hash>() {}
fn compile_time_assert_send<T: Send>() {}
fn compile_time_assert_sync<T: Sync>() {}

#[test]
fn test_server_name_traits() {
    compile_time_assert_hash::<ServerName>();
    compile_time_assert_send::<ServerName>();
    compile_time_assert_sync::<ServerName>();

    let a = ServerName::try_from(&b"example.com"[..]).unwrap();

    // `Clone`
    #[allow(clippy::clone_on_copy)]
    let _ = a.clone();
    // TODO: verify the clone is the same as `a`.

    // TODO: Don't require `alloc` for these.
    #[cfg(feature = "alloc")]
    {
        // `Debug`.
        assert_eq!(format!("{:?}", &a), "DnsName(\"example.com\")");
    }
}

#[cfg(feature = "alloc")]
#[test]
fn test_alloc_server_name_traits() {
    let a_ref = ServerName::try_from(&b"example.com"[..]).unwrap();
    let a = a_ref.to_owned();

    // `Clone`, `Debug`, `PartialEq`.
    assert_eq!(&a, &a.clone());

    // `Debug`.
    assert_eq!(format!("{:?}", &a), "DnsName(\"example.com\")");

    // PartialEq is case-insensitive
    assert_eq!(
        a,
        ServerName::try_from(&b"Example.Com"[..])
            .unwrap()
            .to_owned()
    );

    // PartialEq isn't completely wrong.
    assert_ne!(
        a,
        ServerName::try_from(&b"fxample.com"[..])
            .unwrap()
            .to_owned()
    );
    assert_ne!(
        a,
        ServerName::try_from(&b"example.co"[..]).unwrap().to_owned()
    );
}
