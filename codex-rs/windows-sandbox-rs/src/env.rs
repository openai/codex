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
#[allow(dead_code)]
pub fn inherit_path_env(env_map: &mut HashMap<String, String>) {
    if !env_map.contains_key("PATH") {
        if let Ok(path) = env::var("PATH") {
            env_map.insert("PATH".into(), path);
        }
    }
    if !env_map.contains_key("PATHEXT") {
        if let Ok(pathext) = env::var("PATHEXT") {
            env_map.insert("PATHEXT".into(), pathext);
        }
    }
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

    let base = ensure_denybin(&["ssh", "scp"], None)?;
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
