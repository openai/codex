use codex_protocol::config_types::EnvironmentVariablePattern;
use codex_protocol::config_types::ShellEnvironmentPolicy;
use codex_protocol::config_types::ShellEnvironmentPolicyFilter;
use codex_protocol::config_types::ShellEnvironmentPolicyInherit;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::HashMap;
use toml::Value as TomlValue;

/// Policy for building the `env` when spawning a process via shell-like tools.
#[derive(Serialize, Debug, Clone, PartialEq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ShellEnvironmentPolicyToml {
    pub inherit: Option<ShellEnvironmentPolicyInherit>,

    pub ignore_default_excludes: Option<bool>,

    /// Legacy list of regular expressions to exclude.
    pub exclude: Option<Vec<String>>,

    pub r#set: Option<HashMap<String, String>>,

    /// Legacy list of regular expressions to include.
    pub include_only: Option<Vec<String>>,

    /// Pattern actions used by the canonical table representation.
    ///
    /// Ordinary config keeps accepting the legacy arrays above during the
    /// migration. Requirements will accept only this keyed form, keeping array
    /// compatibility isolated so the legacy fields can be deprecated later.
    /// Pattern keys merge case-insensitively across config layers, matching how
    /// the resulting patterns match environment variable names.
    pub filters: Option<BTreeMap<String, ShellEnvironmentPolicyFilter>>,

    pub experimental_use_profile: Option<bool>,
}

#[derive(Deserialize)]
struct ShellEnvironmentPolicyTomlRaw {
    inherit: Option<ShellEnvironmentPolicyInherit>,
    ignore_default_excludes: Option<bool>,
    exclude: Option<Vec<String>>,
    r#set: Option<HashMap<String, String>>,
    include_only: Option<Vec<String>>,
    filters: Option<BTreeMap<String, ShellEnvironmentPolicyFilter>>,
    experimental_use_profile: Option<bool>,
}

#[derive(Deserialize)]
pub(crate) struct ShellEnvironmentPolicyFilterConfigToml {
    #[serde(
        default,
        rename = "shell_environment_policy",
        deserialize_with = "deserialize_shell_environment_policy_filters"
    )]
    _shell_environment_policy: (),
}

/// Validates only the shell-environment filter representation in a raw config overlay.
pub fn validate_shell_environment_policy_filter_config(
    value: &TomlValue,
) -> Result<(), toml::de::Error> {
    let _: ShellEnvironmentPolicyFilterConfigToml = value.clone().try_into()?;
    Ok(())
}

fn deserialize_shell_environment_policy_filters<'de, D>(deserializer: D) -> Result<(), D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = TomlValue::deserialize(deserializer)?;
    let Some(policy) = value.as_table() else {
        return Ok(());
    };
    let filter_fields = ["exclude", "include_only", "filters"]
        .into_iter()
        .filter_map(|field| {
            policy
                .get(field)
                .cloned()
                .map(|value| (field.to_string(), value))
        })
        .collect();
    let _: ShellEnvironmentPolicyToml = TomlValue::Table(filter_fields)
        .try_into()
        .map_err(|error: toml::de::Error| serde::de::Error::custom(error.message()))?;
    Ok(())
}

impl<'de> Deserialize<'de> for ShellEnvironmentPolicyToml {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let ShellEnvironmentPolicyTomlRaw {
            inherit,
            ignore_default_excludes,
            exclude,
            r#set,
            include_only,
            filters,
            experimental_use_profile,
        } = ShellEnvironmentPolicyTomlRaw::deserialize(deserializer)?;
        if filters.is_some() && (exclude.is_some() || include_only.is_some()) {
            return Err(serde::de::Error::custom(
                "cannot mix `filters` with legacy `exclude` or `include_only`",
            ));
        }
        if let Some(filters) = filters.as_ref() {
            let mut patterns = std::collections::HashSet::new();
            for pattern in filters.keys() {
                if !patterns.insert(pattern.to_lowercase()) {
                    return Err(serde::de::Error::custom(format!(
                        "duplicate shell environment filter `{pattern}` ignoring case"
                    )));
                }
            }
        }
        Ok(Self {
            inherit,
            ignore_default_excludes,
            exclude,
            r#set,
            include_only,
            filters,
            experimental_use_profile,
        })
    }
}

impl From<ShellEnvironmentPolicyToml> for ShellEnvironmentPolicy {
    fn from(toml: ShellEnvironmentPolicyToml) -> Self {
        let inherit = toml.inherit.unwrap_or(ShellEnvironmentPolicyInherit::All);
        let ignore_default_excludes = toml.ignore_default_excludes.unwrap_or(true);
        let (exclude, include_only) = match toml.filters {
            Some(filters) => filters.into_iter().fold(
                (Vec::new(), Vec::new()),
                |(mut exclude, mut include_only), (pattern, filter)| {
                    match filter {
                        ShellEnvironmentPolicyFilter::Include => include_only.push(pattern),
                        ShellEnvironmentPolicyFilter::Exclude => exclude.push(pattern),
                    }
                    (exclude, include_only)
                },
            ),
            None => (
                toml.exclude.unwrap_or_default(),
                toml.include_only.unwrap_or_default(),
            ),
        };

        Self {
            inherit,
            ignore_default_excludes,
            exclude: exclude
                .into_iter()
                .map(|pattern| EnvironmentVariablePattern::new_case_insensitive(&pattern))
                .collect(),
            r#set: toml.r#set.unwrap_or_default(),
            include_only: include_only
                .into_iter()
                .map(|pattern| EnvironmentVariablePattern::new_case_insensitive(&pattern))
                .collect(),
            use_profile: toml.experimental_use_profile.unwrap_or(false),
        }
    }
}

#[cfg(test)]
#[path = "shell_environment_policy_tests.rs"]
mod tests;
