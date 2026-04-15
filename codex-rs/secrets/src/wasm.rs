use anyhow::Result;

pub fn redact_secrets(text: &str) -> String {
    text.to_string()
}

pub fn environment_id_from_cwd(_cwd: &std::path::Path) -> String {
    "wasm".to_string()
}

pub fn keyring_service() -> &'static str {
    "codex"
}

pub fn compute_keyring_account(_codex_home: &std::path::Path) -> String {
    "secrets|wasm".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SecretName(String);

impl SecretName {
    pub fn new(raw: &str) -> Result<Self> {
        Ok(Self(raw.trim().to_string()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Display for SecretName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SecretScope {
    Global,
    Environment(String),
}

impl SecretScope {
    pub fn environment(environment_id: impl Into<String>) -> Result<Self> {
        Ok(Self::Environment(environment_id.into()))
    }

    pub fn canonical_key(&self, name: &SecretName) -> String {
        match self {
            Self::Global => format!("global/{}", name.as_str()),
            Self::Environment(environment_id) => {
                format!("env/{environment_id}/{}", name.as_str())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretListEntry {
    pub scope: SecretScope,
    pub name: SecretName,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
    Default,
)]
#[serde(rename_all = "lowercase")]
pub enum SecretsBackendKind {
    #[default]
    Local,
}

pub trait SecretsBackend: Send + Sync {
    fn set(&self, _scope: &SecretScope, _name: &SecretName, _value: &str) -> Result<()> {
        Ok(())
    }

    fn get(&self, _scope: &SecretScope, _name: &SecretName) -> Result<Option<String>> {
        Ok(None)
    }

    fn delete(&self, _scope: &SecretScope, _name: &SecretName) -> Result<bool> {
        Ok(false)
    }

    fn list(&self, _scope_filter: Option<&SecretScope>) -> Result<Vec<SecretListEntry>> {
        Ok(Vec::new())
    }
}

#[derive(Default, Clone)]
pub struct LocalSecretsBackend;

#[derive(Clone, Default)]
pub struct SecretsManager;

impl SecretsManager {
    pub fn new(_codex_home: std::path::PathBuf, _backend_kind: SecretsBackendKind) -> Self {
        Self
    }

    pub fn new_with_keyring_store(
        _codex_home: std::path::PathBuf,
        _backend_kind: SecretsBackendKind,
        _keyring_store: std::sync::Arc<dyn std::any::Any + Send + Sync>,
    ) -> Self {
        Self
    }

    pub fn set(&self, _scope: &SecretScope, _name: &SecretName, _value: &str) -> Result<()> {
        Ok(())
    }

    pub fn get(&self, _scope: &SecretScope, _name: &SecretName) -> Result<Option<String>> {
        Ok(None)
    }

    pub fn delete(&self, _scope: &SecretScope, _name: &SecretName) -> Result<bool> {
        Ok(false)
    }

    pub fn list(&self, _scope_filter: Option<&SecretScope>) -> Result<Vec<SecretListEntry>> {
        Ok(Vec::new())
    }
}
