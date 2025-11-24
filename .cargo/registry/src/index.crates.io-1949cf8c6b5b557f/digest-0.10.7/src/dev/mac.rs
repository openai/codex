/// Define MAC test
#[macro_export]
#[cfg(feature = "mac")]
#[cfg_attr(docsrs, doc(cfg(all(feature = "dev", feature = "mac"))))]
macro_rules! new_mac_test {
    ($name:ident, $test_name:expr, $mac:ty $(,)?) => {
        digest::new_mac_test!($name, $test_name, $mac, "");
    };
    ($name:ident, $test_name:expr, $mac:ty, trunc_left $(,)?) => {
        digest::new_mac_test!($name, $test_name, $mac, "left");
    };
    ($name:ident, $test_name:expr, $mac:ty, trunc_right $(,)?) => {
        digest::new_mac_test!($name, $test_name, $mac, "right");
    };
    ($name:ident, $test_name:expr, $mac:ty, $trunc:expr $(,)?) => {
        #[test]
        fn $name() {
            use core::cmp::min;
            use digest::dev::blobby::Blob3Iterator;
            use digest::Mac;

            fn run_test(key: &[u8], input: &[u8], tag: &[u8]) -> Option<&'static str> {
                let mac0 = <$mac as Mac>::new_from_slice(key).unwrap();

                let mut mac = mac0.clone();
                mac.update(input);
                let result = mac.finalize().into_bytes();
                let n = tag.len();
                let result_bytes = match $trunc {
                    "left" => &result[..n],
                    "right" => &result[result.len() - n..],
                    _ => &result[..],
                };
                if result_bytes != tag {
                    return Some("whole message");
                }

                // test reading different chunk sizes
                for chunk_size in 1..min(64, input.len()) {
                    let mut mac = mac0.clone();
                    for chunk in input.chunks(chunk_size) {
                        mac.update(chunk);
                    }
                    let res = match $trunc {
                        "left" => mac.verify_truncated_left(tag),
                        "right" => mac.verify_truncated_right(tag),
                        _ => mac.verify_slice(tag),
                    };
                    if res.is_err() {
                        return Some("chunked message");
                    }
                }

                None
            }

            let data = include_bytes!(concat!("data/", $test_name, ".blb"));

            for (i, row) in Blob3Iterator::new(data).unwrap().enumerate() {
                let [key, input, tag] = row.unwrap();
                if let Some(desc) = run_test(key, input, tag) {
                    panic!(
                        "\n\
                         Failed test №{}: {}\n\
                         key:\t{:?}\n\
                         input:\t{:?}\n\
                         tag:\t{:?}\n",
                        i, desc, key, input, tag,
                    );
                }
            }
        }
    };
}

/// Define resettable MAC test
#[macro_export]
#[cfg(feature = "mac")]
#[cfg_attr(docsrs, doc(cfg(all(feature = "dev", feature = "mac"))))]
macro_rules! new_resettable_mac_test {
    ($name:ident, $test_name:expr, $mac:ty $(,)?) => {
        digest::new_resettable_mac_test!($name, $test_name, $mac, "");
    };
    ($name:ident, $test_name:expr, $mac:ty, trunc_left $(,)?) => {
        digest::new_resettable_mac_test!($name, $test_name, $mac, "left");
    };
    ($name:ident, $test_name:expr, $mac:ty, trunc_right $(,)?) => {
        digest::new_resettable_mac_test!($name, $test_name, $mac, "right");
    };
    ($name:ident, $test_name:expr, $mac:ty, $trunc:expr $(,)?) => {
        #[test]
        fn $name() {
            use core::cmp::min;
            use digest::dev::blobby::Blob3Iterator;
            use digest::Mac;

            fn run_test(key: &[u8], input: &[u8], tag: &[u8]) -> Option<&'static str> {
                let mac0 = <$mac as Mac>::new_from_slice(key).unwrap();

                let mut mac = mac0.clone();
                mac.update(input);
                let result = mac.finalize_reset().into_bytes();
                let n = tag.len();
                let result_bytes = match $trunc {
                    "left" => &result[..n],
                    "right" => &result[result.len() - n..],
                    _ => &result[..],
                };
                if result_bytes != tag {
                    return Some("whole message");
                }

                // test if reset worked correctly
                mac.update(input);
                let res = match $trunc {
                    "left" => mac.verify_truncated_left(tag),
                    "right" => mac.verify_truncated_right(tag),
                    _ => mac.verify_slice(tag),
                };
                if res.is_err() {
                    return Some("after reset");
                }

                // test reading different chunk sizes
                for chunk_size in 1..min(64, input.len()) {
                    let mut mac = mac0.clone();
                    for chunk in input.chunks(chunk_size) {
                        mac.update(chunk);
                    }
                    let res = match $trunc {
                        "left" => mac.verify_truncated_left(tag),
                        "right" => mac.verify_truncated_right(tag),
                        _ => mac.verify_slice(tag),
                    };
                    if res.is_err() {
                        return Some("chunked message");
                    }
                }
                None
            }

            let data = include_bytes!(concat!("data/", $test_name, ".blb"));

            for (i, row) in Blob3Iterator::new(data).unwrap().enumerate() {
                let [key, input, tag] = row.unwrap();
                if let Some(desc) = run_test(key, input, tag) {
                    panic!(
                        "\n\
                         Failed test №{}: {}\n\
                         key:\t{:?}\n\
                         input:\t{:?}\n\
                         tag:\t{:?}\n",
                        i, desc, key, input, tag,
                    );
                }
            }
        }
    };
}
