# nom, eating data byte by byte

[![LICENSE](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Join the chat at https://gitter.im/Geal/nom](https://badges.gitter.im/Join%20Chat.svg)](https://gitter.im/Geal/nom?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge&utm_content=badge)
[![Build Status](https://github.com/Geal/nom/actions/workflows/ci.yml/badge.svg)](https://github.com/Geal/nom/actions/workflows/ci.yml)
[![Coverage Status](https://coveralls.io/repos/github/Geal/nom/badge.svg?branch=main)](https://coveralls.io/github/Geal/nom?branch=main)
[![Crates.io Version](https://img.shields.io/crates/v/nom.svg)](https://crates.io/crates/nom)
[![Minimum rustc version](https://img.shields.io/badge/rustc-1.48.0+-lightgray.svg)](#rust-version-requirements-msrv)

nom is a parser combinators library written in Rust. Its goal is to provide tools
to build safe parsers without compromising the speed or memory consumption. To
that end, it uses extensively Rust's *strong typing* and *memory safety* to produce
fast and correct parsers, and provides functions, macros and traits to abstract most of the
error prone plumbing.

![nom logo in CC0 license, by Ange Albertini](https://raw.githubusercontent.com/Geal/nom/main/assets/nom.png)

*nom will happily take a byte out of your files :)*

<!-- toc -->

- [Example](#example)
- [Documentation](#documentation)
- [Why use nom?](#why-use-nom)
    - [Binary format parsers](#binary-format-parsers)
    - [Text format parsers](#text-format-parsers)
    - [Programming language parsers](#programming-language-parsers)
    - [Streaming formats](#streaming-formats)
- [Parser combinators](#parser-combinators)
- [Technical features](#technical-features)
- [Rust version requirements](#rust-version-requirements-msrv)
- [Installation](#installation)
- [Related projects](#related-projects)
- [Parsers written with nom](#parsers-written-with-nom)
- [Contributors](#contributors)

<!-- tocstop -->

## Example

[Hexadecimal color](https://developer.mozilla.org/en-US/docs/Web/CSS/color) parser:

```rust
extern crate nom;
use nom::{
  IResult,
  bytes::complete::{tag, take_while_m_n},
  combinator::map_res,
  sequence::tuple
};

#[derive(Debug,PartialEq)]
pub struct Color {
  pub red:   u8,
  pub green: u8,
  pub blue:  u8,
}

fn from_hex(input: &str) -> Result<u8, std::num::ParseIntError> {
  u8::from_str_radix(input, 16)
}

fn is_hex_digit(c: char) -> bool {
  c.is_digit(16)
}

fn hex_primary(input: &str) -> IResult<&str, u8> {
  map_res(
    take_while_m_n(2, 2, is_hex_digit),
    from_hex
  )(input)
}

fn hex_color(input: &str) -> IResult<&str, Color> {
  let (input, _) = tag("#")(input)?;
  let (input, (red, green, blue)) = tuple((hex_primary, hex_primary, hex_primary))(input)?;

  Ok((input, Color { red, green, blue }))
}

fn main() {}

#[test]
fn parse_color() {
  assert_eq!(hex_color("#2F14DF"), Ok(("", Color {
    red: 47,
    green: 20,
    blue: 223,
  })));
}
```

## Documentation

- [Reference documentation](https://docs.rs/nom)
- [Various design documents and tutorials](https://github.com/Geal/nom/tree/main/doc)
- [List of combinators and their behaviour](https://github.com/Geal/nom/blob/main/doc/choosing_a_combinator.md)

If you need any help developing your parsers, please ping `geal` on IRC (libera, geeknode, oftc), go to `#nom-parsers` on Libera IRC, or on the [Gitter chat room](https://gitter.im/Geal/nom).

## Why use nom

If you want to write:

### Binary format parsers

nom was designed to properly parse binary formats from the beginning. Compared
to the usual handwritten C parsers, nom parsers are just as fast, free from
buffer overflow vulnerabilities, and handle common patterns for you:

- [TLV](https://en.wikipedia.org/wiki/Type-length-value)
- Bit level parsing
- Hexadecimal viewer in the debugging macros for easy data analysis
- Streaming parsers for network formats and huge files

Example projects:

- [FLV parser](https://github.com/rust-av/flavors)
- [Matroska parser](https://github.com/rust-av/matroska)
- [tar parser](https://github.com/Keruspe/tar-parser.rs)

### Text format parsers

While nom was made for binary format at first, it soon grew to work just as
well with text formats. From line based formats like CSV, to more complex, nested
formats such as JSON, nom can manage it, and provides you with useful tools:

- Fast case insensitive comparison
- Recognizers for escaped strings
- Regular expressions can be embedded in nom parsers to represent complex character patterns succinctly
- Special care has been given to managing non ASCII characters properly

Example projects:

- [HTTP proxy](https://github.com/sozu-proxy/sozu/tree/main/lib/src/protocol/http/parser)
- [TOML parser](https://github.com/joelself/tomllib)

### Programming language parsers

While programming language parsers are usually written manually for more
flexibility and performance, nom can be (and has been successfully) used
as a prototyping parser for a language.

nom will get you started quickly with powerful custom error types, that you
can leverage with [nom_locate](https://github.com/fflorent/nom_locate) to
pinpoint the exact line and column of the error. No need for separate
tokenizing, lexing and parsing phases: nom can automatically handle whitespace
parsing, and construct an AST in place.

Example projects:

- [PHP VM](https://github.com/tagua-vm/parser)
- eve language prototype
- [xshade shading language](https://github.com/xshade-lang/xshade/)

### Streaming formats

While a lot of formats (and the code handling them) assume that they can fit
the complete data in memory, there are formats for which we only get a part
of the data at once, like network formats, or huge files.
nom has been designed for a correct behaviour with partial data: If there is
not enough data to decide, nom will tell you it needs more instead of silently
returning a wrong result. Whether your data comes entirely or in chunks, the
result should be the same.

It allows you to build powerful, deterministic state machines for your protocols.

Example projects:

- [HTTP proxy](https://github.com/sozu-proxy/sozu/tree/main/lib/src/protocol/http/parser)
- [Using nom with generators](https://github.com/Geal/generator_nom)

## Parser combinators

Parser combinators are an approach to parsers that is very different from
software like [lex](https://en.wikipedia.org/wiki/Lex_(software)) and
[yacc](https://en.wikipedia.org/wiki/Yacc). Instead of writing the grammar
in a separate file and generating the corresponding code, you use very
small functions with very specific purpose, like "take 5 bytes", or
"recognize the word 'HTTP'", and assemble them in meaningful patterns
like "recognize 'HTTP', then a space, then a version".
The resulting code is small, and looks like the grammar you would have
written with other parser approaches.

This has a few advantages:

- The parsers are small and easy to write
- The parsers components are easy to reuse (if they're general enough, please add them to nom!)
- The parsers components are easy to test separately (unit tests and property-based tests)
- The parser combination code looks close to the grammar you would have written
- You can build partial parsers, specific to the data you need at the moment, and ignore the rest

## Technical features

nom parsers are for:
- [x] **byte-oriented**: The basic type is `&[u8]` and parsers will work as much as possible on byte array slices (but are not limited to them)
- [x] **bit-oriented**: nom can address a byte slice as a bit stream
- [x] **string-oriented**: The same kind of combinators can apply on UTF-8 strings as well
- [x] **zero-copy**: If a parser returns a subset of its input data, it will return a slice of that input, without copying
- [x] **streaming**: nom can work on partial data and detect when it needs more data to produce a correct result
- [x] **descriptive errors**: The parsers can aggregate a list of error codes with pointers to the incriminated input slice. Those error lists can be pattern matched to provide useful messages.
- [x] **custom error types**: You can provide a specific type to improve errors returned by parsers
- [x] **safe parsing**: nom leverages Rust's safe memory handling and powerful types, and parsers are routinely fuzzed and tested with real world data. So far, the only flaws found by fuzzing were in code written outside of nom
- [x] **speed**: Benchmarks have shown that nom parsers often outperform many parser combinators library like Parsec and attoparsec, some regular expression engines and even handwritten C parsers

Some benchmarks are available on [Github](https://github.com/Geal/nom_benchmarks).

## Rust version requirements (MSRV)

The 7.0 series of nom supports **Rustc version 1.48 or greater**. It is known to work properly on Rust 1.41.1 but there is no guarantee it will stay the case through this major release.

The current policy is that this will only be updated in the next major nom release.

## Installation

nom is available on [crates.io](https://crates.io/crates/nom) and can be included in your Cargo enabled project like this:

```toml
[dependencies]
nom = "7"
```

There are a few compilation features:

* `alloc`: (activated by default) if disabled, nom can work in `no_std` builds without memory allocators. If enabled, combinators that allocate (like `many0`) will be available
* `std`: (activated by default, activates `alloc` too) if disabled, nom can work in `no_std` builds

You can configure those features like this:

```toml
[dependencies.nom]
version = "7"
default-features = false
features = ["alloc"]
```

# Related projects

- [Get line and column info in nom's input type](https://github.com/fflorent/nom_locate)
- [Using nom as lexer and parser](https://github.com/Rydgel/monkey-rust)

# Parsers written with nom

Here is a (non exhaustive) list of known projects using nom:

- Text file formats: [Ceph Crush](https://github.com/cholcombe973/crushtool),
[Cronenberg](https://github.com/ayrat555/cronenberg),
[XFS Runtime Stats](https://github.com/ChrisMacNaughton/xfs-rs),
[CSV](https://github.com/GuillaumeGomez/csv-parser),
[FASTA](https://github.com/TianyiShi2001/nom-fasta),
[FASTQ](https://github.com/elij/fastq.rs),
[INI](https://github.com/Geal/nom/blob/main/tests/ini.rs),
[ISO 8601 dates](https://github.com/badboy/iso8601),
[libconfig-like configuration file format](https://github.com/filipegoncalves/rust-config),
[Web archive](https://github.com/sbeckeriv/warc_nom_parser),
[PDB](https://github.com/TianyiShi2001/nom-pdb),
[proto files](https://github.com/tafia/protobuf-parser),
[Fountain screenplay markup](https://github.com/adamchalmers/fountain-rs),
[vimwiki](https://github.com/chipsenkbeil/vimwiki-server/tree/master/vimwiki) & [vimwiki_macros](https://github.com/chipsenkbeil/vimwiki-server/tree/master/vimwiki_macros)
- Programming languages:
[PHP](https://github.com/tagua-vm/parser),
[Basic Calculator](https://github.com/balajisivaraman/basic_calculator_rs),
[GLSL](https://github.com/phaazon/glsl),
[Lua](https://github.com/doomrobo/nom-lua53),
[Python](https://github.com/ProgVal/rust-python-parser),
[SQL](https://github.com/ms705/nom-sql),
[Elm](https://github.com/cout970/Elm-interpreter),
[SystemVerilog](https://github.com/dalance/sv-parser),
[Turtle](https://github.com/vandenoever/rome/tree/master/src/io/turtle),
[CSML](https://github.com/CSML-by-Clevy/csml-interpreter),
[Wasm](https://github.com/Strytyp/wasm-nom),
[Pseudocode](https://github.com/Gungy2/pseudocode)
[Filter for MeiliSearch](https://github.com/meilisearch/meilisearch)
- Interface definition formats: [Thrift](https://github.com/thehydroimpulse/thrust)
- Audio, video and image formats:
[GIF](https://github.com/Geal/gif.rs),
[MagicaVoxel .vox](https://github.com/davidedmonds/dot_vox),
[midi](https://github.com/derekdreery/nom-midi-rs),
[SWF](https://github.com/open-flash/swf-parser),
[WAVE](http://github.com/noise-Labs/wave),
[Matroska (MKV)](https://github.com/rust-av/matroska)
- Document formats:
[TAR](https://github.com/Keruspe/tar-parser.rs),
[GZ](https://github.com/nharward/nom-gzip),
[GDSII](https://github.com/erihsu/gds2-io)
- Cryptographic formats:
[X.509](https://github.com/rusticata/x509-parser)
- Network protocol formats:
[Bencode](https://github.com/jbaum98/bencode.rs),
[D-Bus](https://github.com/toshokan/misato),
[DHCP](https://github.com/rusticata/dhcp-parser),
[HTTP](https://github.com/sozu-proxy/sozu/tree/main/lib/src/protocol/http),
[URI](https://github.com/santifa/rrp/blob/master/src/uri.rs),
[IMAP](https://github.com/djc/tokio-imap),
[IRC](https://github.com/Detegr/RBot-parser),
[Pcap-NG](https://github.com/richo/pcapng-rs),
[Pcap](https://github.com/ithinuel/pcap-rs),
[Pcap + PcapNG](https://github.com/rusticata/pcap-parser),
[IKEv2](https://github.com/rusticata/ipsec-parser),
[NTP](https://github.com/rusticata/ntp-parser),
[SNMP](https://github.com/rusticata/snmp-parser),
[Kerberos v5](https://github.com/rusticata/kerberos-parser),
[DER](https://github.com/rusticata/der-parser),
[TLS](https://github.com/rusticata/tls-parser),
[IPFIX / Netflow v10](https://github.com/dominotree/rs-ipfix),
[GTP](https://github.com/fuerstenau/gorrosion-gtp),
[SIP](https://github.com/armatusmiles/sipcore/tree/master/crates/sipmsg),
[Prometheus](https://github.com/timberio/vector/blob/master/lib/prometheus-parser/src/line.rs)
- Language specifications:
[BNF](https://github.com/snewt/bnf)
- Misc formats:
[Gameboy ROM](https://github.com/MarkMcCaskey/gameboy-rom-parser),
[ANT FIT](https://github.com/stadelmanma/fitparse-rs),
[Version Numbers](https://github.com/fosskers/rs-versions),
[Telcordia/Bellcore SR-4731 SOR OTDR files](https://github.com/JamesHarrison/otdrs),
[MySQL binary log](https://github.com/PrivateRookie/boxercrab),
[URI](https://github.com/Skasselbard/nom-uri),
[Furigana](https://github.com/sachaarbonel/furigana.rs),
[Wordle Result](https://github.com/Fyko/wordle-stats/tree/main/parser)

Want to create a new parser using `nom`? A list of not yet implemented formats is available [here](https://github.com/Geal/nom/issues/14).

Want to add your parser here? Create a pull request for it!

# Contributors

nom is the fruit of the work of many contributors over the years, many thanks for your help!

<a href="https://github.com/geal/nom/graphs/contributors">
  <img src="https://contributors-img.web.app/image?repo=geal/nom" />
</a>
