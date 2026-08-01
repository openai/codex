use super::KeybindingSpec;
use super::KeybindingsSpec;
use super::TuiKeymap;
use super::normalize_keybinding_spec;
use pretty_assertions::assert_eq;

#[test]
fn normalizes_each_stroke_in_a_key_chord() {
    assert_eq!(
        normalize_keybinding_spec("  CONTROL-X   Option-Return  "),
        Ok("ctrl-x alt-enter".to_string())
    );
}

#[test]
fn deserializes_a_two_stroke_global_key_chord() {
    let actual = toml::from_str::<TuiKeymap>(
        r#"
        [global]
        open_transcript = "ctrl-x ctrl-t"
        "#,
    )
    .expect("two-stroke key chords should deserialize");

    let expected = TuiKeymap {
        global: super::TuiGlobalKeymap {
            open_transcript: Some(KeybindingsSpec::One(KeybindingSpec(
                "ctrl-x ctrl-t".to_string(),
            ))),
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(actual, expected);
}

#[test]
fn keeps_single_keys_and_chords_as_alternative_bindings() {
    let actual = toml::from_str::<TuiKeymap>(
        r#"
        [global]
        open_transcript = ["ctrl-t", "ctrl-x ctrl-t"]
        "#,
    )
    .expect("single keys and key chords should coexist");

    let expected = TuiKeymap {
        global: super::TuiGlobalKeymap {
            open_transcript: Some(KeybindingsSpec::Many(vec![
                KeybindingSpec("ctrl-t".to_string()),
                KeybindingSpec("ctrl-x ctrl-t".to_string()),
            ])),
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(actual, expected);
}

#[test]
fn rejects_chords_longer_than_two_strokes() {
    let error = normalize_keybinding_spec("ctrl-x ctrl-s ctrl-t")
        .expect_err("key chords should have a bounded length");

    assert!(error.contains("at most 2 strokes"), "{error}");
}

#[test]
fn rejects_an_invalid_second_chord_stroke() {
    let error = normalize_keybinding_spec("ctrl-x ctrl-unknown")
        .expect_err("every key chord stroke should be valid");

    assert!(error.contains("unknown key"), "{error}");
}
