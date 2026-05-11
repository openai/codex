use codex_login::default_client::is_first_party_chat_originator;
use codex_login::default_client::originator;

const DISALLOWED_CONNECTOR_IDS: &[&str] = &[
    "asdk_app_6938a94a61d881918ef32cb999ff937c",
    "connector_2b0a9009c9c64bf9933a3dae3f2b1254",
    "connector_3f8d1a79f27c4c7ba1a897ab13bf37dc",
    "connector_68de829bf7648191acd70a907364c67c",
    "connector_68e004f14af881919eb50893d3d9f523",
    "connector_69272cb413a081919685ec3c88d1744e",
];
const FIRST_PARTY_CHAT_DISALLOWED_CONNECTOR_IDS: &[&str] =
    &["connector_0f9c9d4592e54d0a9a12b3f44a1e2010"];
const ALLOWED_OPENAI_CONNECTOR_IDS: &[&str] = &["connector_openai_library"];
const DISALLOWED_CONNECTOR_PREFIX: &str = "connector_openai_";

pub fn is_connector_id_allowed(connector_id: &str) -> bool {
    is_connector_id_allowed_for_originator(connector_id, originator().value.as_str())
}

fn is_connector_id_allowed_for_originator(connector_id: &str, originator_value: &str) -> bool {
    let disallowed_connector_ids = if is_first_party_chat_originator(originator_value) {
        FIRST_PARTY_CHAT_DISALLOWED_CONNECTOR_IDS
    } else {
        DISALLOWED_CONNECTOR_IDS
    };

    if ALLOWED_OPENAI_CONNECTOR_IDS.contains(&connector_id) {
        return true;
    }

    !connector_id.starts_with(DISALLOWED_CONNECTOR_PREFIX)
        && !disallowed_connector_ids.contains(&connector_id)
}

pub fn sanitize_name(name: &str) -> String {
    sanitize_slug(name).replace("-", "_")
}

fn sanitize_slug(name: &str) -> String {
    let mut normalized = String::with_capacity(name.len());
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
        } else {
            normalized.push('-');
        }
    }
    let normalized = normalized.trim_matches('-');
    if normalized.is_empty() {
        "app".to_string()
    } else {
        normalized.to_string()
    }
}
