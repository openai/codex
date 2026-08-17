use super::*;
use pretty_assertions::assert_eq;
use std::time::Duration;
use windows_sys::Win32::Foundation::ERROR_PIPE_CONNECTED;
use windows_sys::Win32::Security::RevertToSelf;
use windows_sys::Win32::Security::SecurityIdentification;
use windows_sys::Win32::Security::TokenImpersonationLevel;
use windows_sys::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
use windows_sys::Win32::System::Pipes::ConnectNamedPipe;
use windows_sys::Win32::System::Pipes::CreateNamedPipeW;
use windows_sys::Win32::System::Pipes::ImpersonateNamedPipeClient;
use windows_sys::Win32::System::Pipes::PIPE_READMODE_BYTE;
use windows_sys::Win32::System::Pipes::PIPE_TYPE_BYTE;
use windows_sys::Win32::System::Pipes::PIPE_WAIT;
use windows_sys::Win32::System::Threading::GetCurrentThread;
use windows_sys::Win32::System::Threading::OpenThreadToken;

#[test]
fn pipe_server_cannot_impersonate_client() {
    let pipe_path = PathBuf::from(format!(
        r"\\.\pipe\codex-ide-context-{}",
        uuid::Uuid::new_v4()
    ));
    let wide_path = pipe_path
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
            /*noutbuffersize*/ 1024,
            /*ninbuffersize*/ 1024,
            /*ndefaulttimeout*/ 0,
            ptr::null(),
        )
    };
    assert_ne!(pipe, INVALID_HANDLE_VALUE);
    let pipe = OwnedHandle(pipe);

    let server = std::thread::spawn(move || {
        if unsafe { ConnectNamedPipe(pipe.raw(), ptr::null_mut()) } == FALSE {
            assert_eq!(
                io::Error::last_os_error().raw_os_error(),
                Some(ERROR_PIPE_CONNECTED as i32)
            );
        }

        let mut message = [0_u8];
        let mut bytes_read = 0;
        assert_ne!(
            unsafe {
                ReadFile(
                    pipe.raw(),
                    message.as_mut_ptr(),
                    message.len() as u32,
                    &mut bytes_read,
                    ptr::null_mut(),
                )
            },
            FALSE
        );
        assert_ne!(unsafe { ImpersonateNamedPipeClient(pipe.raw()) }, FALSE);

        let mut token = NULL_HANDLE;
        assert_ne!(
            unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, TRUE, &mut token) },
            FALSE
        );
        let token = OwnedHandle(token);
        let mut impersonation_level = 0;
        let mut return_length = 0;
        let result = unsafe {
            GetTokenInformation(
                token.raw(),
                TokenImpersonationLevel,
                (&raw mut impersonation_level).cast(),
                std::mem::size_of_val(&impersonation_level) as u32,
                &mut return_length,
            )
        };
        let reverted = unsafe { RevertToSelf() };
        assert_ne!(result, FALSE);
        assert_ne!(reverted, FALSE);
        impersonation_level
    });

    let deadline = Instant::now() + Duration::from_secs(/*secs*/ 5);
    let mut stream = WindowsPipeStream::connect(pipe_path, deadline).expect("connect pipe client");
    stream.write_all(&[1]).expect("write pipe message");

    assert_eq!(
        server.join().expect("join pipe server"),
        SecurityIdentification
    );
}
