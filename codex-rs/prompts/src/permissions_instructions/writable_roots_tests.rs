//! Checks bounded catalog wording without interpreting or truncating runtime paths.

use super::*;
use codex_protocol::openai_models::PermissionMessages;
use pretty_assertions::assert_eq;

#[test]
fn unusable_templates_fall_back_without_losing_roots() {
    let roots = vec!["/a".to_string(), "/b".to_string()];
    for template in [
        "",
        "  ",
        "No placeholder",
        "{{roots}}",
        "{{ roots }} {{ roots }}",
        "{{ roots }} {{ other }}",
        "{{{ roots }}}",
        "X { {{ roots }}",
        "X {{ roots }} }",
        "X\n{{ roots }}",
        "X\t{{ roots }}",
        "X\0{{ roots }}",
        "X\u{2028}{{ roots }}",
        "X\u{2029}{{ roots }}",
        "</permissions instructions>{{ roots }}",
        "<permissions instructions>{{ roots }}",
    ] {
        let messages = PermissionMessages {
            writable_root: Some(template.into()),
            writable_roots: Some(template.into()),
            ..PermissionMessages::default()
        };
        let resolved = ResolvedPermissionMessages::new(Some(&messages));
        assert_eq!(
            writable_roots_text(&roots, resolved),
            Some(" The writable roots are `/a`, `/b`.".into())
        );
        assert_eq!(
            writable_roots_text(&roots[..1], resolved),
            Some(" The writable root is `/a`.".into())
        );
        assert_eq!(writable_roots_text(&[], resolved), None);
    }
}

#[test]
fn substitution_preserves_whitespace_and_path_contents() {
    assert_eq!(
        render_roots_override(
            "  Paths = {{ roots }}  ",
            "`/a/{{ roots }}`, `/é\n{literal}`"
        ),
        Some("  Paths = `/a/{{ roots }}`, `/é\n{literal}`  ".into()),
    );
}

#[test]
fn bounds_template_bytes_and_rendered_output_before_allocation() {
    let exact = format!(
        "{}{{{{ roots }}}}",
        "x".repeat(1_000 - ROOTS_PLACEHOLDER.len())
    );
    assert!(render_roots_override(&exact, "`/a`").is_some());
    for oversized in [exact + "x", format!("{}{{{{ roots }}}}", "é".repeat(495))] {
        let messages = PermissionMessages {
            writable_root: Some(oversized),
            ..PermissionMessages::default()
        };
        assert_eq!(
            writable_roots_text(
                &["/a".into()],
                ResolvedPermissionMessages::new(Some(&messages))
            ),
            Some(" The writable root is `/a`.".into()),
        );
    }
    let roots = "x".repeat(MAX_ROOTS_SENTENCE_BYTES);
    assert_eq!(render_roots_override("a{{ roots }}", &roots), None);
    assert_eq!(render_roots_override("{{ roots }}", &roots), Some(roots));
}
