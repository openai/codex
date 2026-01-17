use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

/// User-defined account name used to scope credentials storage.
///
/// Constraints:
/// - ASCII only
/// - Allowed characters: `A-Z a-z 0-9 _ -`
/// - Length: 1..=64
pub fn validate_account_name(name: &str) -> std::io::Result<()> {
    if name.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "account name cannot be empty",
        ));
    }
    if name.len() > 64 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "account name is too long (max 64 chars)",
        ));
    }
    if !name
        .bytes()
        .all(|b| matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-'))
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid account name; use only [A-Za-z0-9_-]",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AccountKind {
    Chatgpt,
    ApiKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<AccountKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsFile {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_account: Option<String>,
    #[serde(default)]
    pub accounts: BTreeMap<String, AccountMeta>,
}

impl Default for AccountsFile {
    fn default() -> Self {
        Self {
            version: 1,
            active_account: None,
            accounts: BTreeMap::new(),
        }
    }
}

pub const ACCOUNTS_FILE_NAME: &str = "accounts.json";
pub const ACCOUNTS_AUTH_DIR_NAME: &str = "accounts";

pub fn accounts_file_path(codex_home: &Path) -> PathBuf {
    codex_home.join(ACCOUNTS_FILE_NAME)
}

pub fn is_accounts_initialized(codex_home: &Path) -> bool {
    accounts_file_path(codex_home).exists()
}

pub fn load_accounts(codex_home: &Path) -> std::io::Result<Option<AccountsFile>> {
    let path = accounts_file_path(codex_home);
    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let parsed: AccountsFile = serde_json::from_str(&contents)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    Ok(Some(parsed))
}

pub fn save_accounts(codex_home: &Path, accounts: &AccountsFile) -> std::io::Result<()> {
    let path = accounts_file_path(codex_home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(accounts)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;

    let mut options = std::fs::OpenOptions::new();
    options.truncate(true).write(true).create(true);
    #[cfg(unix)]
    {
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    use std::io::Write as _;
    file.write_all(json.as_bytes())?;
    file.flush()?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountSummary {
    pub name: String,
    pub meta: AccountMeta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountsSnapshot {
    pub active_account: Option<String>,
    pub accounts: Vec<AccountSummary>,
}

pub fn list_accounts(codex_home: &Path) -> std::io::Result<AccountsSnapshot> {
    let loaded = load_accounts(codex_home)?.unwrap_or_default();
    let accounts = loaded
        .accounts
        .into_iter()
        .map(|(name, meta)| AccountSummary { name, meta })
        .collect();
    Ok(AccountsSnapshot {
        active_account: loaded.active_account,
        accounts,
    })
}

pub fn switch_account(
    codex_home: &Path,
    name: &str,
    create_if_missing: bool,
) -> std::io::Result<()> {
    validate_account_name(name)?;

    let mut loaded = load_accounts(codex_home)?.unwrap_or_default();
    if !loaded.accounts.contains_key(name) {
        if !create_if_missing {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("account `{name}` not found"),
            ));
        }
        loaded.accounts.insert(
            name.to_string(),
            AccountMeta {
                kind: None,
                email: None,
            },
        );
    }
    loaded.active_account = Some(name.to_string());
    save_accounts(codex_home, &loaded)?;
    Ok(())
}

/// Resolve the account scope for auth operations.
///
/// - When `accounts.json` does not exist: legacy single-account mode.
/// - When `accounts.json` exists: an active account must be selected.
pub fn resolve_active_account(codex_home: &Path) -> std::io::Result<Option<String>> {
    let Some(accounts) = load_accounts(codex_home)? else {
        return Ok(None);
    };
    let Some(active) = accounts.active_account else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no active account selected",
        ));
    };
    validate_account_name(&active)?;
    Ok(Some(active))
}

pub fn update_account_meta(
    codex_home: &Path,
    account_name: &str,
    kind: AccountKind,
    email: Option<String>,
) -> std::io::Result<()> {
    validate_account_name(account_name)?;
    let mut loaded = load_accounts(codex_home)?.unwrap_or_default();
    let meta = loaded
        .accounts
        .entry(account_name.to_string())
        .or_insert_with(|| AccountMeta {
            kind: None,
            email: None,
        });
    meta.kind = Some(kind);
    meta.email = email;
    if loaded.active_account.as_deref() == Some(account_name) {
        // Keep as-is
    }
    save_accounts(codex_home, &loaded)?;
    Ok(())
}
