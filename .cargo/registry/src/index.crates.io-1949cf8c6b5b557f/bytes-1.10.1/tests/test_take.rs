#![warn(rust_2018_idioms)]

use bytes::buf::Buf;
use bytes::Bytes;

#[test]
fn long_take() {
    // Tests that get a take with a size greater than the buffer length will not
    // overrun the buffer. Regression test for #138.
    let buf = b"hello world".take(100);
    assert_eq!(11, buf.remaining());
    assert_eq!(b"hello world", buf.chunk());
}

#[test]
fn take_copy_to_bytes() {
    let mut abcd = Bytes::copy_from_slice(b"abcd");
    let abcd_ptr = abcd.as_ptr();
    let mut take = (&mut abcd).take(2);
    let a = take.copy_to_bytes(1);
    assert_eq!(Bytes::copy_from_slice(b"a"), a);
    // assert `to_bytes` did not allocate
    assert_eq!(abcd_ptr, a.as_ptr());
    assert_eq!(Bytes::copy_from_slice(b"bcd"), abcd);
}

#[test]
#[should_panic]
fn take_copy_to_bytes_panics() {
    let abcd = Bytes::copy_from_slice(b"abcd");
    abcd.take(2).copy_to_bytes(3);
}

#[cfg(feature = "std")]
#[test]
fn take_chunks_vectored() {
    fn chain() -> impl Buf {
        Bytes::from([1, 2, 3].to_vec()).chain(Bytes::from([4, 5, 6].to_vec()))
    }

    {
        let mut dst = [std::io::IoSlice::new(&[]); 2];
        let take = chain().take(0);
        assert_eq!(take.chunks_vectored(&mut dst), 0);
    }

    {
        let mut dst = [std::io::IoSlice::new(&[]); 2];
        let take = chain().take(1);
        assert_eq!(take.chunks_vectored(&mut dst), 1);
        assert_eq!(&*dst[0], &[1]);
    }

    {
        let mut dst = [std::io::IoSlice::new(&[]); 2];
        let take = chain().take(3);
        assert_eq!(take.chunks_vectored(&mut dst), 1);
        assert_eq!(&*dst[0], &[1, 2, 3]);
    }

    {
        let mut dst = [std::io::IoSlice::new(&[]); 2];
        let take = chain().take(4);
        assert_eq!(take.chunks_vectored(&mut dst), 2);
        assert_eq!(&*dst[0], &[1, 2, 3]);
        assert_eq!(&*dst[1], &[4]);
    }

    {
        let mut dst = [std::io::IoSlice::new(&[]); 2];
        let take = chain().take(6);
        assert_eq!(take.chunks_vectored(&mut dst), 2);
        assert_eq!(&*dst[0], &[1, 2, 3]);
        assert_eq!(&*dst[1], &[4, 5, 6]);
    }

    {
        let mut dst = [std::io::IoSlice::new(&[]); 2];
        let take = chain().take(7);
        assert_eq!(take.chunks_vectored(&mut dst), 2);
        assert_eq!(&*dst[0], &[1, 2, 3]);
        assert_eq!(&*dst[1], &[4, 5, 6]);
    }
}
