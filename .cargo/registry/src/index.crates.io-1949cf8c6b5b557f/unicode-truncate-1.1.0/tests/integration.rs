use unicode_truncate::{Alignment, UnicodeTruncateStr};

#[test]
fn main() {
    assert_eq!("你好吗".unicode_truncate(5), ("你好", 4));
    assert_eq!("你好吗".unicode_truncate_start(5), ("好吗", 4));

    assert_eq!("你好吗".unicode_pad(5, Alignment::Left, true), "你好 ");
}
