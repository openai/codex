use std::io;
use std::path::Path;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    tokio::fs::read(path).await
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    std::fs::read(path)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    tokio::fs::read_to_string(path).await
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    std::fs::read_to_string(path)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
    tokio::fs::write(path, contents).await
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
    std::fs::write(path, contents)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    tokio::fs::create_dir_all(path).await
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    std::fs::create_dir_all(path)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn metadata(path: impl AsRef<Path>) -> io::Result<std::fs::Metadata> {
    tokio::fs::metadata(path).await
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn metadata(path: impl AsRef<Path>) -> io::Result<std::fs::Metadata> {
    std::fs::metadata(path)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn try_exists(path: impl AsRef<Path>) -> io::Result<bool> {
    tokio::fs::try_exists(path).await
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn try_exists(path: impl AsRef<Path>) -> io::Result<bool> {
    Ok(path.as_ref().exists())
}
