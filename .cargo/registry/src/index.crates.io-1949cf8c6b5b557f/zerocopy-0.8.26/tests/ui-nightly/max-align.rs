// Copyright 2023 The Fuchsia Authors
//
// Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

#[repr(C, align(1))]
struct Align1;

#[repr(C, align(2))]
struct Align2;

#[repr(C, align(4))]
struct Align4;

#[repr(C, align(8))]
struct Align8;

#[repr(C, align(16))]
struct Align16;

#[repr(C, align(32))]
struct Align32;

#[repr(C, align(64))]
struct Align64;

#[repr(C, align(128))]
struct Align128;

#[repr(C, align(256))]
struct Align256;

#[repr(C, align(512))]
struct Align512;

#[repr(C, align(1024))]
struct Align1024;

#[repr(C, align(2048))]
struct Align2048;

#[repr(C, align(4096))]
struct Align4096;

#[repr(C, align(8192))]
struct Align8192;

#[repr(C, align(16384))]
struct Align16384;

#[repr(C, align(32768))]
struct Align32768;

#[repr(C, align(65536))]
struct Align65536;

#[repr(C, align(131072))]
struct Align131072;

#[repr(C, align(262144))]
struct Align262144;

#[repr(C, align(524288))]
struct Align524288;

#[repr(C, align(1048576))]
struct Align1048576;

#[repr(C, align(2097152))]
struct Align2097152;

#[repr(C, align(4194304))]
struct Align4194304;

#[repr(C, align(8388608))]
struct Align8388608;

#[repr(C, align(16777216))]
struct Align16777216;

#[repr(C, align(33554432))]
struct Align33554432;

#[repr(C, align(67108864))]
struct Align67108864;

#[repr(C, align(134217728))]
struct Align13421772;

#[repr(C, align(268435456))]
struct Align26843545;

#[repr(C, align(1073741824))]
struct Align1073741824;

fn main() {}
