#[cfg(any(freebsdlike, linux_android, target_os = "emscripten"))]
use nix::time::clock_getcpuclockid;
use nix::time::{clock_gettime, ClockId};

#[cfg(not(target_os = "redox"))]
#[test]
pub fn test_clock_getres() {
    nix::time::clock_getres(ClockId::CLOCK_REALTIME).expect("assertion failed");
}

#[test]
pub fn test_clock_gettime() {
    clock_gettime(ClockId::CLOCK_REALTIME).expect("assertion failed");
}

#[cfg(any(freebsdlike, linux_android, target_os = "emscripten"))]
#[test]
pub fn test_clock_getcpuclockid() {
    let clock_id = clock_getcpuclockid(nix::unistd::Pid::this()).unwrap();
    clock_gettime(clock_id).unwrap();
}

#[cfg(not(target_os = "redox"))]
#[test]
pub fn test_clock_id_res() {
    ClockId::CLOCK_REALTIME.res().unwrap();
}

#[test]
pub fn test_clock_id_now() {
    ClockId::CLOCK_REALTIME.now().unwrap();
}

#[cfg(any(freebsdlike, linux_android, target_os = "emscripten"))]
#[test]
pub fn test_clock_id_pid_cpu_clock_id() {
    ClockId::pid_cpu_clock_id(nix::unistd::Pid::this())
        .map(ClockId::now)
        .unwrap()
        .unwrap();
}

#[cfg(any(
    linux_android,
    solarish,
    freebsdlike,
    target_os = "netbsd",
    target_os = "hurd",
    target_os = "aix"
))]
#[test]
pub fn test_clock_nanosleep() {
    use nix::{
        sys::time::{TimeSpec, TimeValLike},
        time::{clock_nanosleep, ClockNanosleepFlags},
    };

    let sleep_time = TimeSpec::microseconds(1);
    let res = clock_nanosleep(
        ClockId::CLOCK_MONOTONIC,
        ClockNanosleepFlags::empty(),
        &sleep_time,
    );
    let expected = TimeSpec::microseconds(0);
    assert_eq!(res, Ok(expected));
}
