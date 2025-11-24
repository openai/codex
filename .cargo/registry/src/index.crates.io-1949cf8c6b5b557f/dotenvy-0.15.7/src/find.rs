use std::fs::File;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

use crate::errors::*;
use crate::iter::Iter;

pub struct Finder<'a> {
    filename: &'a Path,
}

impl<'a> Finder<'a> {
    pub fn new() -> Self {
        Finder {
            filename: Path::new(".env"),
        }
    }

    pub fn filename(mut self, filename: &'a Path) -> Self {
        self.filename = filename;
        self
    }

    pub fn find(self) -> Result<(PathBuf, Iter<File>)> {
        let path = find(&env::current_dir().map_err(Error::Io)?, self.filename)?;
        let file = File::open(&path).map_err(Error::Io)?;
        let iter = Iter::new(file);
        Ok((path, iter))
    }
}

/// Searches for `filename` in `directory` and parent directories until found or root is reached.
pub fn find(directory: &Path, filename: &Path) -> Result<PathBuf> {
    let candidate = directory.join(filename);

    match fs::metadata(&candidate) {
        Ok(metadata) => {
            if metadata.is_file() {
                return Ok(candidate);
            }
        }
        Err(error) => {
            if error.kind() != io::ErrorKind::NotFound {
                return Err(Error::Io(error));
            }
        }
    }

    if let Some(parent) = directory.parent() {
        find(parent, filename)
    } else {
        Err(Error::Io(io::Error::new(
            io::ErrorKind::NotFound,
            "path not found",
        )))
    }
}
