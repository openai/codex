// Portions of this file are adapted from Rust's standard library.
// Copyright The Rust Project Developers. Licensed under Apache-2.0 or MIT.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;

use winapi::shared::minwindef::TRUE;
use winapi::um::stringapiset::CompareStringOrdinal;

use self::command_path::current_directory;
use self::command_path::program_exists;
use self::command_path::system_directory;
use self::command_path::to_user_path;
use self::command_path::windows_directory;

#[path = "command_path.rs"]
mod command_path;

const CSTR_LESS_THAN: i32 = 1;
const CSTR_EQUAL: i32 = 2;
const CSTR_GREATER_THAN: i32 = 3;

pub(super) struct PreparedCommand {
    pub application: Vec<u16>,
    pub command_line: Vec<u16>,
    pub environment: Vec<u16>,
    pub current_directory: Vec<u16>,
}

pub(super) fn prepare_command(
    program: &str,
    args: &[String],
    cwd: &Path,
    env: &HashMap<String, String>,
) -> io::Result<PreparedCommand> {
    if program.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing program for pipe spawn",
        ));
    }

    let resolved_program = resolve_program(program, env)?;
    let resolved_program = to_user_path(&resolved_program)?;
    let is_batch = has_batch_extension(&resolved_program);

    let (application, mut command_line) = if is_batch {
        let command_interpreter = system_directory()?.join("cmd.exe");
        (
            to_user_path(&command_interpreter)?,
            batch_command_line(&resolved_program, args)?,
        )
    } else {
        (resolved_program, regular_command_line(program, args)?)
    };
    command_line.push(0);

    Ok(PreparedCommand {
        application,
        command_line,
        environment: environment_block(env)?,
        current_directory: current_directory(cwd)?,
    })
}

fn resolve_program(program: &str, child_env: &HashMap<String, String>) -> io::Result<PathBuf> {
    let program = Path::new(program);
    let file_name = program.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "program path has no file name")
    })?;
    if file_name.encode_wide().any(|unit| unit == 0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "program contains a NUL character",
        ));
    }

    if program
        .parent()
        .is_some_and(|parent| !parent.as_os_str().is_empty())
    {
        return Ok(resolve_program_path(program));
    }

    let search_name = if !file_name.encode_wide().any(|unit| unit == '.' as u16) {
        let mut name = file_name.to_os_string();
        name.push(".exe");
        name
    } else {
        file_name.to_os_string()
    };

    for directory in search_directories(child_env) {
        let candidate = directory.join(&search_name);
        if program_exists(&candidate) {
            return Ok(candidate);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("program `{}` was not found", program.display()),
    ))
}

fn resolve_program_path(program: &Path) -> PathBuf {
    if program
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    {
        return program.to_owned();
    }

    let mut with_exe = program.as_os_str().to_os_string();
    with_exe.push(".exe");
    let with_exe = PathBuf::from(with_exe);
    if program_exists(&with_exe) {
        with_exe
    } else {
        program.to_owned()
    }
}

fn search_directories(child_env: &HashMap<String, String>) -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(path) = get_env_case_insensitive(child_env, "PATH") {
        directories
            .extend(env::split_paths(OsStr::new(path)).filter(|path| !path.as_os_str().is_empty()));
    }
    if let Ok(executable) = env::current_exe()
        && let Some(parent) = executable.parent()
    {
        directories.push(parent.to_owned());
    }
    if let Ok(directory) = system_directory() {
        directories.push(directory);
    }
    if let Ok(directory) = windows_directory() {
        directories.push(directory);
    }
    if let Some(path) = env::var_os("PATH") {
        directories.extend(env::split_paths(&path).filter(|path| !path.as_os_str().is_empty()));
    }
    directories
}

fn has_batch_extension(program: &[u16]) -> bool {
    const DOT: u16 = b'.' as u16;
    const LOWER_A: u16 = b'a' as u16;
    const LOWER_B: u16 = b'b' as u16;
    const LOWER_C: u16 = b'c' as u16;
    const LOWER_D: u16 = b'd' as u16;
    const LOWER_M: u16 = b'm' as u16;
    const LOWER_T: u16 = b't' as u16;
    const UPPER_A: u16 = b'A' as u16;
    const UPPER_B: u16 = b'B' as u16;
    const UPPER_C: u16 = b'C' as u16;
    const UPPER_D: u16 = b'D' as u16;
    const UPPER_M: u16 = b'M' as u16;
    const UPPER_T: u16 = b'T' as u16;
    let program = program.strip_suffix(&[0]).unwrap_or(program);
    matches!(
        program.get(program.len().saturating_sub(4)..),
        Some(
            [DOT, LOWER_B | UPPER_B, LOWER_A | UPPER_A, LOWER_T | UPPER_T]
                | [DOT, LOWER_C | UPPER_C, LOWER_M | UPPER_M, LOWER_D | UPPER_D]
        )
    )
}

fn get_env_case_insensitive<'a>(
    environment: &'a HashMap<String, String>,
    key: &str,
) -> Option<&'a str> {
    environment
        .iter()
        .filter_map(|(candidate, value)| {
            candidate
                .eq_ignore_ascii_case(key)
                .then_some(value.as_str())
        })
        .last()
}

fn regular_command_line(program: &str, args: &[String]) -> io::Result<Vec<u16>> {
    if program.contains(['\0', '"']) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "program contains an invalid character",
        ));
    }

    let mut command_line = Vec::new();
    command_line.push('"' as u16);
    command_line.extend(OsStr::new(program).encode_wide());
    command_line.push('"' as u16);
    for arg in args {
        command_line.push(' ' as u16);
        append_regular_arg(arg, &mut command_line)?;
    }
    Ok(command_line)
}

fn append_regular_arg(arg: &str, command_line: &mut Vec<u16>) -> io::Result<()> {
    if arg.contains('\0') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "argument contains a NUL character",
        ));
    }

    let quote = arg.is_empty() || arg.contains([' ', '\t']);
    if quote {
        command_line.push('"' as u16);
    }

    let arg: Vec<u16> = OsStr::new(arg).encode_wide().collect();
    let mut index = 0;
    while index < arg.len() {
        let mut backslashes = 0;
        while index < arg.len() && arg[index] == '\\' as u16 {
            index += 1;
            backslashes += 1;
        }

        if index == arg.len() {
            let count = if quote { backslashes * 2 } else { backslashes };
            command_line.extend(std::iter::repeat_n('\\' as u16, count));
            break;
        }
        if arg[index] == '"' as u16 {
            command_line.extend(std::iter::repeat_n('\\' as u16, backslashes * 2 + 1));
        } else {
            command_line.extend(std::iter::repeat_n('\\' as u16, backslashes));
        }
        command_line.push(arg[index]);
        index += 1;
    }

    if quote {
        command_line.push('"' as u16);
    }
    Ok(())
}

fn batch_command_line(script: &[u16], args: &[String]) -> io::Result<Vec<u16>> {
    let mut command_line: Vec<u16> = OsStr::new("cmd.exe /e:ON /v:OFF /d /c \"")
        .encode_wide()
        .collect();
    let script = script.strip_suffix(&[0]).unwrap_or(script);
    if script.contains(&('"' as u16)) || script.last() == Some(&('\\' as u16)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows file names may not contain a quote or end with a backslash",
        ));
    }
    command_line.push('"' as u16);
    command_line.extend_from_slice(script);
    command_line.push('"' as u16);
    for arg in args {
        command_line.push(' ' as u16);
        append_batch_arg(arg, &mut command_line)?;
    }
    command_line.push('"' as u16);
    Ok(command_line)
}

fn append_batch_arg(arg: &str, command_line: &mut Vec<u16>) -> io::Result<()> {
    if arg.contains(['\0', '\r', '\n']) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "batch-file argument contains an invalid character",
        ));
    }

    const UNQUOTED_ASCII: &str = r"#$*+-./:?@\_";
    let quote = arg.is_empty()
        || arg.ends_with('\\')
        || arg.chars().any(|character| {
            character.is_control()
                || (character.is_ascii()
                    && !(character.is_ascii_alphanumeric() || UNQUOTED_ASCII.contains(character)))
        });
    if quote {
        command_line.push('"' as u16);
    }

    let mut backslashes = 0;
    for unit in OsStr::new(arg).encode_wide() {
        if unit == '\\' as u16 {
            backslashes += 1;
        } else {
            if unit == '"' as u16 {
                command_line.extend(std::iter::repeat_n('\\' as u16, backslashes));
                command_line.push('"' as u16);
            } else if unit == '%' as u16 || unit == '\r' as u16 {
                command_line.extend(OsStr::new("%%cd:~,").encode_wide());
            }
            backslashes = 0;
        }
        command_line.push(unit);
    }

    if quote {
        command_line.extend(std::iter::repeat_n('\\' as u16, backslashes));
        command_line.push('"' as u16);
    }
    Ok(())
}

fn environment_block(environment: &HashMap<String, String>) -> io::Result<Vec<u16>> {
    let mut variables: Vec<(&String, &String)> = Vec::new();
    for (key, value) in environment {
        if let Some((_, previous_value)) = variables
            .iter_mut()
            .find(|(previous_key, _)| compare_environment_keys(previous_key, key).is_eq())
        {
            *previous_value = value;
        } else {
            variables.push((key, value));
        }
    }
    variables.sort_by(|(left, _), (right, _)| compare_environment_keys(left, right));

    let mut block = Vec::new();
    for (key, value) in variables {
        if key.contains('\0') || value.contains('\0') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "environment contains an invalid key or value",
            ));
        }
        block.extend(OsStr::new(key).encode_wide());
        block.push('=' as u16);
        block.extend(OsStr::new(value).encode_wide());
        block.push(0);
    }
    block.push(0);
    if block.len() == 1 {
        block.push(0);
    }
    Ok(block)
}

fn compare_environment_keys(left: &str, right: &str) -> Ordering {
    let left: Vec<u16> = left.encode_utf16().collect();
    let right: Vec<u16> = right.encode_utf16().collect();
    match unsafe {
        CompareStringOrdinal(
            left.as_ptr(),
            left.len() as i32,
            right.as_ptr(),
            right.len() as i32,
            TRUE,
        )
    } {
        CSTR_LESS_THAN => Ordering::Less,
        CSTR_EQUAL => Ordering::Equal,
        CSTR_GREATER_THAN => Ordering::Greater,
        _ => panic!(
            "CompareStringOrdinal failed: {}",
            io::Error::last_os_error()
        ),
    }
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
