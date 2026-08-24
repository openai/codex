use codex_file_system::ExecutorFileSystem;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use std::io;
use std::io::ErrorKind;

pub(crate) async fn collect_agent_role_files(
    fs: &dyn ExecutorFileSystem,
    dir: &AbsolutePathBuf,
) -> io::Result<Vec<AbsolutePathBuf>> {
    let mut files = Vec::new();
    let mut dirs = vec![dir.clone()];
    while let Some(dir) = dirs.pop() {
        let dir_uri = PathUri::from_abs_path(&dir);
        let entries = match fs.read_directory(&dir_uri, /*sandbox*/ None).await {
            Ok(entries) => entries,
            Err(err) if err.kind() == ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        };

        for entry in entries {
            let path = dir.join(entry.file_name);
            if entry.is_directory {
                dirs.push(path);
                continue;
            }
            if entry.is_file
                && path
                    .as_path()
                    .extension()
                    .is_some_and(|extension| extension == "toml")
            {
                files.push(path);
            }
        }
    }

    files.sort();
    Ok(files)
}
