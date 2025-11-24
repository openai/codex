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

use std::fmt::Arguments;
use std::io;
use std::process;

/// Like `panic!`, but aborts the process instead of unwinding.
///
/// Although we compile buck2 with `panic=abort`, this is safer because
/// others may copy-paste code.
macro_rules! rtabort {
    ($m:literal) => {
        $crate::util::rtabort::rtabort_impl_fixed_string(
            file!(),
            line!(),
            $m,
        )
    };
    ($($t:tt)*) => {
        $crate::util::rtabort::rtabort_impl(
            file!(),
            line!(),
            format_args!($($t)*),
        )
    };
}

pub(crate) use rtabort;

#[cold]
pub(crate) fn rtabort_impl_fixed_string(file: &str, line: u32, message: &str) -> ! {
    rtabort_impl(file, line, format_args!("{}", message));
}

#[cold]
pub(crate) fn rtabort_impl(file: &str, line: u32, msg: Arguments) -> ! {
    // Make sure we abort even if formatting panics.
    let _abort = AbortOnDrop;

    // `eprintln!` followed by `abort` does not print anything in tests.
    io::Write::write_fmt(
        &mut io::stderr(),
        format_args!("{}:{}: abort: {}\n", file, line, msg),
    )
    .ok();

    // Tell the compiler that we never return.
    process::abort();
}

struct AbortOnDrop;

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        process::abort();
    }
}

#[cfg(test)]
mod tests {
    fn _test_compiles_fixed_string() {
        rtabort!("test");
    }

    fn _test_compiles_with_format_args() {
        rtabort!("test {}", 17);
    }

    #[test]
    fn test_rtabort() {
        // Uncomment to test.
        // rtabort!("test {}", 17);
        // rtabort!("test {}", { panic!(); 17 });
    }
}
