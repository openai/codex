#![no_std]

use unicode_truncate::UnicodeTruncateStr;

#[test]
fn main() {
    assert_eq!("你好吗".unicode_truncate(5), ("你好", 4));
}
