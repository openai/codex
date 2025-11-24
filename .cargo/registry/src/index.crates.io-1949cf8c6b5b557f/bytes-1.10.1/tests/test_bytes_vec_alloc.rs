#![cfg(not(miri))]
use std::alloc::{GlobalAlloc, Layout, System};
use std::ptr::null_mut;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

use bytes::{Buf, Bytes};

#[global_allocator]
static LEDGER: Ledger = Ledger::new();

const LEDGER_LENGTH: usize = 1024 * 1024;

struct Ledger {
    alloc_table: [(AtomicPtr<u8>, AtomicUsize); LEDGER_LENGTH],
}

impl Ledger {
    const fn new() -> Self {
        const ELEM: (AtomicPtr<u8>, AtomicUsize) =
            (AtomicPtr::new(null_mut()), AtomicUsize::new(0));
        let alloc_table = [ELEM; LEDGER_LENGTH];

        Self { alloc_table }
    }

    /// Iterate over our table until we find an open entry, then insert into said entry
    fn insert(&self, ptr: *mut u8, size: usize) {
        for (entry_ptr, entry_size) in self.alloc_table.iter() {
            // SeqCst is good enough here, we don't care about perf, i just want to be correct!
            if entry_ptr
                .compare_exchange(null_mut(), ptr, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                entry_size.store(size, Ordering::SeqCst);
                return;
            }
        }

        panic!("Ledger ran out of space.");
    }

    fn remove(&self, ptr: *mut u8) -> usize {
        for (entry_ptr, entry_size) in self.alloc_table.iter() {
            // set the value to be something that will never try and be deallocated, so that we
            // don't have any chance of a race condition
            //
            // dont worry, LEDGER_LENGTH is really long to compensate for us not reclaiming space
            if entry_ptr
                .compare_exchange(
                    ptr,
                    invalid_ptr(usize::MAX),
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                )
                .is_ok()
            {
                return entry_size.load(Ordering::SeqCst);
            }
        }

        panic!("Couldn't find a matching entry for {:x?}", ptr);
    }
}

unsafe impl GlobalAlloc for Ledger {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let ptr = System.alloc(layout);
        self.insert(ptr, size);
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let orig_size = self.remove(ptr);

        if orig_size != layout.size() {
            panic!(
                "bad dealloc: alloc size was {}, dealloc size is {}",
                orig_size,
                layout.size()
            );
        } else {
            System.dealloc(ptr, layout);
        }
    }
}

#[test]
fn test_bytes_advance() {
    let mut bytes = Bytes::from(vec![10, 20, 30]);
    bytes.advance(1);
    drop(bytes);
}

#[test]
fn test_bytes_truncate() {
    let mut bytes = Bytes::from(vec![10, 20, 30]);
    bytes.truncate(2);
    drop(bytes);
}

#[test]
fn test_bytes_truncate_and_advance() {
    let mut bytes = Bytes::from(vec![10, 20, 30]);
    bytes.truncate(2);
    bytes.advance(1);
    drop(bytes);
}

/// Returns a dangling pointer with the given address. This is used to store
/// integer data in pointer fields.
#[inline]
fn invalid_ptr<T>(addr: usize) -> *mut T {
    let ptr = std::ptr::null_mut::<u8>().wrapping_add(addr);
    debug_assert_eq!(ptr as usize, addr);
    ptr.cast::<T>()
}

#[test]
fn test_bytes_into_vec() {
    let vec = vec![33u8; 1024];

    // Test cases where kind == KIND_VEC
    let b1 = Bytes::from(vec.clone());
    assert_eq!(Vec::from(b1), vec);

    // Test cases where kind == KIND_ARC, ref_cnt == 1
    let b1 = Bytes::from(vec.clone());
    drop(b1.clone());
    assert_eq!(Vec::from(b1), vec);

    // Test cases where kind == KIND_ARC, ref_cnt == 2
    let b1 = Bytes::from(vec.clone());
    let b2 = b1.clone();
    assert_eq!(Vec::from(b1), vec);

    // Test cases where vtable = SHARED_VTABLE, kind == KIND_ARC, ref_cnt == 1
    assert_eq!(Vec::from(b2), vec);

    // Test cases where offset != 0
    let mut b1 = Bytes::from(vec.clone());
    let b2 = b1.split_off(20);

    assert_eq!(Vec::from(b2), vec[20..]);
    assert_eq!(Vec::from(b1), vec[..20]);
}
