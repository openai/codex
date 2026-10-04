//! Exercises recovery and its file-safety boundaries on real Windows files.

use super::*;
use crate::token::LocalSid;
use pretty_assertions::assert_eq;
use std::ffi::OsString;
use std::fs;
use std::fs::File;
use std::os::windows::ffi::OsStringExt;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn load_propagates_io_errors_and_preserves_missing_file_behavior() -> Result<()> {
    let temp = TempDir::new()?;
    let state_path = temp.path().join(DENY_READ_ACL_STATE_FILE);
    assert!(load_state(&state_path)?.principals.is_empty());

    fs::create_dir(&state_path)?;
    let error = load_state(&state_path).err().expect("read error");
    assert!(error.downcast_ref::<std::io::Error>().is_some());
    Ok(())
}

#[test]
fn load_waits_for_a_legacy_writer_without_recovering_over_it() -> Result<()> {
    let temp = TempDir::new()?;
    let path = temp.path().join(DENY_READ_ACL_STATE_FILE);
    let valid = br#"{"principals":{"legacy":["existing"]}}"#;
    fs::write(&path, valid)?;
    let mut writer = OpenOptions::new().write(true).truncate(true).open(&path)?;
    std::thread::scope(|scope| -> Result<()> {
        let write = scope.spawn(move || {
            std::thread::sleep(Duration::from_millis(75));
            writer.write_all(valid)
        });
        let state = load_state(&path)?;
        write.join().expect("legacy writer")?;
        assert_eq!(state.principals["legacy"], vec![PathBuf::from("existing")]);
        Ok(())
    })?;

    let _writer = OpenOptions::new().write(true).truncate(true).open(&path)?;
    let error = load_state(&path)
        .err()
        .expect("active writer must prevent recovery");
    let error = error
        .downcast_ref::<std::io::Error>()
        .expect("sharing error");
    assert_eq!(
        error.raw_os_error(),
        Some(win::ERROR_SHARING_VIOLATION as i32)
    );
    Ok(())
}

#[test]
fn sync_rebuilds_corrupt_bookkeeping_only_after_successful_apply() -> Result<()> {
    let temp = TempDir::new()?;
    let state_dir = sandbox_dir(temp.path());
    fs::create_dir(&state_dir)?;
    let state_path = state_dir.join(DENY_READ_ACL_STATE_FILE);
    let malformed = b"   ";
    fs::write(&state_path, malformed)?;
    let blocker = temp.path().join("ordinary-file");
    fs::write(&blocker, b"not a directory")?;
    let sid_string = "S-1-5-21-10-20-30-40";
    let sid = LocalSid::from_string(sid_string)?;

    // Creating a child beneath a file fails before any ACL can be changed.
    let error = unsafe {
        sync_persistent_deny_read_acls(
            temp.path(),
            sid_string,
            &[blocker.join("child")],
            sid.as_ptr(),
        )
    }
    .expect_err("apply error must propagate");
    assert!(error.downcast_ref::<std::io::Error>().is_some());
    assert_eq!(fs::read(&state_path)?.as_slice(), malformed);

    let historical = temp.path().join("historical");
    let current = temp.path().join("current");
    fs::create_dir(&historical)?;
    unsafe { crate::acl::add_deny_read_ace(&historical, sid.as_ptr()) }?;
    let applied = unsafe {
        sync_persistent_deny_read_acls(
            temp.path(),
            sid_string,
            std::slice::from_ref(&current),
            sid.as_ptr(),
        )
    }?;
    assert_eq!(applied, vec![current.clone()]);
    let rebuilt: PersistentDenyReadAclState = serde_json::from_slice(&fs::read(&state_path)?)?;
    assert_eq!(
        rebuilt.principals,
        BTreeMap::from([(sid_string.to_owned(), vec![current])])
    );
    unsafe {
        let (dacl, descriptor) = crate::acl::fetch_dacl_handle(&historical)?;
        let preserved = crate::acl::dacl_has_read_deny_for_sid(dacl, sid.as_ptr());
        win::LocalFree(descriptor as win::HLOCAL);
        assert!(preserved, "unknown historical restrictions remain in place");
    }

    let error = unsafe {
        sync_persistent_deny_read_acls(
            &temp.path().join("missing-home"),
            sid_string,
            &[],
            sid.as_ptr(),
        )
    }
    .expect_err("save error must propagate");
    assert!(error.downcast_ref::<std::io::Error>().is_some());
    Ok(())
}

/// Linked bookkeeping, valid or malformed, must preserve its victim and existing denies without adding new ones.
#[test]
fn sync_rejects_linked_state_before_changing_acls() -> Result<()> {
    let temp = TempDir::new()?;
    let state_dir = sandbox_dir(temp.path());
    fs::create_dir(&state_dir)?;
    let path = state_dir.join(DENY_READ_ACL_STATE_FILE);
    let victim = temp.path().join("victim");
    let sid_string = "S-1-5-21-10-20-30-40";
    let sid = LocalSid::from_string(sid_string)?;
    let previous = temp.path().join("previous");
    let desired = temp.path().join("desired");
    fs::create_dir(&previous)?;
    fs::create_dir(&desired)?;
    unsafe { crate::acl::add_deny_read_ace(&previous, sid.as_ptr()) }?;
    let valid = serde_json::to_vec(&PersistentDenyReadAclState {
        principals: BTreeMap::from([(sid_string.to_owned(), vec![previous.clone()])]),
    })?;
    for contents in [b"not json: keep me".as_slice(), valid.as_slice()] {
        fs::write(&victim, contents)?;
        for link_kind in ["hardlink", "symlink"] {
            if link_kind == "hardlink" {
                fs::hard_link(&victim, &path)?;
            } else {
                std::os::windows::fs::symlink_file(&victim, &path)?;
            }
            let error = unsafe {
                sync_persistent_deny_read_acls(
                    temp.path(),
                    sid_string,
                    std::slice::from_ref(&desired),
                    sid.as_ptr(),
                )
            }
            .expect_err("linked state must be rejected");
            assert!(format!("{error:#}").contains(if link_kind == "hardlink" {
                "multiple links"
            } else {
                "reparse point"
            }));
            assert_eq!(fs::read(&victim)?, contents);
            for (target, expected_deny) in [(&previous, true), (&desired, false)] {
                unsafe {
                    let (dacl, descriptor) = crate::acl::fetch_dacl_handle(target)?;
                    let denied = crate::acl::dacl_has_read_deny_for_sid(dacl, sid.as_ptr());
                    win::LocalFree(descriptor as win::HLOCAL);
                    assert_eq!(denied, expected_deny, "rejected state must not change ACLs");
                }
            }
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

#[test]
fn store_keeps_file_identity_permissions_and_legacy_writer_compatibility() -> Result<()> {
    let temp = TempDir::new()?;
    let path = temp.path().join(DENY_READ_ACL_STATE_FILE);
    let state = PersistentDenyReadAclState {
        principals: BTreeMap::from([("kept".to_owned(), vec![PathBuf::from("secret")])]),
    };
    fs::write(
        &path,
        b"malformed longer than the new state                                               ",
    )?;
    let sid = LocalSid::from_string("S-1-5-21-10-20-30-40")?;
    unsafe { crate::acl::add_deny_read_ace(&path, sid.as_ptr()) }?;
    let file = File::open(&path)?;
    let mut before = unsafe { std::mem::zeroed::<winfs::BY_HANDLE_FILE_INFORMATION>() };
    assert_ne!(
        unsafe {
            winfs::GetFileInformationByHandle(file.as_raw_handle() as win::HANDLE, &mut before)
        },
        0
    );
    // A legacy writer can remain open after finishing valid bytes; normal store
    // must not newly fail just because that writer has not closed its handle.
    let _writer = OpenOptions::new().write(true).open(&path)?;
    // A delete-access holder may coexist with readers and legacy writers. Wait
    // for it to close without allowing replacement during validation or writing.
    let holder = OpenOptions::new()
        .access_mode(winfs::DELETE)
        .share_mode(winfs::FILE_SHARE_READ | winfs::FILE_SHARE_WRITE | winfs::FILE_SHARE_DELETE)
        .open(&path)?;
    std::thread::scope(|scope| -> Result<()> {
        let release = scope.spawn(move || {
            std::thread::sleep(Duration::from_millis(75));
            drop(holder);
        });
        store_state(&path, &state)?;
        release.join().expect("delete-access holder");
        Ok(())
    })?;
    let root = path.ancestors().last().expect("drive root");
    let mut volume = [0u16; 50];
    assert_ne!(
        unsafe {
            winfs::GetVolumeNameForVolumeMountPointW(
                crate::winutil::to_wide(root).as_ptr(),
                volume.as_mut_ptr(),
                volume.len() as u32,
            )
        },
        0
    );
    let end = volume
        .iter()
        .position(|unit| *unit == 0)
        .expect("volume name");
    let volume_path =
        PathBuf::from(OsString::from_wide(&volume[..end])).join(path.strip_prefix(root)?);
    store_state(&volume_path, &state)?;

    let alias_dir = TempDir::new()?;
    let junction = alias_dir.path().join("sandbox-junction");
    assert!(
        Command::new("cmd.exe")
            .args(["/c", "mklink", "/J"])
            .arg(&junction)
            .arg(temp.path())
            .output()?
            .status
            .success()
    );
    store_state(&junction.join(DENY_READ_ACL_STATE_FILE), &state)?;
    let reopened = File::open(&path)?;
    let mut after = unsafe { std::mem::zeroed::<winfs::BY_HANDLE_FILE_INFORMATION>() };
    assert_ne!(
        unsafe {
            winfs::GetFileInformationByHandle(reopened.as_raw_handle() as win::HANDLE, &mut after)
        },
        0
    );
    assert_eq!(
        (
            after.dwVolumeSerialNumber,
            after.nFileIndexHigh,
            after.nFileIndexLow
        ),
        (
            before.dwVolumeSerialNumber,
            before.nFileIndexHigh,
            before.nFileIndexLow
        )
    );
    assert_eq!(load_state(&path)?.principals, state.principals);
    unsafe {
        let (dacl, descriptor) = crate::acl::fetch_dacl_handle(&path)?;
        let preserved = crate::acl::dacl_has_read_deny_for_sid(dacl, sid.as_ptr());
        win::LocalFree(descriptor as win::HLOCAL);
        assert!(preserved);
    }
    Ok(())
}

// An unreleased delete-access handle must produce a bounded sharing error,
// leaving the existing bytes intact instead of truncating or replacing them.
#[test]
fn store_times_out_without_modifying_state_held_for_delete() -> Result<()> {
    let temp = TempDir::new()?;
    let path = temp.path().join(DENY_READ_ACL_STATE_FILE);
    let original = br#"{"principals":{"kept":["secret"]}}"#;
    fs::write(&path, original)?;
    let holder = OpenOptions::new()
        .access_mode(winfs::DELETE)
        .share_mode(winfs::FILE_SHARE_READ | winfs::FILE_SHARE_WRITE | winfs::FILE_SHARE_DELETE)
        .open(&path)?;
    let started = Instant::now();
    let error = store_state(&path, &PersistentDenyReadAclState::default())
        .expect_err("delete-access holder must block storage");
    assert!(started.elapsed() >= Duration::from_secs(2));
    assert_eq!(
        error
            .downcast_ref::<std::io::Error>()
            .expect("sharing error")
            .raw_os_error(),
        Some(win::ERROR_SHARING_VIOLATION as i32)
    );
    assert_eq!(fs::read(&path)?, original);
    drop(holder);
    assert_eq!(
        load_state(&path)?.principals["kept"],
        vec![PathBuf::from("secret")]
    );
    Ok(())
}

#[test]
fn recovered_bookkeeping_tracks_repeated_calls_and_a_later_profile_change() -> Result<()> {
    let temp = TempDir::new()?;
    let state_dir = sandbox_dir(temp.path());
    fs::create_dir(&state_dir)?;
    let path = state_dir.join(DENY_READ_ACL_STATE_FILE);
    // Simulate an interrupted in-place save, after an older deny was applied.
    fs::write(&path, br#"{"principals":"#)?;
    let sid_string = "S-1-5-21-10-20-30-40";
    let sid = LocalSid::from_string(sid_string)?;
    let historical = temp.path().join("historical");
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    fs::create_dir(&historical)?;
    unsafe { crate::acl::add_deny_read_ace(&historical, sid.as_ptr()) }?;

    for desired in [&first, &first, &second, &second] {
        let applied = unsafe {
            sync_persistent_deny_read_acls(
                temp.path(),
                sid_string,
                std::slice::from_ref(desired),
                sid.as_ptr(),
            )
        }?;
        assert_eq!(applied, vec![desired.clone()]);
        let state = load_state(&path)?;
        assert_eq!(
            state.principals,
            BTreeMap::from([(sid_string.to_owned(), vec![desired.clone()])])
        );
    }
    // Recovery must preserve both unknown historical denies and the current profile.
    for target in [&historical, &second] {
        unsafe {
            let (dacl, descriptor) = crate::acl::fetch_dacl_handle(target)?;
            let denied = crate::acl::dacl_has_read_deny_for_sid(dacl, sid.as_ptr());
            win::LocalFree(descriptor as win::HLOCAL);
            assert!(denied, "deny must remain at {}", target.display());
        }
    }
    Ok(())
}
