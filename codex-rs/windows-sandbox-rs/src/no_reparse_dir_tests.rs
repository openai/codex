use std::fs;
use std::os::windows::io::OwnedHandle;
use std::path::Path;
use std::process::Command;
use windows_sys::Win32::Storage::FileSystem::FILE_READ_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_DELETE;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
use windows_sys::Win32::Storage::FileSystem::FILE_TRAVERSE;
use windows_sys::Win32::Storage::FileSystem::READ_CONTROL;
use windows_sys::Win32::Storage::FileSystem::WRITE_DAC;

use anyhow::Result;
use pretty_assertions::assert_eq;

use super::DirectoryOpenDisposition;
use super::is_disk_volume;
use super::local_directory_nt_path;
use super::open_directory_no_reparse;
use super::validate_local_directory_path;
use windows_sys::Win32::Storage::FileSystem::QueryDosDeviceW;

fn open_for_acl(path: &Path, disposition: DirectoryOpenDisposition) -> Result<OwnedHandle> {
    open_directory_no_reparse(
        path,
        READ_CONTROL | WRITE_DAC,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        disposition,
    )
}

fn create_directory_junction(target: &Path, alias: &Path) -> Result<()> {
    let output = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(alias)
        .arg(target)
        .output()?;
    if !output.status.success() {
        anyhow::bail!(
            "mklink /J failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

#[test]
fn creates_and_opens_plain_directory_leaf() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let directory = temporary.path().join(".sandbox-bin");

    drop(open_for_acl(
        &directory,
        DirectoryOpenDisposition::OpenOrCreate,
    )?);
    let _handle = open_for_acl(&directory, DirectoryOpenDisposition::OpenExisting)?;

    assert!(directory.is_dir());
    Ok(())
}

#[test]
fn opens_drive_root_without_following_filesystem_reparses() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let directory = temporary.path().canonicalize()?;
    let root = directory
        .ancestors()
        .last()
        .expect("absolute path has a root");
    // canonicalize gives a verbatim drive path; exercise both supported forms.
    let ordinary = root
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_owned();
    let drive: Vec<_> = ordinary.encode_utf16().take(2).chain(Some(0)).collect();
    let mut device = vec![0; 32_768];
    if unsafe { QueryDosDeviceW(drive.as_ptr(), device.as_mut_ptr(), device.len() as u32) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    device.truncate(
        device
            .iter()
            .position(|unit| *unit == 0)
            .expect("QueryDosDeviceW returns a NUL-terminated target"),
    );
    assert!(is_disk_volume(&device));
    for path in [root, Path::new(&ordinary)] {
        for suffix in ["", "no-reparse-test"] {
            let mut expected = device.clone();
            expected.extend(format!(r"\{suffix}").encode_utf16());
            expected.push(0);
            assert_eq!(local_directory_nt_path(&path.join(suffix))?, expected);
        }
        drop(open_directory_no_reparse(
            path,
            FILE_READ_ATTRIBUTES | FILE_TRAVERSE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            DirectoryOpenDisposition::OpenExisting,
        )?);
    }
    Ok(())
}

#[test]
fn disk_volume_mapping_excludes_other_devices_and_subpaths() {
    for accepted in [
        r"\Device\HarddiskVolume0",
        r"\Device\HarddiskVolume1",
        r"\device\harddiskvolume123",
    ] {
        assert!(is_disk_volume(&accepted.encode_utf16().collect::<Vec<_>>()));
    }
    for rejected in [
        r"\Device\NamedPipe",
        r"\Device\NamedPipe\pipe",
        r"\Device\HarddiskVolume",
        r"\Device\HarddiskVolume1\directory",
        r"\Device\HarddiskVolume1suffix",
        r"\Device\HarddiskVolumeShadowCopy1",
        r"\Device\CdRom0",
        r"\??\C:\directory",
    ] {
        assert!(
            !is_disk_volume(&rejected.encode_utf16().collect::<Vec<_>>()),
            "accepted {rejected}"
        );
    }
}

#[test]
fn rejects_final_directory_junction() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let target = temporary.path().join("target");
    let alias = temporary.path().join(".sandbox-bin");
    fs::create_dir(&target)?;
    create_directory_junction(&target, &alias)?;

    let _ = open_for_acl(&alias, DirectoryOpenDisposition::OpenOrCreate)
        .expect_err("final directory junction must be rejected");
    fs::remove_dir(&alias)?;
    Ok(())
}

#[test]
fn rejects_ancestor_directory_junction() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let target_home = temporary.path().join("target-home");
    let alias_home = temporary.path().join("linked-home");
    fs::create_dir(&target_home)?;
    fs::create_dir(target_home.join(".sandbox-bin"))?;
    create_directory_junction(&target_home, &alias_home)?;

    let _ = open_for_acl(
        &alias_home.join(".sandbox-bin"),
        DirectoryOpenDisposition::OpenOrCreate,
    )
    .expect_err("ancestor directory junction must be rejected");
    fs::remove_dir(&alias_home)?;
    Ok(())
}

#[test]
fn open_existing_does_not_create_missing_directory() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let directory = temporary.path().join("missing");
    assert!(open_for_acl(&directory, DirectoryOpenDisposition::OpenExisting).is_err());
    assert!(!directory.exists());
    Ok(())
}

#[test]
fn local_directory_path_accepts_drive_and_verbatim_drive_paths() {
    assert!(validate_local_directory_path(Path::new(r"C:\Users\alice\.codex")).is_ok());
    assert!(validate_local_directory_path(Path::new(r"\\?\D:\Codex Data\home")).is_ok());
}

#[test]
fn local_directory_path_rejects_relative_network_parent_and_stream_paths() {
    for path in [
        r"relative\.codex",
        r"C:relative\.codex",
        r"\\server\share\.codex",
        r"\\?\UNC\server\share\.codex",
        r"\\.\C:\Users\alice",
        r"C:\Users\alice\..\other",
        r"C:\Users\alice\.codex:stream",
    ] {
        assert!(
            validate_local_directory_path(Path::new(path)).is_err(),
            "unexpectedly accepted {path}"
        );
    }
}
