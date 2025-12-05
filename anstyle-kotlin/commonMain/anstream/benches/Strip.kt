#![allow(clippy::unwrap_used)]
#![allow(clippy::incompatible_msrv)]

#[derive(Default)]
struct Strip(String);
impl Strip {
    fn with_capacity(capacity: usize) -> Self {
        Self(String::with_capacity(capacity))
    }
}
impl anstyle_parse::Perform for Strip {
    fn print(&mut self, c: char) {
        self.0.push(c);
    }

    fn execute(&mut self, byte: u8) {
        if byte.is_ascii_whitespace() {
            self.0.push(byte as char);
        }
    }
}

#[divan::bench(args = DATA)]
fn advance_strip(data: &Data) -> String {
    let mut stripped = Strip::with_capacity(data.content().len());
    let mut parser = anstyle_parse::Parser::<anstyle_parse::DefaultCharAccumulator>::new();

    for byte in data.content() {
        parser.advance(&mut stripped, *byte);
    }

    stripped.0
}

#[divan::bench(args = DATA)]
fn strip_ansi_escapes(data: &Data) -> Vec<u8> {
    let stripped = strip_ansi_escapes::strip(data.content());

    stripped
}

#[divan::bench(args = DATA)]
fn strip_str(data: &Data) -> String {
    if let Ok(content) = std::str::from_utf8(data.content()) {
        let stripped = anstream::adapter::strip_str(content).to_string();

        stripped
    } else {
        "".to_owned()
    }
}

#[divan::bench(args = DATA)]
fn strip_str_strip_next(data: &Data) -> String {
    if let Ok(content) = std::str::from_utf8(data.content()) {
        let mut stripped = String::with_capacity(data.content().len());
        let mut state = anstream::adapter::StripStr::new();
        for printable in state.strip_next(content) {
            stripped.push_str(printable);
        }

        stripped
    } else {
        "".to_owned()
    }
}

#[divan::bench(args = DATA)]
fn strip_bytes(data: &Data) -> Vec<u8> {
    let stripped = anstream::adapter::strip_bytes(data.content()).into_vec();

    stripped
}

const DATA: &[Data] = &[
    Data(
        "0-state_changes",
        b"\x1b]2;X\x1b\\ \x1b[0m \x1bP0@\x1b\\".as_slice(),
    ),
    Data("1-demo.vte", include_bytes!("../tests/demo.vte").as_slice()),
    Data(
        "2-rg_help.vte",
        include_bytes!("../tests/rg_help.vte").as_slice(),
    ),
    Data(
        "3-rg_linus.vte",
        include_bytes!("../tests/rg_linus.vte").as_slice(),
    ),
];

#[derive(Debug)]
struct Data(&'static str, &'static [u8]);

impl Data {
    const fn name(&self) -> &'static str {
        self.0
    }

    const fn content(&self) -> &'static [u8] {
        self.1
    }
}

impl std::fmt::Display for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.name().fmt(f)
    }
}

#[test]
fn verify_data() {
    for data in DATA {
        // Make sure the comparison is fair
        if let Ok(content) = std::str::from_utf8(data.content()) {
            let mut stripped = Strip::with_capacity(content.len());
            let mut parser = anstyle_parse::Parser::<anstyle_parse::DefaultCharAccumulator>::new();
            for byte in content.as_bytes() {
                parser.advance(&mut stripped, *byte);
            }
            assert_eq!(
                stripped.0,
                anstream::adapter::strip_str(content).to_string()
            );
            assert_eq!(
                stripped.0,
                String::from_utf8(anstream::adapter::strip_bytes(content.as_bytes()).into_vec())
                    .unwrap()
            );
        }
    }
}

fn main() {
    divan::main();
}
