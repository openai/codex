/// tests cases ported over from python standard library
use textwrap::{dedent, indent};

const ROUNDTRIP_CASES: [&str; 3] = [
    // basic test case
    "Hi.\nThis is a test.\nTesting.",
    // include a blank line
    "Hi.\nThis is a test.\n\nTesting.",
    // include leading and trailing blank lines
    "\nHi.\nThis is a test.\nTesting.\n",
];

const WINDOWS_CASES: [&str; 2] = [
    // use windows line endings
    "Hi.\r\nThis is a test.\r\nTesting.",
    // pathological case
    "Hi.\r\nThis is a test.\n\r\nTesting.\r\n\n",
];

#[test]
fn test_indent_nomargin_default() {
    // indent should do nothing if 'prefix' is empty.
    for text in ROUNDTRIP_CASES.iter() {
        assert_eq!(&indent(text, ""), text);
    }
    for text in WINDOWS_CASES.iter() {
        assert_eq!(&indent(text, ""), text);
    }
}

#[test]
fn test_roundtrip_spaces() {
    // A whitespace prefix should roundtrip with dedent
    for text in ROUNDTRIP_CASES.iter() {
        assert_eq!(&dedent(&indent(text, "    ")), text);
    }
}

#[test]
fn test_roundtrip_tabs() {
    // A whitespace prefix should roundtrip with dedent
    for text in ROUNDTRIP_CASES.iter() {
        assert_eq!(&dedent(&indent(text, "\t\t")), text);
    }
}

#[test]
fn test_roundtrip_mixed() {
    // A whitespace prefix should roundtrip with dedent
    for text in ROUNDTRIP_CASES.iter() {
        assert_eq!(&dedent(&indent(text, " \t  \t ")), text);
    }
}

#[test]
fn test_indent_default() {
    // Test default indenting of lines that are not whitespace only
    let prefix = "  ";
    let expected = [
        // Basic test case
        "  Hi.\n  This is a test.\n  Testing.",
        // Include a blank line
        "  Hi.\n  This is a test.\n\n  Testing.",
        // Include leading and trailing blank lines
        "\n  Hi.\n  This is a test.\n  Testing.\n",
    ];
    for (text, expect) in ROUNDTRIP_CASES.iter().zip(expected.iter()) {
        assert_eq!(&indent(text, prefix), expect)
    }
    let expected = [
        // Use Windows line endings
        "  Hi.\r\n  This is a test.\r\n  Testing.",
        // Pathological case
        "  Hi.\r\n  This is a test.\n\r\n  Testing.\r\n\n",
    ];
    for (text, expect) in WINDOWS_CASES.iter().zip(expected.iter()) {
        assert_eq!(&indent(text, prefix), expect)
    }
}

#[test]
fn indented_text_should_have_the_same_number_of_lines_as_the_original_text() {
    let texts = ["foo\nbar", "foo\nbar\n", "foo\nbar\nbaz"];
    for original in texts.iter() {
        let indented = indent(original, "");
        assert_eq!(&indented, original);
    }
}
