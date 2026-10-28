//! Bazel-wired proof-of-concept crate reserved for future V8 experiments.

/// Returns the Bazel label for this proof-of-concept crate.
#[must_use]
pub fn bazel_target() -> &'static str {
    "//codex-rs/v8-poc:v8-poc"
}

/// Returns the embedded V8 version.
#[must_use]
pub fn embedded_v8_version() -> &'static str {
    v8::V8::get_version()
}

/// Returns whether the linked V8 library was built with the in-process sandbox.
#[must_use]
pub fn linked_v8_has_sandbox() -> bool {
    unsafe extern "C" {
        fn v8__V8__IsSandboxEnabled() -> bool;
    }

    // `rusty_v8` exposes this symbol for its own sandbox verification tests.
    unsafe { v8__V8__IsSandboxEnabled() }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use std::sync::Once;

    use super::bazel_target;

    fn initialize_v8() {
        static INIT: Once = Once::new();

        INIT.call_once(|| {
            v8::V8::initialize_platform(v8::new_default_platform(0, false).make_shared());
            v8::V8::initialize();
        });
    }

    fn evaluate_expression(expression: &str) -> String {
        initialize_v8();

        let isolate = &mut v8::Isolate::new(Default::default());
        v8::scope!(let scope, isolate);

        let context = v8::Context::new(scope, Default::default());
        let scope = &mut v8::ContextScope::new(scope, context);

        evaluate_script(scope, expression).to_rust_string_lossy(scope)
    }

    fn evaluate_script<'s>(
        scope: &mut v8::PinScope<'s, '_>,
        expression: &str,
    ) -> v8::Local<'s, v8::Value> {
        let source = v8::String::new(scope, expression).expect("expression should be valid UTF-8");
        let script = v8::Script::compile(scope, source, None).expect("expression should compile");
        script.run(scope).expect("expression should evaluate")
    }

    #[test]
    fn exposes_expected_bazel_target() {
        assert_eq!(bazel_target(), "//codex-rs/v8-poc:v8-poc");
    }

    #[test]
    fn exposes_embedded_v8_version() {
        assert!(!super::embedded_v8_version().is_empty());
    }

    #[test]
    fn sandbox_feature_matches_linked_v8() {
        assert_eq!(super::linked_v8_has_sandbox(), cfg!(feature = "sandbox"));
    }

    #[test]
    #[ignore = "Requires a native V8 pair and corrected rusty_v8; published 152.2.0 has an incompatible CppGC ABI"]
    fn cppgc_member_storage_satisfies_alignment() {
        use v8::cppgc;

        struct Item;
        // SAFETY: Item contains no GC references to trace.
        unsafe impl cppgc::GarbageCollected for Item {
            fn trace(&self, _: &mut cppgc::Visitor) {}

            fn get_name(&self) -> &'static std::ffi::CStr {
                c"Item"
            }
        }

        // In pinned V8 the C++ slot is either a uint32_t or native pointer.
        // No Member construction occurs until we know Rust can satisfy both.
        for (name, size, alignment) in [
            (
                "Member",
                size_of::<cppgc::Member<Item>>(),
                align_of::<cppgc::Member<Item>>(),
            ),
            (
                "WeakMember",
                size_of::<cppgc::WeakMember<Item>>(),
                align_of::<cppgc::WeakMember<Item>>(),
            ),
        ] {
            assert!(matches!(size, 4 | 8), "unexpected {name} size: {size}");
            assert_eq!(
                alignment, size,
                "{name} storage does not meet C++ alignment"
            );
        }

        #[repr(C)]
        struct BytePrefix {
            prefix: u8,
            strong: cppgc::Member<Item>,
            weak: cppgc::WeakMember<Item>,
        }
        #[repr(C)]
        struct WordPrefix {
            prefix: u32,
            strong: cppgc::Member<Item>,
            weak: cppgc::WeakMember<Item>,
        }
        let strong_align = align_of::<cppgc::Member<Item>>();
        let weak_align = align_of::<cppgc::WeakMember<Item>>();
        assert_eq!(std::mem::offset_of!(BytePrefix, strong) % strong_align, 0);
        assert_eq!(std::mem::offset_of!(BytePrefix, weak) % weak_align, 0);
        assert_eq!(std::mem::offset_of!(WordPrefix, strong) % strong_align, 0);
        assert_eq!(std::mem::offset_of!(WordPrefix, weak) % weak_align, 0);
    }

    #[test]
    fn evaluates_integer_addition() {
        assert_eq!(evaluate_expression("1 + 2"), "3");
    }

    #[test]
    fn evaluates_string_concatenation() {
        assert_eq!(evaluate_expression("'hello ' + 'world'"), "hello world");
    }

    #[test]
    fn reads_typed_array_contents_with_the_compiled_binding_size() {
        initialize_v8();
        let isolate = &mut v8::Isolate::new(Default::default());
        v8::scope!(let scope, isolate);
        let context = v8::Context::new(scope, Default::default());
        let scope = &mut v8::ContextScope::new(scope, context);

        for length in [0, 1, 63, 64, 65] {
            let expression = format!("new Uint8Array({length}).fill(90)");
            let value = evaluate_script(scope, &expression);
            let view = v8::Local::<v8::ArrayBufferView>::try_from(value).unwrap();
            let on_heap =
                v8::TYPED_ARRAY_MAX_SIZE_IN_HEAP != 0 && length <= v8::TYPED_ARRAY_MAX_SIZE_IN_HEAP;
            // Check native placement before asking C++ to copy into storage:
            // stale size-zero bindings must fail here rather than abort in FFI.
            assert_eq!(view.has_buffer(), !on_heap, "byte length {length}");

            let mut storage = vec![0; v8::TYPED_ARRAY_MAX_SIZE_IN_HEAP];
            let storage_ptr = storage.as_ptr();
            let contents = view.get_contents(&mut storage);
            assert_eq!(contents, vec![90; length], "byte length {length}");
            if on_heap {
                assert_eq!(contents.as_ptr(), storage_ptr);
            }
            // Reading inline contents copies bytes without promoting the view.
            assert_eq!(view.has_buffer(), !on_heap, "byte length {length}");
        }
    }

    #[test]
    fn typed_array_backing_store_promotion_survives_collection() {
        enum Promotion {
            Data,
            BackingStore,
        }

        initialize_v8();
        let isolate = &mut v8::Isolate::new(Default::default());
        v8::scope!(let scope, isolate);
        let context = v8::Context::new(scope, Default::default());
        let scope = &mut v8::ContextScope::new(scope, context);

        for promotion in [Promotion::Data, Promotion::BackingStore] {
            let value = evaluate_script(scope, "new Uint8Array(64).fill(90)");
            let view = v8::Local::<v8::ArrayBufferView>::try_from(value).unwrap();
            let data = match promotion {
                Promotion::Data => view.data(),
                Promotion::BackingStore => {
                    view.get_backing_store().unwrap().data().unwrap().as_ptr()
                }
            };
            let backing_store = view.get_backing_store().unwrap();
            assert!(view.has_buffer());
            assert_eq!(backing_store.data().unwrap().as_ptr(), data);

            scope.low_memory_notification();

            assert_eq!(view.data(), data);
            assert_eq!(backing_store.data().unwrap().as_ptr(), data);
            assert_eq!(
                backing_store
                    .iter()
                    .map(std::cell::Cell::get)
                    .collect::<Vec<_>>(),
                vec![90; 64]
            );
        }
    }

    #[test]
    fn microtask_queue_runs_promises_across_collection_and_context_reassignment() {
        initialize_v8();
        let isolate = &mut v8::Isolate::new(Default::default());
        let queue = v8::MicrotaskQueue::new(isolate, v8::MicrotasksPolicy::Explicit);
        {
            v8::scope!(let scope, isolate);
            let context = v8::Context::new(scope, Default::default());
            let default_queue = context.get_microtask_queue().unwrap();
            context.set_microtask_queue(queue.as_ref());
            {
                let scope = &mut v8::ContextScope::new(scope, context);
                evaluate_script(
                    scope,
                    "globalThis.results = []; \
                     Promise.resolve(1).then(value => results.push(value)); \
                     (async () => { results.push(await Promise.resolve(2)); })();",
                );

                scope.low_memory_notification();
                assert_eq!(
                    evaluate_script(scope, "JSON.stringify(results)").to_rust_string_lossy(scope),
                    "[]"
                );
                queue.perform_checkpoint(scope);
                assert_eq!(
                    evaluate_script(scope, "JSON.stringify(results)").to_rust_string_lossy(scope),
                    "[1,2]"
                );
            }

            // V8 requires exiting the context before replacing its queue.
            // Keep the handle alive until the context releases its queue, for
            // both GC-rooted and uniquely owned queue storage.
            context.set_microtask_queue(default_queue);
            assert!(std::ptr::eq(
                context.get_microtask_queue().unwrap(),
                default_queue
            ));
        }
        drop(queue);
        isolate.low_memory_notification();
    }

    #[test]
    #[ignore = "Requires a native V8 pair and corrected rusty_v8; published 152.2.0 has an incompatible CppGC ABI"]
    fn compiled_cppgc_members_match_bindings_and_survive_collection() {
        use std::ffi::c_void;
        use v8::cppgc;

        // A small prefix exercises the Rust slot alignment before C++ access.
        #[repr(C)]
        struct Node {
            value: u32,
            next: cppgc::Member<Node>,
            weak: cppgc::WeakMember<Node>,
        }

        // SAFETY: every Member and WeakMember owned by Node is traced.
        unsafe impl cppgc::GarbageCollected for Node {
            fn trace(&self, visitor: &mut cppgc::Visitor) {
                visitor.trace(&self.next);
                visitor.trace(&self.weak);
            }

            fn get_name(&self) -> &'static std::ffi::CStr {
                c"v8-poc cppgc Node"
            }
        }

        unsafe extern "C" {
            fn cppgc__Member__CONSTRUCT(storage: *mut c_void, object: *mut c_void);
            fn cppgc__Member__Get(storage: *const c_void) -> *mut c_void;
            fn cppgc__Member__DESTRUCT(storage: *mut c_void);
            fn cppgc__WeakMember__CONSTRUCT(storage: *mut c_void, object: *mut c_void);
            fn cppgc__WeakMember__Get(storage: *const c_void) -> *mut c_void;
            fn cppgc__WeakMember__DESTRUCT(storage: *mut c_void);
        }

        // Fail before accessing an incorrectly aligned Rust slot through FFI.
        cppgc_member_storage_satisfies_alignment();
        initialize_v8();
        // Member::empty() calls C++ into a Rust stack slot sized by bindgen.
        // Validate that size BEFORE calling it: a stale 4-byte binding for an
        // 8-byte archive must fail an assertion, never overwrite a Rust slot.
        // All supported native CPPGC pointers fit a usize. A null pointer
        // initializes its full declared storage to zero in both encodings.
        #[repr(C, align(16))]
        struct Slot([u8; 4 * size_of::<usize>()]);
        for (name, size, construct, get, destruct) in [
            (
                "Member",
                size_of::<cppgc::Member<Node>>(),
                cppgc__Member__CONSTRUCT as unsafe extern "C" fn(*mut c_void, *mut c_void),
                cppgc__Member__Get as unsafe extern "C" fn(*const c_void) -> *mut c_void,
                cppgc__Member__DESTRUCT as unsafe extern "C" fn(*mut c_void),
            ),
            (
                "WeakMember",
                size_of::<cppgc::WeakMember<Node>>(),
                cppgc__WeakMember__CONSTRUCT as unsafe extern "C" fn(*mut c_void, *mut c_void),
                cppgc__WeakMember__Get as unsafe extern "C" fn(*const c_void) -> *mut c_void,
                cppgc__WeakMember__DESTRUCT as unsafe extern "C" fn(*mut c_void),
            ),
        ] {
            let mut slot = Slot([0xa5; 4 * size_of::<usize>()]);
            assert!(
                size <= size_of::<usize>(),
                "unexpected {name} binding size {size}"
            );
            let ptr = slot.0.as_mut_ptr().cast();
            // SAFETY: the aligned slot has enough room for either pointer
            // encoding and its guard; the object is destroyed before reuse.
            let is_null = unsafe {
                construct(ptr, std::ptr::null_mut());
                let is_null = get(ptr).is_null();
                destruct(ptr);
                is_null
            };
            assert!(is_null, "{name} failed to return its constructed null");
            assert!(
                slot.0[..size].iter().all(|byte| *byte == 0),
                "{name} binding is too large"
            );
            assert!(
                slot.0[size..].iter().all(|byte| *byte == 0xa5),
                "{name} binding is too small"
            );
        }

        let mut heap = cppgc::Heap::create(v8::V8::get_current_platform(), Default::default());
        heap.enable_detached_garbage_collections_for_testing();
        {
            // SAFETY: each temporary pointer is kept on this stack and moved
            // immediately into a traced Member or a Persistent GC root.
            let root = unsafe {
                let child = cppgc::make_garbage_collected(
                    &heap,
                    Node {
                        value: 42,
                        next: cppgc::Member::empty(),
                        weak: cppgc::WeakMember::empty(),
                    },
                );
                let weak_only = cppgc::make_garbage_collected(
                    &heap,
                    Node {
                        value: 7,
                        next: cppgc::Member::empty(),
                        weak: cppgc::WeakMember::empty(),
                    },
                );
                let parent = cppgc::make_garbage_collected(
                    &heap,
                    Node {
                        value: 1,
                        next: cppgc::Member::new(&child),
                        weak: cppgc::WeakMember::new(&weak_only),
                    },
                );
                cppgc::Persistent::new(&parent)
            };
            // SAFETY: root and its traced Member are the only strong pointers
            // in use. The weak-only object must be collectible without stack
            // scanning. No UnsafePtr from allocation is dereferenced again.
            unsafe {
                heap.collect_garbage_for_testing(cppgc::EmbedderStackState::NoHeapPointers);
                let parent = root.get().expect("persistent root should survive");
                assert_eq!(parent.value, 1);
                assert_eq!(
                    parent
                        .next
                        .get()
                        .expect("traced child should survive")
                        .value,
                    42
                );
                assert!(
                    parent.weak.get().is_none(),
                    "weak-only target should be collected"
                );
            }
        }
        heap.terminate();
    }

    #[test]
    fn parses_crdtp_dispatchable_messages() {
        let cbor = v8::crdtp::json_to_cbor(br#"{"id":7,"method":"Runtime.evaluate","params":{}}"#)
            .expect("JSON should convert to CBOR");
        let dispatchable = v8::crdtp::Dispatchable::new(&cbor);

        assert!(dispatchable.ok());
        assert_eq!(dispatchable.call_id(), 7);
        assert_eq!(dispatchable.method(), b"Runtime.evaluate");
    }
}
