pub fn get_codex_user_agent(originator: Option<&str>) -> String {
    let build_version = env!("CARGO_PKG_VERSION");
    let os_info = os_info::get();
    format!(
        "{}/{build_version} ({} {}; {})",
        originator.unwrap_or("codex_cli_rs"),
        os_info.os_type(),
        os_info.version(),
        os_info.architecture().unwrap_or("unknown"),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use regex::Regex;

    #[test]
    fn test_get_codex_user_agent() {
        let user_agent = get_codex_user_agent(None);
        assert!(user_agent.starts_with("codex_cli_rs/"));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_macos() {
        let user_agent = get_codex_user_agent(None);
        let re =
            Regex::new(r"^codex_cli_rs/\d+\.\d+\.\d+ \(Mac OS \d+\.\d+\.\d+; (x86_64|arm64)\)$")
                .unwrap();
        assert!(re.is_match(&user_agent));
    }
}
