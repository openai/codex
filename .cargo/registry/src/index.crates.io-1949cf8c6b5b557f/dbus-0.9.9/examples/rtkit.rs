/* This example asks the rtkit service to make our thread realtime priority.
   Rtkit puts a few limitations on us to let us become realtime, such as setting
   RLIMIT_RTTIME correctly, hence the syscalls. */

use std::cmp;
use std::time::Duration;

fn make_realtime(prio: u32) -> Result<u32, Box<dyn std::error::Error>> {
    let c = dbus::blocking::Connection::new_system()?;

    let proxy = c.with_proxy("org.freedesktop.RealtimeKit1", "/org/freedesktop/RealtimeKit1",
        Duration::from_millis(10000));
    use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;

    // Make sure we don't fail by wanting too much
    let max_prio: i32 = proxy.get("org.freedesktop.RealtimeKit1", "MaxRealtimePriority")?;
    let prio = cmp::min(prio, max_prio as u32);

    // Enforce RLIMIT_RTPRIO, also a must before asking rtkit for rtprio
    let max_rttime: i64 = proxy.get("org.freedesktop.RealtimeKit1", "RTTimeUSecMax")?;
    let new_limit = libc::rlimit64 { rlim_cur: max_rttime as u64, rlim_max: max_rttime as u64 };
    let mut old_limit = new_limit;
    if unsafe { libc::getrlimit64(libc::RLIMIT_RTTIME, &mut old_limit) } < 0 {
        return Err(Box::from("getrlimit failed"));
    }
    if unsafe { libc::setrlimit64(libc::RLIMIT_RTTIME, &new_limit) } < 0 {
        return Err(Box::from("setrlimit failed"));
    }

    // Finally, let's ask rtkit to make us realtime
    let thread_id = unsafe { libc::syscall(libc::SYS_gettid) };
    let r: Result<(), _> = proxy.method_call("org.freedesktop.RealtimeKit1", "MakeThreadRealtime", (thread_id as u64, prio));

    if r.is_err() {
        unsafe { libc::setrlimit64(libc::RLIMIT_RTTIME, &old_limit) };
    }

    r?;
    Ok(prio)
}


fn main() {
    match make_realtime(5) {
        Ok(n) => println!("Got rtprio, level {}", n),
        Err(e) => println!("No rtprio: {}", e),
    }
}
