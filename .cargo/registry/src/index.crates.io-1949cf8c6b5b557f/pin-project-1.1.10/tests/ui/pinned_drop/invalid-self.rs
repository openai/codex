// SPDX-License-Identifier: Apache-2.0 OR MIT

// by-ref binding `ref (mut) self` and sub-patterns `@` are not allowed in receivers (rejected by rustc).

use std::pin::Pin;

struct S {}

impl S {
    fn take_ref_self(ref self: Pin<&mut Self>) {} //~ ERROR expected identifier, found keyword `self`
    fn take_ref_mut_self(ref mut self: Pin<&mut Self>) {} //~ ERROR expected identifier, found keyword `self`

    fn self_subpat(self @ S {}: Self) {} //~ ERROR expected one of `)`, `,`, or `:`, found `@`
}

fn main() {}
