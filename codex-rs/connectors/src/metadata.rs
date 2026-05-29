use codex_app_server_protocol::AppInfo;

pub fn connector_display_label(connector: &AppInfo) -> String {
    connector.name.clone()
}

pub fn connector_mention_slug(connector: &AppInfo) -> String {
    crate::connector_name_slug(&connector_display_label(connector))
}

pub fn connector_install_url(name: &str, connector_id: &str) -> String {
    crate::connector_install_url(name, connector_id)
}

pub fn sanitize_name(name: &str) -> String {
    crate::connector_name_slug(name).replace("-", "_")
}

pub(crate) fn sort_connectors_by_accessibility_and_name(connectors: &mut [AppInfo]) {
    connectors.sort_by(|left, right| {
        right
            .is_accessible
            .cmp(&left.is_accessible)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.id.cmp(&right.id))
    });
}
