// SPDX-License-Identifier: Apache-2.0 OR MIT

// Even if allows private_in_public, these are errors.

#![allow(private_interfaces, dead_code)]

pub enum PublicEnum {
    V(PrivateEnum), //~ ERROR E0446
}

enum PrivateEnum {
    V(u8),
}

mod foo {
    pub(crate) enum CrateEnum {
        V(PrivateEnum), //~ ERROR E0446
    }

    enum PrivateEnum {
        V(u8),
    }
}

fn main() {}
