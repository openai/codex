use std::collections::BTreeMap;
use std::env;

use super::DoctorCheck;
use super::LOCALE_ENV_VARS;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SystemCheckInputs {
    os: String,
    os_type: String,
    os_version: String,
    os_language: Option<String>,
    locale_env: BTreeMap<String, String>,
}

impl SystemCheckInputs {
    fn detect() -> Self {
        let info = os_info::get();
        let locale_env = LOCALE_ENV_VARS
            .iter()
            .filter_map(|name| {
                env::var(name)
                    .ok()
                    .map(|value| ((*name).to_string(), value))
            })
            .collect();
        Self {
            os: info.to_string(),
            os_type: info.os_type().to_string(),
            os_version: info.version().to_string(),
            os_language: sys_locale::get_locale(),
            locale_env,
        }
    }
}

pub(super) fn system_check() -> DoctorCheck {
    system_check_from_inputs(SystemCheckInputs::detect())
}

fn system_check_from_inputs(inputs: SystemCheckInputs) -> DoctorCheck {
    let mut details = vec![
        format!("os: {}", inputs.os),
        format!("os type: {}", inputs.os_type),
        format!("os version: {}", inputs.os_version),
    ];
    if let Some(language) = inputs.os_language.as_deref() {
        details.push(format!("os language: {language}"));
    } else {
        details.push("os language: unavailable".to_string());
    }
    for name in LOCALE_ENV_VARS {
        if let Some(value) = inputs.locale_env.get(*name) {
            details.push(format!("{name}: {value}"));
        }
    }

    let summary = inputs
        .os_language
        .as_deref()
        .map(|language| format!("OS language {language}"))
        .unwrap_or_else(|| "OS language unavailable".to_string());
    DoctorCheck::new(
        "system.environment",
        "system",
        super::CheckStatus::Ok,
        summary,
    )
    .details(details)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn system_check_reports_os_language_and_locale_env() {
        let mut locale_env = BTreeMap::new();
        locale_env.insert("LANG".to_string(), "en_US.UTF-8".to_string());
        let check = system_check_from_inputs(SystemCheckInputs {
            os: "macOS 15.0".to_string(),
            os_type: "macos".to_string(),
            os_version: "15.0".to_string(),
            os_language: Some("en-US".to_string()),
            locale_env,
        });

        assert_eq!(check.summary, "OS language en-US");
        assert!(check.details.contains(&"os language: en-US".to_string()));
        assert!(check.details.contains(&"LANG: en_US.UTF-8".to_string()));
    }

    #[test]
    fn system_check_handles_missing_os_language() {
        let check = system_check_from_inputs(SystemCheckInputs {
            os: "Linux".to_string(),
            os_type: "linux".to_string(),
            os_version: "unknown".to_string(),
            os_language: None,
            locale_env: BTreeMap::new(),
        });

        assert_eq!(check.summary, "OS language unavailable");
        assert!(
            check
                .details
                .contains(&"os language: unavailable".to_string())
        );
    }
}
