use super::DenyAceKind;
use super::WRITE_DENY_MASK;
use pretty_assertions::assert_eq;
use windows_sys::Win32::Storage::FileSystem::DELETE;
use windows_sys::Win32::Storage::FileSystem::FILE_APPEND_DATA;
use windows_sys::Win32::Storage::FileSystem::FILE_DELETE_CHILD;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_WRITE_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::FILE_WRITE_DATA;
use windows_sys::Win32::Storage::FileSystem::FILE_WRITE_EA;

#[test]
fn write_deny_mask_contains_only_mutation_rights() {
    assert_eq!(
        WRITE_DENY_MASK,
        FILE_WRITE_DATA
            | FILE_APPEND_DATA
            | FILE_WRITE_EA
            | FILE_WRITE_ATTRIBUTES
            | DELETE
            | FILE_DELETE_CHILD
    );
    assert_eq!(WRITE_DENY_MASK & FILE_GENERIC_READ, 0);
    assert_eq!(DenyAceKind::Write.mask(), WRITE_DENY_MASK);
}
