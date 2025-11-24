use block_buffer::{
    generic_array::typenum::{U10, U16, U24, U4, U8},
    Block, EagerBuffer, LazyBuffer,
};

#[test]
fn test_eager_digest_pad() {
    let mut buf = EagerBuffer::<U4>::default();
    let inputs = [
        &b"01234567"[..],
        &b"89"[..],
        &b"abcdefghij"[..],
        &b"klmnopqrs"[..],
        &b"tuv"[..],
        &b"wx"[..],
    ];
    let exp_blocks = [
        (0, &[b"0123", b"4567"][..]),
        (2, &[b"89ab"][..]),
        (2, &[b"cdef", b"ghij"][..]),
        (3, &[b"klmn", b"opqr"][..]),
        (4, &[b"stuv"][..]),
    ];
    let exp_poses = [0, 2, 0, 1, 0, 2];

    let mut n = 0;
    for (i, input) in inputs.iter().enumerate() {
        buf.digest_blocks(input, |b| {
            let (j, exp) = exp_blocks[n];
            n += 1;
            assert_eq!(i, j);
            assert_eq!(b.len(), exp.len());
            assert!(b.iter().zip(exp.iter()).all(|v| v.0[..] == v.1[..]));
        });
        assert_eq!(exp_poses[i], buf.get_pos());
    }
    assert_eq!(buf.pad_with_zeros()[..], b"wx\0\0"[..]);
    assert_eq!(buf.get_pos(), 0);
}

#[test]
fn test_lazy_digest_pad() {
    let mut buf = LazyBuffer::<U4>::default();
    let inputs = [
        &b"01234567"[..],
        &b"89"[..],
        &b"abcdefghij"[..],
        &b"klmnopqrs"[..],
    ];
    let expected = [
        (0, &[b"0123"][..]),
        (1, &[b"4567"][..]),
        (2, &[b"89ab"][..]),
        (2, &[b"cdef"][..]),
        (3, &[b"ghij"][..]),
        (3, &[b"klmn", b"opqr"][..]),
    ];
    let exp_poses = [4, 2, 4, 1];

    let mut n = 0;
    for (i, input) in inputs.iter().enumerate() {
        buf.digest_blocks(input, |b| {
            let (j, exp) = expected[n];
            n += 1;
            assert_eq!(i, j);
            assert_eq!(b.len(), exp.len());
            assert!(b.iter().zip(exp.iter()).all(|v| v.0[..] == v.1[..]));
        });
        assert_eq!(exp_poses[i], buf.get_pos());
    }
    assert_eq!(buf.pad_with_zeros()[..], b"s\0\0\0"[..]);
    assert_eq!(buf.get_pos(), 0);
}

#[test]
fn test_eager_set_data() {
    let mut buf = EagerBuffer::<U4>::default();

    let mut n = 0u8;
    let mut gen = |blocks: &mut [Block<U4>]| {
        for block in blocks {
            block.iter_mut().for_each(|b| *b = n);
            n += 1;
        }
    };

    let mut out = [0u8; 6];
    buf.set_data(&mut out, &mut gen);
    assert_eq!(out, [0, 0, 0, 0, 1, 1]);
    assert_eq!(buf.get_pos(), 2);

    let mut out = [0u8; 3];
    buf.set_data(&mut out, &mut gen);
    assert_eq!(out, [1, 1, 2]);
    assert_eq!(buf.get_pos(), 1);

    let mut out = [0u8; 3];
    buf.set_data(&mut out, &mut gen);
    assert_eq!(out, [2, 2, 2]);
    assert_eq!(n, 3);
    assert_eq!(buf.get_pos(), 0);
}

#[test]
#[rustfmt::skip]
fn test_eager_paddings() {
    let mut buf_be = EagerBuffer::<U8>::new(&[0x42]);
    let mut buf_le = buf_be.clone();
    let mut out_be = Vec::<u8>::new();
    let mut out_le = Vec::<u8>::new();
    let len = 0x0001_0203_0405_0607;
    buf_be.len64_padding_be(len, |block| out_be.extend(block));
    buf_le.len64_padding_le(len, |block| out_le.extend(block));

    assert_eq!(
        out_be,
        [
            0x42, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
        ],
    );
    assert_eq!(
        out_le,
        [
            0x42, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x00,
        ],
    );

    let mut buf_be = EagerBuffer::<U10>::new(&[0x42]);
    let mut buf_le = buf_be.clone();
    let mut out_be = Vec::<u8>::new();
    let mut out_le = Vec::<u8>::new();
    buf_be.len64_padding_be(len, |block| out_be.extend(block));
    buf_le.len64_padding_le(len, |block| out_le.extend(block));

    assert_eq!(
        out_be,
        [0x42, 0x80, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07],
    );
    assert_eq!(
        out_le,
        [0x42, 0x80, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x00],
    );

    let mut buf = EagerBuffer::<U16>::new(&[0x42]);
    let mut out = Vec::<u8>::new();
    let len = 0x0001_0203_0405_0607_0809_0a0b_0c0d_0e0f;
    buf.len128_padding_be(len, |block| out.extend(block));
    assert_eq!(
        out,
        [
            0x42, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
            0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        ],
    );

    let mut buf = EagerBuffer::<U24>::new(&[0x42]);
    let mut out = Vec::<u8>::new();
    let len = 0x0001_0203_0405_0607_0809_0a0b_0c0d_0e0f;
    buf.len128_padding_be(len, |block| out.extend(block));
    assert_eq!(
        out,
        [
            0x42, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
            0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        ],
    );

    let mut buf = EagerBuffer::<U4>::new(&[0x42]);
    let mut out = Vec::<u8>::new();
    buf.digest_pad(0xff, &[0x10, 0x11, 0x12], |block| out.extend(block));
    assert_eq!(
        out,
        [0x42, 0xff, 0x00, 0x00, 0x00, 0x10, 0x11, 0x12],
    );

    let mut buf = EagerBuffer::<U4>::new(&[0x42]);
    let mut out = Vec::<u8>::new();
    buf.digest_pad(0xff, &[0x10, 0x11], |block| out.extend(block));
    assert_eq!(
        out,
        [0x42, 0xff, 0x10, 0x11],
    );
}

#[test]
fn test_try_new() {
    assert!(EagerBuffer::<U4>::try_new(&[0; 3]).is_ok());
    assert!(EagerBuffer::<U4>::try_new(&[0; 4]).is_err());
    assert!(LazyBuffer::<U4>::try_new(&[0; 4]).is_ok());
    assert!(LazyBuffer::<U4>::try_new(&[0; 5]).is_err());
}
