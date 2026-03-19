use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let exe_path =
        env::current_exe().map_err(|err| format!("failed to locate current executable: {err}"))?;
    let bin_dir = exe_path.parent().ok_or_else(|| {
        format!(
            "failed to locate parent directory for executable {}",
            exe_path.display()
        )
    })?;
    let package_root = bin_dir.parent().ok_or_else(|| {
        format!(
            "failed to locate package root for executable {}",
            exe_path.display()
        )
    })?;
    let cargo_dylint = bin_dir.join(cargo_dylint_binary_name());
    let library_dir = package_root.join("lib");
    let library_path = find_bundled_library(&library_dir)?;

    ensure_exists(&cargo_dylint, "bundled cargo-dylint executable")?;
    ensure_exists(
        &library_dir,
        "bundled argument-comment lint library directory",
    )?;

    let args: Vec<OsString> = env::args_os().skip(1).collect();
    let mut command = Command::new(&cargo_dylint);
    command.arg("dylint");
    command.arg("--lib-path").arg(&library_path);
    if !has_library_selection(&args) {
        command.arg("--all");
    }
    command.args(&args);
    set_default_env(&mut command);

    let status = command
        .status()
        .map_err(|err| format!("failed to execute {}: {err}", cargo_dylint.display()))?;
    Ok(exit_code_from_status(status.code()))
}

fn has_library_selection(args: &[OsString]) -> bool {
    let mut expect_value = false;
    for arg in args {
        if expect_value {
            return true;
        }

        match arg.to_string_lossy().as_ref() {
            "--" => break,
            "--lib" | "--lib-path" => {
                expect_value = true;
            }
            "--lib=" | "--lib-path=" => return true,
            value if value.starts_with("--lib=") || value.starts_with("--lib-path=") => {
                return true;
            }
            _ => {}
        }
    }

    false
}

fn set_default_env(command: &mut Command) {
    if let Some(flags) = env::var_os("DYLINT_RUSTFLAGS") {
        let mut flags = flags.to_string_lossy().to_string();
        append_flag_if_missing(&mut flags, "-D uncommented-anonymous-literal-argument");
        append_flag_if_missing(&mut flags, "-A unknown_lints");
        command.env("DYLINT_RUSTFLAGS", flags);
    } else {
        command.env(
            "DYLINT_RUSTFLAGS",
            "-D uncommented-anonymous-literal-argument -A unknown_lints",
        );
    }

    if env::var_os("CARGO_INCREMENTAL").is_none() {
        command.env("CARGO_INCREMENTAL", "0");
    }
}

fn append_flag_if_missing(flags: &mut String, flag: &str) {
    if flags.contains(flag) {
        return;
    }

    if !flags.is_empty() {
        flags.push(' ');
    }
    flags.push_str(flag);
}

fn cargo_dylint_binary_name() -> &'static str {
    if cfg!(windows) {
        "cargo-dylint.exe"
    } else {
        "cargo-dylint"
    }
}

fn ensure_exists(path: &Path, label: &str) -> Result<(), String> {
    if path.exists() {
        Ok(())
    } else {
        Err(format!("{label} not found at {}", path.display()))
    }
}

fn find_bundled_library(library_dir: &Path) -> Result<PathBuf, String> {
    let entries = fs::read_dir(library_dir).map_err(|err| {
        format!(
            "failed to read bundled library directory {}: {err}",
            library_dir.display()
        )
    })?;
    let mut candidates = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().contains('@'))
                .unwrap_or(false)
        });

    let Some(first) = candidates.next() else {
        return Err(format!(
            "no packaged Dylint library found in {}",
            library_dir.display()
        ));
    };
    if candidates.next().is_some() {
        return Err(format!(
            "expected exactly one packaged Dylint library in {}",
            library_dir.display()
        ));
    }

    Ok(first)
}

fn exit_code_from_status(code: Option<i32>) -> ExitCode {
    code.and_then(|value| u8::try_from(value).ok())
        .map_or_else(|| ExitCode::from(1), ExitCode::from)
}
