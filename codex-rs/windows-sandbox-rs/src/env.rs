use anyhow::{anyhow, Result};
use dirs_next::home_dir;
use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

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
    if !env_map.contains_key("PATH")
        && let Ok(path) = env::var("PATH")
    {
        env_map.insert("PATH".into(), path);
    }
    if !env_map.contains_key("PATHEXT")
        && let Ok(pathext) = env::var("PATHEXT")
    {
        env_map.insert("PATHEXT".into(), pathext);
    }
}

fn normalize_windows_path_for_compare(path: &str) -> String {
    let normalized = path.trim().replace('/', "\\");
    if normalized.len() <= 3 {
        return normalized.to_ascii_lowercase();
    }
    normalized
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

fn path_is_within_windows_root(path: &str, root: &str) -> bool {
    let path = normalize_windows_path_for_compare(path);
    let root = normalize_windows_path_for_compare(root);
    if root.is_empty() || !path.starts_with(&root) {
        return false;
    }
    path.len() == root.len() || path.as_bytes().get(root.len()) == Some(&b'\\')
}

fn split_windows_drive(path: &str) -> Option<(String, String)> {
    if path.as_bytes().get(1) != Some(&b':') {
        return None;
    }
    let drive = path[..2].to_string();
    let rest = match path.get(2..) {
        Some("") | None => "\\".to_string(),
        Some(rest) => rest.to_string(),
    };
    Some((drive, rest))
}

fn default_roaming_appdata(userprofile: &str) -> String {
    format!(r"{userprofile}\AppData\Roaming")
}

fn default_local_appdata(userprofile: &str) -> String {
    format!(r"{userprofile}\AppData\Local")
}

fn sanitize_windows_profile_env_with_current(
    env_map: &mut HashMap<String, String>,
    current_userprofile: Option<String>,
    current_appdata: Option<String>,
    current_localappdata: Option<String>,
    current_temp: Option<String>,
    current_tmp: Option<String>,
) {
    let Some(current_userprofile) = current_userprofile.filter(|value| !value.trim().is_empty())
    else {
        return;
    };

    let original_userprofile = env_map.get("USERPROFILE").cloned();
    let appdata = current_appdata.unwrap_or_else(|| default_roaming_appdata(&current_userprofile));
    let localappdata =
        current_localappdata.unwrap_or_else(|| default_local_appdata(&current_userprofile));

    env_map.insert("USERPROFILE".into(), current_userprofile.clone());
    env_map.insert("HOME".into(), current_userprofile.clone());
    if let Some((drive, homedir)) = split_windows_drive(&current_userprofile) {
        env_map.insert("HOMEDRIVE".into(), drive);
        env_map.insert("HOMEPATH".into(), homedir);
    }
    env_map.insert("APPDATA".into(), appdata);
    env_map.insert("LOCALAPPDATA".into(), localappdata);
    if let Some(temp) = current_temp {
        env_map.insert("TEMP".into(), temp);
    }
    if let Some(tmp) = current_tmp {
        env_map.insert("TMP".into(), tmp);
    }

    if let Some(original_userprofile) = original_userprofile
        && !original_userprofile.eq_ignore_ascii_case(&current_userprofile)
        && let Some(path) = env_map.get_mut("PATH")
    {
        let filtered: Vec<&str> = path
            .split(';')
            .filter(|entry| !path_is_within_windows_root(entry, &original_userprofile))
            .collect();
        *path = filtered.join(";");
    }
}

pub fn sanitize_windows_profile_env(env_map: &mut HashMap<String, String>) {
    sanitize_windows_profile_env_with_current(
        env_map,
        env::var("USERPROFILE").ok(),
        env::var("APPDATA").ok(),
        env::var("LOCALAPPDATA").ok(),
        env::var("TEMP").ok(),
        env::var("TMP").ok(),
    );
}

fn prepend_path(env_map: &mut HashMap<String, String>, prefix: &str) {
    let existing = env_map
        .get("PATH")
        .cloned()
        .or_else(|| env::var("PATH").ok())
        .unwrap_or_default();
    let parts: Vec<String> = existing.split(';').map(|s| s.to_string()).collect();
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
        .map(|s| s.to_string())
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
            let path = base.join(format!("{}{}", tool, ext));
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
            let p = base.join(format!("{}{}", tool, ext));
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
    use super::sanitize_windows_profile_env_with_current;
    use std::collections::HashMap;

    #[test]
    fn rewrites_profile_vars_and_filters_host_profile_path_entries() {
        let mut env_map = HashMap::from([
            ("USERPROFILE".to_string(), r"C:\Users\andre".to_string()),
            ("HOME".to_string(), r"C:\Users\andre".to_string()),
            ("APPDATA".to_string(), r"C:\Users\andre\AppData\Roaming".to_string()),
            (
                "LOCALAPPDATA".to_string(),
                r"C:\Users\andre\AppData\Local".to_string(),
            ),
            ("TEMP".to_string(), r"C:\Users\andre\AppData\Local\Temp".to_string()),
            ("TMP".to_string(), r"C:\Users\andre\AppData\Local\Temp".to_string()),
            (
                "PATH".to_string(),
                [
                    r"C:\Windows\System32",
                    r"C:\Users\andre\AppData\Local\Microsoft\WindowsApps",
                    r"C:\Users\andre\AppData\Roaming\npm",
                    r"D:\tools",
                ]
                .join(";"),
            ),
        ]);

        sanitize_windows_profile_env_with_current(
            &mut env_map,
            Some(r"C:\Users\CodexSandboxOffline".to_string()),
            Some(r"C:\Users\CodexSandboxOffline\AppData\Roaming".to_string()),
            Some(r"C:\Users\CodexSandboxOffline\AppData\Local".to_string()),
            Some(r"C:\Users\CodexSandboxOffline\AppData\Local\Temp".to_string()),
            Some(r"C:\Users\CodexSandboxOffline\AppData\Local\Temp".to_string()),
        );

        assert_eq!(
            env_map.get("USERPROFILE").map(String::as_str),
            Some(r"C:\Users\CodexSandboxOffline")
        );
        assert_eq!(
            env_map.get("HOME").map(String::as_str),
            Some(r"C:\Users\CodexSandboxOffline")
        );
        assert_eq!(env_map.get("HOMEDRIVE").map(String::as_str), Some("C:"));
        assert_eq!(
            env_map.get("HOMEPATH").map(String::as_str),
            Some(r"\Users\CodexSandboxOffline")
        );
        assert_eq!(
            env_map.get("APPDATA").map(String::as_str),
            Some(r"C:\Users\CodexSandboxOffline\AppData\Roaming")
        );
        assert_eq!(
            env_map.get("LOCALAPPDATA").map(String::as_str),
            Some(r"C:\Users\CodexSandboxOffline\AppData\Local")
        );
        assert_eq!(
            env_map.get("TEMP").map(String::as_str),
            Some(r"C:\Users\CodexSandboxOffline\AppData\Local\Temp")
        );
        assert_eq!(
            env_map.get("PATH").map(String::as_str),
            Some(r"C:\Windows\System32;D:\tools")
        );
    }

    #[test]
    fn leaves_path_unchanged_when_userprofile_already_matches() {
        let mut env_map = HashMap::from([
            ("USERPROFILE".to_string(), r"C:\Users\CodexSandboxOffline".to_string()),
            (
                "PATH".to_string(),
                [r"C:\Windows\System32", r"C:\Users\CodexSandboxOffline\bin"].join(";"),
            ),
        ]);

        sanitize_windows_profile_env_with_current(
            &mut env_map,
            Some(r"C:\Users\CodexSandboxOffline".to_string()),
            None,
            None,
            None,
            None,
        );

        assert_eq!(
            env_map.get("PATH").map(String::as_str),
            Some(r"C:\Windows\System32;C:\Users\CodexSandboxOffline\bin")
        );
        assert_eq!(
            env_map.get("APPDATA").map(String::as_str),
            Some(r"C:\Users\CodexSandboxOffline\AppData\Roaming")
        );
        assert_eq!(
            env_map.get("LOCALAPPDATA").map(String::as_str),
            Some(r"C:\Users\CodexSandboxOffline\AppData\Local")
        );
    }
}
