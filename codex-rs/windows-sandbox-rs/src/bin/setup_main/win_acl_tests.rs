use super::DELETE;
use super::FILE_GENERIC_EXECUTE;
use super::FILE_GENERIC_READ;
use super::FILE_GENERIC_WRITE;
use super::GRANT_ACCESS;
use super::SetupMode;
use super::lock_sandbox_dir;
use super::resolve_sid;

#[test]
fn provision_only_locks_plain_directory_via_handle() {
    let temp = tempfile::tempdir().expect("tempdir");
    let sandbox_bin = temp.path().join(".sandbox-bin");
    let real_user = std::env::var("USERNAME").unwrap_or_else(|_| "Administrators".to_string());
    let real_sid = resolve_sid(&real_user).expect("resolve real user SID");

    lock_sandbox_dir(
        &sandbox_bin,
        &real_user,
        &real_sid,
        GRANT_ACCESS,
        FILE_GENERIC_READ | FILE_GENERIC_EXECUTE,
        FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE | DELETE,
        SetupMode::ProvisionOnly,
    )
    .expect("lock sandbox bin via no-reparse handle");

    assert!(sandbox_bin.is_dir());
}
