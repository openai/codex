use serde::Deserialize;

/// Managed requirements for the interactive in-app browser, not agent Browser Use.
#[derive(Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct InAppBrowserRequirementsToml {
    /// Whether external browser settings may be imported. Only `Some(false)`
    /// denies import; omission is effectively `true`. Keep omission unset while
    /// composing managed layers so it cannot replace an explicit requirement.
    pub allow_external_browser_settings_import: Option<bool>,
}

#[cfg(test)]
#[path = "in_app_browser_requirements_tests.rs"]
mod tests;
