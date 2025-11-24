// Copyright 2021 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

#![cfg(feature = "json")]
#![allow(clippy::undocumented_unsafe_blocks)]

use seccompiler::{apply_filter, compile_from_json, BpfProgram};
use std::convert::TryInto;
use std::env::consts::ARCH;
use std::io::Read;
use std::thread;

const FAILURE_CODE: i32 = 1000;

enum Errno {
    Equals(i32),
    NotEquals(i32),
    None,
}

fn validate_json_filter<R: Read>(reader: R, validation_fn: fn(), errno: Errno) {
    let mut filters = compile_from_json(reader, ARCH.try_into().unwrap()).unwrap();
    let filter: BpfProgram = filters.remove("main_thread").unwrap();

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
fn test_empty_filter_allow_all() {
    // An empty filter should always return the default action.
    // For example, for an empty allowlist, it should always trap/kill,
    // for an empty denylist, it should allow all system calls.

    let json_input = r#"{
            "main_thread": {
                "mismatch_action": "allow",
                "match_action": "trap",
                "filter": []
            }
        }"#;

    let mut filters = compile_from_json(json_input.as_bytes(), ARCH.try_into().unwrap()).unwrap();
    let filter = filters.remove("main_thread").unwrap();
    // This should allow any system calls.
    let pid = thread::spawn(move || {
        let seccomp_level = unsafe { libc::prctl(libc::PR_GET_SECCOMP) };
        assert_eq!(seccomp_level, 0);
        // Install the filter.
        apply_filter(&filter).unwrap();
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
fn test_empty_filter_deny_all() {
    let json_input = r#"{
            "main_thread": {
                "mismatch_action": "kill_process",
                "match_action": "allow",
                "filter": []
            }
        }"#;

    let mut filters = compile_from_json(json_input.as_bytes(), ARCH.try_into().unwrap()).unwrap();
    let filter = filters.remove("main_thread").unwrap();

    // We need to use `fork` instead of `thread::spawn` to prohibit cargo from failing the test
    // due to the SIGSYS exit code.
    let pid = unsafe { libc::fork() };

    match pid {
        0 => {
            let seccomp_level = unsafe { libc::prctl(libc::PR_GET_SECCOMP) };
            assert_eq!(seccomp_level, 0);
            // Install the filter.
            apply_filter(&filter).unwrap();
            // this syscall will fail
            unsafe { libc::prctl(libc::PR_GET_SECCOMP) };
        }
        child_pid => {
            let mut child_status: i32 = -1;
            let pid_done = unsafe { libc::waitpid(child_pid, &mut child_status, 0) };
            assert_eq!(pid_done, child_pid);

            assert!(libc::WIFSIGNALED(child_status));
            assert_eq!(libc::WTERMSIG(child_status), libc::SIGSYS);
        }
    }
}

#[test]
fn test_invalid_architecture() {
    // A filter compiled for another architecture should kill the process upon evaluation.
    // The process will appear as if it received a SIGSYS.
    let mut arch = "aarch64";

    if ARCH == "aarch64" {
        arch = "x86_64";
    }

    let json_input = r#"{
        "main_thread": {
            "mismatch_action": "allow",
            "match_action": "trap",
            "filter": []
        }
    }"#;

    let mut filters = compile_from_json(json_input.as_bytes(), arch.try_into().unwrap()).unwrap();
    let filter = filters.remove("main_thread").unwrap();

    let pid = unsafe { libc::fork() };
    match pid {
        0 => {
            apply_filter(&filter).unwrap();

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
fn test_complex_filter() {
    let json_input = r#"{
            "main_thread": {
                "mismatch_action": {"errno" : 1000},
                "match_action": "allow",
                "filter": [
                    {
                        "syscall": "rt_sigprocmask",
                        "comment": "extra syscalls needed by the test runtime"
                    },
                    {
                        "syscall": "sigaltstack"
                    },
                    {
                        "syscall": "munmap"
                    },
                    {
                        "syscall": "exit"
                    },
                    {
                        "syscall": "rt_sigreturn"
                    },
                    {
                        "syscall": "futex"
                    },
                    {
                        "syscall": "getpid",
                        "comment": "start of the actual filter we want to test."
                    },
                    {
                        "syscall": "ioctl",
                        "args": [
                            {
                                "index": 2,
                                "type": "dword",
                                "op": "le",
                                "val": 14
                            },
                            {
                                "index": 2,
                                "type": "dword",
                                "op": "ne",
                                "val": 13
                            }
                        ]
                    },
                    {
                        "syscall": "ioctl",
                        "args": [
                            {
                                "index": 2,
                                "type": "dword",
                                "op": "gt",
                                "val": 20
                            },
                            {
                                "index": 2,
                                "type": "dword",
                                "op": "lt",
                                "val": 40
                            }
                        ]
                    },
                    {
                        "syscall": "ioctl",
                        "args": [
                            {
                                "index": 0,
                                "type": "dword",
                                "op": "eq",
                                "val": 1
                            },
                            {
                                "index": 2,
                                "type": "dword",
                                "op": "eq",
                                "val": 15
                            }
                        ]
                    },
                    {
                        "syscall": "ioctl",
                        "args": [
                            {
                                "index": 2,
                                "type": "qword",
                                "op": "eq",
                                "val": 4294967336,
                                "comment": "u32::MAX as u64 + 41"
                            }
                        ]
                    },
                    {
                        "syscall": "madvise",
                        "args": [
                            {
                                "index": 0,
                                "type": "dword",
                                "op": "eq",
                                "val": 0
                            },
                            {
                                "index": 1,
                                "type": "dword",
                                "op": "eq",
                                "val": 0
                            }
                        ]
                    }
                ]
            }
        }"#;

    // check syscalls that are supposed to work
    {
        validate_json_filter(
            json_input.as_bytes(),
            || unsafe {
                libc::ioctl(0, 0, 12);
            },
            Errno::NotEquals(FAILURE_CODE),
        );

        validate_json_filter(
            json_input.as_bytes(),
            || unsafe {
                libc::ioctl(0, 0, 14);
            },
            Errno::NotEquals(FAILURE_CODE),
        );

        validate_json_filter(
            json_input.as_bytes(),
            || unsafe {
                libc::ioctl(0, 0, 21);
            },
            Errno::NotEquals(FAILURE_CODE),
        );

        validate_json_filter(
            json_input.as_bytes(),
            || unsafe {
                libc::ioctl(0, 0, 39);
            },
            Errno::NotEquals(FAILURE_CODE),
        );

        validate_json_filter(
            json_input.as_bytes(),
            || unsafe {
                libc::ioctl(1, 0, 15);
            },
            Errno::NotEquals(FAILURE_CODE),
        );

        validate_json_filter(
            json_input.as_bytes(),
            || unsafe {
                libc::ioctl(0, 0, u32::MAX as u64 + 41);
            },
            Errno::NotEquals(FAILURE_CODE),
        );

        validate_json_filter(
            json_input.as_bytes(),
            || unsafe {
                libc::madvise(std::ptr::null_mut(), 0, 0);
            },
            Errno::NotEquals(FAILURE_CODE),
        );

        validate_json_filter(
            json_input.as_bytes(),
            || unsafe {
                assert!(libc::getpid() > 0);
            },
            Errno::None,
        );
    }

    // check syscalls that are not supposed to work
    {
        validate_json_filter(
            json_input.as_bytes(),
            || unsafe {
                libc::ioctl(0, 0, 13);
            },
            Errno::Equals(FAILURE_CODE),
        );

        validate_json_filter(
            json_input.as_bytes(),
            || unsafe {
                libc::ioctl(0, 0, 16);
            },
            Errno::Equals(FAILURE_CODE),
        );

        validate_json_filter(
            json_input.as_bytes(),
            || unsafe {
                libc::ioctl(0, 0, 17);
            },
            Errno::Equals(FAILURE_CODE),
        );

        validate_json_filter(
            json_input.as_bytes(),
            || unsafe {
                libc::ioctl(0, 0, 18);
            },
            Errno::Equals(FAILURE_CODE),
        );

        validate_json_filter(
            json_input.as_bytes(),
            || unsafe {
                libc::ioctl(0, 0, 19);
            },
            Errno::Equals(FAILURE_CODE),
        );

        validate_json_filter(
            json_input.as_bytes(),
            || unsafe {
                libc::ioctl(0, 0, 20);
            },
            Errno::Equals(FAILURE_CODE),
        );

        validate_json_filter(
            json_input.as_bytes(),
            || unsafe {
                libc::ioctl(0, 0, u32::MAX as u64 + 42);
            },
            Errno::Equals(FAILURE_CODE),
        );

        validate_json_filter(
            json_input.as_bytes(),
            || unsafe {
                libc::madvise(std::ptr::null_mut(), 1, 0);
            },
            Errno::Equals(FAILURE_CODE),
        );

        validate_json_filter(
            json_input.as_bytes(),
            || unsafe {
                assert_eq!(libc::getuid() as i32, -FAILURE_CODE);
            },
            Errno::None,
        );
    }
}
