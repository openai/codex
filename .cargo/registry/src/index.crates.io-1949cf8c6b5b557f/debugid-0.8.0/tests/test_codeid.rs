use debugid::CodeId;

#[test]
fn test_new() {
    let id = CodeId::new("dfb8e43af2423d73a453aeb6a777ef75".into());
    assert_eq!(id.as_str(), "dfb8e43af2423d73a453aeb6a777ef75");
}

#[test]
fn test_pe() {
    // Code identifiers of PE files might have an odd length and contain upper case characters.
    let id = CodeId::new("5CCC38584b08000".into());
    assert_eq!(id.as_str(), "5ccc38584b08000");
}

#[test]
fn test_from_binary() {
    let binary = b"\xdf\xb8\xe4\x3a\xf2\x42\x3d\x73\xa4\x53\xae\xb6\xa7\x77\xef\x75";
    let id = CodeId::from_binary(&binary[..]);
    assert_eq!(id.as_str(), "dfb8e43af2423d73a453aeb6a777ef75");
}

#[test]
fn test_is_nil() {
    let id = CodeId::nil();
    assert!(id.is_nil());
}
