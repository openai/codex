use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::io::IsTerminal;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

const MARKER_FILE: &str = "wsl-home-setup.json";
const BEGIN_MARKER: &str = "# >>> codex wsl shared home >>>";
const END_MARKER: &str = "# <<< codex wsl shared home <<<";
const FISH_BEGIN_MARKER: &str = "# >>> codex wsl shared home >>>";
const FISH_END_MARKER: &str = "# <<< codex wsl shared home <<<";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Setup,
    SkipOnce,
    NeverAskAgain,
}

pub(crate) fn maybe_offer_wsl_home_setup() -> io::Result<()> {
    let env = SystemEnv;
    maybe_offer_wsl_home_setup_with(&env)
}

fn maybe_offer_wsl_home_setup_with(env: &impl WslHomeSetupEnv) -> io::Result<()> {
    if !should_offer(env) {
        return Ok(());
    }

    let Some(windows_home) = env.windows_user_profile()? else {
        return Ok(());
    };
    let Some(shared_codex_home) = win_path_to_wsl(&windows_home.join(".codex")) else {
        return Ok(());
    };

    let Some(profile_path) = env.shell_profile_path() else {
        print_manual_setup(env, &shared_codex_home)?;
        return Ok(());
    };

    print_offer(env, &shared_codex_home, &profile_path)?;
    match env.read_action()? {
        Action::Setup => {
            env.create_dir_all(&shared_codex_home)?;
            let block = profile_block(env.shell_kind(), &shared_codex_home);
            update_profile(env, &profile_path, &block)?;
            env.set_var("CODEX_HOME", shared_codex_home.as_os_str());
            let mut stderr = env.stderr();
            writeln!(
                stderr,
                "Configured CODEX_HOME for this shell and future shells: {}",
                shared_codex_home.display()
            )?;
        }
        Action::SkipOnce => {}
        Action::NeverAskAgain => {
            env.create_dir_all(&env.default_codex_home())?;
            env.write(
                &env.default_codex_home().join(MARKER_FILE),
                "{\"wsl_home_setup\":\"never\"}\n",
            )?;
        }
    }

    Ok(())
}

fn should_offer(env: &impl WslHomeSetupEnv) -> bool {
    if !env.is_wsl() || env.var_os("CODEX_HOME").is_some() || !env.is_interactive() {
        return false;
    }
    let codex_home = env.default_codex_home();
    !codex_home.join(MARKER_FILE).exists()
}

fn print_offer(
    env: &impl WslHomeSetupEnv,
    shared_home: &Path,
    profile_path: &Path,
) -> io::Result<()> {
    let mut stderr = env.stderr();
    writeln!(
        stderr,
        "Codex is running in WSL. The Windows app uses %USERPROFILE%\\.codex, but this WSL shell defaults to ~/.codex."
    )?;
    writeln!(
        stderr,
        "Use {} as CODEX_HOME so config, auth, and sessions are shared?",
        shared_home.display()
    )?;
    writeln!(
        stderr,
        "This will update {}. Choose [Y]es, [n]o, or [d]o not ask again.",
        profile_path.display()
    )?;
    Ok(())
}

fn print_manual_setup(env: &impl WslHomeSetupEnv, shared_home: &Path) -> io::Result<()> {
    let mut stderr = env.stderr();
    writeln!(
        stderr,
        "Codex is running in WSL. To share config, auth, and sessions with the Windows app, add this to your shell profile:"
    )?;
    writeln!(stderr, "export CODEX_HOME={}", shared_home.display())?;
    Ok(())
}

fn update_profile(env: &impl WslHomeSetupEnv, profile_path: &Path, block: &str) -> io::Result<()> {
    let existing = match env.read_to_string(profile_path) {
        Ok(existing) => existing,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err),
    };
    let updated = replace_or_append_block(&existing, block);
    env.write(profile_path, &updated)
}

fn replace_or_append_block(existing: &str, block: &str) -> String {
    let Some(start) = existing.find(BEGIN_MARKER) else {
        let mut updated = existing.to_string();
        if !updated.is_empty() && !updated.ends_with('\n') {
            updated.push('\n');
        }
        if !updated.is_empty() {
            updated.push('\n');
        }
        updated.push_str(block);
        if !updated.ends_with('\n') {
            updated.push('\n');
        }
        return updated;
    };

    let Some(end) = existing[start..]
        .find(END_MARKER)
        .map(|idx| start + idx + END_MARKER.len())
    else {
        return existing.to_string();
    };

    let mut updated = String::new();
    updated.push_str(&existing[..start]);
    updated.push_str(block);
    let suffix = if block.ends_with('\n') {
        existing[end..]
            .strip_prefix('\n')
            .unwrap_or(&existing[end..])
    } else {
        &existing[end..]
    };
    updated.push_str(suffix);
    updated
}

fn profile_block(shell_kind: ShellKind, shared_home: &Path) -> String {
    let escaped = shell_escape(shared_home);
    match shell_kind {
        ShellKind::Fish => {
            format!("{FISH_BEGIN_MARKER}\nset -gx CODEX_HOME {escaped}\n{FISH_END_MARKER}\n")
        }
        ShellKind::Bash | ShellKind::Zsh | ShellKind::Unknown => {
            format!("{BEGIN_MARKER}\nexport CODEX_HOME={escaped}\n{END_MARKER}\n")
        }
    }
}

fn shell_escape(path: &Path) -> String {
    let raw = path.to_string_lossy();
    if raw
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-'))
    {
        raw.into_owned()
    } else {
        format!("'{}'", raw.replace('\'', "'\\''"))
    }
}

fn win_path_to_wsl(path: &Path) -> Option<PathBuf> {
    let value = path.to_string_lossy().replace('\\', "/");
    let bytes = value.as_bytes();
    if bytes.len() < 3 || bytes[1] != b':' || bytes[2] != b'/' || !bytes[0].is_ascii_alphabetic() {
        return None;
    }
    let drive = (bytes[0] as char).to_ascii_lowercase();
    Some(PathBuf::from(format!("/mnt/{drive}/{}", &value[3..])))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellKind {
    Bash,
    Zsh,
    Fish,
    Unknown,
}

trait WslHomeSetupEnv {
    fn is_wsl(&self) -> bool;
    fn is_interactive(&self) -> bool;
    fn var_os(&self, key: &str) -> Option<OsString>;
    fn default_codex_home(&self) -> PathBuf;
    fn windows_user_profile(&self) -> io::Result<Option<PathBuf>>;
    fn shell_kind(&self) -> ShellKind;
    fn shell_profile_path(&self) -> Option<PathBuf>;
    fn read_action(&self) -> io::Result<Action>;
    fn read_to_string(&self, path: &Path) -> io::Result<String>;
    fn write(&self, path: &Path, contents: &str) -> io::Result<()>;
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;
    fn set_var(&self, key: &str, value: &std::ffi::OsStr);
    fn stderr(&self) -> Box<dyn Write + '_>;
}

struct SystemEnv;

impl WslHomeSetupEnv for SystemEnv {
    fn is_wsl(&self) -> bool {
        codex_utils_path::is_wsl()
    }

    fn is_interactive(&self) -> bool {
        std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
    }

    fn var_os(&self, key: &str) -> Option<OsString> {
        env::var_os(key)
    }

    fn default_codex_home(&self) -> PathBuf {
        home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".codex")
    }

    fn windows_user_profile(&self) -> io::Result<Option<PathBuf>> {
        let output = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "[Environment]::GetFolderPath('UserProfile')",
            ])
            .output();
        let output = match output {
            Ok(output) => output,
            Err(_) => return Ok(None),
        };
        if !output.status.success() {
            return Ok(None);
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let trimmed = stdout.trim();
        Ok((!trimmed.is_empty()).then(|| PathBuf::from(trimmed)))
    }

    fn shell_kind(&self) -> ShellKind {
        shell_kind_from_shell(env::var_os("SHELL").as_deref())
    }

    fn shell_profile_path(&self) -> Option<PathBuf> {
        let home = home_dir()?;
        match self.shell_kind() {
            ShellKind::Bash => Some(home.join(".bashrc")),
            ShellKind::Zsh => Some(home.join(".zshrc")),
            ShellKind::Fish => Some(home.join(".config/fish/config.fish")),
            ShellKind::Unknown => None,
        }
    }

    fn read_action(&self) -> io::Result<Action> {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        Ok(match input.trim().to_ascii_lowercase().as_str() {
            "y" | "yes" | "" => Action::Setup,
            "d" => Action::NeverAskAgain,
            _ => Action::SkipOnce,
        })
    }

    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(path)
    }

    fn write(&self, path: &Path, contents: &str) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, contents)
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        fs::create_dir_all(path)
    }

    fn set_var(&self, key: &str, value: &std::ffi::OsStr) {
        // SAFETY: Codex is still in single-threaded CLI startup when this preflight runs.
        unsafe {
            env::set_var(key, value);
        }
    }

    fn stderr(&self) -> Box<dyn Write + '_> {
        Box::new(io::stderr())
    }
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

fn shell_kind_from_shell(shell: Option<&std::ffi::OsStr>) -> ShellKind {
    let Some(shell) = shell else {
        return ShellKind::Unknown;
    };
    match Path::new(shell).file_name().and_then(|name| name.to_str()) {
        Some("bash") => ShellKind::Bash,
        Some("zsh") => ShellKind::Zsh,
        Some("fish") => ShellKind::Fish,
        _ => ShellKind::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::cell::RefCell;
    use tempfile::TempDir;

    #[test]
    fn converts_windows_profile_to_wsl_codex_home() {
        assert_eq!(
            win_path_to_wsl(Path::new(r"C:\Users\Alice\.codex")).as_deref(),
            Some(Path::new("/mnt/c/Users/Alice/.codex"))
        );
    }

    #[test]
    fn appends_profile_block() {
        let block = profile_block(ShellKind::Zsh, Path::new("/mnt/c/Users/Alice/.codex"));
        assert_eq!(
            replace_or_append_block("export FOO=bar\n", &block),
            "export FOO=bar\n\n# >>> codex wsl shared home >>>\nexport CODEX_HOME=/mnt/c/Users/Alice/.codex\n# <<< codex wsl shared home <<<\n"
        );
    }

    #[test]
    fn replaces_existing_profile_block() {
        let block = profile_block(ShellKind::Bash, Path::new("/mnt/c/Users/Alice/.codex"));
        let existing = "# >>> codex wsl shared home >>>\nexport CODEX_HOME=/old\n# <<< codex wsl shared home <<<\n";
        assert_eq!(replace_or_append_block(existing, &block), block);
    }

    #[test]
    fn fish_profile_block_uses_fish_export_syntax() {
        assert_eq!(
            profile_block(ShellKind::Fish, Path::new("/mnt/c/Users/Alice/.codex")),
            "# >>> codex wsl shared home >>>\nset -gx CODEX_HOME /mnt/c/Users/Alice/.codex\n# <<< codex wsl shared home <<<\n"
        );
    }

    #[test]
    fn shell_kind_uses_shell_basename() {
        assert_eq!(
            shell_kind_from_shell(Some(std::ffi::OsStr::new("/usr/bin/zsh"))),
            ShellKind::Zsh
        );
        assert_eq!(
            shell_kind_from_shell(Some(std::ffi::OsStr::new("/bin/fish"))),
            ShellKind::Fish
        );
    }

    #[test]
    fn accepting_setup_writes_profile_and_sets_current_codex_home() {
        let temp = TempDir::new().expect("tempdir");
        let codex_home = temp.path().join("linux/.codex");
        fs::create_dir_all(&codex_home).expect("create codex home");
        fs::write(codex_home.join("config.toml"), "model = \"gpt-5\"\n").expect("write config");
        let env = FakeEnv {
            codex_home,
            profile_path: temp.path().join("home/.zshrc"),
            action: Action::Setup,
            vars: RefCell::new(Vec::new()),
            stderr: RefCell::new(Vec::new()),
        };

        maybe_offer_wsl_home_setup_with(&env).expect("setup succeeds");

        assert_eq!(
            env.read_to_string(&env.profile_path).expect("profile"),
            "# >>> codex wsl shared home >>>\nexport CODEX_HOME=/mnt/c/Users/Alice/.codex\n# <<< codex wsl shared home <<<\n"
        );
        assert_eq!(
            env.vars.into_inner(),
            vec![(
                "CODEX_HOME".to_string(),
                OsString::from("/mnt/c/Users/Alice/.codex")
            )]
        );
    }

    struct FakeEnv {
        codex_home: PathBuf,
        profile_path: PathBuf,
        action: Action,
        vars: RefCell<Vec<(String, OsString)>>,
        stderr: RefCell<Vec<u8>>,
    }

    impl WslHomeSetupEnv for FakeEnv {
        fn is_wsl(&self) -> bool {
            true
        }

        fn is_interactive(&self) -> bool {
            true
        }

        fn var_os(&self, _key: &str) -> Option<OsString> {
            None
        }

        fn default_codex_home(&self) -> PathBuf {
            self.codex_home.clone()
        }

        fn windows_user_profile(&self) -> io::Result<Option<PathBuf>> {
            Ok(Some(PathBuf::from(r"C:\Users\Alice")))
        }

        fn shell_kind(&self) -> ShellKind {
            ShellKind::Zsh
        }

        fn shell_profile_path(&self) -> Option<PathBuf> {
            Some(self.profile_path.clone())
        }

        fn read_action(&self) -> io::Result<Action> {
            Ok(self.action)
        }

        fn read_to_string(&self, path: &Path) -> io::Result<String> {
            fs::read_to_string(path)
        }

        fn write(&self, path: &Path, contents: &str) -> io::Result<()> {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, contents)
        }

        fn create_dir_all(&self, path: &Path) -> io::Result<()> {
            if path.starts_with("/mnt") {
                return Ok(());
            }
            fs::create_dir_all(path)
        }

        fn set_var(&self, key: &str, value: &std::ffi::OsStr) {
            self.vars
                .borrow_mut()
                .push((key.to_string(), value.to_os_string()));
        }

        fn stderr(&self) -> Box<dyn Write + '_> {
            Box::new(FakeStderr(&self.stderr))
        }
    }

    struct FakeStderr<'a>(&'a RefCell<Vec<u8>>);

    impl Write for FakeStderr<'_> {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}
