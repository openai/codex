use std::collections::HashMap;
use std::sync::PoisonError;
use std::sync::RwLock;

use crate::exec_env::CODEX_THREAD_ID_ENV_VAR;

/// Session-owned environment changes emitted by `SessionStart` hooks.
#[derive(Default)]
pub(crate) struct SessionStartEnvOverlay {
    values: RwLock<HashMap<String, String>>,
}

impl SessionStartEnvOverlay {
    pub(crate) fn replace(&self, values: HashMap<String, String>) {
        *self.values.write().unwrap_or_else(PoisonError::into_inner) = values;
    }

    pub(crate) fn apply(&self, env: &mut HashMap<String, String>) {
        for (key, value) in self
            .values
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
        {
            if is_runtime_owned(key) {
                continue;
            }
            insert_env_var(env, key.clone(), value.clone());
        }
    }

    pub(crate) fn extend_snapshot_overrides(&self, overrides: &mut HashMap<String, String>) {
        for (key, value) in self
            .values
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
        {
            if is_runtime_owned(key) {
                continue;
            }
            insert_env_var(overrides, key.clone(), value.clone());
        }
    }
}

fn is_runtime_owned(key: &str) -> bool {
    env_key_eq(key, CODEX_THREAD_ID_ENV_VAR)
}

fn insert_env_var(env: &mut HashMap<String, String>, key: String, value: String) {
    remove_env_var(env, &key);
    env.insert(key, value);
}

fn remove_env_var(env: &mut HashMap<String, String>, key: &str) {
    if let Some(existing) = env
        .keys()
        .find(|candidate| env_key_eq(candidate, key))
        .cloned()
    {
        env.remove(&existing);
    }
}

fn env_key_eq(candidate: &str, key: &str) -> bool {
    #[cfg(windows)]
    {
        candidate.eq_ignore_ascii_case(key)
    }

    #[cfg(not(windows))]
    {
        candidate == key
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn applies_hook_env_with_runtime_precedence() {
        let overlay = SessionStartEnvOverlay::default();
        overlay.replace(HashMap::from([
            ("CODEX_THREAD_ID".to_string(), "hook-thread".to_string()),
            ("PATH".to_string(), "/hook/bin".to_string()),
            ("SET_BY_POLICY".to_string(), "hook".to_string()),
        ]));
        let mut env = HashMap::from([
            ("CODEX_THREAD_ID".to_string(), "runtime-thread".to_string()),
            ("PATH".to_string(), "/usr/bin".to_string()),
            ("SET_BY_POLICY".to_string(), "policy".to_string()),
        ]);

        overlay.apply(&mut env);

        assert_eq!(
            env,
            HashMap::from([
                ("CODEX_THREAD_ID".to_string(), "runtime-thread".to_string()),
                ("PATH".to_string(), "/hook/bin".to_string()),
                ("SET_BY_POLICY".to_string(), "hook".to_string()),
            ])
        );
    }

    #[test]
    fn snapshot_overrides_include_hook_env() {
        let overlay = SessionStartEnvOverlay::default();
        overlay.replace(HashMap::from([(
            "PLUGIN_HOME".to_string(),
            "/plugin".to_string(),
        )]));
        let mut overrides = HashMap::new();

        overlay.extend_snapshot_overrides(&mut overrides);

        assert_eq!(
            overrides,
            HashMap::from([("PLUGIN_HOME".to_string(), "/plugin".to_string())])
        );
    }
}
