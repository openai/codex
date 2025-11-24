// Copyright 2014 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use parking_lot::Mutex;
use std::borrow::Cow;
use std::mem;
use std::ptr::NonNull;
use std::sync::atomic::AtomicIsize;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::OnceLock;

const NB_BUCKETS: usize = 1 << 12; // 4096
const BUCKET_MASK: u32 = (1 << 12) - 1;

pub(crate) struct Set {
    buckets: Box<[Mutex<Option<Box<Entry>>>]>,
}

pub(crate) struct Entry {
    pub(crate) string: Box<str>,
    pub(crate) hash: u32,
    pub(crate) ref_count: AtomicIsize,
    next_in_bucket: Option<Box<Entry>>,
}

// Addresses are a multiples of this,
// and therefore have have TAG_MASK bits unset, available for tagging.
pub(crate) const ENTRY_ALIGNMENT: usize = 4;

#[test]
fn entry_alignment_is_sufficient() {
    assert!(mem::align_of::<Entry>() >= ENTRY_ALIGNMENT);
}

pub(crate) fn dynamic_set() -> &'static Set {
    // NOTE: Using const initialization for buckets breaks the small-stack test.
    // ```
    // // buckets: [Mutex<Option<Box<Entry>>>; NB_BUCKETS],
    // const MUTEX: Mutex<Option<Box<Entry>>> = Mutex::new(None);
    // let buckets = Box::new([MUTEX; NB_BUCKETS]);
    // ```
    static DYNAMIC_SET: OnceLock<Set> = OnceLock::new();

    DYNAMIC_SET.get_or_init(|| {
        let buckets = (0..NB_BUCKETS).map(|_| Mutex::new(None)).collect();
        Set { buckets }
    })
}

impl Set {
    pub(crate) fn insert(&self, string: Cow<str>, hash: u32) -> NonNull<Entry> {
        let bucket_index = (hash & BUCKET_MASK) as usize;
        let mut linked_list = self.buckets[bucket_index].lock();

        {
            let mut ptr: Option<&mut Box<Entry>> = linked_list.as_mut();

            while let Some(entry) = ptr.take() {
                if entry.hash == hash && *entry.string == *string {
                    if entry.ref_count.fetch_add(1, SeqCst) > 0 {
                        return NonNull::from(&mut **entry);
                    }
                    // Uh-oh. The pointer's reference count was zero, which means someone may try
                    // to free it. (Naive attempts to defend against this, for example having the
                    // destructor check to see whether the reference count is indeed zero, don't
                    // work due to ABA.) Thus we need to temporarily add a duplicate string to the
                    // list.
                    entry.ref_count.fetch_sub(1, SeqCst);
                    break;
                }
                ptr = entry.next_in_bucket.as_mut();
            }
        }
        debug_assert!(mem::align_of::<Entry>() >= ENTRY_ALIGNMENT);
        let string = string.into_owned();
        let mut entry = Box::new(Entry {
            next_in_bucket: linked_list.take(),
            hash,
            ref_count: AtomicIsize::new(1),
            string: string.into_boxed_str(),
        });
        let ptr = NonNull::from(&mut *entry);
        *linked_list = Some(entry);
        ptr
    }

    pub(crate) fn remove(&self, ptr: *mut Entry) {
        let value: &Entry = unsafe { &*ptr };
        let bucket_index = (value.hash & BUCKET_MASK) as usize;

        let mut linked_list = self.buckets[bucket_index].lock();
        debug_assert!(value.ref_count.load(SeqCst) == 0);
        let mut current: &mut Option<Box<Entry>> = &mut linked_list;

        while let Some(entry_ptr) = current.as_mut() {
            let entry_ptr: *mut Entry = &mut **entry_ptr;
            if entry_ptr == ptr {
                mem::drop(mem::replace(current, unsafe {
                    (*entry_ptr).next_in_bucket.take()
                }));
                break;
            }
            current = unsafe { &mut (*entry_ptr).next_in_bucket };
        }
    }
}
