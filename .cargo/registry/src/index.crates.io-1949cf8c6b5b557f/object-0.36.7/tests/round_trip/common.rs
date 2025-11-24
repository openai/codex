#![cfg(all(feature = "read", feature = "write"))]

use object::read::{Object, ObjectSection, ObjectSymbol};
use object::{read, write};
use object::{
    Architecture, BinaryFormat, Endianness, SectionKind, SymbolFlags, SymbolKind, SymbolScope,
};

#[test]
fn coff_x86_64_common() {
    let mut object =
        write::Object::new(BinaryFormat::Coff, Architecture::X86_64, Endianness::Little);

    let symbol = write::Symbol {
        name: b"v1".to_vec(),
        value: 0,
        size: 0,
        kind: SymbolKind::Data,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Undefined,
        flags: SymbolFlags::None,
    };
    object.add_common_symbol(symbol, 4, 4);

    let symbol = write::Symbol {
        name: b"v2".to_vec(),
        value: 0,
        size: 0,
        kind: SymbolKind::Data,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Undefined,
        flags: SymbolFlags::None,
    };
    object.add_common_symbol(symbol, 8, 8);

    // Also check undefined symbols, which are very similar.
    let symbol = write::Symbol {
        name: b"v3".to_vec(),
        value: 0,
        size: 0,
        kind: SymbolKind::Data,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Undefined,
        flags: SymbolFlags::None,
    };
    object.add_symbol(symbol);

    let bytes = object.write().unwrap();

    //std::fs::write(&"common.o", &bytes).unwrap();

    let object = read::File::parse(&*bytes).unwrap();
    assert_eq!(object.format(), BinaryFormat::Coff);
    assert_eq!(object.architecture(), Architecture::X86_64);

    let mut symbols = object.symbols();

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok("v1"));
    assert_eq!(symbol.kind(), SymbolKind::Data);
    assert_eq!(symbol.section(), read::SymbolSection::Common);
    assert_eq!(symbol.scope(), SymbolScope::Linkage);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());
    assert_eq!(symbol.address(), 0);
    assert_eq!(symbol.size(), 4);

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok("v2"));
    assert_eq!(symbol.kind(), SymbolKind::Data);
    assert_eq!(symbol.section(), read::SymbolSection::Common);
    assert_eq!(symbol.scope(), SymbolScope::Linkage);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());
    assert_eq!(symbol.address(), 0);
    assert_eq!(symbol.size(), 8);

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok("v3"));
    assert_eq!(symbol.kind(), SymbolKind::Data);
    assert_eq!(symbol.section(), read::SymbolSection::Undefined);
    assert_eq!(symbol.scope(), SymbolScope::Linkage);
    assert!(!symbol.is_weak());
    assert!(symbol.is_undefined());
    assert_eq!(symbol.address(), 0);
    assert_eq!(symbol.size(), 0);

    let symbol = symbols.next();
    assert!(symbol.is_none(), "unexpected symbol {:?}", symbol);
}

#[test]
fn elf_x86_64_common() {
    let mut object =
        write::Object::new(BinaryFormat::Elf, Architecture::X86_64, Endianness::Little);

    let symbol = write::Symbol {
        name: b"v1".to_vec(),
        value: 0,
        size: 0,
        kind: SymbolKind::Data,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Undefined,
        flags: SymbolFlags::None,
    };
    object.add_common_symbol(symbol, 4, 4);

    let symbol = write::Symbol {
        name: b"v2".to_vec(),
        value: 0,
        size: 0,
        kind: SymbolKind::Data,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Undefined,
        flags: SymbolFlags::None,
    };
    object.add_common_symbol(symbol, 8, 8);

    let bytes = object.write().unwrap();

    //std::fs::write(&"common.o", &bytes).unwrap();

    let object = read::File::parse(&*bytes).unwrap();
    assert_eq!(object.format(), BinaryFormat::Elf);
    assert_eq!(object.architecture(), Architecture::X86_64);

    let mut symbols = object.symbols();

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok("v1"));
    assert_eq!(symbol.kind(), SymbolKind::Data);
    assert_eq!(symbol.section(), read::SymbolSection::Common);
    assert_eq!(symbol.scope(), SymbolScope::Linkage);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());
    assert_eq!(symbol.address(), 0);
    assert_eq!(symbol.size(), 4);

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok("v2"));
    assert_eq!(symbol.kind(), SymbolKind::Data);
    assert_eq!(symbol.section(), read::SymbolSection::Common);
    assert_eq!(symbol.scope(), SymbolScope::Linkage);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());
    assert_eq!(symbol.address(), 0);
    assert_eq!(symbol.size(), 8);

    let symbol = symbols.next();
    assert!(symbol.is_none(), "unexpected symbol {:?}", symbol);
}

#[test]
fn macho_x86_64_common() {
    let mut object = write::Object::new(
        BinaryFormat::MachO,
        Architecture::X86_64,
        Endianness::Little,
    );

    let symbol = write::Symbol {
        name: b"v1".to_vec(),
        value: 0,
        size: 0,
        kind: SymbolKind::Data,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Undefined,
        flags: SymbolFlags::None,
    };
    object.add_common_symbol(symbol, 4, 4);

    let symbol = write::Symbol {
        name: b"v2".to_vec(),
        value: 0,
        size: 0,
        kind: SymbolKind::Data,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Undefined,
        flags: SymbolFlags::None,
    };
    object.add_common_symbol(symbol, 8, 8);

    let bytes = object.write().unwrap();

    //std::fs::write(&"common.o", &bytes).unwrap();

    let object = read::File::parse(&*bytes).unwrap();
    assert_eq!(object.format(), BinaryFormat::MachO);
    assert_eq!(object.architecture(), Architecture::X86_64);

    let mut sections = object.sections();

    let common = sections.next().unwrap();
    println!("{:?}", common);
    let common_index = common.index();
    assert_eq!(common.name(), Ok("__common"));
    assert_eq!(common.segment_name(), Ok(Some("__DATA")));
    assert_eq!(common.kind(), SectionKind::Common);
    assert_eq!(common.size(), 16);
    assert_eq!(common.data(), Ok(&[][..]));

    let section = sections.next();
    assert!(section.is_none(), "unexpected section {:?}", section);

    let mut symbols = object.symbols();

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok("_v1"));
    assert_eq!(symbol.kind(), SymbolKind::Data);
    assert_eq!(symbol.section_index(), Some(common_index));
    assert_eq!(symbol.scope(), SymbolScope::Linkage);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());
    assert_eq!(symbol.address(), 0);

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok("_v2"));
    assert_eq!(symbol.kind(), SymbolKind::Data);
    assert_eq!(symbol.section_index(), Some(common_index));
    assert_eq!(symbol.scope(), SymbolScope::Linkage);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());
    assert_eq!(symbol.address(), 8);

    let symbol = symbols.next();
    assert!(symbol.is_none(), "unexpected symbol {:?}", symbol);
}
