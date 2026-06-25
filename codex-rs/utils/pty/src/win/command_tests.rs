use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use pretty_assertions::assert_eq;

use super::environment_block;
use super::prepare_command;

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> io::Result<Self> {
        static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

        let path = std::env::temp_dir().join(format!(
            "codex-utils-pty-command-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        Ok(Self { path })
    }

    fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn wide_string(value: &[u16]) -> String {
    let value = value.strip_suffix(&[0]).unwrap_or(value);
    OsString::from_wide(value).to_string_lossy().into_owned()
}

#[test]
fn child_path_resolution_only_appends_exe_without_an_extension() -> anyhow::Result<()> {
    let directory = TestDirectory::new()?;
    let executable = directory.join("path-probe.exe");
    fs::write(&executable, [])?;
    fs::write(directory.join("batch-only.cmd"), "@exit /b 0\r\n")?;
    let env = HashMap::from([(
        "Path".to_string(),
        directory.path.to_string_lossy().into_owned(),
    )]);
    let cwd = std::env::current_dir()?;

    let command = prepare_command("path-probe", &[], &cwd, &env)?;
    assert_eq!(PathBuf::from(wide_string(&command.application)), executable);
    assert_eq!(PathBuf::from(wide_string(&command.current_directory)), cwd);
    assert!(command.environment.ends_with(&[0, 0]));

    let Err(error) = prepare_command("batch-only", &[], &cwd, &env) else {
        anyhow::bail!("extensionless program unexpectedly resolved to a batch file");
    };
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    Ok(())
}

#[test]
fn batch_command_line_uses_cmd_and_escapes_percent() -> anyhow::Result<()> {
    let directory = TestDirectory::new()?;
    let script = directory.join("probe.cmd");
    fs::write(&script, "@exit /b 0\r\n")?;
    let command = prepare_command(
        script.to_string_lossy().as_ref(),
        &["100%".to_string()],
        &directory.path,
        &HashMap::new(),
    )?;

    assert!(wide_string(&command.application).ends_with(r"\cmd.exe"));
    assert_eq!(
        wide_string(&command.command_line),
        format!(
            "cmd.exe /e:ON /v:OFF /d /c \"\"{}\" \"100%%cd:~,%\"\"",
            script.display()
        )
    );
    Ok(())
}

#[test]
fn environment_block_is_unicode_casefold_sorted_and_double_terminated() -> anyhow::Result<()> {
    let environment = HashMap::from([
        ("zebra".to_string(), "3".to_string()),
        ("éclair".to_string(), "2".to_string()),
        ("Alpha".to_string(), "1".to_string()),
        ("=C:".to_string(), r"C:\work".to_string()),
    ]);

    let block = environment_block(&environment)?;
    let variables = block
        .split(|unit| *unit == 0)
        .filter(|variable| !variable.is_empty())
        .map(|variable| OsString::from_wide(variable).to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        variables,
        vec![r"=C:=C:\work", "Alpha=1", "zebra=3", "éclair=2"]
    );
    assert!(block.ends_with(&[0, 0]));
    Ok(())
}

#[test]
fn command_preparation_rejects_a_file_as_current_directory() -> anyhow::Result<()> {
    let directory = TestDirectory::new()?;
    let not_a_directory = directory.join("file");
    fs::write(&not_a_directory, [])?;

    let Err(error) = prepare_command(
        r"C:\codex-nonexistent.exe",
        &[],
        &not_a_directory,
        &HashMap::new(),
    ) else {
        anyhow::bail!("file unexpectedly accepted as the current directory");
    };
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    Ok(())
}
