use std::collections::HashMap;
use std::sync::PoisonError;
use std::sync::RwLock;

use crate::exec_env::CODEX_THREAD_ID_ENV_VAR;

/// Session-owned additive environment changes emitted by `SessionStart` hooks.
#[derive(Default)]
pub(crate) struct SessionStartEnvOverlay {
    values: RwLock<HashMap<String, String>>,
}

impl SessionStartEnvOverlay {
    pub(crate) fn new(values: HashMap<String, String>) -> Self {
        Self {
            values: RwLock::new(values),
        }
    }

    pub(crate) fn merge(&self, values: HashMap<String, String>) {
        let mut current = self.values.write().unwrap_or_else(PoisonError::into_inner);
        for (key, value) in values {
            #[cfg(windows)]
            if let Some(existing) = current
                .keys()
                .find(|candidate| candidate.eq_ignore_ascii_case(&key))
                .cloned()
            {
                current.remove(&existing);
            }
            current.insert(key, value);
        }
    }

    pub(crate) fn snapshot(&self) -> HashMap<String, String> {
        self.values
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn apply(&self, env: &mut HashMap<String, String>) {
        for (key, value) in self
            .values
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
        {
            if key == CODEX_THREAD_ID_ENV_VAR
                || cfg!(windows) && key.eq_ignore_ascii_case(CODEX_THREAD_ID_ENV_VAR)
            {
                continue;
            }
            #[cfg(windows)]
            if let Some(existing) = env
                .keys()
                .find(|candidate| candidate.eq_ignore_ascii_case(key))
                .cloned()
            {
                env.remove(&existing);
            }
            env.insert(key.clone(), value.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn applies_hook_env_with_runtime_precedence() {
        let overlay = SessionStartEnvOverlay::default();
        overlay.merge(HashMap::from([
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
    fn merges_later_session_start_env_additively() {
        let overlay = SessionStartEnvOverlay::new(HashMap::from([
            ("PATH".to_string(), "/startup/bin".to_string()),
            ("STARTUP_ONLY".to_string(), "1".to_string()),
        ]));

        overlay.merge(HashMap::from([
            ("PATH".to_string(), "/compact/bin".to_string()),
            ("COMPACT_ONLY".to_string(), "1".to_string()),
        ]));

        assert_eq!(
            overlay.snapshot(),
            HashMap::from([
                ("PATH".to_string(), "/compact/bin".to_string()),
                ("STARTUP_ONLY".to_string(), "1".to_string()),
                ("COMPACT_ONLY".to_string(), "1".to_string()),
            ])
        );
    }
}
