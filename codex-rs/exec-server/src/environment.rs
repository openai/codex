use crate::fs;
use crate::fs::ExecutorFileSystem;

#[derive(Clone, Debug, Default)]
pub struct Environment;

impl Environment {
    pub fn get_filesystem(&self) -> impl ExecutorFileSystem + use<> {
        fs::LocalFileSystem
    }
}
