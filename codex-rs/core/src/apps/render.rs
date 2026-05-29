use crate::context::AppsInstructions;
use crate::context::ContextualUserFragment;
use codex_app_server_protocol::AppInfo;
use codex_protocol::protocol::APPS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::APPS_INSTRUCTIONS_OPEN_TAG;

pub(crate) fn render_apps_section(connectors: &[AppInfo]) -> Option<String> {
    AppsInstructions::from_connectors(connectors).map(|instructions| instructions.render())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connector(id: &str, is_accessible: bool, is_enabled: bool) -> AppInfo {
        AppInfo {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: None,
            is_accessible,
            is_enabled,
            plugin_display_names: Vec::new(),
        }
    }

    #[test]
    fn omits_apps_section_without_accessible_and_enabled_apps() {
        assert_eq!(render_apps_section(&[]), None);
        assert_eq!(
            render_apps_section(&[connector(
                "calendar", /*is_accessible*/ true, /*is_enabled*/ false
            )]),
            None
        );
        assert_eq!(
            render_apps_section(&[connector(
                "calendar", /*is_accessible*/ false, /*is_enabled*/ true
            )]),
            None
        );
    }

    #[test]
    fn renders_apps_section_with_an_accessible_and_enabled_app() {
        let rendered = render_apps_section(&[connector(
            "calendar", /*is_accessible*/ true, /*is_enabled*/ true,
        )])
        .expect("expected apps section");

        assert!(rendered.starts_with(APPS_INSTRUCTIONS_OPEN_TAG));
        assert!(rendered.contains("## Apps (Connectors)"));
        assert!(rendered.ends_with(APPS_INSTRUCTIONS_CLOSE_TAG));
    }
}
