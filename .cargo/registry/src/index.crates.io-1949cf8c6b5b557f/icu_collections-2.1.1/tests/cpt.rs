// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use icu_collections::codepointtrie::planes::get_planes_trie;
use icu_collections::codepointtrie::*;
use zerovec::ZeroVec;

#[test]
fn planes_trie_deserialize_check_test() {
    // Get expected planes trie from crate::planes::get_planes_trie()

    let exp_planes_trie = get_planes_trie();

    // Compute actual planes trie from planes.toml

    let planes_enum_prop =
        ::toml::from_str::<UnicodeEnumeratedProperty>(include_str!("data/cpt/planes.toml"))
            .unwrap();

    let code_point_trie_struct = planes_enum_prop.code_point_trie.trie_struct;

    let trie_header = CodePointTrieHeader {
        high_start: code_point_trie_struct.high_start,
        shifted12_high_start: code_point_trie_struct.shifted12_high_start,
        index3_null_offset: code_point_trie_struct.index3_null_offset,
        data_null_offset: code_point_trie_struct.data_null_offset,
        null_value: code_point_trie_struct.null_value,
        trie_type: TrieType::try_from(code_point_trie_struct.trie_type_enum_val).unwrap_or_else(
            |_| {
                panic!(
                    "Could not parse trie_type serialized enum value in test data file: {}",
                    code_point_trie_struct.name
                )
            },
        ),
    };

    let data = ZeroVec::from_slice_or_alloc(code_point_trie_struct.data_8.as_ref().unwrap());
    let index = ZeroVec::from_slice_or_alloc(&code_point_trie_struct.index);
    let trie_result = CodePointTrie::try_new(trie_header, index, data);
    let act_planes_trie = trie_result.unwrap();

    // Get check ranges (inversion map-style sequence of range+value) and
    // apply the trie validation test fn on expected and actual tries

    let serialized_ranges: Vec<(u32, u32, u32)> = planes_enum_prop.code_point_map.data.ranges;
    let mut check_ranges: Vec<u32> = vec![];
    for range_tuple in serialized_ranges {
        let range_end = range_tuple.1 + 1;
        let value = range_tuple.2;
        check_ranges.push(range_end);
        check_ranges.push(value);
    }

    check_trie(&act_planes_trie, &check_ranges);
    check_trie(&exp_planes_trie, &check_ranges);
}

#[test]
fn free_blocks_16() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/free-blocks.16.toml"));
}

#[test]
fn free_blocks_32() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/free-blocks.32.toml"));
}

#[test]
fn free_blocks_8() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/free-blocks.8.toml"));
}

#[test]
fn free_blocks_small16() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/free-blocks.small16.toml"));
}

#[test]
fn grow_data_16() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/grow-data.16.toml"));
}

#[test]
fn grow_data_32() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/grow-data.32.toml"));
}

#[test]
fn grow_data_8() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/grow-data.8.toml"));
}

#[test]
fn grow_data_small16() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/grow-data.small16.toml"));
}

#[test]
fn set1_16() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/set1.16.toml"));
}

#[test]
fn set1_32() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/set1.32.toml"));
}

#[test]
fn set1_8() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/set1.8.toml"));
}

#[test]
fn set1_small16() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/set1.small16.toml"));
}

#[test]
fn set2_overlap_16() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/set2-overlap.16.toml"));
}

#[test]
fn set2_overlap_32() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/set2-overlap.32.toml"));
}

#[test]
fn set2_overlap_small16() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/set2-overlap.small16.toml"));
}

#[test]
fn set3_initial_9_16() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/set3-initial-9.16.toml"));
}

#[test]
fn set3_initial_9_32() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/set3-initial-9.32.toml"));
}

#[test]
fn set3_initial_9_8() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/set3-initial-9.8.toml"));
}

#[test]
fn set3_initial_9_small16() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/set3-initial-9.small16.toml"));
}

#[test]
fn set_empty_16() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/set-empty.16.toml"));
}

#[test]
fn set_empty_32() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/set-empty.32.toml"));
}

#[test]
fn set_empty_8() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/set-empty.8.toml"));
}

#[test]
fn set_empty_small16() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/set-empty.small16.toml"));
}

#[test]
fn set_single_value_16() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/set-single-value.16.toml"));
}

#[test]
fn set_single_value_32() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/set-single-value.32.toml"));
}

#[test]
fn set_single_value_8() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/set-single-value.8.toml"));
}

#[test]
fn set_single_value_small16() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/set-single-value.small16.toml"));
}

#[test]
fn short_all_same_16() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/short-all-same.16.toml"));
}

#[test]
fn short_all_same_8() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/short-all-same.8.toml"));
}

#[test]
fn short_all_same_small16() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/short-all-same.small16.toml"));
}

#[test]
fn small0_in_fast_16() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/small0-in-fast.16.toml"));
}

#[test]
fn small0_in_fast_32() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/small0-in-fast.32.toml"));
}

#[test]
fn small0_in_fast_8() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/small0-in-fast.8.toml"));
}

#[test]
fn small0_in_fast_small16() {
    run_deserialize_test_from_test_data(include_str!("data/cpt/small0-in-fast.small16.toml"));
}

/// The width of the elements in the data array of a [`CodePointTrie`].
/// See [`UCPTrieValueWidth`](https://unicode-org.github.io/icu-docs/apidoc/dev/icu4c/ucptrie_8h.html) in ICU4C.
#[derive(Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ValueWidthEnum {
    Bits16 = 0,
    Bits32 = 1,
    Bits8 = 2,
}

/// Test .get() on CodePointTrie by iterating through each range in
/// check_ranges and assert that the associated
/// value matches the trie value for each code point in the range.
pub fn check_trie<T: TrieValue + Into<u32>>(trie: &CodePointTrie<T>, check_ranges: &[u32]) {
    assert_eq!(
        0,
        check_ranges.len() % 2,
        "check_ranges must have an even number of 32-bit values in (limit,value) pairs"
    );

    let mut i: u32 = 0;
    let check_range_tuples = check_ranges.chunks(2);
    // Iterate over each check range
    for range_tuple in check_range_tuples {
        let range_limit = range_tuple[0];
        let range_value = range_tuple[1];
        // Check all values in this range, one-by-one
        while i < range_limit {
            assert_eq!(range_value, trie.get32(i).into(), "trie_get({i})",);
            i += 1;
        }
    }
}

/// Test `.get_range()` / `.iter_ranges()` on CodePointTrie by calling
/// `.iter_ranges()` on the trie.
///
/// `.iter_ranges()` returns an iterator that produces values
/// by calls to .get_range, and this checks if it matches the values in check_ranges.
pub fn test_check_ranges_get_ranges<T: TrieValue + Into<u32>>(
    trie: &CodePointTrie<T>,
    check_ranges: &[u32],
) {
    assert_eq!(
        0,
        check_ranges.len() % 2,
        "check_ranges must have an even number of 32-bit values in (limit,value) pairs"
    );

    let mut trie_ranges = trie.iter_ranges();

    let mut range_start: u32 = 0;
    let check_range_tuples = check_ranges.chunks(2);
    // Iterate over each check range
    for range_tuple in check_range_tuples {
        let range_limit = range_tuple[0];
        let range_value = range_tuple[1];

        // The check ranges array seems to start with a trivial range whose
        // limit is zero. range_start is initialized to 0, so we can skip.
        if range_limit == 0 {
            continue;
        }

        let cpm_range = trie_ranges.next();
        assert!(cpm_range.is_some(), "CodePointTrie iter_ranges() produces fewer ranges than the check_ranges field in testdata has");
        let cpm_range = cpm_range.unwrap();
        let cpmr_start = cpm_range.range.start();
        let cpmr_end = cpm_range.range.end();
        let cpmr_value: u32 = cpm_range.value.into();

        assert_eq!(range_start, *cpmr_start);
        assert_eq!(range_limit, *cpmr_end + 1);
        assert_eq!(range_value, cpmr_value);

        range_start = range_limit;
    }

    assert!(trie_ranges.next().is_none(), "CodePointTrie iter_ranges() produces more ranges than the check_ranges field in testdata has");
}

/// Run above tests that verify the validity of CodePointTrie methods
pub fn run_trie_tests<T: TrieValue + Into<u32>>(trie: &CodePointTrie<T>, check_ranges: &[u32]) {
    check_trie(trie, check_ranges);
    test_check_ranges_get_ranges(trie, check_ranges);
}

// The following structs might be useful later for de-/serialization of the
// main `CodePointTrie` struct in the corresponding data provider.

#[cfg_attr(any(feature = "serde", test), derive(serde::Deserialize))]
pub struct UnicodeEnumeratedProperty {
    pub code_point_map: EnumPropCodePointMap,
    pub code_point_trie: EnumPropSerializedCPT,
}

#[cfg_attr(any(feature = "serde", test), derive(serde::Deserialize))]
pub struct EnumPropCodePointMap {
    pub data: EnumPropCodePointMapData,
}

#[cfg_attr(any(feature = "serde", test), derive(serde::Deserialize))]
pub struct EnumPropCodePointMapData {
    pub long_name: String,
    pub name: String,
    pub ranges: Vec<(u32, u32, u32)>,
}

#[cfg_attr(any(feature = "serde", test), derive(serde::Deserialize))]
pub struct EnumPropSerializedCPT {
    #[cfg_attr(any(feature = "serde", test), serde(rename = "struct"))]
    pub trie_struct: EnumPropSerializedCPTStruct,
}

// These structs support the test data dumped as TOML files from ICU.
// Because the properties CodePointMap data will also be dumped from ICU
// using similar functions, some of these structs may be useful to refactor
// into main code at a later point.

#[cfg_attr(any(feature = "serde", test), derive(serde::Deserialize))]
pub struct EnumPropSerializedCPTStruct {
    #[cfg_attr(any(feature = "serde", test), serde(skip))]
    pub long_name: String,
    pub name: String,
    pub index: Vec<u16>,
    pub data_8: Option<Vec<u8>>,
    pub data_16: Option<Vec<u16>>,
    pub data_32: Option<Vec<u32>>,
    #[cfg_attr(any(feature = "serde", test), serde(skip))]
    pub index_length: u32,
    #[cfg_attr(any(feature = "serde", test), serde(skip))]
    pub data_length: u32,
    #[cfg_attr(any(feature = "serde", test), serde(rename = "highStart"))]
    pub high_start: u32,
    #[cfg_attr(any(feature = "serde", test), serde(rename = "shifted12HighStart"))]
    pub shifted12_high_start: u16,
    #[cfg_attr(any(feature = "serde", test), serde(rename = "type"))]
    pub trie_type_enum_val: u8,
    #[cfg_attr(any(feature = "serde", test), serde(rename = "valueWidth"))]
    pub value_width_enum_val: u8,
    #[cfg_attr(any(feature = "serde", test), serde(rename = "index3NullOffset"))]
    pub index3_null_offset: u16,
    #[cfg_attr(any(feature = "serde", test), serde(rename = "dataNullOffset"))]
    pub data_null_offset: u32,
    #[cfg_attr(any(feature = "serde", test), serde(rename = "nullValue"))]
    pub null_value: u32,
}

// Given a .toml file dumped from ICU4C test data for UCPTrie, run the test
// data file deserialization into the test file struct, convert and construct
// the `CodePointTrie`, and test the constructed struct against the test file's
// "check ranges" (inversion map ranges) using `check_trie` to verify the
// validity of the `CodePointTrie`'s behavior for all code points.
#[allow(dead_code)]
pub fn run_deserialize_test_from_test_data(test_file: &str) {
    // The following structs are specific to the TOML format files for dumped ICU
    // test data.

    #[derive(serde::Deserialize)]
    pub struct TestFile {
        code_point_trie: TestCodePointTrie,
    }

    #[derive(serde::Deserialize)]
    pub struct TestCodePointTrie {
        // The trie_struct field for test data files is dumped from the same source
        // (ICU4C) using the same function (usrc_writeUCPTrie) as property data
        // for the provider, so we can reuse the same struct here.
        #[serde(rename(deserialize = "struct"))]
        trie_struct: EnumPropSerializedCPTStruct,
        #[serde(rename(deserialize = "testdata"))]
        test_data: TestData,
    }

    #[derive(serde::Deserialize)]
    pub struct TestData {
        #[serde(rename(deserialize = "checkRanges"))]
        check_ranges: Vec<u32>,
    }

    let test_file = ::toml::from_str::<TestFile>(test_file).unwrap();

    let test_struct = test_file.code_point_trie.trie_struct;

    println!(
        "Running CodePointTrie reader logic test on test data file: {}",
        test_struct.name
    );

    let trie_type_enum = match TrieType::try_from(test_struct.trie_type_enum_val) {
        Ok(enum_val) => enum_val,
        _ => {
            panic!(
                "Could not parse trie_type serialized enum value in test data file: {}",
                test_struct.name
            );
        }
    };

    let trie_header = CodePointTrieHeader {
        high_start: test_struct.high_start,
        shifted12_high_start: test_struct.shifted12_high_start,
        index3_null_offset: test_struct.index3_null_offset,
        data_null_offset: test_struct.data_null_offset,
        null_value: test_struct.null_value,
        trie_type: trie_type_enum,
    };

    let index = ZeroVec::from_slice_or_alloc(&test_struct.index);

    match (test_struct.data_8, test_struct.data_16, test_struct.data_32) {
        (Some(data_8), _, _) => {
            let data = ZeroVec::from_slice_or_alloc(&data_8);
            let trie_result = CodePointTrie::try_new(trie_header, index, data);
            assert!(trie_result.is_ok(), "Could not construct trie");
            assert_eq!(
                test_struct.value_width_enum_val,
                ValueWidthEnum::Bits8 as u8
            );
            run_trie_tests(
                &trie_result.unwrap(),
                &test_file.code_point_trie.test_data.check_ranges,
            );
        }

        (_, Some(data_16), _) => {
            let data = ZeroVec::from_slice_or_alloc(&data_16);
            let trie_result = CodePointTrie::try_new(trie_header, index, data);
            assert!(trie_result.is_ok(), "Could not construct trie");
            assert_eq!(
                test_struct.value_width_enum_val,
                ValueWidthEnum::Bits16 as u8
            );
            run_trie_tests(
                &trie_result.unwrap(),
                &test_file.code_point_trie.test_data.check_ranges,
            );
        }

        (_, _, Some(data_32)) => {
            let data = ZeroVec::from_slice_or_alloc(&data_32);
            let trie_result = CodePointTrie::try_new(trie_header, index, data);
            assert!(trie_result.is_ok(), "Could not construct trie");
            assert_eq!(
                test_struct.value_width_enum_val,
                ValueWidthEnum::Bits32 as u8
            );
            run_trie_tests(
                &trie_result.unwrap(),
                &test_file.code_point_trie.test_data.check_ranges,
            );
        }

        (_, _, _) => {
            panic!("Could not match test trie data to a known value width or trie type");
        }
    };
}
