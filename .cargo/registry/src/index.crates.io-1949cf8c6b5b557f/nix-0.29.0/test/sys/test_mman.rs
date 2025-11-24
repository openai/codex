#![allow(clippy::redundant_slicing)]

use nix::sys::mman::{mmap_anonymous, MapFlags, ProtFlags};
use std::num::NonZeroUsize;

#[test]
fn test_mmap_anonymous() {
    unsafe {
        let mut ptr = mmap_anonymous(
            None,
            NonZeroUsize::new(1).unwrap(),
            ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
            MapFlags::MAP_PRIVATE,
        )
        .unwrap()
        .cast::<u8>();
        assert_eq!(*ptr.as_ref(), 0x00u8);
        *ptr.as_mut() = 0xffu8;
        assert_eq!(*ptr.as_ref(), 0xffu8);
    }
}

#[test]
#[cfg(any(target_os = "linux", target_os = "netbsd"))]
fn test_mremap_grow() {
    use nix::libc::size_t;
    use nix::sys::mman::{mremap, MRemapFlags};
    use std::ptr::NonNull;

    const ONE_K: size_t = 1024;
    let one_k_non_zero = NonZeroUsize::new(ONE_K).unwrap();

    let slice: &mut [u8] = unsafe {
        let mem = mmap_anonymous(
            None,
            one_k_non_zero,
            ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
            MapFlags::MAP_PRIVATE,
        )
        .unwrap();
        std::slice::from_raw_parts_mut(mem.as_ptr().cast(), ONE_K)
    };
    assert_eq!(slice[ONE_K - 1], 0x00);
    slice[ONE_K - 1] = 0xFF;
    assert_eq!(slice[ONE_K - 1], 0xFF);

    let slice: &mut [u8] = unsafe {
        #[cfg(target_os = "linux")]
        let mem = mremap(
            NonNull::from(&mut slice[..]).cast(),
            ONE_K,
            10 * ONE_K,
            MRemapFlags::MREMAP_MAYMOVE,
            None,
        )
        .unwrap();
        #[cfg(target_os = "netbsd")]
        let mem = mremap(
            NonNull::from(&mut slice[..]).cast(),
            ONE_K,
            10 * ONE_K,
            MRemapFlags::MAP_REMAPDUP,
            None,
        )
        .unwrap();
        std::slice::from_raw_parts_mut(mem.cast().as_ptr(), 10 * ONE_K)
    };

    // The first KB should still have the old data in it.
    assert_eq!(slice[ONE_K - 1], 0xFF);

    // The additional range should be zero-init'd and accessible.
    assert_eq!(slice[10 * ONE_K - 1], 0x00);
    slice[10 * ONE_K - 1] = 0xFF;
    assert_eq!(slice[10 * ONE_K - 1], 0xFF);
}

#[test]
#[cfg(any(target_os = "linux", target_os = "netbsd"))]
// Segfaults for unknown reasons under QEMU for 32-bit targets
#[cfg_attr(all(target_pointer_width = "32", qemu), ignore)]
fn test_mremap_shrink() {
    use nix::libc::size_t;
    use nix::sys::mman::{mremap, MRemapFlags};
    use std::num::NonZeroUsize;
    use std::ptr::NonNull;

    const ONE_K: size_t = 1024;
    let ten_one_k = NonZeroUsize::new(10 * ONE_K).unwrap();
    let slice: &mut [u8] = unsafe {
        let mem = mmap_anonymous(
            None,
            ten_one_k,
            ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
            MapFlags::MAP_PRIVATE,
        )
        .unwrap();
        std::slice::from_raw_parts_mut(mem.as_ptr().cast(), ONE_K)
    };
    assert_eq!(slice[ONE_K - 1], 0x00);
    slice[ONE_K - 1] = 0xFF;
    assert_eq!(slice[ONE_K - 1], 0xFF);

    let slice: &mut [u8] = unsafe {
        let mem = mremap(
            NonNull::from(&mut slice[..]).cast(),
            ten_one_k.into(),
            ONE_K,
            MRemapFlags::empty(),
            None,
        )
        .unwrap();
        // Since we didn't supply MREMAP_MAYMOVE, the address should be the
        // same.
        assert_eq!(mem.as_ptr(), NonNull::from(&mut slice[..]).cast().as_ptr());
        std::slice::from_raw_parts_mut(mem.as_ptr().cast(), ONE_K)
    };

    // The first KB should still be accessible and have the old data in it.
    assert_eq!(slice[ONE_K - 1], 0xFF);
}
