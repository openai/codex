pub mod bufread;
pub mod read;
pub mod write;

#[cfg(test)]
mod tests {
    use std::io::prelude::*;

    use rand::{rng, Rng};

    use super::{read, write};
    use crate::Compression;

    #[test]
    fn roundtrip() {
        let mut real = Vec::new();
        let mut w = write::DeflateEncoder::new(Vec::new(), Compression::default());
        let v = crate::random_bytes().take(1024).collect::<Vec<_>>();
        for _ in 0..200 {
            let to_write = &v[..rng().random_range(0..v.len())];
            real.extend(to_write.iter().copied());
            w.write_all(to_write).unwrap();
        }
        let result = w.finish().unwrap();
        let mut r = read::DeflateDecoder::new(&result[..]);
        let mut ret = Vec::new();
        r.read_to_end(&mut ret).unwrap();
        assert_eq!(ret, real);
    }

    #[test]
    fn drop_writes() {
        let mut data = Vec::new();
        write::DeflateEncoder::new(&mut data, Compression::default())
            .write_all(b"foo")
            .unwrap();
        let mut r = read::DeflateDecoder::new(&data[..]);
        let mut ret = Vec::new();
        r.read_to_end(&mut ret).unwrap();
        assert_eq!(ret, b"foo");
    }

    #[test]
    fn total_in() {
        let mut real = Vec::new();
        let mut w = write::DeflateEncoder::new(Vec::new(), Compression::default());
        let v = crate::random_bytes().take(1024).collect::<Vec<_>>();
        for _ in 0..200 {
            let to_write = &v[..rng().random_range(0..v.len())];
            real.extend(to_write.iter().copied());
            w.write_all(to_write).unwrap();
        }
        let mut result = w.finish().unwrap();

        let result_len = result.len();

        for _ in 0..200 {
            result.extend(v.iter().copied());
        }

        let mut r = read::DeflateDecoder::new(&result[..]);
        let mut ret = Vec::new();
        r.read_to_end(&mut ret).unwrap();
        assert_eq!(ret, real);
        assert_eq!(r.total_in(), result_len as u64);
    }

    #[test]
    fn roundtrip2() {
        let v = crate::random_bytes().take(1024 * 1024).collect::<Vec<_>>();
        let mut r =
            read::DeflateDecoder::new(read::DeflateEncoder::new(&v[..], Compression::default()));
        let mut ret = Vec::new();
        r.read_to_end(&mut ret).unwrap();
        assert_eq!(ret, v);
    }

    #[test]
    fn roundtrip3() {
        let v = crate::random_bytes().take(1024 * 1024).collect::<Vec<_>>();
        let mut w = write::DeflateEncoder::new(
            write::DeflateDecoder::new(Vec::new()),
            Compression::default(),
        );
        w.write_all(&v).unwrap();
        let w = w.finish().unwrap().finish().unwrap();
        assert_eq!(w, v);
    }

    #[test]
    fn reset_writer() {
        let v = crate::random_bytes().take(1024 * 1024).collect::<Vec<_>>();
        let mut w = write::DeflateEncoder::new(Vec::new(), Compression::default());
        w.write_all(&v).unwrap();
        let a = w.reset(Vec::new()).unwrap();
        w.write_all(&v).unwrap();
        let b = w.finish().unwrap();

        let mut w = write::DeflateEncoder::new(Vec::new(), Compression::default());
        w.write_all(&v).unwrap();
        let c = w.finish().unwrap();
        assert!(a == b && b == c);
    }

    #[test]
    fn reset_reader() {
        let v = crate::random_bytes().take(1024 * 1024).collect::<Vec<_>>();
        let (mut a, mut b, mut c) = (Vec::new(), Vec::new(), Vec::new());
        let mut r = read::DeflateEncoder::new(&v[..], Compression::default());
        r.read_to_end(&mut a).unwrap();
        r.reset(&v[..]);
        r.read_to_end(&mut b).unwrap();

        let mut r = read::DeflateEncoder::new(&v[..], Compression::default());
        r.read_to_end(&mut c).unwrap();
        assert!(a == b && b == c);
    }

    #[test]
    fn reset_decoder() {
        let v = crate::random_bytes().take(1024 * 1024).collect::<Vec<_>>();
        let mut w = write::DeflateEncoder::new(Vec::new(), Compression::default());
        w.write_all(&v).unwrap();
        let data = w.finish().unwrap();

        {
            let (mut a, mut b, mut c) = (Vec::new(), Vec::new(), Vec::new());
            let mut r = read::DeflateDecoder::new(&data[..]);
            r.read_to_end(&mut a).unwrap();
            r.reset(&data);
            r.read_to_end(&mut b).unwrap();

            let mut r = read::DeflateDecoder::new(&data[..]);
            r.read_to_end(&mut c).unwrap();
            assert!(a == b && b == c && c == v);
        }

        {
            let mut w = write::DeflateDecoder::new(Vec::new());
            w.write_all(&data).unwrap();
            let a = w.reset(Vec::new()).unwrap();
            w.write_all(&data).unwrap();
            let b = w.finish().unwrap();

            let mut w = write::DeflateDecoder::new(Vec::new());
            w.write_all(&data).unwrap();
            let c = w.finish().unwrap();
            assert!(a == b && b == c && c == v);
        }
    }

    #[test]
    fn zero_length_read_with_data() {
        let m = vec![3u8; 128 * 1024 + 1];
        let mut c = read::DeflateEncoder::new(&m[..], Compression::default());

        let mut result = Vec::new();
        c.read_to_end(&mut result).unwrap();

        let mut d = read::DeflateDecoder::new(&result[..]);
        let mut data = Vec::new();
        assert_eq!(d.read(&mut data).unwrap(), 0);
    }

    #[test]
    fn qc_reader() {
        ::quickcheck::quickcheck(test as fn(_) -> _);

        fn test(v: Vec<u8>) -> bool {
            let mut r = read::DeflateDecoder::new(read::DeflateEncoder::new(
                &v[..],
                Compression::default(),
            ));
            let mut v2 = Vec::new();
            r.read_to_end(&mut v2).unwrap();
            v == v2
        }
    }

    #[test]
    fn qc_writer() {
        ::quickcheck::quickcheck(test as fn(_) -> _);

        fn test(v: Vec<u8>) -> bool {
            let mut w = write::DeflateEncoder::new(
                write::DeflateDecoder::new(Vec::new()),
                Compression::default(),
            );
            w.write_all(&v).unwrap();
            v == w.finish().unwrap().finish().unwrap()
        }
    }
}
