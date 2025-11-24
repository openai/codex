use serde_json::Value;

#[test]
fn test() {
    let x1 = serde_json::from_str::<Value>("18446744073709551615.");
    assert!(x1.is_err());
    let x2 = serde_json::from_str::<Value>("18446744073709551616.");
    assert!(x2.is_err());
}
