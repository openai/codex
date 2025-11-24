// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHOR OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

mod as_chunks;
mod as_chunks_mut;

pub use as_chunks::{as_chunks, AsChunks};
pub use as_chunks_mut::{as_chunks_mut, AsChunksMut};

// TODO(MSRV feature(split_at_checked)): Use `slice::split_at_checked`.
//
// Note that the libcore version is implemented in terms of
// `slice::split_at_unchecked()`, and `slice::split_at()` was changed to be
// implemented in terms of `split_at_checked`. For now, we implement this in
// terms of `split_at` and rely on the optimizer to eliminate the panic.
#[inline(always)]
pub fn split_at_checked<T>(slice: &[T], i: usize) -> Option<(&[T], &[T])> {
    if slice.len() >= i {
        Some(slice.split_at(i))
    } else {
        None
    }
}

// TODO(MSRV-1.77): Use `slice::split_first_chunk_mut`.
#[inline(always)]
pub fn split_first_chunk_mut<T, const N: usize>(
    slice: &mut [T],
) -> Option<(&mut [T; N], &mut [T])> {
    if slice.len() >= N {
        let (head, tail) = slice.split_at_mut(N);
        head.try_into().ok().map(|head| (head, tail))
    } else {
        None
    }
}
