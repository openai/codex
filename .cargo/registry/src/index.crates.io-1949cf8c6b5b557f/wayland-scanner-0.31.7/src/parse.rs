use super::protocol::*;
use std::{
    io::{BufRead, BufReader, Read},
    str::FromStr,
};

use quick_xml::{
    events::{attributes::Attributes, Event},
    Reader,
};

macro_rules! extract_from(
    ($it: expr => $pattern: pat => $result: tt) => (
        match $it.read_event_into(&mut Vec::new()) {
            Ok($pattern) => { $result },
            e => panic!("Ill-formed protocol file: {:?}", e)
        }
    )
);

macro_rules! extract_end_tag(
    ($it: expr => $tag: expr) => (
        extract_from!($it => Event::End(bytes) => {
            assert!(bytes.name().into_inner() == $tag.as_bytes(), "Ill-formed protocol file");
        });
    )
);

pub fn parse<S: Read>(stream: S) -> Protocol {
    let mut reader = Reader::from_reader(BufReader::new(stream));
    let reader_config = reader.config_mut();
    reader_config.trim_text(true);
    reader_config.expand_empty_elements = true;
    parse_protocol(reader)
}

fn decode_utf8_or_panic(txt: Vec<u8>) -> String {
    match String::from_utf8(txt) {
        Ok(txt) => txt,
        Err(e) => panic!("Invalid UTF8: '{}'", String::from_utf8_lossy(&e.into_bytes())),
    }
}

fn parse_or_panic<T: FromStr>(txt: &[u8]) -> T {
    match std::str::from_utf8(txt).ok().and_then(|val| val.parse().ok()) {
        Some(version) => version,
        None => panic!(
            "Invalid value '{}' for parsing type '{}'",
            String::from_utf8_lossy(txt),
            std::any::type_name::<T>()
        ),
    }
}

fn init_protocol<R: BufRead>(reader: &mut Reader<R>) -> Protocol {
    // Check two firsts lines for protocol tag
    for _ in 0..3 {
        match reader.read_event_into(&mut Vec::new()) {
            Ok(Event::Decl(_) | Event::DocType(_)) => {
                continue;
            }
            Ok(Event::Start(bytes)) => {
                assert!(bytes.name().into_inner() == b"protocol", "Missing protocol toplevel tag");
                if let Some(attr) = bytes
                    .attributes()
                    .filter_map(|res| res.ok())
                    .find(|attr| attr.key.into_inner() == b"name")
                {
                    return Protocol::new(decode_utf8_or_panic(attr.value.into_owned()));
                } else {
                    panic!("Protocol must have a name");
                }
            }
            _ => panic!("Ill-formed protocol file"),
        }
    }
    panic!("Ill-formed protocol file");
}

fn parse_protocol<R: BufRead>(mut reader: Reader<R>) -> Protocol {
    let mut protocol = init_protocol(&mut reader);

    loop {
        match reader.read_event_into(&mut Vec::new()) {
            Ok(Event::Start(bytes)) => {
                match bytes.name().into_inner() {
                    b"copyright" => {
                        // parse the copyright
                        let copyright = match reader.read_event_into(&mut Vec::new()) {
                            Ok(Event::Text(copyright)) => {
                                copyright.unescape().ok().map(|x| x.to_string())
                            }
                            Ok(Event::CData(copyright)) => {
                                String::from_utf8(copyright.into_inner().into()).ok()
                            }
                            e => panic!("Ill-formed protocol file: {e:?}"),
                        };

                        extract_end_tag!(reader => "copyright");
                        protocol.copyright = copyright
                    }
                    b"interface" => {
                        protocol.interfaces.push(parse_interface(&mut reader, bytes.attributes()));
                    }
                    b"description" => {
                        protocol.description =
                            Some(parse_description(&mut reader, bytes.attributes()));
                    }
                    name => panic!(
                        "Ill-formed protocol file: unexpected token `{}` in protocol {}",
                        String::from_utf8_lossy(name),
                        protocol.name
                    ),
                }
            }
            Ok(Event::End(bytes)) => {
                let name = bytes.name().into_inner();
                assert!(
                    name == b"protocol",
                    "Unexpected closing token `{}`",
                    String::from_utf8_lossy(name)
                );
                break;
            }
            // ignore comments
            Ok(Event::Comment(_)) => {}
            e => panic!("Ill-formed protocol file: unexpected token {e:?}"),
        }
    }

    protocol
}

fn parse_interface<R: BufRead>(reader: &mut Reader<R>, attrs: Attributes) -> Interface {
    let mut interface = Interface::new();
    for attr in attrs.filter_map(|res| res.ok()) {
        match attr.key.into_inner() {
            b"name" => interface.name = decode_utf8_or_panic(attr.value.into_owned()),
            b"version" => interface.version = parse_or_panic(&attr.value),
            _ => {}
        }
    }

    loop {
        match reader.read_event_into(&mut Vec::new()) {
            Ok(Event::Start(bytes)) => match bytes.name().into_inner() {
                b"description" => {
                    interface.description = Some(parse_description(reader, bytes.attributes()))
                }
                b"request" => interface.requests.push(parse_request(reader, bytes.attributes())),
                b"event" => interface.events.push(parse_event(reader, bytes.attributes())),
                b"enum" => interface.enums.push(parse_enum(reader, bytes.attributes())),
                name => panic!("Unexpected token: `{}`", String::from_utf8_lossy(name)),
            },
            Ok(Event::End(bytes)) if bytes.name().into_inner() == b"interface" => break,
            _ => {}
        }
    }

    interface
}

fn parse_description<R: BufRead>(reader: &mut Reader<R>, attrs: Attributes) -> (String, String) {
    let mut summary = String::new();
    for attr in attrs.filter_map(|res| res.ok()) {
        if attr.key.into_inner() == b"summary" {
            summary = String::from_utf8_lossy(&attr.value)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
        }
    }

    let mut description = String::new();
    // Some protocols have comments inside their descriptions, so we need to parse them in a loop and
    // concatenate the parts into a single block of text
    loop {
        match reader.read_event_into(&mut Vec::new()) {
            Ok(Event::Text(bytes)) => {
                if !description.is_empty() {
                    description.push_str("\n\n");
                }
                description.push_str(&bytes.unescape().unwrap_or_default())
            }
            Ok(Event::End(bytes)) if bytes.name().into_inner() == b"description" => break,
            Ok(Event::Comment(_)) => {}
            e => panic!("Ill-formed protocol file: {e:?}"),
        }
    }

    (summary, description)
}

fn parse_request<R: BufRead>(reader: &mut Reader<R>, attrs: Attributes) -> Message {
    let mut request = Message::new();
    for attr in attrs.filter_map(|res| res.ok()) {
        match attr.key.into_inner() {
            b"name" => request.name = decode_utf8_or_panic(attr.value.into_owned()),
            b"type" => request.typ = Some(parse_type(&attr.value)),
            b"since" => request.since = parse_or_panic(&attr.value),
            _ => {}
        }
    }

    loop {
        match reader.read_event_into(&mut Vec::new()) {
            Ok(Event::Start(bytes)) => match bytes.name().into_inner() {
                b"description" => {
                    request.description = Some(parse_description(reader, bytes.attributes()))
                }
                b"arg" => request.args.push(parse_arg(reader, bytes.attributes())),
                name => panic!("Unexpected token: `{}`", String::from_utf8_lossy(name)),
            },
            Ok(Event::End(bytes)) if bytes.name().into_inner() == b"request" => break,
            _ => {}
        }
    }

    request
}

fn parse_enum<R: BufRead>(reader: &mut Reader<R>, attrs: Attributes) -> Enum {
    let mut enu = Enum::new();
    for attr in attrs.filter_map(|res| res.ok()) {
        match attr.key.into_inner() {
            b"name" => enu.name = decode_utf8_or_panic(attr.value.into_owned()),
            b"since" => enu.since = parse_or_panic(&attr.value),
            b"bitfield" => {
                if &attr.value[..] == b"true" {
                    enu.bitfield = true
                }
            }
            _ => {}
        }
    }

    loop {
        match reader.read_event_into(&mut Vec::new()) {
            Ok(Event::Start(bytes)) => match bytes.name().into_inner() {
                b"description" => {
                    enu.description = Some(parse_description(reader, bytes.attributes()))
                }
                b"entry" => enu.entries.push(parse_entry(reader, bytes.attributes())),
                name => panic!("Unexpected token: `{}`", String::from_utf8_lossy(name)),
            },
            Ok(Event::End(bytes)) if bytes.name().into_inner() == b"enum" => break,
            _ => {}
        }
    }

    enu
}

fn parse_event<R: BufRead>(reader: &mut Reader<R>, attrs: Attributes) -> Message {
    let mut event = Message::new();
    for attr in attrs.filter_map(|res| res.ok()) {
        match attr.key.into_inner() {
            b"name" => event.name = decode_utf8_or_panic(attr.value.into_owned()),
            b"type" => event.typ = Some(parse_type(&attr.value)),
            b"since" => event.since = parse_or_panic(&attr.value),
            _ => {}
        }
    }

    loop {
        match reader.read_event_into(&mut Vec::new()) {
            Ok(Event::Start(bytes)) => match bytes.name().into_inner() {
                b"description" => {
                    event.description = Some(parse_description(reader, bytes.attributes()))
                }
                b"arg" => event.args.push(parse_arg(reader, bytes.attributes())),
                name => panic!("Unexpected token: `{}`", String::from_utf8_lossy(name)),
            },
            Ok(Event::End(bytes)) if bytes.name().into_inner() == b"event" => break,
            _ => {}
        }
    }

    event
}

fn parse_arg<R: BufRead>(reader: &mut Reader<R>, attrs: Attributes) -> Arg {
    let mut arg = Arg::new();
    for attr in attrs.filter_map(|res| res.ok()) {
        match attr.key.into_inner() {
            b"name" => arg.name = decode_utf8_or_panic(attr.value.into_owned()),
            b"type" => arg.typ = parse_type(&attr.value),
            b"summary" => {
                arg.summary = Some(
                    String::from_utf8_lossy(&attr.value)
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                )
            }
            b"interface" => arg.interface = Some(parse_or_panic(&attr.value)),
            b"allow-null" => {
                if &*attr.value == b"true" {
                    arg.allow_null = true
                }
            }
            b"enum" => arg.enum_ = Some(decode_utf8_or_panic(attr.value.into_owned())),
            _ => {}
        }
    }

    loop {
        match reader.read_event_into(&mut Vec::new()) {
            Ok(Event::Start(bytes)) => match bytes.name().into_inner() {
                b"description" => {
                    arg.description = Some(parse_description(reader, bytes.attributes()))
                }
                name => panic!("Unexpected token: `{}`", String::from_utf8_lossy(name)),
            },
            Ok(Event::End(bytes)) if bytes.name().into_inner() == b"arg" => break,
            _ => {}
        }
    }

    arg
}

fn parse_type(txt: &[u8]) -> Type {
    match txt {
        b"int" => Type::Int,
        b"uint" => Type::Uint,
        b"fixed" => Type::Fixed,
        b"string" => Type::String,
        b"object" => Type::Object,
        b"new_id" => Type::NewId,
        b"array" => Type::Array,
        b"fd" => Type::Fd,
        b"destructor" => Type::Destructor,
        e => panic!("Unexpected type: {}", String::from_utf8_lossy(e)),
    }
}

fn parse_entry<R: BufRead>(reader: &mut Reader<R>, attrs: Attributes) -> Entry {
    let mut entry = Entry::new();
    for attr in attrs.filter_map(|res| res.ok()) {
        match attr.key.into_inner() {
            b"name" => entry.name = decode_utf8_or_panic(attr.value.into_owned()),
            b"value" => {
                entry.value = if attr.value.starts_with(b"0x") {
                    if let Some(val) = std::str::from_utf8(&attr.value[2..])
                        .ok()
                        .and_then(|s| u32::from_str_radix(s, 16).ok())
                    {
                        val
                    } else {
                        panic!("Invalid number: {}", String::from_utf8_lossy(&attr.value))
                    }
                } else {
                    parse_or_panic(&attr.value)
                };
            }
            b"since" => entry.since = parse_or_panic(&attr.value),
            b"summary" => {
                entry.summary = Some(
                    String::from_utf8_lossy(&attr.value)
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                )
            }
            _ => {}
        }
    }

    loop {
        match reader.read_event_into(&mut Vec::new()) {
            Ok(Event::Start(bytes)) => match bytes.name().into_inner() {
                b"description" => {
                    entry.description = Some(parse_description(reader, bytes.attributes()))
                }
                name => panic!("Unexpected token: `{}`", String::from_utf8_lossy(name)),
            },
            Ok(Event::End(bytes)) if bytes.name().into_inner() == b"entry" => break,
            _ => {}
        }
    }

    entry
}

#[cfg(test)]
mod tests {
    #[test]
    fn xml_parse() {
        let protocol_file =
            std::fs::File::open("./tests/scanner_assets/test-protocol.xml").unwrap();
        let _ = crate::parse::parse(protocol_file);
    }

    #[test]
    fn headerless_xml_parse() {
        let protocol_file =
            std::fs::File::open("./tests/scanner_assets/test-headerless-protocol.xml").unwrap();
        let _ = crate::parse::parse(protocol_file);
    }
}
