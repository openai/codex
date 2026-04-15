use anyhow::{anyhow, Result};
use dirs_next::home_dir;
use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

fn env_key_eq(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

fn has_env_key(env_map: &HashMap<String, String>, key: &str) -> bool {
    env_map.keys().any(|existing| env_key_eq(existing, key))
}

fn inherit_env_var_with<F>(env_map: &mut HashMap<String, String>, key: &str, lookup: F)
where
    F: FnOnce(&str) -> Option<String>,
{
    if !has_env_key(env_map, key)
        && let Some(value) = lookup(key)
    {
        env_map.insert(key.to_string(), value);
    }
}

pub fn normalize_null_device_env(env_map: &mut HashMap<String, String>) {
    let keys: Vec<String> = env_map.keys().cloned().collect();
    for k in keys {
        if let Some(v) = env_map.get(&k).cloned() {
            let t = v.trim().to_ascii_lowercase();
            if t == "/dev/null" || t == "\\\\\\\\dev\\\\\\\\null" {
                env_map.insert(k, "NUL".to_string());
            }
        }
    }
}

pub fn ensure_non_interactive_pager(env_map: &mut HashMap<String, String>) {
    env_map
        .entry("GIT_PAGER".into())
        .or_insert_with(|| "more.com".into());
    env_map
        .entry("PAGER".into())
        .or_insert_with(|| "more.com".into());
    env_map.entry("LESS".into()).or_insert_with(|| "".into());
}

// Keep PATH and PATHEXT stable for callers that rely on inheriting the parent process env.
pub fn inherit_path_env(env_map: &mut HashMap<String, String>) {
    inherit_env_var_with(env_map, "PATH", |key| env::var(key).ok());
    inherit_env_var_with(env_map, "PATHEXT", |key| env::var(key).ok());
}

fn inherit_windows_bootstrap_env_with<F>(env_map: &mut HashMap<String, String>, mut lookup: F)
where
    F: FnMut(&str) -> Option<String>,
{
    for key in [
        "PATH",
        "PATHEXT",
        "SystemRoot",
        "WINDIR",
        "ComSpec",
        "TEMP",
        "TMP",
        "USERPROFILE",
        "HOMEDRIVE",
        "HOMEPATH",
    ] {
        inherit_env_var_with(env_map, key, |name| lookup(name));
    }
}

pub fn inherit_windows_bootstrap_env(env_map: &mut HashMap<String, String>) {
    inherit_windows_bootstrap_env_with(env_map, |key| env::var(key).ok());
}

fn prepend_path(env_map: &mut HashMap<String, String>, prefix: &str) {
    let existing = env_map
        .get("PATH")
        .cloned()
        .or_else(|| env::var("PATH").ok())
        .unwrap_or_default();
    let parts: Vec<String> = existing.split(';').map(ToString::to_string).collect();
    if parts
        .first()
        .map(|p| p.eq_ignore_ascii_case(prefix))
        .unwrap_or(false)
    {
        return;
    }
    let mut new_path = String::new();
    new_path.push_str(prefix);
    if !existing.is_empty() {
        new_path.push(';');
        new_path.push_str(&existing);
    }
    env_map.insert("PATH".into(), new_path);
}

fn reorder_pathext_for_stubs(env_map: &mut HashMap<String, String>) {
    let default = env_map
        .get("PATHEXT")
        .cloned()
        .or_else(|| env::var("PATHEXT").ok())
        .unwrap_or(".COM;.EXE;.BAT;.CMD".to_string());
    let exts: Vec<String> = default
        .split(';')
        .filter(|e| !e.is_empty())
        .map(ToString::to_string)
        .collect();
    let exts_norm: Vec<String> = exts.iter().map(|e| e.to_ascii_uppercase()).collect();
    let want = [".BAT", ".CMD"];
    let mut front: Vec<String> = Vec::new();
    for w in want {
        if let Some(idx) = exts_norm.iter().position(|e| e == w) {
            front.push(exts[idx].clone());
        }
    }
    let rest: Vec<String> = exts
        .into_iter()
        .enumerate()
        .filter(|(i, _)| {
            let up = &exts_norm[*i];
            up != ".BAT" && up != ".CMD"
        })
        .map(|(_, e)| e)
        .collect();
    let mut combined = Vec::new();
    combined.extend(front);
    combined.extend(rest);
    env_map.insert("PATHEXT".into(), combined.join(";"));
}

fn ensure_denybin(tools: &[&str], denybin_dir: Option<&Path>) -> Result<PathBuf> {
    let base = match denybin_dir {
        Some(p) => p.to_path_buf(),
        None => {
            let home = home_dir().ok_or_else(|| anyhow!("no home dir"))?;
            home.join(".sbx-denybin")
        }
    };
    fs::create_dir_all(&base)?;
    for tool in tools {
        for ext in [".bat", ".cmd"] {
            let path = base.join(format!("{tool}{ext}"));
            if !path.exists() {
                let mut f = File::create(&path)?;
                f.write_all(b"@echo off\\r\\nexit /b 1\\r\\n")?;
            }
        }
    }
    Ok(base)
}

pub fn apply_no_network_to_env(env_map: &mut HashMap<String, String>) -> Result<()> {
    env_map.insert("SBX_NONET_ACTIVE".into(), "1".into());
    env_map
        .entry("HTTP_PROXY".into())
        .or_insert_with(|| "http://127.0.0.1:9".into());
    env_map
        .entry("HTTPS_PROXY".into())
        .or_insert_with(|| "http://127.0.0.1:9".into());
    env_map
        .entry("ALL_PROXY".into())
        .or_insert_with(|| "http://127.0.0.1:9".into());
    env_map
        .entry("NO_PROXY".into())
        .or_insert_with(|| "localhost,127.0.0.1,::1".into());
    env_map
        .entry("PIP_NO_INDEX".into())
        .or_insert_with(|| "1".into());
    env_map
        .entry("PIP_DISABLE_PIP_VERSION_CHECK".into())
        .or_insert_with(|| "1".into());
    env_map
        .entry("NPM_CONFIG_OFFLINE".into())
        .or_insert_with(|| "true".into());
    env_map
        .entry("CARGO_NET_OFFLINE".into())
        .or_insert_with(|| "true".into());
    env_map
        .entry("GIT_HTTP_PROXY".into())
        .or_insert_with(|| "http://127.0.0.1:9".into());
    env_map
        .entry("GIT_HTTPS_PROXY".into())
        .or_insert_with(|| "http://127.0.0.1:9".into());
    env_map
        .entry("GIT_SSH_COMMAND".into())
        .or_insert_with(|| "cmd /c exit 1".into());
    env_map
        .entry("GIT_ALLOW_PROTOCOLS".into())
        .or_insert_with(|| "".into());

    let base = ensure_denybin(&["ssh", "scp"], /*denybin_dir*/ None)?;
    for tool in ["curl", "wget"] {
        for ext in [".bat", ".cmd"] {
            let p = base.join(format!("{tool}{ext}"));
            if p.exists() {
                let _ = fs::remove_file(&p);
            }
        }
    }
    prepend_path(env_map, &base.to_string_lossy());
    reorder_pathext_for_stubs(env_map);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::inherit_windows_bootstrap_env_with;
    use std::collections::HashMap;

    #[test]
    fn inherit_windows_bootstrap_env_fills_missing_bootstrap_vars() {
        let mut env_map = HashMap::from([("FOO".to_string(), "bar".to_string())]);
        inherit_windows_bootstrap_env_with(&mut env_map, |key| match key {
            "PATH" => Some(r"C:\Windows\System32".to_string()),
            "PATHEXT" => Some(".COM;.EXE;.BAT;.CMD".to_string()),
            "SystemRoot" => Some(r"C:\Windows".to_string()),
            "WINDIR" => Some(r"C:\Windows".to_string()),
            "ComSpec" => Some(r"C:\Windows\System32\cmd.exe".to_string()),
            "TEMP" => Some(r"C:\Temp".to_string()),
            "TMP" => Some(r"C:\Temp".to_string()),
            "USERPROFILE" => Some(r"C:\Users\codex".to_string()),
            "HOMEDRIVE" => Some("C:".to_string()),
            "HOMEPATH" => Some(r"\Users\codex".to_string()),
            _ => None,
        });

        assert_eq!(
            env_map.get("SystemRoot").map(String::as_str),
            Some(r"C:\Windows")
        );
        assert_eq!(
            env_map.get("ComSpec").map(String::as_str),
            Some(r"C:\Windows\System32\cmd.exe")
        );
        assert_eq!(env_map.get("TEMP").map(String::as_str), Some(r"C:\Temp"));
        assert_eq!(
            env_map.get("USERPROFILE").map(String::as_str),
            Some(r"C:\Users\codex")
        );
    }

    #[test]
    fn inherit_windows_bootstrap_env_respects_existing_case_insensitive_entries() {
        let mut env_map = HashMap::from([
            ("Path".to_string(), "custom-path".to_string()),
            ("systemroot".to_string(), "custom-root".to_string()),
        ]);
        inherit_windows_bootstrap_env_with(&mut env_map, |key| match key {
            "PATH" => Some("fallback-path".to_string()),
            "SystemRoot" => Some("fallback-root".to_string()),
            _ => None,
        });

        assert_eq!(env_map.get("Path").map(String::as_str), Some("custom-path"));
        assert_eq!(
            env_map.get("systemroot").map(String::as_str),
            Some("custom-root")
        );
        assert!(!env_map.contains_key("PATH"));
        assert!(!env_map.contains_key("SystemRoot"));
    }
}
