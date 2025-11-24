#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]
#![cfg(windows)]
#![cfg(not(miri))]

use tokio::process::Command;
use windows_sys::Win32::System::Threading::GetProcessId;

#[tokio::test]
async fn obtain_raw_handle() {
    let mut cmd = Command::new("cmd");
    cmd.kill_on_drop(true);
    cmd.arg("/c");
    cmd.arg("pause");

    let child = cmd.spawn().unwrap();

    let orig_id = child.id().expect("missing id");
    assert!(orig_id > 0);

    let handle = child.raw_handle().expect("process stopped");
    let handled_id = unsafe { GetProcessId(handle as _) };
    assert_eq!(handled_id, orig_id);
}
