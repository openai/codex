use super::*;
use async_trait::async_trait;
use codex_file_system::CopyOptions;
use codex_file_system::CreateDirectoryOptions;
use codex_file_system::FileMetadata;
use codex_file_system::ReadDirectoryEntry;
use codex_file_system::RemoveOptions;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::Notify;
use tokio::sync::Semaphore;

struct BlockingFileSystem {
    read_started: Notify,
    read_release: Semaphore,
    read_handle_dropped: Arc<AtomicBool>,
    write_started: Notify,
    write_release: Semaphore,
    write_handle_dropped: Arc<AtomicBool>,
}

impl Default for BlockingFileSystem {
    fn default() -> Self {
        Self {
            read_started: Notify::new(),
            read_release: Semaphore::new(0),
            read_handle_dropped: Arc::new(AtomicBool::new(false)),
            write_started: Notify::new(),
            write_release: Semaphore::new(0),
            write_handle_dropped: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[async_trait]
impl ExecutorFileSystem for BlockingFileSystem {
    async fn canonicalize(
        &self,
        _path: &AbsolutePathBuf,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<AbsolutePathBuf> {
        unreachable!("canonicalize is not used by these tests")
    }

    async fn join(
        &self,
        _base_path: &AbsolutePathBuf,
        _path: &Path,
    ) -> io::Result<AbsolutePathBuf> {
        unreachable!("join is not used by these tests")
    }

    async fn parent(&self, _path: &AbsolutePathBuf) -> io::Result<Option<AbsolutePathBuf>> {
        unreachable!("parent is not used by these tests")
    }

    async fn read_file(
        &self,
        _path: &AbsolutePathBuf,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<Vec<u8>> {
        unreachable!("read_file is not used by these tests")
    }

    async fn write_file(
        &self,
        _path: &AbsolutePathBuf,
        _contents: Vec<u8>,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<()> {
        unreachable!("write_file is not used by these tests")
    }

    async fn open_file_for_read(
        &self,
        _path: &AbsolutePathBuf,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<Box<dyn FileReadHandle>> {
        self.read_started.notify_one();
        self.read_release
            .acquire()
            .await
            .expect("release read open")
            .forget();
        Ok(Box::new(TestReadHandle {
            dropped: Arc::clone(&self.read_handle_dropped),
        }))
    }

    async fn open_file_for_write(
        &self,
        _path: &AbsolutePathBuf,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<Box<dyn FileWriteHandle>> {
        self.write_started.notify_one();
        self.write_release
            .acquire()
            .await
            .expect("release write open")
            .forget();
        Ok(Box::new(TestWriteHandle {
            dropped: Arc::clone(&self.write_handle_dropped),
        }))
    }

    async fn create_directory(
        &self,
        _path: &AbsolutePathBuf,
        _create_directory_options: CreateDirectoryOptions,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<()> {
        unreachable!("create_directory is not used by these tests")
    }

    async fn get_metadata(
        &self,
        _path: &AbsolutePathBuf,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<FileMetadata> {
        unreachable!("get_metadata is not used by these tests")
    }

    async fn read_directory(
        &self,
        _path: &AbsolutePathBuf,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<Vec<ReadDirectoryEntry>> {
        unreachable!("read_directory is not used by these tests")
    }

    async fn remove(
        &self,
        _path: &AbsolutePathBuf,
        _remove_options: RemoveOptions,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<()> {
        unreachable!("remove is not used by these tests")
    }

    async fn copy(
        &self,
        _source_path: &AbsolutePathBuf,
        _destination_path: &AbsolutePathBuf,
        _copy_options: CopyOptions,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<()> {
        unreachable!("copy is not used by these tests")
    }
}

struct TestReadHandle {
    dropped: Arc<AtomicBool>,
}

impl FileReadHandle for TestReadHandle {
    fn max_chunk_bytes(&self) -> usize {
        16
    }

    fn read(
        &self,
        _offset: u64,
        max_bytes: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = io::Result<FileReadChunk>> + Send + '_>>
    {
        Box::pin(async move {
            Ok(FileReadChunk {
                data: vec![b'x'; max_bytes],
                eof: true,
            })
        })
    }

    fn metadata(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = io::Result<OpenFileMetadata>> + Send + '_>,
    > {
        unreachable!("metadata is not used by these tests")
    }

    fn close(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = io::Result<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }
}

impl Drop for TestReadHandle {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}

struct TestWriteHandle {
    dropped: Arc<AtomicBool>,
}

impl FileWriteHandle for TestWriteHandle {
    fn max_chunk_bytes(&self) -> usize {
        16
    }

    fn write<'a>(
        &'a self,
        _data: &'a [u8],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = io::Result<()>> + Send + 'a>> {
        unreachable!("write is not used by these tests")
    }

    fn close(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = io::Result<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }
}

impl Drop for TestWriteHandle {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn close_during_read_open_prevents_handle_from_becoming_live() {
    let manager = FileHandleManager::default();
    let file_system = Arc::new(BlockingFileSystem::default());
    let path = AbsolutePathBuf::from_absolute_path(std::env::temp_dir()).expect("absolute path");
    let open = {
        let manager = manager.clone();
        let file_system = Arc::clone(&file_system);
        tokio::spawn(async move {
            manager
                .open_read(file_system, "read-1".to_string(), &path, None)
                .await
        })
    };

    file_system.read_started.notified().await;
    let close = {
        let manager = manager.clone();
        tokio::spawn(async move { manager.close("read-1").await })
    };
    tokio::task::yield_now().await;
    assert!(!close.is_finished());
    file_system.read_release.add_permits(1);

    let err = open
        .await
        .expect("join read open")
        .expect_err("closed read open should fail");
    assert_eq!(err.kind(), io::ErrorKind::Interrupted);
    close.await.expect("join read close").expect("close read");
    assert!(file_system.read_handle_dropped.load(Ordering::SeqCst));
    assert_eq!(
        manager
            .read("read-1", 0, None)
            .await
            .expect_err("closed read handle should not exist")
            .kind(),
        io::ErrorKind::NotFound
    );
}

#[tokio::test]
async fn close_during_write_open_prevents_handle_from_becoming_live() {
    let manager = FileHandleManager::default();
    let file_system = Arc::new(BlockingFileSystem::default());
    let path = AbsolutePathBuf::from_absolute_path(std::env::temp_dir()).expect("absolute path");
    let open = {
        let manager = manager.clone();
        let file_system = Arc::clone(&file_system);
        tokio::spawn(async move {
            manager
                .open_write(file_system, "write-1".to_string(), &path, None)
                .await
        })
    };

    file_system.write_started.notified().await;
    let close = {
        let manager = manager.clone();
        tokio::spawn(async move { manager.close("write-1").await })
    };
    tokio::task::yield_now().await;
    assert!(!close.is_finished());
    file_system.write_release.add_permits(1);

    let err = open
        .await
        .expect("join write open")
        .expect_err("closed write open should fail");
    assert_eq!(err.kind(), io::ErrorKind::Interrupted);
    close.await.expect("join write close").expect("close write");
    assert!(file_system.write_handle_dropped.load(Ordering::SeqCst));
    assert_eq!(
        manager
            .write("write-1", b"x")
            .await
            .expect_err("closed write handle should not exist")
            .kind(),
        io::ErrorKind::NotFound
    );
}

#[tokio::test]
async fn closing_open_reserves_handle_id_until_cleanup_finishes() {
    let manager = FileHandleManager::default();
    let first_file_system = Arc::new(BlockingFileSystem::default());
    let path = AbsolutePathBuf::from_absolute_path(std::env::temp_dir()).expect("absolute path");
    let first_open = {
        let manager = manager.clone();
        let file_system = Arc::clone(&first_file_system);
        let path = path.clone();
        tokio::spawn(async move {
            manager
                .open_read(file_system, "read-1".to_string(), &path, None)
                .await
        })
    };

    first_file_system.read_started.notified().await;
    let close = {
        let manager = manager.clone();
        tokio::spawn(async move { manager.close("read-1").await })
    };
    tokio::task::yield_now().await;

    let second_file_system = Arc::new(BlockingFileSystem::default());
    assert_eq!(
        manager
            .open_read(
                second_file_system.clone(),
                "read-1".to_string(),
                &path,
                None,
            )
            .await
            .expect_err("closing handle id should remain reserved")
            .kind(),
        io::ErrorKind::AlreadyExists
    );

    first_file_system.read_release.add_permits(1);
    let err = first_open
        .await
        .expect("join first read open")
        .expect_err("closed first read open should fail");
    assert_eq!(err.kind(), io::ErrorKind::Interrupted);
    close.await.expect("join read close").expect("close read");
    assert!(first_file_system.read_handle_dropped.load(Ordering::SeqCst));

    let second_open = {
        let manager = manager.clone();
        let file_system = Arc::clone(&second_file_system);
        tokio::spawn(async move {
            manager
                .open_read(file_system, "read-1".to_string(), &path, None)
                .await
        })
    };
    second_file_system.read_started.notified().await;
    second_file_system.read_release.add_permits(1);
    assert_eq!(
        second_open
            .await
            .expect("join second read open")
            .expect("second read open"),
        16
    );

    manager.close("read-1").await.expect("close second read");
    assert!(
        second_file_system
            .read_handle_dropped
            .load(Ordering::SeqCst)
    );
}

#[tokio::test]
async fn read_uses_handle_authoritative_eof() {
    let manager = FileHandleManager::default();
    let file_system = Arc::new(BlockingFileSystem::default());
    let path = AbsolutePathBuf::from_absolute_path(std::env::temp_dir()).expect("absolute path");
    let open = {
        let manager = manager.clone();
        let file_system = Arc::clone(&file_system);
        tokio::spawn(async move {
            manager
                .open_read(file_system, "read-1".to_string(), &path, None)
                .await
        })
    };

    file_system.read_started.notified().await;
    file_system.read_release.add_permits(1);
    open.await
        .expect("join read open")
        .expect("open read handle");

    assert_eq!(
        manager.read("read-1", 0, Some(16)).await.expect("read"),
        FileReadChunk {
            data: vec![b'x'; 16],
            eof: true,
        }
    );
}

#[tokio::test]
async fn open_handle_count_is_bounded() {
    let manager = FileHandleManager::with_max_handles(1);
    let file_system = Arc::new(BlockingFileSystem::default());
    let path = AbsolutePathBuf::from_absolute_path(std::env::temp_dir()).expect("absolute path");
    let open = {
        let manager = manager.clone();
        let file_system = Arc::clone(&file_system);
        let path = path.clone();
        tokio::spawn(async move {
            manager
                .open_read(file_system, "read-1".to_string(), &path, None)
                .await
        })
    };

    file_system.read_started.notified().await;
    assert_eq!(
        manager
            .open_read(file_system.clone(), "read-2".to_string(), &path, None)
            .await
            .expect_err("second handle should exceed limit")
            .kind(),
        io::ErrorKind::InvalidInput
    );

    file_system.read_release.add_permits(1);
    open.await
        .expect("join read open")
        .expect("open read handle");
    manager.close("read-1").await.expect("close read");
}
