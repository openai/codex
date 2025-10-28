use anyhow::Result;
use code_core::custom_prompts::{default_prompts_dir, discover_prompts_in};
use once_cell::sync::Lazy;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use tempfile::TempDir;

static ENV_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

struct EnvBackup {
    entries: Vec<(&'static str, Option<String>)>,
}

impl EnvBackup {
    fn new(keys: &[&'static str]) -> Self {
        let mut entries = Vec::with_capacity(keys.len());
        for key in keys {
            entries.push((*key, std::env::var(key).ok()));
            // SAFETY: all test cases grab `ENV_MUTEX`, so env mutations stay
            // serialized within this process and satisfy Rust 1.90's
            // unsafety contract for environment updates.
            unsafe { std::env::remove_var(key) };
        }
        Self { entries }
    }

    fn set_path(&self, key: &'static str, path: &Path) {
        // SAFETY: guarded by `ENV_MUTEX`; see comment in `new`.
        unsafe { std::env::set_var(key, path) };
    }

    fn remove(&self, key: &'static str) {
        unsafe { std::env::remove_var(key) };
    }
}

impl Drop for EnvBackup {
    fn drop(&mut self) {
        for (key, value) in self.entries.drain(..) {
            match value {
                Some(v) => unsafe { std::env::set_var(key, v) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
    }
}

fn prompt_names(prompts: &[code_protocol::custom_prompts::CustomPrompt]) -> Vec<String> {
    prompts.iter().map(|p| p.name.clone()).collect()
}

#[tokio::test]
async fn discovers_prompts_from_code_home() -> Result<()> {
    let _env_lock = ENV_MUTEX.lock().unwrap();
    let env = EnvBackup::new(&["HOME", "CODE_HOME", "CODEX_HOME"]);

    let code_home = TempDir::new()?;
    let prompts_dir = code_home.path().join("prompts");
    fs::create_dir_all(&prompts_dir)?;
    fs::write(prompts_dir.join("alpha.md"), "# alpha")?;
    fs::write(prompts_dir.join("beta.MD"), "# beta")?;

    env.set_path("CODE_HOME", code_home.path());
    env.remove("CODEX_HOME");

    let default_dir = default_prompts_dir().expect("expected prompts dir");
    assert_eq!(default_dir, prompts_dir);

    let prompts = discover_prompts_in(&default_dir).await;
    let names = prompt_names(&prompts);
    assert_eq!(names, vec!["alpha", "beta"]);

    Ok(())
}

#[tokio::test]
async fn discovers_prompts_from_legacy_codex_home() -> Result<()> {
    let _env_lock = ENV_MUTEX.lock().unwrap();
    let env = EnvBackup::new(&["HOME", "CODE_HOME", "CODEX_HOME"]);

    let fake_home = TempDir::new()?;
    let codex_home = fake_home.path().join(".codex");
    let legacy_prompts = codex_home.join("prompts");
    fs::create_dir_all(&legacy_prompts)?;
    fs::write(legacy_prompts.join("legacy.md"), "# legacy")?;

    env.set_path("HOME", fake_home.path());
    env.remove("CODE_HOME");
    env.remove("CODEX_HOME");

    let default_dir = default_prompts_dir().expect("expected prompts dir");
    assert_eq!(default_dir, legacy_prompts);

    let prompts = discover_prompts_in(&default_dir).await;
    let names = prompt_names(&prompts);
    assert_eq!(names, vec!["legacy"]);

    Ok(())
}

#[tokio::test]
async fn prefers_code_home_when_both_locations_exist() -> Result<()> {
    let _env_lock = ENV_MUTEX.lock().unwrap();
    let env = EnvBackup::new(&["HOME", "CODE_HOME", "CODEX_HOME"]);

    let fake_home = TempDir::new()?;
    let code_home = fake_home.path().join(".code");
    let codex_home = fake_home.path().join(".codex");
    let code_prompts = code_home.join("prompts");
    let codex_prompts = codex_home.join("prompts");
    fs::create_dir_all(&code_prompts)?;
    fs::create_dir_all(&codex_prompts)?;
    fs::write(code_prompts.join("active.md"), "# active")?;
    fs::write(codex_prompts.join("legacy.md"), "# legacy")?;

    env.set_path("HOME", fake_home.path());
    env.remove("CODE_HOME");
    env.remove("CODEX_HOME");

    let default_dir = default_prompts_dir().expect("expected prompts dir");
    assert_eq!(default_dir, code_prompts);

    let prompts = discover_prompts_in(&default_dir).await;
    let names = prompt_names(&prompts);
    assert_eq!(names, vec!["active"]);

    Ok(())
}

#[tokio::test]
async fn ignores_non_markdown_files() -> Result<()> {
    let _env_lock = ENV_MUTEX.lock().unwrap();
    let env = EnvBackup::new(&["HOME", "CODE_HOME", "CODEX_HOME"]);

    let code_home = TempDir::new()?;
    let prompts_dir = code_home.path().join("prompts");
    fs::create_dir_all(&prompts_dir)?;
    fs::write(prompts_dir.join("keep.md"), "# keep")?;
    fs::write(prompts_dir.join("ignore.txt"), "# ignore")?;

    env.set_path("CODE_HOME", code_home.path());
    env.remove("CODEX_HOME");

    let prompts = discover_prompts_in(&prompts_dir).await;
    let names = prompt_names(&prompts);
    assert_eq!(names, vec!["keep"]);

    Ok(())
}
