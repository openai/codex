// Copyright 2013-2014 The Rust Project Developers.
// Copyright 2018 The Uuid Project Developers.
//
// See the COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::{non_nil::NonNilUuid, Uuid};

impl slog::Value for Uuid {
    fn serialize(
        &self,
        _: &slog::Record<'_>,
        key: slog::Key,
        serializer: &mut dyn slog::Serializer,
    ) -> Result<(), slog::Error> {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}

impl slog::Value for NonNilUuid {
    fn serialize(
        &self,
        record: &slog::Record<'_>,
        key: slog::Key,
        serializer: &mut dyn slog::Serializer,
    ) -> Result<(), slog::Error> {
        Uuid::from(*self).serialize(record, key, serializer)
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::new;

    use slog::{crit, Drain};

    #[test]
    fn test_slog_kv() {
        let root = slog::Logger::root(slog::Discard.fuse(), slog::o!());
        let u1 = new();
        crit!(root, "test"; "u1" => u1);
    }
}
