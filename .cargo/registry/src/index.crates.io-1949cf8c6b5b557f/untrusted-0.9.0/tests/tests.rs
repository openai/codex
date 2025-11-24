// Copyright 2015-2019 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHORS DISCLAIM ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

#[test]
fn test_debug() {
    const INPUTS: &[&[u8]] = &[b"", b"foo"];
    for input in INPUTS {
        let input = untrusted::Input::from(input);
        assert_eq!(format!("{:?}", &input), "Input");
        input
            .read_all(untrusted::EndOfInput, |r| {
                assert_eq!(format!("{:?}", r), "Reader");
                r.skip_to_end();
                assert_eq!(format!("{:?}", r), "Reader");
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn test_input_clone_and_copy() {
    const INPUTS: &[&[u8]] = &[b"", b"a", b"foo"];
    for input in INPUTS {
        let input = untrusted::Input::from(input);
        let copy = input;
        assert_eq!(input.as_slice_less_safe(), copy.as_slice_less_safe());
        assert_eq!(
            input.as_slice_less_safe(),
            input.clone().as_slice_less_safe()
        );
    }
}

#[test]
fn test_input_from() {
    let _ = untrusted::Input::from(b"foo");
}

#[test]
fn test_input_is_empty() {
    let input = untrusted::Input::from(b"");
    assert!(input.is_empty());
    let input = untrusted::Input::from(b"foo");
    assert!(!input.is_empty());
}

#[test]
fn test_input_len() {
    let input = untrusted::Input::from(b"foo");
    assert_eq!(input.len(), 3);
}

#[test]
fn test_input_read_all() {
    let input = untrusted::Input::from(b"foo");
    let result = input.read_all(untrusted::EndOfInput, |input| {
        assert_eq!(b'f', input.read_byte()?);
        assert_eq!(b'o', input.read_byte()?);
        assert_eq!(b'o', input.read_byte()?);
        assert!(input.at_end());
        Ok(())
    });
    assert_eq!(result, Ok(()));
}

#[test]
fn test_input_read_all_unconsume() {
    let input = untrusted::Input::from(b"foo");
    let result = input.read_all(untrusted::EndOfInput, |input| {
        assert_eq!(b'f', input.read_byte()?);
        assert!(!input.at_end());
        Ok(())
    });
    assert_eq!(result, Err(untrusted::EndOfInput));
}

#[test]
fn test_input_as_slice_less_safe() {
    let slice = b"foo";
    let input = untrusted::Input::from(slice);
    assert_eq!(input.as_slice_less_safe(), slice);
}

#[test]
fn using_reader_after_skip_and_get_error_returns_error_must_not_panic() {
    let input = untrusted::Input::from(&[]);
    let r = input.read_all(untrusted::EndOfInput, |input| {
        let r = input.read_bytes(1);
        assert_eq!(r.unwrap_err(), untrusted::EndOfInput);
        Ok(input.read_bytes_to_end())
    });
    let _ = r; // "Use" r. The value of `r` is undefined here.
}

#[test]
fn size_assumptions() {
    // Assume that a pointer can address any point in the address space, and
    // infer that this implies that a byte slice will never be
    // `core::usize::MAX` bytes long.
    assert_eq!(
        core::mem::size_of::<*const u8>(),
        core::mem::size_of::<usize>()
    );
}

#[test]
fn const_fn() {
    const _INPUT: untrusted::Input<'static> = untrusted::Input::from(&[]);
}

#[test]
fn test_vec_into() {
    extern crate std;
    let vec = vec![0u8; 0];
    let _x: untrusted::Input = (&vec[..]).into();
}

#[test]
fn test_from_slice() {
    let slice: &[u8] = &[0u8];
    let _x: untrusted::Input = slice.into();
}
