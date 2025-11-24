use super::*;

use crate::Flags;

#[test]
fn cases() {
    let flags = TestFlags::FLAGS
        .iter()
        .map(|flag| (flag.name(), flag.value().bits()))
        .collect::<Vec<_>>();

    assert_eq!(
        vec![
            ("A", 1u8),
            ("B", 1 << 1),
            ("C", 1 << 2),
            ("ABC", 1 | 1 << 1 | 1 << 2),
        ],
        flags,
    );

    assert_eq!(0, TestEmpty::FLAGS.iter().count());
}

mod external {
    use super::*;

    #[test]
    fn cases() {
        let flags = TestExternal::FLAGS
            .iter()
            .map(|flag| (flag.name(), flag.value().bits()))
            .collect::<Vec<_>>();

        assert_eq!(
            vec![
                ("A", 1u8),
                ("B", 1 << 1),
                ("C", 1 << 2),
                ("ABC", 1 | 1 << 1 | 1 << 2),
                ("", !0),
            ],
            flags,
        );
    }
}
