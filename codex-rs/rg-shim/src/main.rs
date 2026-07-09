use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

const PACKAGE_METADATA_FILENAME: &str = "codex-package.json";
const PATH_DIRNAME: &str = "codex-path";
const RESOURCES_DIRNAME: &str = "codex-resources";

fn main() {
    let result = std::env::current_exe()
        .and_then(|current_exe| real_rg_path(&current_exe))
        .and_then(run_real_rg);
    match result {
        Ok(exit_code) => std::process::exit(exit_code),
        Err(error) => {
            eprintln!("codex-rg: {error}");
            std::process::exit(127);
        }
    }
}

fn real_rg_path(current_exe: &Path) -> io::Result<PathBuf> {
    let path_dir = current_exe.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "executable has no parent directory",
        )
    })?;
    if path_dir.file_name() != Some(OsStr::new(PATH_DIRNAME)) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("executable is not inside a {PATH_DIRNAME} directory"),
        ));
    }

    let package_dir = path_dir.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "package has no parent directory")
    })?;
    if !package_dir.join(PACKAGE_METADATA_FILENAME).is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("package is missing {PACKAGE_METADATA_FILENAME}"),
        ));
    }

    let executable_name = current_exe
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "executable has no file name"))?;
    let real_rg = package_dir.join(RESOURCES_DIRNAME).join(executable_name);
    if !real_rg.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("bundled ripgrep was not found at {}", real_rg.display()),
        ));
    }

    Ok(real_rg)
}

#[cfg(unix)]
fn run_real_rg(real_rg: PathBuf) -> io::Result<i32> {
    use std::os::unix::process::CommandExt;

    Err(Command::new(real_rg)
        .args(std::env::args_os().skip(1))
        .exec())
}

#[cfg(windows)]
fn run_real_rg(real_rg: PathBuf) -> io::Result<i32> {
    let status = Command::new(real_rg)
        .args(std::env::args_os().skip(1))
        .status()?;
    Ok(status.code().unwrap_or(1))
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
