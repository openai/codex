// Copyright 2021 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

#![allow(clippy::undocumented_unsafe_blocks)]

use std::collections::BTreeMap;

use seccompiler::SeccompCmpArgLen::*;
use seccompiler::SeccompCmpOp::*;
use seccompiler::{
    apply_filter, sock_filter, BpfProgram, Error, SeccompAction, SeccompCondition as Cond,
    SeccompFilter, SeccompRule,
};
use std::convert::TryInto;
use std::env::consts::ARCH;
use std::thread;

// The type of the `req` parameter is different for the `musl` library. This will enable
// successful build for other non-musl libraries.
#[cfg(target_env = "musl")]
type IoctlRequest = i32;
#[cfg(not(target_env = "musl"))]
type IoctlRequest = u64;

// We use KVM_GET_PIT2 as the second parameter for ioctl syscalls in some unit tests
// because has non-0 MSB and LSB.
const KVM_GET_PIT2: u64 = 0x8070_ae9f;
const KVM_GET_PIT2_MSB: u64 = 0x0000_ae9f;
const KVM_GET_PIT2_LSB: u64 = 0x8070_0000;

const FAILURE_CODE: i32 = 1000;

const EXTRA_SYSCALLS: [i64; 6] = [
    libc::SYS_rt_sigprocmask,
    libc::SYS_sigaltstack,
    libc::SYS_munmap,
    libc::SYS_exit,
    libc::SYS_rt_sigreturn,
    libc::SYS_futex,
];

enum Errno {
    Equals(i32),
    NotEquals(i32),
    None,
}

fn validate_seccomp_filter(rules: Vec<(i64, Vec<SeccompRule>)>, validation_fn: fn(), errno: Errno) {
    let mut rule_map: BTreeMap<i64, Vec<SeccompRule>> = rules.into_iter().collect();

    // Make sure the extra needed syscalls are allowed
    for syscall in EXTRA_SYSCALLS.iter() {
        rule_map.entry(*syscall).or_default();
    }

    // Build seccomp filter.
    let filter = SeccompFilter::new(
        rule_map,
        SeccompAction::Errno(FAILURE_CODE as u32),
        SeccompAction::Allow,
        ARCH.try_into().unwrap(),
    )
    .unwrap();

    let filter: BpfProgram = filter.try_into().unwrap();

    // We need to run the validation inside another thread in order to avoid setting
    // the seccomp filter for the entire unit tests process.
    let returned_errno = thread::spawn(move || {
        // Install the filter.
        apply_filter(&filter).unwrap();

        // Call the validation fn.
        validation_fn();

        // Return errno.
        std::io::Error::last_os_error().raw_os_error().unwrap()
    })
    .join()
    .unwrap();

    match errno {
        Errno::Equals(no) => assert_eq!(returned_errno, no),
        Errno::NotEquals(no) => assert_ne!(returned_errno, no),
        Errno::None => {}
    };
}

#[test]
fn test_empty_filter() {
    // An empty filter should always return the default action.
    // For example, for an empty allowlist, it should always trap/kill,
    // for an empty denylist, it should allow allow all system calls.

    let filter = SeccompFilter::new(
        BTreeMap::new(),
        SeccompAction::Allow,
        SeccompAction::Trap,
        ARCH.try_into().unwrap(),
    )
    .unwrap();
    let prog: BpfProgram = filter.try_into().unwrap();

    // This should allow any system calls.
    let pid = thread::spawn(move || {
        let seccomp_level = unsafe { libc::prctl(libc::PR_GET_SECCOMP) };
        assert_eq!(seccomp_level, 0);
        // Install the filter.
        apply_filter(&prog).unwrap();

        let seccomp_level = unsafe { libc::prctl(libc::PR_GET_SECCOMP) };
        assert_eq!(seccomp_level, 2);

        unsafe { libc::getpid() }
    })
    .join()
    .unwrap();

    // Check that the getpid syscall returned successfully.
    assert!(pid > 0);
}

#[test]
fn test_invalid_architecture() {
    // A filter compiled for another architecture should kill the process upon evaluation.
    // The process will appear as if it received a SIGSYS.
    let mut arch = "aarch64";

    if ARCH == "aarch64" {
        arch = "x86_64";
    }

    let filter = SeccompFilter::new(
        BTreeMap::new(),
        SeccompAction::Allow,
        SeccompAction::Trap,
        arch.try_into().unwrap(),
    )
    .unwrap();
    let prog: BpfProgram = filter.try_into().unwrap();

    let pid = unsafe { libc::fork() };
    match pid {
        0 => {
            apply_filter(&prog).unwrap();

            unsafe {
                libc::getpid();
            }
        }
        child_pid => {
            let mut child_status: i32 = -1;
            let pid_done = unsafe { libc::waitpid(child_pid, &mut child_status, 0) };
            assert_eq!(pid_done, child_pid);

            assert!(libc::WIFSIGNALED(child_status));
            assert_eq!(libc::WTERMSIG(child_status), libc::SIGSYS);
        }
    };
}

#[test]
fn test_eq_operator() {
    // check use cases for DWORD
    let rules = vec![(
        libc::SYS_ioctl,
        vec![SeccompRule::new(vec![Cond::new(1, Dword, Eq, KVM_GET_PIT2).unwrap()]).unwrap()],
    )];
    // check syscalls that are supposed to work
    validate_seccomp_filter(
        rules.clone(),
        || unsafe {
            libc::ioctl(0, KVM_GET_PIT2 as IoctlRequest);
        },
        Errno::NotEquals(FAILURE_CODE),
    );
    // check syscalls that are not supposed to work
    validate_seccomp_filter(
        rules,
        || unsafe {
            libc::ioctl(0, 0);
        },
        Errno::Equals(FAILURE_CODE),
    );

    // check use cases for QWORD
    let rules = vec![(
        libc::SYS_ioctl,
        vec![SeccompRule::new(vec![Cond::new(2, Qword, Eq, u64::MAX).unwrap()]).unwrap()],
    )];
    // check syscalls that are supposed to work
    validate_seccomp_filter(
        rules.clone(),
        || unsafe {
            libc::ioctl(0, 0, u64::MAX);
        },
        Errno::NotEquals(FAILURE_CODE),
    );
    // check syscalls that are not supposed to work
    validate_seccomp_filter(
        rules,
        || unsafe {
            libc::ioctl(0, 0, 0);
        },
        Errno::Equals(FAILURE_CODE),
    );
}

#[test]
fn test_ge_operator() {
    // check use case for DWORD
    let rules = vec![(
        libc::SYS_ioctl,
        vec![SeccompRule::new(vec![Cond::new(1, Dword, Ge, KVM_GET_PIT2).unwrap()]).unwrap()],
    )];
    // check syscalls that are supposed to work
    validate_seccomp_filter(
        rules.clone(),
        || unsafe {
            libc::ioctl(0, KVM_GET_PIT2 as IoctlRequest);
            libc::ioctl(0, (KVM_GET_PIT2 + 1) as IoctlRequest);
        },
        Errno::NotEquals(FAILURE_CODE),
    );
    // check syscalls that are not supposed to work
    validate_seccomp_filter(
        rules,
        || unsafe {
            libc::ioctl(0, (KVM_GET_PIT2 - 1) as IoctlRequest);
        },
        Errno::Equals(FAILURE_CODE),
    );

    // check use case for QWORD
    let rules = vec![(
        libc::SYS_ioctl,
        vec![
            SeccompRule::new(vec![Cond::new(2, Qword, Ge, u64::from(u32::MAX)).unwrap()]).unwrap(),
        ],
    )];
    // check syscalls that are supposed to work
    validate_seccomp_filter(
        rules.clone(),
        || unsafe {
            libc::ioctl(0, 0, u64::from(u32::MAX));
            libc::ioctl(0, 0, u64::from(u32::MAX) + 1);
        },
        Errno::NotEquals(FAILURE_CODE),
    );
    // check syscalls that are not supposed to work
    validate_seccomp_filter(
        rules,
        || unsafe {
            libc::ioctl(0, 0, 1);
        },
        Errno::Equals(FAILURE_CODE),
    );
}

#[test]
fn test_gt_operator() {
    // check use case for DWORD
    let rules = vec![(
        libc::SYS_ioctl,
        vec![SeccompRule::new(vec![Cond::new(1, Dword, Gt, KVM_GET_PIT2).unwrap()]).unwrap()],
    )];
    // check syscalls that are supposed to work
    validate_seccomp_filter(
        rules.clone(),
        || unsafe {
            libc::ioctl(0, (KVM_GET_PIT2 + 1) as IoctlRequest);
        },
        Errno::NotEquals(FAILURE_CODE),
    );
    // check syscalls that are not supposed to work
    validate_seccomp_filter(
        rules,
        || unsafe {
            libc::ioctl(0, KVM_GET_PIT2 as IoctlRequest);
        },
        Errno::Equals(FAILURE_CODE),
    );

    // check use case for QWORD
    let rules = vec![(
        libc::SYS_ioctl,
        vec![SeccompRule::new(vec![
            Cond::new(2, Qword, Gt, u64::from(u32::MAX) + 10).unwrap()
        ])
        .unwrap()],
    )];
    // check syscalls that are supposed to work
    validate_seccomp_filter(
        rules.clone(),
        || unsafe {
            libc::ioctl(0, 0, u64::from(u32::MAX) + 11);
        },
        Errno::NotEquals(FAILURE_CODE),
    );
    // check syscalls that are not supposed to work
    validate_seccomp_filter(
        rules,
        || unsafe {
            libc::ioctl(0, 0, u64::from(u32::MAX) + 10);
        },
        Errno::Equals(FAILURE_CODE),
    );
}

#[test]
fn test_le_operator() {
    // check use case for DWORD
    let rules = vec![(
        libc::SYS_ioctl,
        vec![SeccompRule::new(vec![Cond::new(1, Dword, Le, KVM_GET_PIT2).unwrap()]).unwrap()],
    )];
    // check syscalls that are supposed to work
    validate_seccomp_filter(
        rules.clone(),
        || unsafe {
            libc::ioctl(0, KVM_GET_PIT2 as IoctlRequest);
            libc::ioctl(0, (KVM_GET_PIT2 - 1) as IoctlRequest);
        },
        Errno::NotEquals(FAILURE_CODE),
    );
    // check syscalls that are not supposed to work
    validate_seccomp_filter(
        rules,
        || unsafe {
            libc::ioctl(0, (KVM_GET_PIT2 + 1) as IoctlRequest);
        },
        Errno::Equals(FAILURE_CODE),
    );

    // check use case for QWORD
    let rules = vec![(
        libc::SYS_ioctl,
        vec![SeccompRule::new(vec![
            Cond::new(2, Qword, Le, u64::from(u32::MAX) + 10).unwrap()
        ])
        .unwrap()],
    )];
    // check syscalls that are supposed to work
    validate_seccomp_filter(
        rules.clone(),
        || unsafe {
            libc::ioctl(0, 0, u64::from(u32::MAX) + 10);
            libc::ioctl(0, 0, u64::from(u32::MAX) + 9);
        },
        Errno::NotEquals(FAILURE_CODE),
    );
    // check syscalls that are not supposed to work
    validate_seccomp_filter(
        rules,
        || unsafe {
            libc::ioctl(0, 0, u64::from(u32::MAX) + 11);
        },
        Errno::Equals(FAILURE_CODE),
    );
}

#[test]
fn test_lt_operator() {
    // check use case for DWORD
    let rules = vec![(
        libc::SYS_ioctl,
        vec![SeccompRule::new(vec![Cond::new(1, Dword, Lt, KVM_GET_PIT2).unwrap()]).unwrap()],
    )];
    // check syscalls that are supposed to work
    validate_seccomp_filter(
        rules.clone(),
        || unsafe {
            libc::ioctl(0, (KVM_GET_PIT2 - 1) as IoctlRequest);
        },
        Errno::NotEquals(FAILURE_CODE),
    );
    // check syscalls that are not supposed to work
    validate_seccomp_filter(
        rules,
        || unsafe {
            libc::ioctl(0, KVM_GET_PIT2 as IoctlRequest);
        },
        Errno::Equals(FAILURE_CODE),
    );

    // check use case for QWORD
    let rules = vec![(
        libc::SYS_ioctl,
        vec![SeccompRule::new(vec![
            Cond::new(2, Qword, Lt, u64::from(u32::MAX) + 10).unwrap()
        ])
        .unwrap()],
    )];
    // check syscalls that are supposed to work
    validate_seccomp_filter(
        rules.clone(),
        || unsafe {
            libc::ioctl(0, 0, u64::from(u32::MAX) + 9);
        },
        Errno::NotEquals(FAILURE_CODE),
    );
    // check syscalls that are not supposed to work
    validate_seccomp_filter(
        rules,
        || unsafe {
            libc::ioctl(0, 0, u64::from(u32::MAX) + 10);
        },
        Errno::Equals(FAILURE_CODE),
    );
}

#[test]
fn test_masked_eq_operator() {
    // check use case for DWORD
    let rules = vec![(
        libc::SYS_ioctl,
        vec![SeccompRule::new(vec![Cond::new(
            1,
            Dword,
            MaskedEq(KVM_GET_PIT2_MSB),
            KVM_GET_PIT2,
        )
        .unwrap()])
        .unwrap()],
    )];
    // check syscalls that are supposed to work
    validate_seccomp_filter(
        rules.clone(),
        || unsafe {
            libc::ioctl(0, KVM_GET_PIT2 as IoctlRequest);
            libc::ioctl(0, KVM_GET_PIT2_MSB as IoctlRequest);
        },
        Errno::NotEquals(FAILURE_CODE),
    );
    // check syscalls that are not supposed to work
    validate_seccomp_filter(
        rules,
        || unsafe {
            libc::ioctl(0, KVM_GET_PIT2_LSB as IoctlRequest);
        },
        Errno::Equals(FAILURE_CODE),
    );

    // check use case for QWORD
    let rules = vec![(
        libc::SYS_ioctl,
        vec![SeccompRule::new(vec![Cond::new(
            2,
            Qword,
            MaskedEq(u64::from(u32::MAX)),
            u64::MAX,
        )
        .unwrap()])
        .unwrap()],
    )];
    // check syscalls that are supposed to work
    validate_seccomp_filter(
        rules.clone(),
        || unsafe {
            libc::ioctl(0, 0, u64::from(u32::MAX));
            libc::ioctl(0, 0, u64::MAX);
        },
        Errno::NotEquals(FAILURE_CODE),
    );
    // check syscalls that are not supposed to work
    validate_seccomp_filter(
        rules,
        || unsafe {
            libc::ioctl(0, 0, 0);
        },
        Errno::Equals(FAILURE_CODE),
    );
}

#[test]
fn test_ne_operator() {
    // check use case for DWORD
    let rules = vec![(
        libc::SYS_ioctl,
        vec![SeccompRule::new(vec![Cond::new(1, Dword, Ne, KVM_GET_PIT2).unwrap()]).unwrap()],
    )];
    // check syscalls that are supposed to work
    validate_seccomp_filter(
        rules.clone(),
        || unsafe {
            libc::ioctl(0, 0);
        },
        Errno::NotEquals(FAILURE_CODE),
    );
    // check syscalls that are not supposed to work
    validate_seccomp_filter(
        rules,
        || unsafe {
            libc::ioctl(0, KVM_GET_PIT2 as IoctlRequest);
        },
        Errno::Equals(FAILURE_CODE),
    );

    // check use case for QWORD
    let rules = vec![(
        libc::SYS_ioctl,
        vec![SeccompRule::new(vec![Cond::new(2, Qword, Ne, u64::MAX).unwrap()]).unwrap()],
    )];
    // check syscalls that are supposed to work
    validate_seccomp_filter(
        rules.clone(),
        || unsafe {
            libc::ioctl(0, 0, 0);
        },
        Errno::NotEquals(FAILURE_CODE),
    );
    // check syscalls that are not supposed to work
    validate_seccomp_filter(
        rules,
        || unsafe {
            libc::ioctl(0, 0, u64::MAX);
        },
        Errno::Equals(FAILURE_CODE),
    );
}

#[test]
fn test_complex_filter() {
    let rules = vec![
        (
            libc::SYS_ioctl,
            vec![
                SeccompRule::new(vec![
                    Cond::new(2, Dword, Le, 14).unwrap(),
                    Cond::new(2, Dword, Ne, 13).unwrap(),
                ])
                .unwrap(),
                SeccompRule::new(vec![
                    Cond::new(2, Dword, Gt, 20).unwrap(),
                    Cond::new(2, Dword, Lt, 40).unwrap(),
                ])
                .unwrap(),
                SeccompRule::new(vec![
                    Cond::new(0, Dword, Eq, 1).unwrap(),
                    Cond::new(2, Dword, Eq, 15).unwrap(),
                ])
                .unwrap(),
                SeccompRule::new(vec![Cond::new(2, Qword, Eq, u32::MAX as u64 + 41).unwrap()])
                    .unwrap(),
            ],
        ),
        (
            libc::SYS_madvise,
            vec![SeccompRule::new(vec![
                Cond::new(0, Dword, Eq, 0).unwrap(),
                Cond::new(1, Dword, Eq, 0).unwrap(),
            ])
            .unwrap()],
        ),
        (libc::SYS_getpid, vec![]),
    ];
    // check syscalls that are supposed to work
    {
        validate_seccomp_filter(
            rules.clone(),
            || unsafe {
                libc::ioctl(0, 0, 12);
            },
            Errno::NotEquals(FAILURE_CODE),
        );

        validate_seccomp_filter(
            rules.clone(),
            || unsafe {
                libc::ioctl(0, 0, 14);
            },
            Errno::NotEquals(FAILURE_CODE),
        );

        validate_seccomp_filter(
            rules.clone(),
            || unsafe {
                libc::ioctl(0, 0, 21);
            },
            Errno::NotEquals(FAILURE_CODE),
        );

        validate_seccomp_filter(
            rules.clone(),
            || unsafe {
                libc::ioctl(0, 0, 39);
            },
            Errno::NotEquals(FAILURE_CODE),
        );

        validate_seccomp_filter(
            rules.clone(),
            || unsafe {
                libc::ioctl(1, 0, 15);
            },
            Errno::NotEquals(FAILURE_CODE),
        );

        validate_seccomp_filter(
            rules.clone(),
            || unsafe {
                libc::ioctl(0, 0, u32::MAX as u64 + 41);
            },
            Errno::NotEquals(FAILURE_CODE),
        );

        validate_seccomp_filter(
            rules.clone(),
            || unsafe {
                libc::madvise(std::ptr::null_mut(), 0, 0);
            },
            Errno::NotEquals(FAILURE_CODE),
        );

        validate_seccomp_filter(
            rules.clone(),
            || unsafe {
                assert!(libc::getpid() > 0);
            },
            Errno::None,
        );
    }

    // check syscalls that are not supposed to work
    {
        validate_seccomp_filter(
            rules.clone(),
            || unsafe {
                libc::ioctl(0, 0, 13);
            },
            Errno::Equals(FAILURE_CODE),
        );

        validate_seccomp_filter(
            rules.clone(),
            || unsafe {
                libc::ioctl(0, 0, 16);
            },
            Errno::Equals(FAILURE_CODE),
        );

        validate_seccomp_filter(
            rules.clone(),
            || unsafe {
                libc::ioctl(0, 0, 17);
            },
            Errno::Equals(FAILURE_CODE),
        );

        validate_seccomp_filter(
            rules.clone(),
            || unsafe {
                libc::ioctl(0, 0, 18);
            },
            Errno::Equals(FAILURE_CODE),
        );

        validate_seccomp_filter(
            rules.clone(),
            || unsafe {
                libc::ioctl(0, 0, 19);
            },
            Errno::Equals(FAILURE_CODE),
        );

        validate_seccomp_filter(
            rules.clone(),
            || unsafe {
                libc::ioctl(0, 0, 20);
            },
            Errno::Equals(FAILURE_CODE),
        );

        validate_seccomp_filter(
            rules.clone(),
            || unsafe {
                libc::ioctl(0, 0, u32::MAX as u64 + 42);
            },
            Errno::Equals(FAILURE_CODE),
        );

        validate_seccomp_filter(
            rules.clone(),
            || unsafe {
                libc::madvise(std::ptr::null_mut(), 1, 0);
            },
            Errno::Equals(FAILURE_CODE),
        );

        validate_seccomp_filter(
            rules,
            || unsafe {
                assert_eq!(libc::getuid() as i32, -FAILURE_CODE);
            },
            Errno::None,
        );
    }
}

#[test]
fn test_filter_apply() {
    // Test filter too large.
    thread::spawn(|| {
        let filter: BpfProgram = vec![
                sock_filter {
                    code: 6,
                    jt: 0,
                    jf: 0,
                    k: 0,
                };
                5000 // Limit is 4096
            ];

        // Apply seccomp filter.
        assert!(matches!(
            apply_filter(&filter).unwrap_err(),
            Error::Seccomp(_)
        ));
    })
    .join()
    .unwrap();

    // Test empty filter.
    thread::spawn(|| {
        let filter: BpfProgram = vec![];

        assert_eq!(filter.len(), 0);

        let seccomp_level = unsafe { libc::prctl(libc::PR_GET_SECCOMP) };
        assert_eq!(seccomp_level, 0);

        assert!(matches!(
            apply_filter(&filter).unwrap_err(),
            Error::EmptyFilter
        ));

        // test that seccomp level remains 0 on failure.
        let seccomp_level = unsafe { libc::prctl(libc::PR_GET_SECCOMP) };
        assert_eq!(seccomp_level, 0);
    })
    .join()
    .unwrap();

    // Test invalid BPF code.
    thread::spawn(|| {
        let filter = vec![sock_filter {
            // invalid opcode
            code: 9999,
            jt: 0,
            jf: 0,
            k: 0,
        }];

        let seccomp_level = unsafe { libc::prctl(libc::PR_GET_SECCOMP) };
        assert_eq!(seccomp_level, 0);

        assert!(matches!(
            apply_filter(&filter).unwrap_err(),
            Error::Seccomp(_)
        ));

        // test that seccomp level remains 0 on failure.
        let seccomp_level = unsafe { libc::prctl(libc::PR_GET_SECCOMP) };
        assert_eq!(seccomp_level, 0);
    })
    .join()
    .unwrap();

    // Test valid filter and assert seccomp level.
    thread::spawn(|| {
        let filter = SeccompFilter::new(
            BTreeMap::new(),
            SeccompAction::Allow,
            SeccompAction::Trap,
            ARCH.try_into().unwrap(),
        )
        .unwrap();
        let prog: BpfProgram = filter.try_into().unwrap();

        let seccomp_level = unsafe { libc::prctl(libc::PR_GET_SECCOMP) };
        assert_eq!(seccomp_level, 0);

        apply_filter(&prog).unwrap();

        // test that seccomp level is 2 (SECCOMP_MODE_FILTER).
        let seccomp_level = unsafe { libc::prctl(libc::PR_GET_SECCOMP) };
        assert_eq!(seccomp_level, 2);
    })
    .join()
    .unwrap();
}
