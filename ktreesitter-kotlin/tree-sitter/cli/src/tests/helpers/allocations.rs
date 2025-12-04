use std::{
    collections::HashMap,
    os::raw::c_void,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
        Mutex,
    },
};

#[ctor::ctor]
unsafe fn initialize_allocation_recording() {
    tree_sitter::set_allocator(
        Some(ts_record_malloc),
        Some(ts_record_calloc),
        Some(ts_record_realloc),
        Some(ts_record_free),
    );
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct Allocation(*const c_void);
unsafe impl Send for Allocation {}
unsafe impl Sync for Allocation {}

#[derive(Default)]
struct AllocationRecorder {
    enabled: AtomicBool,
    allocation_count: AtomicUsize,
    outstanding_allocations: Mutex<HashMap<Allocation, usize>>,
}

thread_local! {
    static RECORDER: AllocationRecorder = AllocationRecorder::default();
}

extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn calloc(count: usize, size: usize) -> *mut c_void;
    fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
}

pub fn record<T>(f: impl FnOnce() -> T) -> T {
    RECORDER.with(|recorder| {
        recorder.enabled.store(true, SeqCst);
        recorder.allocation_count.store(0, SeqCst);
        recorder.outstanding_allocations.lock().unwrap().clear();
    });

    let value = f();

    let outstanding_allocation_indices = RECORDER.with(|recorder| {
        recorder.enabled.store(false, SeqCst);
        recorder.allocation_count.store(0, SeqCst);
        recorder
            .outstanding_allocations
            .lock()
            .unwrap()
            .drain()
            .map(|e| e.1)
            .collect::<Vec<_>>()
    });
    assert!(
        outstanding_allocation_indices.is_empty(),
        "Leaked allocation indices: {outstanding_allocation_indices:?}"
    );
    value
}

fn record_alloc(ptr: *mut c_void) {
    RECORDER.with(|recorder| {
        if recorder.enabled.load(SeqCst) {
            let count = recorder.allocation_count.fetch_add(1, SeqCst);
            recorder
                .outstanding_allocations
                .lock()
                .unwrap()
                .insert(Allocation(ptr), count);
        }
    });
}

fn record_dealloc(ptr: *mut c_void) {
    RECORDER.with(|recorder| {
        if recorder.enabled.load(SeqCst) {
            recorder
                .outstanding_allocations
                .lock()
                .unwrap()
                .remove(&Allocation(ptr));
        }
    });
}

unsafe extern "C" fn ts_record_malloc(size: usize) -> *mut c_void {
    let result = malloc(size);
    record_alloc(result);
    result
}

unsafe extern "C" fn ts_record_calloc(count: usize, size: usize) -> *mut c_void {
    let result = calloc(count, size);
    record_alloc(result);
    result
}

unsafe extern "C" fn ts_record_realloc(ptr: *mut c_void, size: usize) -> *mut c_void {
    let result = realloc(ptr, size);
    if ptr.is_null() {
        record_alloc(result);
    } else if !core::ptr::eq(ptr, result) {
        record_dealloc(ptr);
        record_alloc(result);
    }
    result
}

unsafe extern "C" fn ts_record_free(ptr: *mut c_void) {
    record_dealloc(ptr);
    free(ptr);
}
