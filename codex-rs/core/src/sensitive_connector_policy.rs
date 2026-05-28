use crate::connectors::AppInfo;

pub(crate) const FINANCES_CONNECTOR_ID: &str = "connector_693864f100e4819093e6ed9b651239f1";
pub(crate) const USED_CONNECTOR_IDS_META_KEY: &str = "used_connector_ids";

pub(crate) fn used_finances_connector(used_connector_ids: &[String]) -> bool {
    used_connector_ids
        .iter()
        .any(|connector_id| connector_id == FINANCES_CONNECTOR_ID)
}

pub(crate) fn connector_allowed_after_sensitive_usage(
    used_connector_ids: &[String],
    connector_id: &str,
) -> bool {
    !used_finances_connector(used_connector_ids) || connector_id == FINANCES_CONNECTOR_ID
}

pub(crate) fn filter_connectors_after_sensitive_usage(
    connectors: Vec<AppInfo>,
    used_connector_ids: &[String],
) -> Vec<AppInfo> {
    if !used_finances_connector(used_connector_ids) {
        return connectors;
    }

    connectors
        .into_iter()
        .filter(|connector| connector.id == FINANCES_CONNECTOR_ID)
        .collect()
}

pub(crate) fn append_used_connector_id(used_connector_ids: &mut Vec<String>, connector_id: &str) {
    if !used_connector_ids
        .iter()
        .any(|used_connector_id| used_connector_id == connector_id)
    {
        used_connector_ids.push(connector_id.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finances_usage_limits_future_connector_ids_to_finances() {
        let used_connector_ids = vec![FINANCES_CONNECTOR_ID.to_string()];

        assert!(connector_allowed_after_sensitive_usage(
            &used_connector_ids,
            FINANCES_CONNECTOR_ID
        ));
        assert!(!connector_allowed_after_sensitive_usage(
            &used_connector_ids,
            "connector_calendar"
        ));
    }

    #[test]
    fn non_finances_usage_does_not_limit_future_connector_ids() {
        let used_connector_ids = vec!["connector_calendar".to_string()];

        assert!(connector_allowed_after_sensitive_usage(
            &used_connector_ids,
            "connector_drive"
        ));
    }
}
