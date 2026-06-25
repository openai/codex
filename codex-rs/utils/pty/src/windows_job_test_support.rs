use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

pub(super) struct TestDirectory {
    pub(super) path: PathBuf,
}

impl TestDirectory {
    pub(super) fn new(label: &str) -> io::Result<Self> {
        static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

        let path = std::env::temp_dir().join(format!(
            "codex-utils-pty-{label}-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        Ok(Self { path })
    }

    pub(super) fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub(super) fn write_descendant_scripts(
    directory: &TestDirectory,
    root_exits: bool,
) -> anyhow::Result<(PathBuf, PathBuf, PathBuf)> {
    let root = directory.join("root.cmd");
    let grandchild = directory.join("grandchild.cmd");
    let ready = directory.join("grandchild-ready");
    let escaped = directory.join("grandchild-escaped");
    fs::write(
        &grandchild,
        "@echo off\r\necho inherited-grandchild-ready\r\necho ready>\"%~dp0grandchild-ready\"\r\nping.exe -n 4 127.0.0.1 >NUL\r\necho inherited-grandchild-escaped\r\necho escaped>\"%~dp0grandchild-escaped\"\r\n",
    )?;
    let final_command = if root_exits {
        "exit /b 37"
    } else {
        "ping.exe -n 30 127.0.0.1 >NUL"
    };
    fs::write(
        &root,
        format!(
            "@echo off\r\nstart \"\" /b cmd.exe /d /q /c call \"%~dp0grandchild.cmd\"\r\n:wait\r\nif not exist \"%~dp0grandchild-ready\" (\r\n  ping.exe -n 2 127.0.0.1 >NUL\r\n  goto wait\r\n)\r\n{final_command}\r\n"
        ),
    )?;
    Ok((root, ready, escaped))
}

pub(super) async fn wait_for_path(path: &Path) -> anyhow::Result<()> {
    let timeout = tokio::time::Duration::from_secs(10);
    tokio::time::timeout(timeout, async {
        while !path.exists() {
            tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("timed out waiting for {}", path.display()))
}
