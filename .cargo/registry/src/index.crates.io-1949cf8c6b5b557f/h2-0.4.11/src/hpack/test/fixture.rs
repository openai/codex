use crate::hpack::{Decoder, Encoder, Header};

use bytes::BytesMut;
use hex::FromHex;
use serde_json::Value;

use std::fs::File;
use std::io::prelude::*;
use std::io::Cursor;
use std::path::Path;
use std::str;

fn test_fixture(path: &Path) {
    let mut file = File::open(path).unwrap();
    let mut data = String::new();
    file.read_to_string(&mut data).unwrap();

    let story: Value = serde_json::from_str(&data).unwrap();
    test_story(story);
}

fn test_story(story: Value) {
    let story = story.as_object().unwrap();

    if let Some(cases) = story.get("cases") {
        let mut cases: Vec<_> = cases
            .as_array()
            .unwrap()
            .iter()
            .map(|case| {
                let case = case.as_object().unwrap();

                let size = case
                    .get("header_table_size")
                    .map(|v| v.as_u64().unwrap() as usize);

                let wire = case.get("wire").unwrap().as_str().unwrap();
                let wire: Vec<u8> = FromHex::from_hex(wire.as_bytes()).unwrap();

                let expect: Vec<_> = case
                    .get("headers")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|h| {
                        let h = h.as_object().unwrap();
                        let (name, val) = h.iter().next().unwrap();
                        (name.clone(), val.as_str().unwrap().to_string())
                    })
                    .collect();

                Case {
                    seqno: case.get("seqno").unwrap().as_u64().unwrap(),
                    wire,
                    expect,
                    header_table_size: size,
                }
            })
            .collect();

        cases.sort_by_key(|c| c.seqno);

        let mut decoder = Decoder::default();

        // First, check decoding against the fixtures
        for case in &cases {
            let mut expect = case.expect.clone();

            if let Some(size) = case.header_table_size {
                decoder.queue_size_update(size);
            }

            let mut buf = BytesMut::with_capacity(case.wire.len());
            buf.extend_from_slice(&case.wire);
            decoder
                .decode(&mut Cursor::new(&mut buf), |e| {
                    let (name, value) = expect.remove(0);
                    assert_eq!(name, key_str(&e));
                    assert_eq!(value, value_str(&e));
                })
                .unwrap();

            assert_eq!(0, expect.len());
        }

        let mut encoder = Encoder::default();
        let mut decoder = Decoder::default();

        // Now, encode the headers
        for case in &cases {
            let limit = 64 * 1024;
            let mut buf = BytesMut::with_capacity(limit);

            if let Some(size) = case.header_table_size {
                encoder.update_max_size(size);
                decoder.queue_size_update(size);
            }

            let mut input: Vec<_> = case
                .expect
                .iter()
                .map(|(name, value)| {
                    Header::new(name.clone().into(), value.clone().into())
                        .unwrap()
                        .into()
                })
                .collect();

            encoder.encode(input.clone(), &mut buf);

            decoder
                .decode(&mut Cursor::new(&mut buf), |e| {
                    assert_eq!(e, input.remove(0).reify().unwrap());
                })
                .unwrap();

            assert_eq!(0, input.len());
        }
    }
}

struct Case {
    seqno: u64,
    wire: Vec<u8>,
    expect: Vec<(String, String)>,
    header_table_size: Option<usize>,
}

fn key_str(e: &Header) -> &str {
    match *e {
        Header::Field { ref name, .. } => name.as_str(),
        Header::Authority(..) => ":authority",
        Header::Method(..) => ":method",
        Header::Scheme(..) => ":scheme",
        Header::Path(..) => ":path",
        Header::Protocol(..) => ":protocol",
        Header::Status(..) => ":status",
    }
}

fn value_str(e: &Header) -> &str {
    match *e {
        Header::Field { ref value, .. } => value.to_str().unwrap(),
        Header::Authority(ref v) => v,
        Header::Method(ref m) => m.as_str(),
        Header::Scheme(ref v) => v,
        Header::Path(ref v) => v,
        Header::Protocol(ref v) => v.as_str(),
        Header::Status(ref v) => v.as_str(),
    }
}

macro_rules! fixture_mod {
    ($module:ident => {
        $(
            ($fn:ident, $path:expr);
        )+
    }) => {
        mod $module {
            $(
                #[test]
                fn $fn() {
                    let path = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("fixtures/hpack")
                        .join($path);

                    super::test_fixture(path.as_ref());
                }
            )+
        }
    }
}

fixture_mod!(
    haskell_http2_linear_huffman => {
        (story_00, "haskell-http2-linear-huffman/story_00.json");
        (story_01, "haskell-http2-linear-huffman/story_01.json");
        (story_02, "haskell-http2-linear-huffman/story_02.json");
        (story_03, "haskell-http2-linear-huffman/story_03.json");
        (story_04, "haskell-http2-linear-huffman/story_04.json");
        (story_05, "haskell-http2-linear-huffman/story_05.json");
        (story_06, "haskell-http2-linear-huffman/story_06.json");
        (story_07, "haskell-http2-linear-huffman/story_07.json");
        (story_08, "haskell-http2-linear-huffman/story_08.json");
        (story_09, "haskell-http2-linear-huffman/story_09.json");
        (story_10, "haskell-http2-linear-huffman/story_10.json");
        (story_11, "haskell-http2-linear-huffman/story_11.json");
        (story_12, "haskell-http2-linear-huffman/story_12.json");
        (story_13, "haskell-http2-linear-huffman/story_13.json");
        (story_14, "haskell-http2-linear-huffman/story_14.json");
        (story_15, "haskell-http2-linear-huffman/story_15.json");
        (story_16, "haskell-http2-linear-huffman/story_16.json");
        (story_17, "haskell-http2-linear-huffman/story_17.json");
        (story_18, "haskell-http2-linear-huffman/story_18.json");
        (story_19, "haskell-http2-linear-huffman/story_19.json");
        (story_20, "haskell-http2-linear-huffman/story_20.json");
        (story_21, "haskell-http2-linear-huffman/story_21.json");
        (story_22, "haskell-http2-linear-huffman/story_22.json");
        (story_23, "haskell-http2-linear-huffman/story_23.json");
        (story_24, "haskell-http2-linear-huffman/story_24.json");
        (story_25, "haskell-http2-linear-huffman/story_25.json");
        (story_26, "haskell-http2-linear-huffman/story_26.json");
        (story_27, "haskell-http2-linear-huffman/story_27.json");
        (story_28, "haskell-http2-linear-huffman/story_28.json");
        (story_29, "haskell-http2-linear-huffman/story_29.json");
        (story_30, "haskell-http2-linear-huffman/story_30.json");
        (story_31, "haskell-http2-linear-huffman/story_31.json");
    }
);

fixture_mod!(
    python_hpack => {
        (story_00, "python-hpack/story_00.json");
        (story_01, "python-hpack/story_01.json");
        (story_02, "python-hpack/story_02.json");
        (story_03, "python-hpack/story_03.json");
        (story_04, "python-hpack/story_04.json");
        (story_05, "python-hpack/story_05.json");
        (story_06, "python-hpack/story_06.json");
        (story_07, "python-hpack/story_07.json");
        (story_08, "python-hpack/story_08.json");
        (story_09, "python-hpack/story_09.json");
        (story_10, "python-hpack/story_10.json");
        (story_11, "python-hpack/story_11.json");
        (story_12, "python-hpack/story_12.json");
        (story_13, "python-hpack/story_13.json");
        (story_14, "python-hpack/story_14.json");
        (story_15, "python-hpack/story_15.json");
        (story_16, "python-hpack/story_16.json");
        (story_17, "python-hpack/story_17.json");
        (story_18, "python-hpack/story_18.json");
        (story_19, "python-hpack/story_19.json");
        (story_20, "python-hpack/story_20.json");
        (story_21, "python-hpack/story_21.json");
        (story_22, "python-hpack/story_22.json");
        (story_23, "python-hpack/story_23.json");
        (story_24, "python-hpack/story_24.json");
        (story_25, "python-hpack/story_25.json");
        (story_26, "python-hpack/story_26.json");
        (story_27, "python-hpack/story_27.json");
        (story_28, "python-hpack/story_28.json");
        (story_29, "python-hpack/story_29.json");
        (story_30, "python-hpack/story_30.json");
        (story_31, "python-hpack/story_31.json");
    }
);

fixture_mod!(
    nghttp2_16384_4096 => {
        (story_00, "nghttp2-16384-4096/story_00.json");
        (story_01, "nghttp2-16384-4096/story_01.json");
        (story_02, "nghttp2-16384-4096/story_02.json");
        (story_03, "nghttp2-16384-4096/story_03.json");
        (story_04, "nghttp2-16384-4096/story_04.json");
        (story_05, "nghttp2-16384-4096/story_05.json");
        (story_06, "nghttp2-16384-4096/story_06.json");
        (story_07, "nghttp2-16384-4096/story_07.json");
        (story_08, "nghttp2-16384-4096/story_08.json");
        (story_09, "nghttp2-16384-4096/story_09.json");
        (story_10, "nghttp2-16384-4096/story_10.json");
        (story_11, "nghttp2-16384-4096/story_11.json");
        (story_12, "nghttp2-16384-4096/story_12.json");
        (story_13, "nghttp2-16384-4096/story_13.json");
        (story_14, "nghttp2-16384-4096/story_14.json");
        (story_15, "nghttp2-16384-4096/story_15.json");
        (story_16, "nghttp2-16384-4096/story_16.json");
        (story_17, "nghttp2-16384-4096/story_17.json");
        (story_18, "nghttp2-16384-4096/story_18.json");
        (story_19, "nghttp2-16384-4096/story_19.json");
        (story_20, "nghttp2-16384-4096/story_20.json");
        (story_21, "nghttp2-16384-4096/story_21.json");
        (story_22, "nghttp2-16384-4096/story_22.json");
        (story_23, "nghttp2-16384-4096/story_23.json");
        (story_24, "nghttp2-16384-4096/story_24.json");
        (story_25, "nghttp2-16384-4096/story_25.json");
        (story_26, "nghttp2-16384-4096/story_26.json");
        (story_27, "nghttp2-16384-4096/story_27.json");
        (story_28, "nghttp2-16384-4096/story_28.json");
        (story_29, "nghttp2-16384-4096/story_29.json");
        (story_30, "nghttp2-16384-4096/story_30.json");
    }
);

fixture_mod!(
    node_http2_hpack => {
        (story_00, "node-http2-hpack/story_00.json");
        (story_01, "node-http2-hpack/story_01.json");
        (story_02, "node-http2-hpack/story_02.json");
        (story_03, "node-http2-hpack/story_03.json");
        (story_04, "node-http2-hpack/story_04.json");
        (story_05, "node-http2-hpack/story_05.json");
        (story_06, "node-http2-hpack/story_06.json");
        (story_07, "node-http2-hpack/story_07.json");
        (story_08, "node-http2-hpack/story_08.json");
        (story_09, "node-http2-hpack/story_09.json");
        (story_10, "node-http2-hpack/story_10.json");
        (story_11, "node-http2-hpack/story_11.json");
        (story_12, "node-http2-hpack/story_12.json");
        (story_13, "node-http2-hpack/story_13.json");
        (story_14, "node-http2-hpack/story_14.json");
        (story_15, "node-http2-hpack/story_15.json");
        (story_16, "node-http2-hpack/story_16.json");
        (story_17, "node-http2-hpack/story_17.json");
        (story_18, "node-http2-hpack/story_18.json");
        (story_19, "node-http2-hpack/story_19.json");
        (story_20, "node-http2-hpack/story_20.json");
        (story_21, "node-http2-hpack/story_21.json");
        (story_22, "node-http2-hpack/story_22.json");
        (story_23, "node-http2-hpack/story_23.json");
        (story_24, "node-http2-hpack/story_24.json");
        (story_25, "node-http2-hpack/story_25.json");
        (story_26, "node-http2-hpack/story_26.json");
        (story_27, "node-http2-hpack/story_27.json");
        (story_28, "node-http2-hpack/story_28.json");
        (story_29, "node-http2-hpack/story_29.json");
        (story_30, "node-http2-hpack/story_30.json");
        (story_31, "node-http2-hpack/story_31.json");
    }
);

fixture_mod!(
    nghttp2_change_table_size => {
        (story_00, "nghttp2-change-table-size/story_00.json");
        (story_01, "nghttp2-change-table-size/story_01.json");
        (story_02, "nghttp2-change-table-size/story_02.json");
        (story_03, "nghttp2-change-table-size/story_03.json");
        (story_04, "nghttp2-change-table-size/story_04.json");
        (story_05, "nghttp2-change-table-size/story_05.json");
        (story_06, "nghttp2-change-table-size/story_06.json");
        (story_07, "nghttp2-change-table-size/story_07.json");
        (story_08, "nghttp2-change-table-size/story_08.json");
        (story_09, "nghttp2-change-table-size/story_09.json");
        (story_10, "nghttp2-change-table-size/story_10.json");
        (story_11, "nghttp2-change-table-size/story_11.json");
        (story_12, "nghttp2-change-table-size/story_12.json");
        (story_13, "nghttp2-change-table-size/story_13.json");
        (story_14, "nghttp2-change-table-size/story_14.json");
        (story_15, "nghttp2-change-table-size/story_15.json");
        (story_16, "nghttp2-change-table-size/story_16.json");
        (story_17, "nghttp2-change-table-size/story_17.json");
        (story_18, "nghttp2-change-table-size/story_18.json");
        (story_19, "nghttp2-change-table-size/story_19.json");
        (story_20, "nghttp2-change-table-size/story_20.json");
        (story_21, "nghttp2-change-table-size/story_21.json");
        (story_22, "nghttp2-change-table-size/story_22.json");
        (story_23, "nghttp2-change-table-size/story_23.json");
        (story_24, "nghttp2-change-table-size/story_24.json");
        (story_25, "nghttp2-change-table-size/story_25.json");
        (story_26, "nghttp2-change-table-size/story_26.json");
        (story_27, "nghttp2-change-table-size/story_27.json");
        (story_28, "nghttp2-change-table-size/story_28.json");
        (story_29, "nghttp2-change-table-size/story_29.json");
        (story_30, "nghttp2-change-table-size/story_30.json");
    }
);

fixture_mod!(
    haskell_http2_static_huffman => {
        (story_00, "haskell-http2-static-huffman/story_00.json");
        (story_01, "haskell-http2-static-huffman/story_01.json");
        (story_02, "haskell-http2-static-huffman/story_02.json");
        (story_03, "haskell-http2-static-huffman/story_03.json");
        (story_04, "haskell-http2-static-huffman/story_04.json");
        (story_05, "haskell-http2-static-huffman/story_05.json");
        (story_06, "haskell-http2-static-huffman/story_06.json");
        (story_07, "haskell-http2-static-huffman/story_07.json");
        (story_08, "haskell-http2-static-huffman/story_08.json");
        (story_09, "haskell-http2-static-huffman/story_09.json");
        (story_10, "haskell-http2-static-huffman/story_10.json");
        (story_11, "haskell-http2-static-huffman/story_11.json");
        (story_12, "haskell-http2-static-huffman/story_12.json");
        (story_13, "haskell-http2-static-huffman/story_13.json");
        (story_14, "haskell-http2-static-huffman/story_14.json");
        (story_15, "haskell-http2-static-huffman/story_15.json");
        (story_16, "haskell-http2-static-huffman/story_16.json");
        (story_17, "haskell-http2-static-huffman/story_17.json");
        (story_18, "haskell-http2-static-huffman/story_18.json");
        (story_19, "haskell-http2-static-huffman/story_19.json");
        (story_20, "haskell-http2-static-huffman/story_20.json");
        (story_21, "haskell-http2-static-huffman/story_21.json");
        (story_22, "haskell-http2-static-huffman/story_22.json");
        (story_23, "haskell-http2-static-huffman/story_23.json");
        (story_24, "haskell-http2-static-huffman/story_24.json");
        (story_25, "haskell-http2-static-huffman/story_25.json");
        (story_26, "haskell-http2-static-huffman/story_26.json");
        (story_27, "haskell-http2-static-huffman/story_27.json");
        (story_28, "haskell-http2-static-huffman/story_28.json");
        (story_29, "haskell-http2-static-huffman/story_29.json");
        (story_30, "haskell-http2-static-huffman/story_30.json");
        (story_31, "haskell-http2-static-huffman/story_31.json");
    }
);

fixture_mod!(
    haskell_http2_naive_huffman => {
        (story_00, "haskell-http2-naive-huffman/story_00.json");
        (story_01, "haskell-http2-naive-huffman/story_01.json");
        (story_02, "haskell-http2-naive-huffman/story_02.json");
        (story_03, "haskell-http2-naive-huffman/story_03.json");
        (story_04, "haskell-http2-naive-huffman/story_04.json");
        (story_05, "haskell-http2-naive-huffman/story_05.json");
        (story_06, "haskell-http2-naive-huffman/story_06.json");
        (story_07, "haskell-http2-naive-huffman/story_07.json");
        (story_08, "haskell-http2-naive-huffman/story_08.json");
        (story_09, "haskell-http2-naive-huffman/story_09.json");
        (story_10, "haskell-http2-naive-huffman/story_10.json");
        (story_11, "haskell-http2-naive-huffman/story_11.json");
        (story_12, "haskell-http2-naive-huffman/story_12.json");
        (story_13, "haskell-http2-naive-huffman/story_13.json");
        (story_14, "haskell-http2-naive-huffman/story_14.json");
        (story_15, "haskell-http2-naive-huffman/story_15.json");
        (story_16, "haskell-http2-naive-huffman/story_16.json");
        (story_17, "haskell-http2-naive-huffman/story_17.json");
        (story_18, "haskell-http2-naive-huffman/story_18.json");
        (story_19, "haskell-http2-naive-huffman/story_19.json");
        (story_20, "haskell-http2-naive-huffman/story_20.json");
        (story_21, "haskell-http2-naive-huffman/story_21.json");
        (story_22, "haskell-http2-naive-huffman/story_22.json");
        (story_23, "haskell-http2-naive-huffman/story_23.json");
        (story_24, "haskell-http2-naive-huffman/story_24.json");
        (story_25, "haskell-http2-naive-huffman/story_25.json");
        (story_26, "haskell-http2-naive-huffman/story_26.json");
        (story_27, "haskell-http2-naive-huffman/story_27.json");
        (story_28, "haskell-http2-naive-huffman/story_28.json");
        (story_29, "haskell-http2-naive-huffman/story_29.json");
        (story_30, "haskell-http2-naive-huffman/story_30.json");
        (story_31, "haskell-http2-naive-huffman/story_31.json");
    }
);

fixture_mod!(
    haskell_http2_naive => {
        (story_00, "haskell-http2-naive/story_00.json");
        (story_01, "haskell-http2-naive/story_01.json");
        (story_02, "haskell-http2-naive/story_02.json");
        (story_03, "haskell-http2-naive/story_03.json");
        (story_04, "haskell-http2-naive/story_04.json");
        (story_05, "haskell-http2-naive/story_05.json");
        (story_06, "haskell-http2-naive/story_06.json");
        (story_07, "haskell-http2-naive/story_07.json");
        (story_08, "haskell-http2-naive/story_08.json");
        (story_09, "haskell-http2-naive/story_09.json");
        (story_10, "haskell-http2-naive/story_10.json");
        (story_11, "haskell-http2-naive/story_11.json");
        (story_12, "haskell-http2-naive/story_12.json");
        (story_13, "haskell-http2-naive/story_13.json");
        (story_14, "haskell-http2-naive/story_14.json");
        (story_15, "haskell-http2-naive/story_15.json");
        (story_16, "haskell-http2-naive/story_16.json");
        (story_17, "haskell-http2-naive/story_17.json");
        (story_18, "haskell-http2-naive/story_18.json");
        (story_19, "haskell-http2-naive/story_19.json");
        (story_20, "haskell-http2-naive/story_20.json");
        (story_21, "haskell-http2-naive/story_21.json");
        (story_22, "haskell-http2-naive/story_22.json");
        (story_23, "haskell-http2-naive/story_23.json");
        (story_24, "haskell-http2-naive/story_24.json");
        (story_25, "haskell-http2-naive/story_25.json");
        (story_26, "haskell-http2-naive/story_26.json");
        (story_27, "haskell-http2-naive/story_27.json");
        (story_28, "haskell-http2-naive/story_28.json");
        (story_29, "haskell-http2-naive/story_29.json");
        (story_30, "haskell-http2-naive/story_30.json");
        (story_31, "haskell-http2-naive/story_31.json");
    }
);

fixture_mod!(
    haskell_http2_static => {
        (story_00, "haskell-http2-static/story_00.json");
        (story_01, "haskell-http2-static/story_01.json");
        (story_02, "haskell-http2-static/story_02.json");
        (story_03, "haskell-http2-static/story_03.json");
        (story_04, "haskell-http2-static/story_04.json");
        (story_05, "haskell-http2-static/story_05.json");
        (story_06, "haskell-http2-static/story_06.json");
        (story_07, "haskell-http2-static/story_07.json");
        (story_08, "haskell-http2-static/story_08.json");
        (story_09, "haskell-http2-static/story_09.json");
        (story_10, "haskell-http2-static/story_10.json");
        (story_11, "haskell-http2-static/story_11.json");
        (story_12, "haskell-http2-static/story_12.json");
        (story_13, "haskell-http2-static/story_13.json");
        (story_14, "haskell-http2-static/story_14.json");
        (story_15, "haskell-http2-static/story_15.json");
        (story_16, "haskell-http2-static/story_16.json");
        (story_17, "haskell-http2-static/story_17.json");
        (story_18, "haskell-http2-static/story_18.json");
        (story_19, "haskell-http2-static/story_19.json");
        (story_20, "haskell-http2-static/story_20.json");
        (story_21, "haskell-http2-static/story_21.json");
        (story_22, "haskell-http2-static/story_22.json");
        (story_23, "haskell-http2-static/story_23.json");
        (story_24, "haskell-http2-static/story_24.json");
        (story_25, "haskell-http2-static/story_25.json");
        (story_26, "haskell-http2-static/story_26.json");
        (story_27, "haskell-http2-static/story_27.json");
        (story_28, "haskell-http2-static/story_28.json");
        (story_29, "haskell-http2-static/story_29.json");
        (story_30, "haskell-http2-static/story_30.json");
        (story_31, "haskell-http2-static/story_31.json");
    }
);

fixture_mod!(
    nghttp2 => {
        (story_00, "nghttp2/story_00.json");
        (story_01, "nghttp2/story_01.json");
        (story_02, "nghttp2/story_02.json");
        (story_03, "nghttp2/story_03.json");
        (story_04, "nghttp2/story_04.json");
        (story_05, "nghttp2/story_05.json");
        (story_06, "nghttp2/story_06.json");
        (story_07, "nghttp2/story_07.json");
        (story_08, "nghttp2/story_08.json");
        (story_09, "nghttp2/story_09.json");
        (story_10, "nghttp2/story_10.json");
        (story_11, "nghttp2/story_11.json");
        (story_12, "nghttp2/story_12.json");
        (story_13, "nghttp2/story_13.json");
        (story_14, "nghttp2/story_14.json");
        (story_15, "nghttp2/story_15.json");
        (story_16, "nghttp2/story_16.json");
        (story_17, "nghttp2/story_17.json");
        (story_18, "nghttp2/story_18.json");
        (story_19, "nghttp2/story_19.json");
        (story_20, "nghttp2/story_20.json");
        (story_21, "nghttp2/story_21.json");
        (story_22, "nghttp2/story_22.json");
        (story_23, "nghttp2/story_23.json");
        (story_24, "nghttp2/story_24.json");
        (story_25, "nghttp2/story_25.json");
        (story_26, "nghttp2/story_26.json");
        (story_27, "nghttp2/story_27.json");
        (story_28, "nghttp2/story_28.json");
        (story_29, "nghttp2/story_29.json");
        (story_30, "nghttp2/story_30.json");
        (story_31, "nghttp2/story_31.json");
    }
);

fixture_mod!(
    haskell_http2_linear => {
        (story_00, "haskell-http2-linear/story_00.json");
        (story_01, "haskell-http2-linear/story_01.json");
        (story_02, "haskell-http2-linear/story_02.json");
        (story_03, "haskell-http2-linear/story_03.json");
        (story_04, "haskell-http2-linear/story_04.json");
        (story_05, "haskell-http2-linear/story_05.json");
        (story_06, "haskell-http2-linear/story_06.json");
        (story_07, "haskell-http2-linear/story_07.json");
        (story_08, "haskell-http2-linear/story_08.json");
        (story_09, "haskell-http2-linear/story_09.json");
        (story_10, "haskell-http2-linear/story_10.json");
        (story_11, "haskell-http2-linear/story_11.json");
        (story_12, "haskell-http2-linear/story_12.json");
        (story_13, "haskell-http2-linear/story_13.json");
        (story_14, "haskell-http2-linear/story_14.json");
        (story_15, "haskell-http2-linear/story_15.json");
        (story_16, "haskell-http2-linear/story_16.json");
        (story_17, "haskell-http2-linear/story_17.json");
        (story_18, "haskell-http2-linear/story_18.json");
        (story_19, "haskell-http2-linear/story_19.json");
        (story_20, "haskell-http2-linear/story_20.json");
        (story_21, "haskell-http2-linear/story_21.json");
        (story_22, "haskell-http2-linear/story_22.json");
        (story_23, "haskell-http2-linear/story_23.json");
        (story_24, "haskell-http2-linear/story_24.json");
        (story_25, "haskell-http2-linear/story_25.json");
        (story_26, "haskell-http2-linear/story_26.json");
        (story_27, "haskell-http2-linear/story_27.json");
        (story_28, "haskell-http2-linear/story_28.json");
        (story_29, "haskell-http2-linear/story_29.json");
        (story_30, "haskell-http2-linear/story_30.json");
        (story_31, "haskell-http2-linear/story_31.json");
    }
);

fixture_mod!(
    go_hpack => {
        (story_00, "go-hpack/story_00.json");
        (story_01, "go-hpack/story_01.json");
        (story_02, "go-hpack/story_02.json");
        (story_03, "go-hpack/story_03.json");
        (story_04, "go-hpack/story_04.json");
        (story_05, "go-hpack/story_05.json");
        (story_06, "go-hpack/story_06.json");
        (story_07, "go-hpack/story_07.json");
        (story_08, "go-hpack/story_08.json");
        (story_09, "go-hpack/story_09.json");
        (story_10, "go-hpack/story_10.json");
        (story_11, "go-hpack/story_11.json");
        (story_12, "go-hpack/story_12.json");
        (story_13, "go-hpack/story_13.json");
        (story_14, "go-hpack/story_14.json");
        (story_15, "go-hpack/story_15.json");
        (story_16, "go-hpack/story_16.json");
        (story_17, "go-hpack/story_17.json");
        (story_18, "go-hpack/story_18.json");
        (story_19, "go-hpack/story_19.json");
        (story_20, "go-hpack/story_20.json");
        (story_21, "go-hpack/story_21.json");
        (story_22, "go-hpack/story_22.json");
        (story_23, "go-hpack/story_23.json");
        (story_24, "go-hpack/story_24.json");
        (story_25, "go-hpack/story_25.json");
        (story_26, "go-hpack/story_26.json");
        (story_27, "go-hpack/story_27.json");
        (story_28, "go-hpack/story_28.json");
        (story_29, "go-hpack/story_29.json");
        (story_30, "go-hpack/story_30.json");
        (story_31, "go-hpack/story_31.json");
    }
);
