#![allow(clippy::let_underscore_untyped, clippy::unreadable_literal)]

use std::mem::size_of_val;

#[test]
fn test_size() {
    #[allow(dead_code)]
    #[path = "../src/tables.rs"]
    mod tables;

    let size = size_of_val(&tables::ASCII_START)
        + size_of_val(&tables::ASCII_CONTINUE)
        + size_of_val(&tables::TRIE_START)
        + size_of_val(&tables::TRIE_CONTINUE)
        + size_of_val(&tables::LEAF);
    assert_eq!(10400, size);
}

#[test]
fn test_xid_size() {
    #[deny(dead_code)]
    #[path = "tables/mod.rs"]
    mod tables;

    let size = size_of_val(tables::XID_START) + size_of_val(tables::XID_CONTINUE);
    assert_eq!(11872, size);

    let _ = tables::BY_NAME;
}

#[cfg(target_pointer_width = "64")]
#[test]
fn test_trieset_size() {
    #[deny(dead_code)]
    #[allow(clippy::redundant_static_lifetimes)]
    #[path = "trie/trie.rs"]
    mod trie;

    let ucd_trie::TrieSet {
        tree1_level1,
        tree2_level1,
        tree2_level2,
        tree3_level1,
        tree3_level2,
        tree3_level3,
    } = *trie::XID_START;

    let start_size = size_of_val(trie::XID_START)
        + size_of_val(tree1_level1)
        + size_of_val(tree2_level1)
        + size_of_val(tree2_level2)
        + size_of_val(tree3_level1)
        + size_of_val(tree3_level2)
        + size_of_val(tree3_level3);

    let ucd_trie::TrieSet {
        tree1_level1,
        tree2_level1,
        tree2_level2,
        tree3_level1,
        tree3_level2,
        tree3_level3,
    } = *trie::XID_CONTINUE;

    let continue_size = size_of_val(trie::XID_CONTINUE)
        + size_of_val(tree1_level1)
        + size_of_val(tree2_level1)
        + size_of_val(tree2_level2)
        + size_of_val(tree3_level1)
        + size_of_val(tree3_level2)
        + size_of_val(tree3_level3);

    assert_eq!(10344, start_size + continue_size);

    let _ = trie::BY_NAME;
}

#[test]
fn test_fst_size() {
    let xid_start_fst = include_bytes!("fst/xid_start.fst");
    let xid_continue_fst = include_bytes!("fst/xid_continue.fst");
    let size = xid_start_fst.len() + xid_continue_fst.len();
    assert_eq!(144140, size);
}

#[test]
fn test_roaring_size() {
    #[path = "roaring/mod.rs"]
    mod roaring;

    let xid_start_bitmap = roaring::xid_start_bitmap();
    let xid_continue_bitmap = roaring::xid_continue_bitmap();
    let size = xid_start_bitmap.serialized_size() + xid_continue_bitmap.serialized_size();
    assert_eq!(66104, size);
}
