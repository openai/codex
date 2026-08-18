#![cfg(windows)]

use codex_utils_pty::RawConPty;
use std::ffi::OsStr;
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;
use winapi::um::libloaderapi::GetModuleHandleW;

struct CurrentDirGuard {
    original: PathBuf,
}

impl CurrentDirGuard {
    fn enter(path: &Path) -> std::io::Result<Self> {
        let original = std::env::current_dir()?;
        std::env::set_current_dir(path)?;
        Ok(Self { original })
    }
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.original);
    }
}

#[test]
fn conpty_ignores_dll_in_current_directory() -> anyhow::Result<()> {
    let temp_dir = std::env::temp_dir().join(format!(
        "codex-conpty-search-path-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir(&temp_dir)?;

    let system_root =
        std::env::var_os("SystemRoot").ok_or_else(|| anyhow::anyhow!("SystemRoot is not set"))?;
    let candidate = temp_dir.join("conpty.dll");
    // A kernel32 copy exports the ConPTY functions, so the old bare-name probe
    // would successfully load and retain this repository-controlled file.
    fs::copy(
        PathBuf::from(system_root)
            .join("System32")
            .join("kernel32.dll"),
        &candidate,
    )?;

    {
        let _current_dir = CurrentDirGuard::enter(&temp_dir)?;
        drop(RawConPty::new(/*cols*/ 80, /*rows*/ 24)?);

        let conpty_dll = OsStr::new("conpty.dll")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let module = unsafe { GetModuleHandleW(conpty_dll.as_ptr()) };
        assert!(
            module.is_null(),
            "ConPTY loaded conpty.dll from the process current directory"
        );
    }

    fs::remove_file(candidate)?;
    fs::remove_dir(temp_dir)?;
    Ok(())
}
