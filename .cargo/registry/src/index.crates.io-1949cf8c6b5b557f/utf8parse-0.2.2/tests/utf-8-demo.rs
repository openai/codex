use utf8parse::{Parser, Receiver};

static UTF8_DEMO: &[u8] = include_bytes!("UTF-8-demo.txt");

#[derive(Debug, PartialEq)]
struct StringWrapper(String);

impl Receiver for StringWrapper {
    fn codepoint(&mut self, c: char) {
        self.0.push(c);
    }

    fn invalid_sequence(&mut self) {}
}

#[test]
fn utf8parse_test() {
    let mut parser = Parser::new();

    // utf8parse implementation
    let mut actual = StringWrapper(String::new());

    for byte in UTF8_DEMO {
        parser.advance(&mut actual, *byte)
    }

    // standard library implementation
    let expected = String::from_utf8_lossy(UTF8_DEMO);

    assert_eq!(actual.0, expected);
}
