use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::Path;

static STABLE_APP_SERVER_TS_TYPES_DIR: include_dir::Dir<'_> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/stable");

/// Write the bundled stable TypeScript artifacts to `out_dir`.
pub fn write_stable_ts_types(out_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(out_dir)?;
    write_dir_recursive(&STABLE_APP_SERVER_TS_TYPES_DIR, out_dir, "ts")
}

fn write_dir_recursive(
    dir: &include_dir::Dir<'_>,
    out_dir: &Path,
    extension: &str,
) -> io::Result<()> {
    for file in dir.files() {
        if file
            .path()
            .extension()
            .is_some_and(|ext| ext == OsStr::new(extension))
        {
            let out_path = out_dir.join(file.path());
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(out_path, file.contents())?;
        }
    }

    for child in dir.dirs() {
        write_dir_recursive(child, out_dir, extension)?;
    }

    Ok(())
}
