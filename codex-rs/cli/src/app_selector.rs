use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopAppSelector {
    BundleId(String),
    AppPath(PathBuf),
}

pub fn app_selector_from_options(
    bundle_id: Option<String>,
    app_path: Option<PathBuf>,
) -> Result<Option<DesktopAppSelector>, String> {
    match (bundle_id, app_path) {
        (Some(bundle_id), None) => {
            let bundle_id = bundle_id.trim();
            if bundle_id.is_empty() {
                return Err("--bundle-id must not be empty".to_string());
            }
            Ok(Some(DesktopAppSelector::BundleId(bundle_id.to_string())))
        }
        (None, Some(app_path)) => Ok(Some(DesktopAppSelector::AppPath(app_path))),
        (None, None) => Ok(None),
        (Some(_), Some(_)) => Err("--bundle-id and --app-path cannot be used together".to_string()),
    }
}

pub fn validate_download_url_selector_combination(
    selector: &Option<DesktopAppSelector>,
    download_url_override: &Option<String>,
) -> Result<(), String> {
    if selector.is_some() && download_url_override.is_some() {
        Err("--download-url cannot be used with --bundle-id or --app-path".to_string())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::DesktopAppSelector;
    use super::app_selector_from_options;
    use super::validate_download_url_selector_combination;
    use std::path::PathBuf;

    #[test]
    fn selects_bundle_id_route() {
        assert_eq!(
            app_selector_from_options(Some("com.openai.codex.nightly".to_string()), None).unwrap(),
            Some(DesktopAppSelector::BundleId(
                "com.openai.codex.nightly".to_string()
            ))
        );
    }

    #[test]
    fn selects_app_path_route() {
        assert_eq!(
            app_selector_from_options(
                None,
                Some(PathBuf::from("/Applications/Codex (Nightly).app"))
            )
            .unwrap(),
            Some(DesktopAppSelector::AppPath(PathBuf::from(
                "/Applications/Codex (Nightly).app"
            )))
        );
    }

    #[test]
    fn rejects_empty_bundle_id_selector() {
        assert_eq!(
            app_selector_from_options(Some("   ".to_string()), None).unwrap_err(),
            "--bundle-id must not be empty"
        );
    }

    #[test]
    fn rejects_conflicting_selectors() {
        assert_eq!(
            app_selector_from_options(
                Some("com.openai.codex.nightly".to_string()),
                Some(PathBuf::from("/Applications/Codex (Nightly).app"))
            )
            .unwrap_err(),
            "--bundle-id and --app-path cannot be used together"
        );
    }

    #[test]
    fn rejects_download_url_with_selector() {
        let selector = Some(DesktopAppSelector::BundleId(
            "com.openai.codex.nightly".to_string(),
        ));
        assert_eq!(
            validate_download_url_selector_combination(
                &selector,
                &Some("https://example.test/Codex.dmg".to_string())
            )
            .unwrap_err(),
            "--download-url cannot be used with --bundle-id or --app-path"
        );
    }
}
