use std::path::PathBuf;

use pretty_assertions::assert_eq;

use super::DEV_VOLUME;
use super::volume_flags;

#[test]
fn the_windows_system_volume_is_not_a_dev_drive() {
    let system_root = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .expect("Windows exposes its system directory");
    let flags = volume_flags(&system_root).expect("the system volume can be inspected");

    assert_eq!(flags & DEV_VOLUME, 0);
}
