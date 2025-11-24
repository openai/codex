//! Development-related functionality

/// Test core functionality of synchronous stream cipher
#[macro_export]
#[cfg_attr(docsrs, doc(cfg(feature = "dev")))]
macro_rules! stream_cipher_test {
    ($name:ident, $test_name:expr, $cipher:ty $(,)?) => {
        #[test]
        fn $name() {
            use cipher::generic_array::GenericArray;
            use cipher::{blobby::Blob4Iterator, KeyIvInit, StreamCipher};

            let data = include_bytes!(concat!("data/", $test_name, ".blb"));
            for (i, row) in Blob4Iterator::new(data).unwrap().enumerate() {
                let [key, iv, pt, ct] = row.unwrap();

                for chunk_n in 1..256 {
                    let mut mode = <$cipher>::new_from_slices(key, iv).unwrap();
                    let mut pt = pt.to_vec();
                    for chunk in pt.chunks_mut(chunk_n) {
                        mode.apply_keystream(chunk);
                    }
                    if pt != &ct[..] {
                        panic!(
                            "Failed main test №{}, chunk size: {}\n\
                            key:\t{:?}\n\
                            iv:\t{:?}\n\
                            plaintext:\t{:?}\n\
                            ciphertext:\t{:?}\n",
                            i, chunk_n, key, iv, pt, ct,
                        );
                    }
                }
            }
        }
    };
}

/// Test stream synchronous stream cipher seeking capabilities
#[macro_export]
#[cfg_attr(docsrs, doc(cfg(feature = "dev")))]
macro_rules! stream_cipher_seek_test {
    ($name:ident, $cipher:ty) => {
        #[test]
        fn $name() {
            use cipher::generic_array::GenericArray;
            use cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};

            fn get_cipher() -> $cipher {
                <$cipher>::new(&Default::default(), &Default::default())
            }

            const MAX_SEEK: usize = 512;

            let mut ct = [0u8; MAX_SEEK];
            get_cipher().apply_keystream(&mut ct[..]);

            for n in 0..MAX_SEEK {
                let mut cipher = get_cipher();
                assert_eq!(cipher.current_pos::<usize>(), 0);
                cipher.seek(n);
                assert_eq!(cipher.current_pos::<usize>(), n);
                let mut buf = [0u8; MAX_SEEK];
                cipher.apply_keystream(&mut buf[n..]);
                assert_eq!(cipher.current_pos::<usize>(), MAX_SEEK);
                assert_eq!(&buf[n..], &ct[n..]);
            }

            const MAX_CHUNK: usize = 128;
            const MAX_LEN: usize = 1024;

            let mut buf = [0u8; MAX_CHUNK];
            let mut cipher = get_cipher();
            assert_eq!(cipher.current_pos::<usize>(), 0);
            cipher.apply_keystream(&mut []);
            assert_eq!(cipher.current_pos::<usize>(), 0);
            for n in 1..MAX_CHUNK {
                assert_eq!(cipher.current_pos::<usize>(), 0);
                for m in 1.. {
                    cipher.apply_keystream(&mut buf[..n]);
                    assert_eq!(cipher.current_pos::<usize>(), n * m);
                    if n * m > MAX_LEN {
                        break;
                    }
                }
                cipher.seek(0);
            }
        }
    };
}

/// Create stream cipher benchmarks
#[macro_export]
#[cfg_attr(docsrs, doc(cfg(feature = "dev")))]
macro_rules! stream_cipher_bench {
    (
        $cipher:ty;
        $($name:ident $bs:expr;)*
    ) => {
        $crate::stream_cipher_bench!(
            Init: {
                use $crate::KeyIvInit;
                let key = test::black_box(Default::default());
                let iv = test::black_box(Default::default());
                <$cipher>::new(&key, &iv)
            };
            $($name $bs;)*
        );
    };
    (
        Key: $cipher:ty;
        $($name:ident $bs:expr;)*
    ) => {
        $crate::stream_cipher_bench!(
            Init: {
                use $crate::KeyInit;
                let key = test::black_box(Default::default());
                let iv = test::black_box(Default::default());
                <$cipher>::new(&key, &iv)
            };
            $($name $bs;)*
        );
    };
    (
        Init: $init:expr;
        $($name:ident $bs:expr;)*
    ) => {
        $(
            #[bench]
            fn $name(b: &mut test::Bencher) {
                use $crate::StreamCipher;

                let mut cipher = $init;
                let mut buf = vec![0; $bs];

                b.iter(|| {
                    cipher.apply_keystream(&mut buf);
                    test::black_box(&buf);
                });

                b.bytes = $bs;
            }
        )*
    };
}
