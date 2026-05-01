use std::path::Path;
use std::path::PathBuf;

pub(crate) fn managed_codex_bin(codex_home: &Path) -> PathBuf {
    codex_home
        .join("packages")
        .join("standalone")
        .join("current")
        .join(managed_codex_file_name())
}

pub(crate) fn preferred_codex_bin(codex_home: &Path, current_exe: PathBuf) -> PathBuf {
    let managed_codex_bin = managed_codex_bin(codex_home);
    if managed_codex_bin.is_file() {
        managed_codex_bin
    } else {
        current_exe
    }
}

fn managed_codex_file_name() -> &'static str {
    if cfg!(windows) { "codex.exe" } else { "codex" }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::managed_codex_bin;
    use super::preferred_codex_bin;

    #[test]
    fn preferred_codex_bin_uses_managed_install_when_present() {
        let temp_dir = TempDir::new().expect("temp dir");
        let managed_bin = managed_codex_bin(temp_dir.path());
        fs::create_dir_all(managed_bin.parent().expect("managed parent")).expect("mkdir");
        fs::write(&managed_bin, "managed").expect("write managed bin");

        assert_eq!(
            preferred_codex_bin(temp_dir.path(), temp_dir.path().join("current")),
            managed_bin
        );
    }

    #[test]
    fn preferred_codex_bin_falls_back_to_current_executable() {
        let temp_dir = TempDir::new().expect("temp dir");
        let current_exe = temp_dir.path().join("current");

        assert_eq!(
            preferred_codex_bin(temp_dir.path(), current_exe.clone()),
            current_exe
        );
    }
}
