// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

// This example illustrates a very simple type implementing Writeable.

#![no_main] // https://github.com/unicode-org/icu4x/issues/395
icu_benchmark_macros::instrument!();

use std::fmt;
use writeable::*;

struct WriteableMessage<W: Writeable>(W);

const GREETING: Part = Part {
    category: "meaning",
    value: "greeting",
};

const EMOJI: Part = Part {
    category: "meaning",
    value: "emoji",
};

impl<V: Writeable> Writeable for WriteableMessage<V> {
    fn write_to_parts<W: PartsWrite + ?Sized>(&self, sink: &mut W) -> fmt::Result {
        use fmt::Write;
        sink.with_part(GREETING, |g| {
            g.write_str("Hello")?;
            g.write_str(" ")?;
            self.0.write_to(g)
        })?;
        sink.write_char(' ')?;
        sink.with_part(EMOJI, |e| e.write_char('😅'))
    }

    fn writeable_length_hint(&self) -> LengthHint {
        LengthHint::exact(11) + self.0.writeable_length_hint()
    }
}

impl<V: Writeable> fmt::Display for WriteableMessage<V> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.write_to(f)
    }
}

fn main() {
    let (string, parts) =
        writeable::_internal::writeable_to_parts_for_test(&WriteableMessage("world"));

    assert_eq!(string, "Hello world 😅");

    // Print the greeting only
    let (start, end, _) = parts
        .into_iter()
        .find(|(_, _, part)| part == &GREETING)
        .unwrap();
    println!("{}", &string[start..end]);
}
