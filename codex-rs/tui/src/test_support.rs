pub(crate) use codex_utils_absolute_path::test_support::PathBufExt;
pub(crate) use codex_utils_absolute_path::test_support::test_path_buf;

pub(crate) fn test_path_display(path: &str) -> String {
    test_path_buf(path).display().to_string()
}

pub(crate) fn session_source_cli() -> codex_protocol::protocol::SessionSource {
    codex_protocol::protocol::SessionSource::Cli
}

pub(crate) fn skill_scope_user() -> codex_protocol::protocol::SkillScope {
    codex_protocol::protocol::SkillScope::User
}

pub(crate) fn skill_scope_repo() -> codex_protocol::protocol::SkillScope {
    codex_protocol::protocol::SkillScope::Repo
}
