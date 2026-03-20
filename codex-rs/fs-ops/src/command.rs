use crate::constants::READ_FILE_OPERATION_ARG2;
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsCommand {
    ReadFile { path: PathBuf },
    WriteFile { path: PathBuf },
}

pub fn parse_command_from_args(
    mut args: impl Iterator<Item = OsString>,
) -> Result<FsCommand, String> {
    let Some(operation) = args.next() else {
        return Err("missing operation".to_string());
    };
    let Some(operation) = operation.to_str() else {
        return Err("operation must be valid UTF-8".to_string());
    };
    let Some(path) = args.next() else {
        return Err(format!("missing path for operation `{operation}`"));
    };
    if args.next().is_some() {
        return Err(format!(
            "unexpected extra arguments for operation `{operation}`"
        ));
    }

    let path = PathBuf::from(path);
    match operation {
        READ_FILE_OPERATION_ARG2 => Ok(FsCommand::ReadFile { path }),
        "write" => Ok(FsCommand::WriteFile { path }),
        _ => Err(format!(
            "unsupported filesystem operation `{operation}`; expected `read` or `write`"
        )),
    }
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
