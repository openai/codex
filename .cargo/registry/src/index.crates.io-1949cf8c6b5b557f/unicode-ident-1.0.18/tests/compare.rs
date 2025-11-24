#![allow(
    clippy::incompatible_msrv, // https://github.com/rust-lang/rust-clippy/issues/12257
)]

mod fst;
mod roaring;
mod trie;

#[test]
fn compare_all_implementations() {
    let xid_start_fst = fst::xid_start_fst();
    let xid_continue_fst = fst::xid_continue_fst();
    let xid_start_roaring = roaring::xid_start_bitmap();
    let xid_continue_roaring = roaring::xid_continue_bitmap();

    for ch in '\0'..=char::MAX {
        let thought_to_be_start = unicode_ident::is_xid_start(ch);
        let thought_to_be_continue = unicode_ident::is_xid_continue(ch);

        // unicode-xid
        assert_eq!(
            thought_to_be_start,
            unicode_xid::UnicodeXID::is_xid_start(ch),
            "{ch:?}",
        );
        assert_eq!(
            thought_to_be_continue,
            unicode_xid::UnicodeXID::is_xid_continue(ch),
            "{ch:?}",
        );

        // ucd-trie
        assert_eq!(
            thought_to_be_start,
            trie::XID_START.contains_char(ch),
            "{ch:?}",
        );
        assert_eq!(
            thought_to_be_continue,
            trie::XID_CONTINUE.contains_char(ch),
            "{ch:?}",
        );

        // fst
        assert_eq!(
            thought_to_be_start,
            xid_start_fst.contains((ch as u32).to_be_bytes()),
            "{ch:?}",
        );
        assert_eq!(
            thought_to_be_continue,
            xid_continue_fst.contains((ch as u32).to_be_bytes()),
            "{ch:?}",
        );

        // roaring
        assert_eq!(
            thought_to_be_start,
            xid_start_roaring.contains(ch as u32),
            "{ch:?}",
        );
        assert_eq!(
            thought_to_be_continue,
            xid_continue_roaring.contains(ch as u32),
            "{ch:?}",
        );
    }
}
