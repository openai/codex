use std::collections::BTreeMap;
use std::env;

use super::DoctorCheck;
use super::LOCALE_ENV_VARS;

const EDITOR_ENV_VARS: &[&str] = &["VISUAL", "EDITOR"];
const PAGER_ENV_VARS: &[&str] = &["PAGER", "GIT_PAGER", "GH_PAGER", "LESS"];

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SystemCheckInputs {
    os: String,
    os_type: String,
    os_version: String,
    os_language: Option<String>,
    locale_env: BTreeMap<String, String>,
    editor_env: BTreeMap<String, String>,
    pager_env: BTreeMap<String, String>,
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
        let editor_env = EDITOR_ENV_VARS
            .iter()
            .map(|name| {
                let value = env::var_os(name)
                    .map(|value| value.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "not set".to_string());
                ((*name).to_string(), value)
            })
            .collect();
        let pager_env = PAGER_ENV_VARS
            .iter()
            .filter_map(|name| {
                env::var_os(name)
                    .map(|value| ((*name).to_string(), value.to_string_lossy().into_owned()))
            })
            .collect();
        Self {
            os: info.to_string(),
            os_type: info.os_type().to_string(),
            os_version: info.version().to_string(),
            os_language: sys_locale::get_locale(),
            locale_env,
            editor_env,
            pager_env,
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
    for name in EDITOR_ENV_VARS {
        if let Some(value) = inputs.editor_env.get(*name) {
            details.push(format!("{name}: {value}"));
        }
    }
    for name in PAGER_ENV_VARS {
        if let Some(value) = inputs.pager_env.get(*name) {
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
    fn system_check_reports_os_language_locale_editor_and_pager_env() {
        let mut locale_env = BTreeMap::new();
        locale_env.insert("LANG".to_string(), "en_US.UTF-8".to_string());
        let editor_env = BTreeMap::from([
            ("EDITOR".to_string(), "vim".to_string()),
            ("VISUAL".to_string(), "code --wait".to_string()),
        ]);
        let pager_env = BTreeMap::from([
            ("GH_PAGER".to_string(), "less".to_string()),
            ("GIT_PAGER".to_string(), "delta".to_string()),
            ("LESS".to_string(), "-FRX".to_string()),
            ("PAGER".to_string(), "less -R".to_string()),
        ]);
        let check = system_check_from_inputs(SystemCheckInputs {
            os: "macOS 15.0".to_string(),
            os_type: "macos".to_string(),
            os_version: "15.0".to_string(),
            os_language: Some("en-US".to_string()),
            locale_env,
            editor_env,
            pager_env,
        });

        assert_eq!(check.summary, "OS language en-US");
        assert_eq!(
            check.details,
            vec![
                "os: macOS 15.0",
                "os type: macos",
                "os version: 15.0",
                "os language: en-US",
                "LANG: en_US.UTF-8",
                "VISUAL: code --wait",
                "EDITOR: vim",
                "PAGER: less -R",
                "GIT_PAGER: delta",
                "GH_PAGER: less",
                "LESS: -FRX",
            ]
        );
    }

    #[test]
    fn system_check_handles_missing_os_language() {
        let check = system_check_from_inputs(SystemCheckInputs {
            os: "Linux".to_string(),
            os_type: "linux".to_string(),
            os_version: "unknown".to_string(),
            os_language: None,
            locale_env: BTreeMap::new(),
            editor_env: BTreeMap::from([
                ("EDITOR".to_string(), "not set".to_string()),
                ("VISUAL".to_string(), "not set".to_string()),
            ]),
            pager_env: BTreeMap::new(),
        });

        assert_eq!(check.summary, "OS language unavailable");
        assert_eq!(
            check.details,
            vec![
                "os: Linux",
                "os type: linux",
                "os version: unknown",
                "os language: unavailable",
                "VISUAL: not set",
                "EDITOR: not set",
            ]
        );
    }
}
