//! Exercise mutating fcntls through read-only descriptors in actual Seatbelt children.

#![cfg(target_os = "macos")]

use super::CreateSeatbeltCommandArgsParams;
use super::MACOS_PATH_TO_SEATBELT_EXECUTABLE;
use super::create_seatbelt_command_args;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use pretty_assertions::assert_eq;
use std::fs;
use std::fs::File;
use std::io;
use std::mem::size_of_val;
use std::os::fd::AsRawFd;
use std::os::macos::fs::MetadataExt;
use std::path::Path;
use std::process::Command;

const F_MAKECOMPRESSED: libc::c_int = 80;
const FIXTURE_ENV: &str = "CODEX_SEATBELT_FCNTL_FIXTURE";
const EXPECTATION_ENV: &str = "CODEX_SEATBELT_FCNTL_EXPECTATION";
const TRANSFER_UNSUPPORTED: &str = "F_TRANSFEREXTENTS positive control unsupported";
const FILES: [&str; 3] = ["canary", "workspace/donor", "receiver"];

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    bytes: Vec<u8>,
    blocks: u64,
    flags: u32,
    modified: (i64, i64),
    changed: (i64, i64),
}

fn snapshots(directory: &Path) -> Vec<Snapshot> {
    FILES
        .iter()
        .map(|name| {
            let path = directory.join(name);
            let metadata = fs::metadata(&path).expect("canary metadata");
            Snapshot {
                bytes: fs::read(path).expect("canary contents"),
                blocks: metadata.st_blocks(),
                flags: metadata.st_flags(),
                modified: (metadata.st_mtime(), metadata.st_mtime_nsec()),
                changed: (metadata.st_ctime(), metadata.st_ctime_nsec()),
            }
        })
        .collect()
}

#[test]
fn restricted_policies_deny_mutating_fcntls_through_read_only_descriptors() {
    let policies = [
        ("allow", FileSystemSandboxPolicy::unrestricted()),
        ("deny", FileSystemSandboxPolicy::read_only()),
        (
            "deny",
            FileSystemSandboxPolicy::workspace_write(
                &[],
                /*exclude_tmpdir_env_var*/ true,
                /*exclude_slash_tmp*/ true,
            ),
        ),
    ];
    for (expectation, policy) in policies {
        let fixture = tempfile::tempdir().expect("fixture directory");
        let workspace = fixture.path().join("workspace");
        fs::create_dir(&workspace).expect("workspace directory");
        fs::write(fixture.path().join(FILES[0]), b"outside sandbox canary\n")
            .expect("compression canary");
        fs::write(fixture.path().join(FILES[2]), []).expect("extent receiver");
        // Retain the parent's CLOEXEC descriptor until verification so closing
        // the child cannot discard the donor's unused preallocation.
        let donor = File::create(fixture.path().join(FILES[1])).expect("extent donor");
        {
            let mut allocation = libc::fstore_t {
                fst_flags: libc::F_ALLOCATEALL,
                fst_posmode: libc::F_PEOFPOSMODE,
                fst_offset: 0,
                fst_length: 65536,
                fst_bytesalloc: 0,
            };
            // SAFETY: the variadic argument points to a live fstore_t for F_PREALLOCATE.
            let result =
                unsafe { libc::fcntl(donor.as_raw_fd(), libc::F_PREALLOCATE, &raw mut allocation) };
            assert_eq!(
                result,
                0,
                "preallocate donor: {}",
                io::Error::last_os_error()
            );
        }
        let before = snapshots(fixture.path());
        assert!(
            before[1].blocks > 0,
            "control needs allocated extents beyond EOF"
        );
        let module = module_path!().split_once("::").expect("test module path").1;
        let args = create_seatbelt_command_args(CreateSeatbeltCommandArgsParams {
            command: vec![
                std::env::current_exe()
                    .expect("test executable")
                    .to_string_lossy()
                    .into_owned(),
                "--exact".into(),
                format!("{module}::fcntl_child"),
                "--ignored".into(),
                "--nocapture".into(),
            ],
            file_system_sandbox_policy: &policy,
            network_sandbox_policy: NetworkSandboxPolicy::Restricted,
            sandbox_policy_cwd: &workspace,
            enforce_managed_network: false,
            managed_network: None,
            environment_id: None,
            network: None,
            extra_allow_unix_sockets: &[],
        })
        .expect("generated Seatbelt arguments");
        let output = Command::new(MACOS_PATH_TO_SEATBELT_EXECUTABLE)
            .args(args)
            .current_dir(&workspace)
            .env(FIXTURE_ENV, fixture.path())
            .env(EXPECTATION_ENV, expectation)
            .output()
            .expect("run Seatbelt child");
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success()
            && stderr.contains("sandbox-exec: sandbox_apply: Operation not permitted")
        {
            eprintln!("skipping fcntl regression: nested Seatbelt unavailable");
            return;
        }
        assert!(output.status.success(), "{expectation}: {output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("fcntl probes completed"));
        let after = snapshots(fixture.path());
        if expectation == "allow" {
            assert!(after[0].bytes.is_empty(), "positive control must truncate");
            if stdout.contains(TRANSFER_UNSUPPORTED) {
                assert_eq!(
                    &after[1..],
                    &before[1..],
                    "unsupported transfer must preserve both files"
                );
            } else {
                assert!(
                    after[1].blocks < before[1].blocks,
                    "positive control must move extents"
                );
                assert!(
                    after[2].blocks > before[2].blocks,
                    "receiver must gain extents"
                );
            }
        } else {
            assert_eq!(
                after, before,
                "restricted policy must preserve all canaries"
            );
        }
        drop(donor);
    }
}

#[test]
#[ignore]
fn fcntl_child() {
    let Some(directory) = std::env::var_os(FIXTURE_ENV) else {
        return;
    };
    let directory = Path::new(&directory);
    let expectation = std::env::var(EXPECTATION_ENV).expect("expected fcntl result");
    // The positive control uses writable descriptors so it still works if the
    // OS independently fixes mutation through read-only descriptors.
    let files = FILES.map(|name| {
        fs::OpenOptions::new()
            .read(true)
            .write(expectation == "allow")
            .open(directory.join(name))
            .expect("canary descriptor")
    });
    let expected_mode = if expectation == "allow" {
        libc::O_RDWR
    } else {
        libc::O_RDONLY
    };
    for file in &files {
        // SAFETY: these fcntls need only the live descriptor and no variadic argument.
        assert!(unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFD) } >= 0);
        assert_eq!(
            unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) } & libc::O_ACCMODE,
            expected_mode
        );
    }
    if expectation == "deny" {
        let error = fs::OpenOptions::new()
            .write(true)
            .open(directory.join(FILES[0]))
            .expect_err("outside canary must be read-only");
        assert_eq!(error.raw_os_error(), Some(libc::EPERM));
    }
    let mut attributes = libc::attrlist {
        bitmapcount: 5,
        reserved: 0,
        commonattr: libc::ATTR_CMN_GEN_COUNT,
        volattr: 0,
        dirattr: 0,
        fileattr: 0,
        forkattr: 0,
    };
    let mut generation = [0_u32; 2];
    // SAFETY: both buffers have the layout and size requested by fgetattrlist.
    let result = unsafe {
        libc::fgetattrlist(
            files[0].as_raw_fd(),
            (&raw mut attributes).cast(),
            generation.as_mut_ptr().cast(),
            size_of_val(&generation),
            libc::FSOPT_ATTR_CMN_EXTENDED,
        )
    };
    assert_eq!(
        result,
        0,
        "generation query: {}",
        io::Error::last_os_error()
    );
    assert_eq!(generation[0] as usize, size_of_val(&generation));
    for (descriptor, selector, argument) in [
        (files[0].as_raw_fd(), F_MAKECOMPRESSED, generation[1]),
        (
            files[1].as_raw_fd(),
            libc::F_TRANSFEREXTENTS,
            files[2].as_raw_fd() as u32,
        ),
    ] {
        // SAFETY: these selectors take numeric arguments and all descriptors remain live.
        let result = unsafe { libc::fcntl(descriptor, selector, argument) };
        let error = io::Error::last_os_error().raw_os_error();
        match expectation.as_str() {
            "allow"
                if selector == libc::F_TRANSFEREXTENTS
                    && result == -1
                    && matches!(error, Some(libc::EINVAL) | Some(libc::ENOTSUP)) =>
            {
                println!("{TRANSFER_UNSUPPORTED}");
            }
            "allow" => assert_eq!(result, 0, "fcntl {selector}: {error:?}"),
            "deny" => assert_eq!((result, error), (-1, Some(libc::EPERM)), "fcntl {selector}"),
            _ => panic!("invalid fcntl expectation: {expectation}"),
        }
    }
    println!("fcntl probes completed");
}
