use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug)]
pub(crate) struct BwrapMountPointRegistration {
    mount_point: PathBuf,
    marker_file: PathBuf,
    marker_dir: PathBuf,
}

pub(crate) fn register_bwrap_mount_points(
    mount_points: &[PathBuf],
) -> Vec<BwrapMountPointRegistration> {
    let mut mount_points = mount_points.to_vec();
    mount_points.sort();
    mount_points.dedup();

    let mut registrations = Vec::new();
    for mount_point in mount_points {
        let marker_dir = bwrap_mount_point_marker_dir(&mount_point);
        if fs::create_dir_all(&marker_dir).is_err() {
            continue;
        }
        let marker_file = marker_dir.join(std::process::id().to_string());
        if fs::write(&marker_file, b"").is_err() {
            continue;
        }
        registrations.push(BwrapMountPointRegistration {
            mount_point,
            marker_file,
            marker_dir,
        });
    }
    registrations
}

pub(crate) fn cleanup_bwrap_mount_points(registrations: &[BwrapMountPointRegistration]) {
    for registration in registrations {
        let _ = fs::remove_file(&registration.marker_file);
        if has_active_bwrap_mount_point_markers(&registration.marker_dir) {
            continue;
        }
        remove_empty_bwrap_mount_point(&registration.mount_point);
        let _ = fs::remove_dir(&registration.marker_dir);
    }
}

fn has_active_bwrap_mount_point_markers(marker_dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(marker_dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let marker_file = entry.path();
        if marker_pid_is_active(marker_file.file_name()) {
            return true;
        }
        let _ = fs::remove_file(marker_file);
    }
    false
}

fn marker_pid_is_active(pid: Option<&OsStr>) -> bool {
    let Some(pid) = pid.and_then(OsStr::to_str) else {
        return false;
    };
    let Ok(pid) = pid.parse::<i32>() else {
        return false;
    };
    let kill_res = unsafe { libc::kill(pid, 0) };
    kill_res == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

fn bwrap_mount_point_marker_dir(mount_point: &Path) -> PathBuf {
    std::env::temp_dir()
        .join("codex-bwrap-mountpoints")
        .join(hash_os_str(mount_point.as_os_str()))
}

fn hash_os_str(value: &OsStr) -> String {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET_BASIS;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

fn remove_empty_bwrap_mount_point(mount_point: &Path) {
    let Ok(metadata) = fs::symlink_metadata(mount_point) else {
        return;
    };
    let file_type = metadata.file_type();
    if file_type.is_file() && metadata.len() == 0 {
        let _ = fs::remove_file(mount_point);
    } else if file_type.is_dir()
        && fs::read_dir(mount_point)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false)
    {
        let _ = fs::remove_dir(mount_point);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_bwrap_mount_points_removes_empty_mount_points() {
        let temp_dir = tempfile::TempDir::new().expect("tempdir");
        let empty_file = temp_dir.path().join("empty-file");
        let empty_dir = temp_dir.path().join("empty-dir");
        std::fs::write(&empty_file, "").expect("create empty file");
        std::fs::create_dir(&empty_dir).expect("create empty dir");
        let registrations = register_bwrap_mount_points(&[empty_file.clone(), empty_dir.clone()]);

        cleanup_bwrap_mount_points(&registrations);

        assert!(!empty_file.exists());
        assert!(!empty_dir.exists());
    }

    #[test]
    fn cleanup_bwrap_mount_points_keeps_non_empty_paths() {
        let temp_dir = tempfile::TempDir::new().expect("tempdir");
        let non_empty_file = temp_dir.path().join("non-empty-file");
        let non_empty_dir = temp_dir.path().join("non-empty-dir");
        std::fs::write(&non_empty_file, "content").expect("create non-empty file");
        std::fs::create_dir(&non_empty_dir).expect("create non-empty dir");
        std::fs::write(non_empty_dir.join("child"), "").expect("create child");
        let registrations =
            register_bwrap_mount_points(&[non_empty_file.clone(), non_empty_dir.clone()]);

        cleanup_bwrap_mount_points(&registrations);

        assert!(non_empty_file.exists());
        assert!(non_empty_dir.exists());
    }

    #[test]
    fn cleanup_bwrap_mount_points_defers_when_another_sandbox_is_active() {
        let temp_dir = tempfile::TempDir::new().expect("tempdir");
        let empty_file = temp_dir.path().join("empty-file");
        std::fs::write(&empty_file, "").expect("create empty file");
        let registrations = register_bwrap_mount_points(std::slice::from_ref(&empty_file));
        let active_marker = registrations[0].marker_dir.join("1");
        std::fs::write(&active_marker, "").expect("create active marker");

        cleanup_bwrap_mount_points(&registrations);

        assert!(empty_file.exists());
        std::fs::remove_file(active_marker).expect("remove active marker");
        let registrations = register_bwrap_mount_points(std::slice::from_ref(&empty_file));
        cleanup_bwrap_mount_points(&registrations);
        assert!(!empty_file.exists());
    }
}
