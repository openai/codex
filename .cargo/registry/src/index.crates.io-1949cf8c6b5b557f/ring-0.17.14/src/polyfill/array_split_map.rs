// Copyright 2023 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
// OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

pub trait ArraySplitMap<I, O, const CN: usize, const ON: usize> {
    fn array_split_map(self, f: impl Fn([I; CN]) -> O) -> [O; ON];
}

impl<I, O> ArraySplitMap<I, O, 4, 3> for [I; 12] {
    #[inline]
    fn array_split_map(self, f: impl Fn([I; 4]) -> O) -> [O; 3] {
        let [a0, a1, a2, a3, b0, b1, b2, b3, c0, c1, c2, c3] = self;
        [
            f([a0, a1, a2, a3]),
            f([b0, b1, b2, b3]),
            f([c0, c1, c2, c3]),
        ]
    }
}

impl<I, O> ArraySplitMap<I, O, 4, 4> for [I; 16] {
    #[inline]
    fn array_split_map(self, f: impl Fn([I; 4]) -> O) -> [O; 4] {
        let [a0, a1, a2, a3, b0, b1, b2, b3, c0, c1, c2, c3, d0, d1, d2, d3] = self;
        [
            f([a0, a1, a2, a3]),
            f([b0, b1, b2, b3]),
            f([c0, c1, c2, c3]),
            f([d0, d1, d2, d3]),
        ]
    }
}

impl<I, O> ArraySplitMap<I, O, 4, 8> for [I; 32] {
    #[inline]
    fn array_split_map(self, f: impl Fn([I; 4]) -> O) -> [O; 8] {
        let [a0, a1, a2, a3, b0, b1, b2, b3, c0, c1, c2, c3, d0, d1, d2, d3, e0, e1, e2, e3, f0, f1, f2, f3, g0, g1, g2, g3, h0, h1, h2, h3] =
            self;
        [
            f([a0, a1, a2, a3]),
            f([b0, b1, b2, b3]),
            f([c0, c1, c2, c3]),
            f([d0, d1, d2, d3]),
            f([e0, e1, e2, e3]),
            f([f0, f1, f2, f3]),
            f([g0, g1, g2, g3]),
            f([h0, h1, h2, h3]),
        ]
    }
}

impl<I, O> ArraySplitMap<I, O, 8, 2> for [I; 16] {
    #[inline]
    fn array_split_map(self, f: impl Fn([I; 8]) -> O) -> [O; 2] {
        let [a0, a1, a2, a3, a4, a5, a6, a7, b0, b1, b2, b3, b4, b5, b6, b7] = self;
        [
            f([a0, a1, a2, a3, a4, a5, a6, a7]),
            f([b0, b1, b2, b3, b4, b5, b6, b7]),
        ]
    }
}
