use super::PluginId;

#[test]
fn accepts_dotted_plugin_names() {
    let plugin_id =
        PluginId::new("acme.tools".to_string(), "marketplace".to_string()).expect("plugin id");

    assert_eq!(plugin_id.as_key(), "acme.tools@marketplace");
}

#[test]
fn marketplace_names_preserve_legacy_character_set() {
    PluginId::new("acme".to_string(), "marketplace_name-1".to_string()).expect("marketplace name");

    let err = PluginId::new("acme".to_string(), "market.place".to_string())
        .expect_err("dotted marketplace name");
    assert_eq!(
        err.to_string(),
        "invalid marketplace name: only ASCII letters, digits, `_`, and `-` are allowed"
    );
}

#[test]
fn rejects_dot_path_segments() {
    assert!(PluginId::new(".".to_string(), "marketplace".to_string()).is_err());
    assert!(PluginId::new("plugin".to_string(), "..".to_string()).is_err());
}

#[test]
fn rejects_dots_that_can_alias_path_segments() {
    for plugin_name in [".plugin", "plugin.", "plugin..name"] {
        assert!(PluginId::new(plugin_name.to_string(), "marketplace".to_string()).is_err());
    }
}
