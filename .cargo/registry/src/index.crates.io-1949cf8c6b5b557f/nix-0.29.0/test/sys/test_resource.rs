use nix::sys::resource::{getrlimit, setrlimit, Resource};
use nix::sys::resource::{getrusage, UsageWho};

/// Tests the RLIMIT_NOFILE functionality of getrlimit(), where the resource RLIMIT_NOFILE refers
/// to the maximum file descriptor number that can be opened by the process (aka the maximum number
/// of file descriptors that the process can open, since Linux 4.5).
///
/// We first fetch the existing file descriptor maximum values using getrlimit(), then edit the
/// soft limit to make sure it has a new and distinct value to the hard limit. We then setrlimit()
/// to put the new soft limit in effect, and then getrlimit() once more to ensure the limits have
/// been updated.
#[test]
pub fn test_resource_limits_nofile() {
    let (mut soft_limit, hard_limit) =
        getrlimit(Resource::RLIMIT_NOFILE).unwrap();

    soft_limit -= 1;
    assert_ne!(soft_limit, hard_limit);
    setrlimit(Resource::RLIMIT_NOFILE, soft_limit, hard_limit).unwrap();

    let (new_soft_limit, _) = getrlimit(Resource::RLIMIT_NOFILE).unwrap();
    assert_eq!(new_soft_limit, soft_limit);
}

#[test]
pub fn test_self_cpu_time() {
    // Make sure some CPU time is used.
    let mut numbers: Vec<i32> = (1..1_000_000).collect();
    numbers.iter_mut().for_each(|item| *item *= 2);

    // FIXME: this is here to help ensure the compiler does not optimize the whole
    // thing away. Replace the assert with test::black_box once stabilized.
    assert_eq!(numbers[100..200].iter().sum::<i32>(), 30_100);

    let usage = getrusage(UsageWho::RUSAGE_SELF)
        .expect("Failed to call getrusage for SELF");
    let rusage = usage.as_ref();

    let user = usage.user_time();
    assert!(user.tv_sec() > 0 || user.tv_usec() > 0);
    assert_eq!(user.tv_sec(), rusage.ru_utime.tv_sec);
    assert_eq!(user.tv_usec(), rusage.ru_utime.tv_usec);
}
