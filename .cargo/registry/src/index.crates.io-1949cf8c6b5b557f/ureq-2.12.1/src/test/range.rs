#[test]
#[cfg(feature = "tls")]
fn read_range_rustls() {
    use std::io::Read;

    use super::super::*;

    // rustls is used via crate level convenience calls
    let resp = get("https://ureq.s3.eu-central-1.amazonaws.com/sherlock.txt")
        .set("Range", "bytes=1000-1999")
        .call()
        .unwrap();
    assert_eq!(resp.status(), 206);
    let mut reader = resp.into_reader();
    let mut buf = vec![];
    let len = reader.read_to_end(&mut buf).unwrap();
    assert_eq!(len, 1000);
    assert_eq!(
        &buf[0..20],
        [83, 99, 111, 116, 116, 34, 10, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32]
    )
}

#[test]
#[cfg(feature = "native-tls")]
fn read_range_native_tls() {
    use std::io::Read;
    use std::sync::Arc;

    use super::super::*;

    let tls_config = native_tls::TlsConnector::new().unwrap();
    let agent = builder().tls_connector(Arc::new(tls_config)).build();

    let resp = agent
        .get("https://ureq.s3.eu-central-1.amazonaws.com/sherlock.txt")
        .set("Range", "bytes=1000-1999")
        .call()
        .unwrap();
    assert_eq!(resp.status(), 206);
    let mut reader = resp.into_reader();
    let mut buf = vec![];
    let len = reader.read_to_end(&mut buf).unwrap();
    assert_eq!(len, 1000);
    assert_eq!(
        &buf[0..20],
        [83, 99, 111, 116, 116, 34, 10, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32]
    )
}
