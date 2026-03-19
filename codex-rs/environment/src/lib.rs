pub mod fs;

pub use fs::CopyOptions;
pub use fs::CreateDirectoryOptions;
pub use fs::ExecutorFileSystem;
pub use fs::FileMetadata;
pub use fs::FileSystemResult;
pub use fs::ReadDirectoryEntry;
pub use fs::RemoveOptions;

#[derive(Clone, Debug, Default)]
pub struct Environment;

impl Environment {
    pub fn get_filesystem(&self) -> impl ExecutorFileSystem + use<> {
        fs::LocalFileSystem
    }
}
