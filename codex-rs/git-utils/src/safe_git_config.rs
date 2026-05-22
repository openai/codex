use std::collections::BTreeSet;

pub const ATTRIBUTE_FILTER_CONFIG_REGEXP: &str = "^filter\\..*\\.(clean|smudge|process|required)$";

const DISABLED_HOOKS_PATH: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };

pub fn base_internal_git_config_overrides() -> Vec<String> {
    vec![
        "-c".to_string(),
        format!("core.hooksPath={DISABLED_HOOKS_PATH}"),
        "-c".to_string(),
        // Empty disables both legacy hook-backed and current fsmonitor behavior.
        "core.fsmonitor=".to_string(),
    ]
}

pub fn safe_attribute_filter_overrides_from_config_keys(stdout: &str) -> Vec<String> {
    let mut drivers = BTreeSet::new();
    for key in stdout.lines() {
        let Some(key) = key.strip_prefix("filter.") else {
            continue;
        };
        let Some((driver, setting)) = key.rsplit_once('.') else {
            continue;
        };
        if matches!(setting, "clean" | "smudge" | "process" | "required") {
            drivers.insert(driver);
        }
    }

    let mut config_overrides = vec![
        "-c".to_string(),
        "attr.tree=".to_string(),
        "-c".to_string(),
        "core.attributesFile=".to_string(),
    ];
    for driver in drivers {
        for (setting, value) in [
            ("clean", ""),
            ("smudge", ""),
            ("process", ""),
            ("required", "false"),
        ] {
            config_overrides.push("-c".to_string());
            config_overrides.push(format!("filter.{driver}.{setting}={value}"));
        }
    }
    config_overrides
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_filter_overrides_disable_each_configured_driver() {
        let overrides = safe_attribute_filter_overrides_from_config_keys(
            "filter.beta.process\nfilter.alpha.clean\nfilter.alpha.required\n",
        );

        assert!(overrides.contains(&"attr.tree=".to_string()));
        assert!(overrides.contains(&"core.attributesFile=".to_string()));
        assert!(overrides.contains(&"filter.alpha.clean=".to_string()));
        assert!(overrides.contains(&"filter.alpha.required=false".to_string()));
        assert!(overrides.contains(&"filter.beta.process=".to_string()));
    }
}
