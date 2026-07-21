use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EnvironmentNameSemantics {
    CaseSensitive,
    WindowsAsciiCaseInsensitive,
}

impl EnvironmentNameSemantics {
    const fn for_target() -> Self {
        if cfg!(windows) {
            Self::WindowsAsciiCaseInsensitive
        } else {
            Self::CaseSensitive
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct EnvironmentOverrideError {
    first_key: String,
    second_key: String,
}

impl fmt::Display for EnvironmentOverrideError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "environment override keys '{}' and '{}' are equivalent on Windows",
            self.first_key, self.second_key
        )
    }
}

pub(super) fn apply_environment_overrides(
    env: &mut HashMap<String, String>,
    overrides: HashMap<String, Option<String>>,
) -> Result<(), EnvironmentOverrideError> {
    apply_environment_overrides_with_semantics(
        env,
        overrides,
        EnvironmentNameSemantics::for_target(),
    )
}

fn apply_environment_overrides_with_semantics(
    env: &mut HashMap<String, String>,
    overrides: HashMap<String, Option<String>>,
    semantics: EnvironmentNameSemantics,
) -> Result<(), EnvironmentOverrideError> {
    match semantics {
        EnvironmentNameSemantics::CaseSensitive => {
            for (key, value) in overrides {
                match value {
                    Some(value) => {
                        env.insert(key, value);
                    }
                    None => {
                        env.remove(&key);
                    }
                }
            }
        }
        EnvironmentNameSemantics::WindowsAsciiCaseInsensitive => {
            validate_windows_override_keys(&overrides)?;
            for (key, value) in overrides {
                env.retain(|existing_key, _| !existing_key.eq_ignore_ascii_case(&key));
                if let Some(value) = value {
                    env.insert(key, value);
                }
            }
        }
    }
    Ok(())
}

fn validate_windows_override_keys(
    overrides: &HashMap<String, Option<String>>,
) -> Result<(), EnvironmentOverrideError> {
    let mut keys = overrides.keys().collect::<Vec<_>>();
    keys.sort_unstable();
    for (index, first_key) in keys.iter().enumerate() {
        for second_key in &keys[index + 1..] {
            if first_key.eq_ignore_ascii_case(second_key) {
                return Err(EnvironmentOverrideError {
                    first_key: (*first_key).clone(),
                    second_key: (*second_key).clone(),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "environment_overrides_tests.rs"]
mod tests;
