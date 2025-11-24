#![cfg(all(feature = "read", feature = "write"))]

use object::read::{Object, ObjectSection, ObjectSymbol};
use object::{read, write};
use object::{
    Architecture, BinaryFormat, Endianness, SectionKind, SymbolFlags, SymbolKind, SymbolScope,
};

#[test]
fn coff_x86_64_bss() {
    let mut object =
        write::Object::new(BinaryFormat::Coff, Architecture::X86_64, Endianness::Little);

    let section = object.section_id(write::StandardSection::UninitializedData);

    let _bss_section_symbol = object.section_symbol(section);

    let symbol = object.add_symbol(write::Symbol {
        name: b"v1".to_vec(),
        value: 0,
        size: 0,
        kind: SymbolKind::Data,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Undefined,
        flags: SymbolFlags::None,
    });
    object.add_symbol_bss(symbol, section, 18, 4);

    let symbol = object.add_symbol(write::Symbol {
        name: b"v2".to_vec(),
        value: 0,
        size: 0,
        kind: SymbolKind::Data,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Undefined,
        flags: SymbolFlags::None,
    });
    object.add_symbol_bss(symbol, section, 34, 8);

    let bytes = object.write().unwrap();

    //std::fs::write(&"bss.o", &bytes).unwrap();

    let object = read::File::parse(&*bytes).unwrap();
    assert_eq!(object.format(), BinaryFormat::Coff);
    assert_eq!(object.architecture(), Architecture::X86_64);

    let mut sections = object.sections();

    let bss = sections.next().unwrap();
    println!("{:?}", bss);
    let bss_index = bss.index();
    assert_eq!(bss.name(), Ok(".bss"));
    assert_eq!(bss.kind(), SectionKind::UninitializedData);
    assert_eq!(bss.size(), 58);
    assert_eq!(bss.data(), Ok(&[][..]));

    let section = sections.next();
    assert!(section.is_none(), "unexpected section {:?}", section);

    let mut symbols = object.symbols();

    let section_symbol = symbols.next().unwrap();
    println!("{:?}", section_symbol);
    assert_eq!(section_symbol.name(), Ok(".bss"));
    assert_eq!(section_symbol.kind(), SymbolKind::Section);
    assert_eq!(section_symbol.section_index(), Some(bss_index));
    assert_eq!(section_symbol.scope(), SymbolScope::Compilation);
    assert!(!section_symbol.is_weak());
    assert!(!section_symbol.is_undefined());
    assert_eq!(section_symbol.address(), 0);

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok("v1"));
    assert_eq!(symbol.kind(), SymbolKind::Data);
    assert_eq!(symbol.section_index(), Some(bss_index));
    assert_eq!(symbol.scope(), SymbolScope::Linkage);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());
    assert_eq!(symbol.address(), 0);

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok("v2"));
    assert_eq!(symbol.kind(), SymbolKind::Data);
    assert_eq!(symbol.section_index(), Some(bss_index));
    assert_eq!(symbol.scope(), SymbolScope::Linkage);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());
    assert_eq!(symbol.address(), 24);

    let symbol = symbols.next();
    assert!(symbol.is_none(), "unexpected symbol {:?}", symbol);
}

#[test]
fn elf_x86_64_bss() {
    let mut object =
        write::Object::new(BinaryFormat::Elf, Architecture::X86_64, Endianness::Little);

    let section = object.section_id(write::StandardSection::UninitializedData);

    let symbol = object.add_symbol(write::Symbol {
        name: b"v1".to_vec(),
        value: 0,
        size: 0,
        kind: SymbolKind::Data,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Undefined,
        flags: SymbolFlags::None,
    });
    object.add_symbol_bss(symbol, section, 18, 4);

    let symbol = object.add_symbol(write::Symbol {
        name: b"v2".to_vec(),
        value: 0,
        size: 0,
        kind: SymbolKind::Data,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Undefined,
        flags: SymbolFlags::None,
    });
    object.add_symbol_bss(symbol, section, 34, 8);

    let bytes = object.write().unwrap();

    //std::fs::write(&"bss.o", &bytes).unwrap();

    let object = read::File::parse(&*bytes).unwrap();
    assert_eq!(object.format(), BinaryFormat::Elf);
    assert_eq!(object.architecture(), Architecture::X86_64);

    let mut sections = object.sections();

    let bss = sections.next().unwrap();
    println!("{:?}", bss);
    let bss_index = bss.index();
    assert_eq!(bss.name(), Ok(".bss"));
    assert_eq!(bss.kind(), SectionKind::UninitializedData);
    assert_eq!(bss.size(), 58);
    assert_eq!(bss.data(), Ok(&[][..]));

    let mut symbols = object.symbols();

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok("v1"));
    assert_eq!(symbol.kind(), SymbolKind::Data);
    assert_eq!(symbol.section_index(), Some(bss_index));
    assert_eq!(symbol.scope(), SymbolScope::Linkage);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());
    assert_eq!(symbol.address(), 0);
    assert_eq!(symbol.size(), 18);

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok("v2"));
    assert_eq!(symbol.kind(), SymbolKind::Data);
    assert_eq!(symbol.section_index(), Some(bss_index));
    assert_eq!(symbol.scope(), SymbolScope::Linkage);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());
    assert_eq!(symbol.address(), 24);
    assert_eq!(symbol.size(), 34);

    let symbol = symbols.next();
    assert!(symbol.is_none(), "unexpected symbol {:?}", symbol);
}

#[test]
fn macho_x86_64_bss() {
    let mut object = write::Object::new(
        BinaryFormat::MachO,
        Architecture::X86_64,
        Endianness::Little,
    );

    let section = object.section_id(write::StandardSection::UninitializedData);

    let symbol = object.add_symbol(write::Symbol {
        name: b"v1".to_vec(),
        value: 0,
        size: 0,
        kind: SymbolKind::Data,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Undefined,
        flags: SymbolFlags::None,
    });
    object.add_symbol_bss(symbol, section, 18, 4);

    let symbol = object.add_symbol(write::Symbol {
        name: b"v2".to_vec(),
        value: 0,
        size: 0,
        kind: SymbolKind::Data,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Undefined,
        flags: SymbolFlags::None,
    });
    object.add_symbol_bss(symbol, section, 34, 8);

    let bytes = object.write().unwrap();

    //std::fs::write(&"bss.o", &bytes).unwrap();

    let object = read::File::parse(&*bytes).unwrap();
    assert_eq!(object.format(), BinaryFormat::MachO);
    assert_eq!(object.architecture(), Architecture::X86_64);

    let mut sections = object.sections();

    let bss = sections.next().unwrap();
    println!("{:?}", bss);
    let bss_index = bss.index();
    assert_eq!(bss.name(), Ok("__bss"));
    assert_eq!(bss.segment_name(), Ok(Some("__DATA")));
    assert_eq!(bss.kind(), SectionKind::UninitializedData);
    assert_eq!(bss.size(), 58);
    assert_eq!(bss.data(), Ok(&[][..]));

    let section = sections.next();
    assert!(section.is_none(), "unexpected section {:?}", section);

    let mut symbols = object.symbols();

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok("_v1"));
    assert_eq!(symbol.kind(), SymbolKind::Data);
    assert_eq!(symbol.section_index(), Some(bss_index));
    assert_eq!(symbol.scope(), SymbolScope::Linkage);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());
    assert_eq!(symbol.address(), 0);

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok("_v2"));
    assert_eq!(symbol.kind(), SymbolKind::Data);
    assert_eq!(symbol.section_index(), Some(bss_index));
    assert_eq!(symbol.scope(), SymbolScope::Linkage);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());
    assert_eq!(symbol.address(), 24);

    let symbol = symbols.next();
    assert!(symbol.is_none(), "unexpected symbol {:?}", symbol);
}
