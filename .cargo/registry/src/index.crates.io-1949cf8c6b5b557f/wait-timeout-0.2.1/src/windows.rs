use std::io;
use std::os::windows::prelude::*;
use std::process::{Child, ExitStatus};
use std::time::Duration;

type DWORD = u32;
type HANDLE = *mut u8;

const WAIT_OBJECT_0: DWORD = 0x00000000;
const WAIT_TIMEOUT: DWORD = 258;

extern "system" {
    fn WaitForSingleObject(hHandle: HANDLE, dwMilliseconds: DWORD) -> DWORD;
}

pub fn wait_timeout(child: &mut Child, dur: Duration) -> io::Result<Option<ExitStatus>> {
    let ms = dur.as_millis();
    let ms = if ms > (DWORD::max_value() as u128) {
        DWORD::max_value()
    } else {
        ms as DWORD
    };
    unsafe {
        match WaitForSingleObject(child.as_raw_handle() as *mut _, ms) {
            WAIT_OBJECT_0 => {}
            WAIT_TIMEOUT => return Ok(None),
            _ => return Err(io::Error::last_os_error()),
        }
    }
    child.try_wait()
}
