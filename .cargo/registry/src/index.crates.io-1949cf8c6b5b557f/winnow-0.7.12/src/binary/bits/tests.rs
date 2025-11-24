use super::*;
use crate::error::ErrMode;
use crate::error::InputError;
use crate::prelude::*;
use crate::Partial;

#[test]
/// Take the `bits` function and assert that remaining bytes are correctly returned, if the
/// previous bytes are fully consumed
fn test_complete_byte_consumption_bits() {
    let input = &[0x12, 0x34, 0x56, 0x78][..];

    // Take 3 bit slices with sizes [4, 8, 4].
    #[allow(clippy::type_complexity)]
    let result: ModalResult<(&[u8], (u8, u8, u8)), InputError<_>> =
        bits::<_, _, ErrMode<InputError<(&[u8], usize)>>, _, _>((
            take(4usize),
            take(8usize),
            take(4usize),
        ))
        .parse_peek(input);

    let output = result.expect("We take 2 bytes and the input is longer than 2 bytes");

    let remaining = output.0;
    assert_eq!(remaining, [0x56, 0x78]);

    let parsed = output.1;
    assert_eq!(parsed.0, 0x01);
    assert_eq!(parsed.1, 0x23);
    assert_eq!(parsed.2, 0x04);
}

#[test]
/// Take the `bits` function and assert that remaining bytes are correctly returned, if the
/// previous bytes are NOT fully consumed. Partially consumed bytes are supposed to be dropped.
/// I.e. if we consume 1.5 bytes of 4 bytes, 2 bytes will be returned, bits 13-16 will be
/// dropped.
fn test_partial_byte_consumption_bits() {
    let input = &[0x12, 0x34, 0x56, 0x78][..];

    // Take bit slices with sizes [4, 8].
    let result: ModalResult<(&[u8], (u8, u8)), InputError<_>> =
        bits::<_, _, ErrMode<InputError<(&[u8], usize)>>, _, _>((take(4usize), take(8usize)))
            .parse_peek(input);

    let output = result.expect("We take 1.5 bytes and the input is longer than 2 bytes");

    let remaining = output.0;
    assert_eq!(remaining, [0x56, 0x78]);

    let parsed = output.1;
    assert_eq!(parsed.0, 0x01);
    assert_eq!(parsed.1, 0x23);
}

#[test]
#[cfg(feature = "std")]
/// Ensure that in Incomplete error is thrown, if too few bytes are passed for a given parser.
fn test_incomplete_bits() {
    let input = Partial::new(&[0x12][..]);

    // Take bit slices with sizes [4, 8].
    let result: ModalResult<(_, (u8, u8)), InputError<_>> =
        bits::<_, _, ErrMode<InputError<(_, usize)>>, _, _>((take(4usize), take(8usize)))
            .parse_peek(input);

    assert!(result.is_err());
    let error = result.err().unwrap();
    assert_eq!("Parsing requires 2 more data", error.to_string());
}

#[test]
fn test_take_complete_0() {
    let input = &[0b00010010][..];
    let count = 0usize;
    assert_eq!(count, 0usize);
    let offset = 0usize;

    let result: ModalResult<((&[u8], usize), usize), InputError<_>> =
        take(count).parse_peek((input, offset));

    assert_eq!(result, Ok(((input, offset), 0)));
}

#[test]
fn test_take_complete_eof() {
    let input = &[0b00010010][..];

    let result: ModalResult<((&[u8], usize), usize), InputError<_>> =
        take(1usize).parse_peek((input, 8));

    assert_eq!(
        result,
        Err(crate::error::ErrMode::Backtrack(
            InputError::at((input, 8),)
        ))
    );
}

#[test]
fn test_take_complete_span_over_multiple_bytes() {
    let input = &[0b00010010, 0b00110100, 0b11111111, 0b11111111][..];

    let result: ModalResult<((&[u8], usize), usize), InputError<_>> =
        take(24usize).parse_peek((input, 4));

    assert_eq!(
        result,
        Ok((([0b11111111].as_ref(), 4), 0b1000110100111111111111))
    );
}

#[test]
fn test_take_partial_0() {
    let input = Partial::new(&[][..]);
    let count = 0usize;
    assert_eq!(count, 0usize);
    let offset = 0usize;

    let result: ModalResult<((_, usize), usize), InputError<_>> =
        take(count).parse_peek((input, offset));

    assert_eq!(result, Ok(((input, offset), 0)));
}

#[test]
fn test_pattern_partial_ok() {
    let input = Partial::new(&[0b00011111][..]);
    let offset = 0usize;
    let bits_to_take = 4usize;
    let value_to_pattern = 0b0001;

    let result: ModalResult<((_, usize), usize), InputError<_>> =
        pattern(value_to_pattern, bits_to_take).parse_peek((input, offset));

    assert_eq!(result, Ok(((input, bits_to_take), value_to_pattern)));
}

#[test]
fn test_pattern_partial_err() {
    let input = Partial::new(&[0b00011111][..]);
    let offset = 0usize;
    let bits_to_take = 4usize;
    let value_to_pattern = 0b1111;

    let result: ModalResult<((_, usize), usize), InputError<_>> =
        pattern(value_to_pattern, bits_to_take).parse_peek((input, offset));

    assert_eq!(
        result,
        Err(crate::error::ErrMode::Backtrack(InputError::at((
            input, offset
        ),)))
    );
}

#[test]
fn test_bool_0_complete() {
    let input = [0b10000000].as_ref();

    let result: ModalResult<((&[u8], usize), bool), InputError<_>> = bool.parse_peek((input, 0));

    assert_eq!(result, Ok(((input, 1), true)));
}

#[test]
fn test_bool_eof_complete() {
    let input = [0b10000000].as_ref();

    let result: ModalResult<((&[u8], usize), bool), InputError<_>> = bool.parse_peek((input, 8));

    assert_eq!(
        result,
        Err(crate::error::ErrMode::Backtrack(
            InputError::at((input, 8),)
        ))
    );
}

#[test]
fn test_bool_0_partial() {
    let input = Partial::new([0b10000000].as_ref());

    #[allow(clippy::type_complexity)]
    let result: ModalResult<((Partial<&[u8]>, usize), bool), InputError<_>> =
        bool.parse_peek((input, 0));

    assert_eq!(result, Ok(((input, 1), true)));
}

#[test]
fn test_bool_eof_partial() {
    let input = Partial::new([0b10000000].as_ref());

    #[allow(clippy::type_complexity)]
    let result: ModalResult<((Partial<&[u8]>, usize), bool), InputError<_>> =
        bool.parse_peek((input, 8));

    assert_eq!(
        result,
        Err(crate::error::ErrMode::Incomplete(Needed::new(1)))
    );
}
