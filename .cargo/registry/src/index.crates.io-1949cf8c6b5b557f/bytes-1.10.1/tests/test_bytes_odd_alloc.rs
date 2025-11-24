//! Test using `Bytes` with an allocator that hands out "odd" pointers for
//! vectors (pointers where the LSB is set).

#![cfg(not(miri))] // Miri does not support custom allocators (also, Miri is "odd" by default with 50% chance)

use std::alloc::{GlobalAlloc, Layout, System};
use std::ptr;

use bytes::{Bytes, BytesMut};

#[global_allocator]
static ODD: Odd = Odd;

struct Odd;

unsafe impl GlobalAlloc for Odd {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.align() == 1 && layout.size() > 0 {
            // Allocate slightly bigger so that we can offset the pointer by 1
            let size = layout.size() + 1;
            let new_layout = match Layout::from_size_align(size, 1) {
                Ok(layout) => layout,
                Err(_err) => return ptr::null_mut(),
            };
            let ptr = System.alloc(new_layout);
            if !ptr.is_null() {
                ptr.offset(1)
            } else {
                ptr
            }
        } else {
            System.alloc(layout)
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if layout.align() == 1 && layout.size() > 0 {
            let size = layout.size() + 1;
            let new_layout = match Layout::from_size_align(size, 1) {
                Ok(layout) => layout,
                Err(_err) => std::process::abort(),
            };
            System.dealloc(ptr.offset(-1), new_layout);
        } else {
            System.dealloc(ptr, layout);
        }
    }
}

#[test]
fn sanity_check_odd_allocator() {
    let vec = vec![33u8; 1024];
    let p = vec.as_ptr() as usize;
    assert!(p & 0x1 == 0x1, "{:#b}", p);
}

#[test]
fn test_bytes_from_vec_drop() {
    let vec = vec![33u8; 1024];
    let _b = Bytes::from(vec);
}

#[test]
fn test_bytes_clone_drop() {
    let vec = vec![33u8; 1024];
    let b1 = Bytes::from(vec);
    let _b2 = b1.clone();
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

#[test]
fn test_bytesmut_from_bytes_vec() {
    let vec = vec![33u8; 1024];

    // Test case where kind == KIND_VEC
    let b1 = Bytes::from(vec.clone());
    let b1m = BytesMut::from(b1);
    assert_eq!(b1m, vec);
}

#[test]
fn test_bytesmut_from_bytes_arc_1() {
    let vec = vec![33u8; 1024];

    // Test case where kind == KIND_ARC, ref_cnt == 1
    let b1 = Bytes::from(vec.clone());
    drop(b1.clone());
    let b1m = BytesMut::from(b1);
    assert_eq!(b1m, vec);
}

#[test]
fn test_bytesmut_from_bytes_arc_2() {
    let vec = vec![33u8; 1024];

    // Test case where kind == KIND_ARC, ref_cnt == 2
    let b1 = Bytes::from(vec.clone());
    let b2 = b1.clone();
    let b1m = BytesMut::from(b1);
    assert_eq!(b1m, vec);

    // Test case where vtable = SHARED_VTABLE, kind == KIND_ARC, ref_cnt == 1
    let b2m = BytesMut::from(b2);
    assert_eq!(b2m, vec);
}

#[test]
fn test_bytesmut_from_bytes_arc_offset() {
    let vec = vec![33u8; 1024];

    // Test case where offset != 0
    let mut b1 = Bytes::from(vec.clone());
    let b2 = b1.split_off(20);
    let b1m = BytesMut::from(b1);
    let b2m = BytesMut::from(b2);

    assert_eq!(b2m, vec[20..]);
    assert_eq!(b1m, vec[..20]);
}
