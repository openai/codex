use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitCode;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

const STRICT_LINTS: [&str; 2] = [
    "argument-comment-mismatch",
    "uncommented-anonymous-literal-argument",
];

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
    let library_path = prepare_library_path_for_dylint(&find_bundled_library(&library_dir)?)?;

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
    set_default_env(&mut command)?;

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

fn set_default_env(command: &mut Command) -> Result<(), String> {
    if let Some(flags) = env::var_os("DYLINT_RUSTFLAGS") {
        let mut flags = flags.to_string_lossy().to_string();
        for strict_lint in STRICT_LINTS {
            append_flag_if_missing(&mut flags, &format!("-D {strict_lint}"));
        }
        append_flag_if_missing(&mut flags, "-A unknown_lints");
        command.env("DYLINT_RUSTFLAGS", flags);
    } else {
        command.env("DYLINT_RUSTFLAGS", strict_rustflags());
    }

    if env::var_os("CARGO_INCREMENTAL").is_none() {
        command.env("CARGO_INCREMENTAL", "0");
    }

    if env::var_os("RUSTUP_HOME").is_none()
        && let Some(rustup_home) = infer_rustup_home()?
    {
        command.env("RUSTUP_HOME", rustup_home);
    }

    Ok(())
}

fn strict_rustflags() -> String {
    let strict_flags = STRICT_LINTS
        .iter()
        .map(|lint| format!("-D {lint}"))
        .collect::<Vec<_>>()
        .join(" ");
    format!("{strict_flags} -A unknown_lints")
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

fn infer_rustup_home() -> Result<Option<OsString>, String> {
    let output = Command::new("rustup")
        .args(["show", "home"])
        .output()
        .map_err(|err| format!("failed to query rustup home via `rustup show home`: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "`rustup show home` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let home = String::from_utf8(output.stdout)
        .map_err(|err| format!("`rustup show home` returned invalid UTF-8: {err}"))?;
    let home = home.trim();
    if home.is_empty() {
        Ok(None)
    } else {
        Ok(Some(OsString::from(home)))
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

fn prepare_library_path_for_dylint(library_path: &Path) -> Result<PathBuf, String> {
    let Some(normalized_filename) = normalize_nightly_library_filename(library_path) else {
        return Ok(library_path.to_path_buf());
    };

    let temp_dir = env::temp_dir().join(format!(
        "argument-comment-lint-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("failed to compute timestamp for temp dir: {err}"))?
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir).map_err(|err| {
        format!(
            "failed to create temporary directory {}: {err}",
            temp_dir.display()
        )
    })?;
    let normalized_path = temp_dir.join(normalized_filename);
    fs::copy(library_path, &normalized_path).map_err(|err| {
        format!(
            "failed to copy packaged library {} to {}: {err}",
            library_path.display(),
            normalized_path.display()
        )
    })?;
    Ok(normalized_path)
}

fn normalize_nightly_library_filename(library_path: &Path) -> Option<String> {
    let stem = library_path.file_stem()?.to_string_lossy();
    let extension = library_path.extension()?.to_string_lossy();
    let (lib_name, toolchain) = stem.rsplit_once('@')?;
    let normalized_toolchain = normalize_nightly_toolchain(toolchain)?;
    Some(format!("{lib_name}@{normalized_toolchain}.{extension}"))
}

fn normalize_nightly_toolchain(toolchain: &str) -> Option<String> {
    let parts: Vec<_> = toolchain.split('-').collect();
    if parts.len() > 4
        && parts[0] == "nightly"
        && parts[1].len() == 4
        && parts[2].len() == 2
        && parts[3].len() == 2
        && parts[1..4]
            .iter()
            .all(|part| part.chars().all(|ch| ch.is_ascii_digit()))
    {
        Some(format!("nightly-{}-{}-{}", parts[1], parts[2], parts[3]))
    } else {
        None
    }
}

fn exit_code_from_status(code: Option<i32>) -> ExitCode {
    code.and_then(|value| u8::try_from(value).ok())
        .map_or_else(|| ExitCode::from(1), ExitCode::from)
}

#[cfg(test)]
mod tests {
    use super::normalize_nightly_library_filename;
    use super::strict_rustflags;
    use std::path::Path;

    #[test]
    fn strips_host_triple_from_nightly_filename() {
        assert_eq!(
            normalize_nightly_library_filename(Path::new(
                "libargument_comment_lint@nightly-2025-09-18-aarch64-apple-darwin.dylib"
            )),
            Some(String::from(
                "libargument_comment_lint@nightly-2025-09-18.dylib"
            ))
        );
    }

    #[test]
    fn leaves_unqualified_nightly_filename_alone() {
        assert_eq!(
            normalize_nightly_library_filename(Path::new(
                "libargument_comment_lint@nightly-2025-09-18.dylib"
            )),
            None
        );
    }

    #[test]
    fn strict_rustflags_promotes_both_enforced_lints() {
        assert_eq!(
            strict_rustflags(),
            "-D argument-comment-mismatch -D uncommented-anonymous-literal-argument -A unknown_lints"
        );
    }
}
