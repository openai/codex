use http::*;

#[test]
fn from_bytes() {
    for ok in &[
        "100", "101", "199", "200", "250", "299", "321", "399", "499", "599", "600", "999",
    ] {
        assert!(StatusCode::from_bytes(ok.as_bytes()).is_ok());
    }

    for not_ok in &[
        "0", "00", "10", "40", "99", "000", "010", "099", "1000", "1999",
    ] {
        assert!(StatusCode::from_bytes(not_ok.as_bytes()).is_err());
    }
}

#[test]
fn equates_with_u16() {
    let status = StatusCode::from_u16(200u16).unwrap();
    assert_eq!(200u16, status);
    assert_eq!(status, 200u16);
}

#[test]
fn roundtrip() {
    for s in 100..1000 {
        let sstr = s.to_string();
        let status = StatusCode::from_bytes(sstr.as_bytes()).unwrap();
        assert_eq!(s, u16::from(status));
        assert_eq!(sstr, status.as_str());
    }
}

#[test]
fn is_informational() {
    assert!(status_code(100).is_informational());
    assert!(status_code(199).is_informational());

    assert!(!status_code(200).is_informational());
}

#[test]
fn is_success() {
    assert!(status_code(200).is_success());
    assert!(status_code(299).is_success());

    assert!(!status_code(199).is_success());
    assert!(!status_code(300).is_success());
}

#[test]
fn is_redirection() {
    assert!(status_code(300).is_redirection());
    assert!(status_code(399).is_redirection());

    assert!(!status_code(299).is_redirection());
    assert!(!status_code(400).is_redirection());
}

#[test]
fn is_client_error() {
    assert!(status_code(400).is_client_error());
    assert!(status_code(499).is_client_error());

    assert!(!status_code(399).is_client_error());
    assert!(!status_code(500).is_client_error());
}

#[test]
fn is_server_error() {
    assert!(status_code(500).is_server_error());
    assert!(status_code(599).is_server_error());

    assert!(!status_code(499).is_server_error());
    assert!(!status_code(600).is_server_error());
}

/// Helper method for readability
fn status_code(status_code: u16) -> StatusCode {
    StatusCode::from_u16(status_code).unwrap()
}
