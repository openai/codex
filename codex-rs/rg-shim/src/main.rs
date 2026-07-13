use codex_rg::CACHE_ROOT_ENV;
use codex_rg::open_file_inventory;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

const PACKAGE_METADATA_FILENAME: &str = "codex-package.json";
const PATH_DIRNAME: &str = "codex-path";
const RESOURCES_DIRNAME: &str = "codex-resources";

fn main() {
    let result = run();
    match result {
        Ok(exit_code) => std::process::exit(exit_code),
        Err(error) => {
            eprintln!("codex-rg: {error}");
            std::process::exit(127);
        }
    }
}

fn run() -> io::Result<i32> {
    let current_exe = std::env::current_exe()?;
    let real_rg = real_rg_path(&current_exe)?;
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if is_default_files_request(&args)
        && std::env::var_os("RIPGREP_CONFIG_PATH").is_none()
        && let Some(cache_root) = std::env::var_os(CACHE_ROOT_ENV)
        && let Some(mut inventory) =
            open_file_inventory(Path::new(&cache_root), &std::env::current_dir()?)
    {
        let mut stdout = io::stdout().lock();
        return match io::copy(&mut inventory, &mut stdout) {
            Ok(_) => {
                stdout.flush()?;
                Ok(0)
            }
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(0),
            Err(error) => Err(error),
        };
    }
    run_real_rg(real_rg, args)
}

fn is_default_files_request(args: &[OsString]) -> bool {
    args == [OsStr::new("--files")]
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
fn run_real_rg(real_rg: PathBuf, args: Vec<OsString>) -> io::Result<i32> {
    use std::os::unix::process::CommandExt;

    Err(Command::new(real_rg).args(args).exec())
}

#[cfg(windows)]
fn run_real_rg(real_rg: PathBuf, args: Vec<OsString>) -> io::Result<i32> {
    let status = Command::new(real_rg).args(args).status()?;
    Ok(status.code().unwrap_or(1))
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
