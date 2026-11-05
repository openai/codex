//! Checks the access rights of the synchronous pipe endpoint inherited by a helper.

#![cfg(test)]

use std::io;
use std::os::windows::io::AsRawHandle;
use std::ptr;

use windows_sys::Win32::System::Pipes::GetNamedPipeHandleStateW;

use super::Direction;
use super::PipeSecurity;
use super::connected_pipe;

#[test]
fn child_can_read_output_pipe_mode() -> io::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let _guard = runtime.enter();
    let (_server, child) = connected_pipe(Direction::ChildWrites, &PipeSecurity::new()?)?;
    let mut mode = 0;
    // SAFETY: the file owns a live pipe handle and mode is writable for the call.
    if unsafe {
        GetNamedPipeHandleStateW(
            child.as_raw_handle(),
            &mut mode,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            /*nmaxusernamesize*/ 0,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
