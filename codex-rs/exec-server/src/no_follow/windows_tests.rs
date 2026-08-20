use super::*;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_WRITE;
use windows_sys::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
use windows_sys::Win32::System::Pipes::CreateNamedPipeW;
use windows_sys::Win32::System::Pipes::PIPE_READMODE_BYTE;
use windows_sys::Win32::System::Pipes::PIPE_TYPE_BYTE;
use windows_sys::Win32::System::Pipes::PIPE_WAIT;

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
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            /*nmaxinstances*/ 1,
            /*noutbuffersize*/ 1,
            /*ninbuffersize*/ 1,
            /*ndefaulttimeout*/ 1_000,
            ptr::null(),
        )
    };
    assert_ne!(pipe, INVALID_HANDLE_VALUE);
    let _pipe = unsafe { OwnedHandle::from_raw_handle(pipe as RawHandle) };

    let error = open_handle(
        &client_path,
        FILE_GENERIC_READ | FILE_GENERIC_WRITE,
        FILE_OPEN,
        FILE_NON_DIRECTORY_FILE,
    )
    .expect_err("strict native open should reject named pipes");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert_eq!(error.to_string(), "path contains a reparse point");
}
