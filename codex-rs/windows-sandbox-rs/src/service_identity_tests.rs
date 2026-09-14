use super::validate_service_family_hint;

#[test]
fn service_routing_accepts_package_families() {
    for family in [
        "OpenAI.Codex_3k8sg7r9htsxt",
        "OpenAI.CodexBeta_jabp31b5fhs74",
    ] {
        assert!(validate_service_family_hint(family).is_ok());
    }
}

#[test]
fn service_routing_rejects_paths_and_malformed_families() {
    for family in [
        "",
        "_3k8sg7r9htsxt",
        "OpenAI.Codex_bad",
        "OpenAI.Codex_3k8sg7r9htsxt\\child",
        "..\\OpenAI.Codex_3k8sg7r9htsxt",
        "OpenAI.Codex_extra_3k8sg7r9htsxt",
        "OpenAI.Codex\0_3k8sg7r9htsxt",
    ] {
        assert!(validate_service_family_hint(family).is_err(), "{family:?}");
    }
}
