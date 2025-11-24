/*
 * Copyright 2018 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//! Test of runtime.

use std::fmt::Write;
use std::mem;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use derive_more::Display;
use once_cell::sync::Lazy;
use starlark_derive::starlark_module;

use crate as starlark;
use crate::assert;
use crate::assert::Assert;
use crate::environment::GlobalsBuilder;
use crate::eval::Evaluator;
use crate::values::any::StarlarkAny;
use crate::values::FrozenHeap;
use crate::values::Heap;

#[test]
fn test_garbage_collect() {
    assert::pass(
        r#"
x = (100, [{"test": None}], True)
y = str(x)
garbage_collect()
assert_eq(y, str(x))
    "#,
    );
}

#[test]
fn test_deallocation() {
    // Check that we really do deallocate values we create
    static COUNT: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));

    #[derive(Default, Debug, Display)]
    struct Dealloc;

    impl Drop for Dealloc {
        fn drop(&mut self) {
            COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[starlark_module]
    fn globals(builder: &mut GlobalsBuilder) {
        fn mk() -> anyhow::Result<StarlarkAny<Dealloc>> {
            Ok(StarlarkAny::new(Dealloc))
        }
    }

    COUNT.store(0, Ordering::SeqCst);
    let mut a = Assert::new();
    a.disable_gc();
    a.globals_add(globals);
    a.module("test", "x = [mk(), mk()]\ndef y(): return mk()");
    a.pass(
        r#"
load("test", "x", "y")
z = x[1]
q = mk()
r = [y(), mk()]
"#,
    );
    // The three that were run in pass should have gone
    assert_eq!(COUNT.load(Ordering::SeqCst), 3);
    mem::drop(a);
    // Now the frozen ones should have gone too
    assert_eq!(COUNT.load(Ordering::SeqCst), 5);
}

// This test relies on stack behavior which does not hold when
// ASAN is enabled. See D47571173 for more context.
#[cfg_attr(rust_nightly, cfg(not(sanitize = "address")))]
#[test]
fn test_stack_depth() {
    #[starlark_module]
    fn measure_stack(builder: &mut GlobalsBuilder) {
        fn stack_depth() -> anyhow::Result<String> {
            // Put a variable on the stack, and get a reference to it
            // Not entirely documented as to what this does, but hopefully
            // a mut pointer is harder to elide or optimise away
            let mut s = std::hint::black_box(1i32);
            Ok((&mut s as *mut i32 as usize).to_string())
        }
    }

    let mut a = Assert::new();
    a.globals_add(measure_stack);
    let s = a.pass(
        r#"
for i in range(1001):
    if i == 1:
        v1 = stack_depth()
    if i == 100:
        v100 = stack_depth()
    elif i == 1000:
        v1000 = stack_depth()
v1 + " " + v100 + " " + v1000
"#,
    );
    let s = s.unpack_str().unwrap();
    let words = s
        .split(' ')
        .map(|x| x.parse::<isize>().unwrap())
        .collect::<Vec<_>>();
    let v1 = words[0];
    let v100 = words[1];
    let v1000 = words[2];

    // We want to ensure they don't keep increasing, as that would be very bad
    // so ensure that the increase from v0 to v100 is less than the increase from v100 to v1000
    // with a 1000 for random noise.
    assert!(
        (v1 - v100).abs() + 1000 >= (v1000 - v100).abs(),
        "Stack change exceeded, FAILED {} + 1000 >= {} (relative to v1), 100={}, 1000={}",
        (v1 - v100).abs(),
        (v1000 - v100).abs(),
        v100 - v1,
        v1000 - v1
    );
}

#[test]
fn test_garbage_collect_happens() {
    // GC is meant to be "not observable", but if we break it, we want this test to fail
    #[starlark_module]
    fn helpers(builder: &mut GlobalsBuilder) {
        fn current_usage(heap: &Heap) -> anyhow::Result<i32> {
            Ok(heap.allocated_bytes() as i32)
        }

        fn is_gc_disabled(eval: &mut Evaluator) -> anyhow::Result<bool> {
            Ok(eval.disable_gc)
        }
    }

    let mut a = Assert::new();
    a.globals_add(helpers);

    // Approach is to keep doing something expensive, and we want to see the memory usage decrease.
    let mut code = r#"
globals = []
maximum = [0]
success = [is_gc_disabled()]

def update_maximum():
    maximum[0] = max(current_usage(), maximum[0])

def expensive(n):
    if success[0]:
        return
    now = current_usage()
    if now < maximum[0]:
        print("Success in " + str(n))
        success[0] = True
        return
    update_maximum()
    globals.append(str(n))
    locals = []
    for i in range(10 * n):
        locals.append(str(i))
    update_maximum()
"#
    .to_owned();
    // I expect success in approx 25 times, so do 100 for safety
    for i in 0..100 {
        writeln!(code, "expensive({})", i).unwrap();
    }
    code.push_str("assert_eq(success[0], True)\nis_gc_disabled()");
    // I expect to run with GC disabled some of the time, but not on the last run
    // so make sure at least once GC was enabled
    assert!(!a.pass(&code).unpack_bool().unwrap());
}

#[test]
fn test_callstack() {
    // Make sure that even for native functions that fail, the
    // name of the function is on the call stack.
    let d = assert::fail(
        r#"
def f():
    fail("bad")
f()
"#,
        "bad",
    );
    assert!(d.to_string().contains("fail(\"bad\")"));
}

#[test]
fn test_display_debug() {
    let heap = Heap::new();
    let val = heap.alloc((vec![1, 2], "test", true));
    assert_eq!(format!("{}", val), "([1, 2], \"test\", True)");
    assert_eq!(val.to_repr(), "([1, 2], \"test\", True)");
    assert_eq!(val.to_str(), "([1, 2], \"test\", True)");
    assert_eq!(
        format!("{:?}", val),
        "Value(TupleGen { content: [Value(ListGen(ListData { content: Cell { value: ValueTyped(Value(Array { len: 2, capacity: 4, iter_count: 0, content: [Value(1), Value(2)] })) } })), Value(\"test\"), Value(StarlarkBool(true))] })"
    );
    let v = heap.alloc("test");
    assert_eq!(format!("{}", v), "\"test\"");
    assert_eq!(v.to_repr(), "\"test\"");
    assert_eq!(v.to_str(), "test");
    assert_eq!(format!("{:?}", v), "Value(\"test\")");
    assert_eq!(format!("{:#?}", v), "Value(\n    \"test\",\n)");

    let frozen_heap = FrozenHeap::new();
    let v = frozen_heap.alloc("test");
    assert_eq!(format!("{}", v), "\"test\"");
    assert_eq!(v.to_value().to_repr(), "\"test\"");
    assert_eq!(v.to_value().to_str(), "test");
    assert_eq!(format!("{:?}", v), "FrozenValue(\"test\")");
    assert_eq!(format!("{:#?}", v), "FrozenValue(\n    \"test\",\n)");
}
