//! Protects fingerprint bytes when nested configuration keys are reordered.

use super::version_for_toml;
use pretty_assertions::assert_eq;

#[test]
fn fingerprint_preserves_nested_key_order_independence_and_existing_hash() {
    for input in [
        r#"rows = [{ z = 2, a = 1 }, { z = 4, a = 3 }]
[nested]
z = "text"
a = true
"#,
        r#"rows = [{ a = 1, z = 2 }, { a = 3, z = 4 }]
[nested]
a = true
z = "text"
"#,
    ] {
        let value = toml::from_str(input).expect("valid config");
        assert_eq!(
            version_for_toml(&value),
            "sha256:a5e4c8fbdbc75d185b0f2b5291bfb2de10e8c523d6c89248876fd78a97c83176"
        );
    }
}
