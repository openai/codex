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

use std::slice;
use std::str::Chars;

pub struct CursorBytes<'a>(&'a str, slice::Iter<'a, u8>);

impl<'a> CursorBytes<'a> {
    pub fn new(x: &'a str) -> Self {
        Self(x, x.as_bytes().iter())
    }

    pub fn next(&mut self) -> Option<u8> {
        self.1.next().copied()
    }

    // If it returns a value great than 127, it should not be trusted
    pub fn next_char(&mut self) -> Option<char> {
        self.next().map(|x| x as char)
    }

    pub fn pos(&self) -> usize {
        self.0.len() - self.1.as_slice().len()
    }
}

pub struct CursorChars<'a>(&'a str, Chars<'a>);

impl<'a> CursorChars<'a> {
    pub fn new_offset(x: &'a str, offset: usize) -> Self {
        Self(x, x[offset..].chars())
    }

    pub fn next(&mut self) -> Option<char> {
        self.1.next()
    }

    /// Call `unnext` to put back a character you grabbed with next.
    /// It is an error if the character isn't what you declared.
    pub fn unnext(&mut self, c: char) {
        let pos = self.pos();
        self.1 = self.0[pos - c.len_utf8()..].chars();
        debug_assert_eq!(self.peek(), Some(c))
    }

    pub fn peek(&self) -> Option<char> {
        self.1.as_str().chars().next()
    }

    pub fn pos(&self) -> usize {
        self.0.len() - self.1.as_str().len()
    }
}
