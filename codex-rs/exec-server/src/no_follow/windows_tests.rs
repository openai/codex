use super::*;
use pretty_assertions::assert_eq;
use windows_sys::Win32::Foundation::ERROR_PIPE_LISTENING;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_WRITE;
use windows_sys::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
use windows_sys::Win32::System::Pipes::ConnectNamedPipe;
use windows_sys::Win32::System::Pipes::CreateNamedPipeW;
use windows_sys::Win32::System::Pipes::PIPE_NOWAIT;
use windows_sys::Win32::System::Pipes::PIPE_READMODE_BYTE;
use windows_sys::Win32::System::Pipes::PIPE_TYPE_BYTE;

#[test]
fn relative_volume_opens_files_and_rejects_junctions() -> io::Result<()> {
    let temp = tempfile::tempdir()?;
    let real = std::fs::canonicalize(temp.path())?;
    let open = |path: &Path, access, disposition, options| {
        volume_fallback::open_relative_to_volume(&mut nt_path(path)?, access, disposition, options)
    };
    let existing = real.join("existing.txt");
    std::fs::File::from(open(
        &existing,
        FILE_WRITE_DATA,
        FILE_CREATE,
        FILE_NON_DIRECTORY_FILE,
    )?)
    .write_all(b"updated")?;
    let file = std::fs::File::from(open(
        &existing,
        FILE_GENERIC_READ,
        FILE_OPEN,
        FILE_NON_DIRECTORY_FILE,
    )?);
    assert_eq!(file.metadata()?.len(), 7);

    let drive_root = existing.ancestors().last().expect("absolute path root");
    let root = std::fs::File::from(open(
        drive_root,
        FILE_READ_ATTRIBUTES,
        FILE_OPEN,
        FILE_DIRECTORY_FILE,
    )?);
    assert!(root.metadata()?.is_dir());

    let junction_parent = tempfile::tempdir()?;
    let junction = junction_parent.path().join("junction");
    let result = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&real)
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    for (path, disposition, options) in [
        (junction.clone(), FILE_OPEN, FILE_DIRECTORY_FILE),
        (
            junction.join("existing.txt"),
            FILE_OPEN,
            FILE_NON_DIRECTORY_FILE,
        ),
        (
            junction.join("blocked.txt"),
            FILE_CREATE,
            FILE_NON_DIRECTORY_FILE,
        ),
    ] {
        let error = open(&path, FILE_READ_ATTRIBUTES, disposition, options)
            .expect_err("a junction must never be traversed");
        assert_eq!(error.to_string(), "path contains a reparse point");
    }
    assert!(!real.join("blocked.txt").exists());
    Ok(())
}

#[test]
fn volume_root_validation_rejects_devices_and_subdirectories() {
    for (path, expected) in [
        (r"\Device\HarddiskVolume1\", true),
        (r"\Device\HarddiskVolume123\", true),
        (r"\Device\HarddiskVolume\", false),
        (r"\Device\HarddiskVolume1", false),
        (r"\Device\HarddiskVolume1\workspace\", false),
        (r"\Device\HarddiskVolume1x\", false),
        (r"\Device\NamedPipe\", false),
        (r"\Device\Mup\", false),
        (r"\Device\CdRom0\", false),
    ] {
        let path = OsStr::new(path).encode_wide().collect::<Vec<_>>();
        assert_eq!(volume_fallback::is_local_volume_root(&path), expected);
    }
}

#[test]
fn native_open_rejects_named_pipes_before_connecting() {
    let pipe_name = format!("codex-no-follow-{}", uuid::Uuid::new_v4());
    let server_path = PathBuf::from(format!(r"\\.\pipe\{pipe_name}"));
    let client_path = PathBuf::from(format!(r"\\localhost\pipe\{pipe_name}"));
    let wide_path = server_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let pipe = unsafe {
        CreateNamedPipeW(
            wide_path.as_ptr(),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_NOWAIT,
            /*nmaxinstances*/ 1,
            /*noutbuffersize*/ 1,
            /*ninbuffersize*/ 1,
            /*ndefaulttimeout*/ 1_000,
            ptr::null(),
        )
    };
    assert_ne!(pipe, INVALID_HANDLE_VALUE);
    let _pipe = unsafe { OwnedHandle::from_raw_handle(pipe) };

    // A DOS drive alias could target a pipe; the root open must reject a pipe
    // before obtaining a client connection, even before final-path validation.
    let mut pipe_root: Vec<_> = format!(r"\Device\NamedPipe\{pipe_name}\")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    assert!(volume_fallback::open_volume_root(&mut pipe_root).is_err());

    let error = open_handle(
        &client_path,
        FILE_GENERIC_READ | FILE_GENERIC_WRITE,
        FILE_OPEN,
        FILE_NON_DIRECTORY_FILE,
    )
    .expect_err("strict native open should reject named pipes");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert_eq!(error.to_string(), "path contains a reparse point");
    assert_eq!(unsafe { ConnectNamedPipe(pipe, ptr::null_mut()) }, 0);
    assert_eq!(unsafe { GetLastError() }, ERROR_PIPE_LISTENING);
}
