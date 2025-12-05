#![allow(clippy::unwrap_used)]
#![allow(clippy::incompatible_msrv)]

use std::io::Write as _;

#[divan::bench(args = DATA)]
fn nop(data: &Data) -> Vec<u8> {
    let buffer = Vec::with_capacity(data.content().len());
    let mut stream = buffer;

    stream.write_all(data.content()).unwrap();

    stream
}

#[divan::bench(args = DATA)]
fn strip_stream(data: &Data) -> Vec<u8> {
    let buffer = Vec::with_capacity(data.content().len());
    let mut stream = anstream::StripStream::new(buffer);

    stream.write_all(data.content()).unwrap();

    stream.into_inner()
}

#[divan::bench(args = DATA)]
#[cfg(all(windows, feature = "wincon"))]
fn wincon_stream(data: &Data) -> Vec<u8> {
    let buffer = Vec::with_capacity(data.content().len());
    let mut stream = anstream::WinconStream::new(buffer);

    stream.write_all(data.content()).unwrap();

    stream.into_inner()
}

#[divan::bench(args = DATA)]
fn auto_stream_always_ansi(data: &Data) -> Vec<u8> {
    let buffer = Vec::with_capacity(data.content().len());
    let mut stream = anstream::AutoStream::always_ansi(buffer);

    stream.write_all(data.content()).unwrap();

    stream.into_inner()
}

#[divan::bench(args = DATA)]
fn auto_stream_always(data: &Data) -> Vec<u8> {
    let buffer = Vec::with_capacity(data.content().len());
    let mut stream = anstream::AutoStream::always(buffer);

    stream.write_all(data.content()).unwrap();

    stream.into_inner()
}

#[divan::bench(args = DATA)]
fn auto_stream_never(data: &Data) -> Vec<u8> {
    let buffer = Vec::with_capacity(data.content().len());
    let mut stream = anstream::AutoStream::never(buffer);

    stream.write_all(data.content()).unwrap();

    stream.into_inner()
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
