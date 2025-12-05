#![allow(clippy::incompatible_msrv)]

#[divan::bench(args = DATA)]
fn nop(data: &Data) -> Vec<(anstyle::Style, String)> {
    let mut state = anstream::adapter::WinconBytes::new();
    let stripped = state.extract_next(data.content()).collect::<Vec<_>>();

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

fn main() {
    divan::main();
}
