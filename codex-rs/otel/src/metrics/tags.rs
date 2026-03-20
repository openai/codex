use crate::metrics::Result;
use crate::metrics::validation::validate_tag_key;
use crate::metrics::validation::validate_tag_value;

pub const APP_VERSION_TAG: &str = "app.version";
pub const AUTH_MODE_TAG: &str = "auth_mode";
pub const MODEL_TAG: &str = "model";
pub const ORIGINATOR_TAG: &str = "originator";
pub const SERVICE_NAME_TAG: &str = "service_name";
pub const SESSION_SOURCE_TAG: &str = "session_source";

pub struct SessionMetricTagValues<'a> {
    pub auth_mode: Option<&'a str>,
    pub session_source: &'a str,
    pub originator: &'a str,
    pub service_name: Option<&'a str>,
    pub model: &'a str,
    pub app_version: &'a str,
}

impl<'a> SessionMetricTagValues<'a> {
    pub fn into_tags(self) -> Result<Vec<(&'static str, &'a str)>> {
        let mut tags = Vec::with_capacity(6);
        Self::push_optional_tag(&mut tags, AUTH_MODE_TAG, self.auth_mode)?;
        Self::push_optional_tag(&mut tags, SESSION_SOURCE_TAG, Some(self.session_source))?;
        Self::push_optional_tag(&mut tags, ORIGINATOR_TAG, Some(self.originator))?;
        Self::push_optional_tag(&mut tags, SERVICE_NAME_TAG, self.service_name)?;
        Self::push_optional_tag(&mut tags, MODEL_TAG, Some(self.model))?;
        Self::push_optional_tag(&mut tags, APP_VERSION_TAG, Some(self.app_version))?;
        Ok(tags)
    }

    fn push_optional_tag(
        tags: &mut Vec<(&'static str, &'a str)>,
        key: &'static str,
        value: Option<&'a str>,
    ) -> Result<()> {
        let Some(value) = value else {
            return Ok(());
        };
        validate_tag_key(key)?;
        validate_tag_value(value)?;
        tags.push((key, value));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::APP_VERSION_TAG;
    use super::AUTH_MODE_TAG;
    use super::MODEL_TAG;
    use super::ORIGINATOR_TAG;
    use super::SERVICE_NAME_TAG;
    use super::SESSION_SOURCE_TAG;
    use super::SessionMetricTagValues;
    use pretty_assertions::assert_eq;

    #[test]
    fn session_metric_tags_include_expected_tags_in_order() {
        let tags = SessionMetricTagValues {
            auth_mode: Some("api_key"),
            session_source: "cli",
            originator: "codex_cli",
            service_name: Some("desktop_app"),
            model: "gpt-5.1",
            app_version: "1.2.3",
        }
        .into_tags()
        .expect("tags");

        assert_eq!(
            tags,
            vec![
                (AUTH_MODE_TAG, "api_key"),
                (SESSION_SOURCE_TAG, "cli"),
                (ORIGINATOR_TAG, "codex_cli"),
                (SERVICE_NAME_TAG, "desktop_app"),
                (MODEL_TAG, "gpt-5.1"),
                (APP_VERSION_TAG, "1.2.3"),
            ]
        );
    }

    #[test]
    fn session_metric_tags_skip_missing_optional_tags() {
        let tags = SessionMetricTagValues {
            auth_mode: None,
            session_source: "exec",
            originator: "codex_exec",
            service_name: None,
            model: "gpt-5.1",
            app_version: "1.2.3",
        }
        .into_tags()
        .expect("tags");

        assert_eq!(
            tags,
            vec![
                (SESSION_SOURCE_TAG, "exec"),
                (ORIGINATOR_TAG, "codex_exec"),
                (MODEL_TAG, "gpt-5.1"),
                (APP_VERSION_TAG, "1.2.3"),
            ]
        );
    }
}
