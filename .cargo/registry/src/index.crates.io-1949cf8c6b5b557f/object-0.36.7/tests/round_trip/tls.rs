#![cfg(all(feature = "read", feature = "write"))]

use object::read::{Object, ObjectSection, ObjectSymbol};
use object::{read, write};
use object::{
    Architecture, BinaryFormat, Endianness, RelocationEncoding, RelocationKind, SectionKind,
    SymbolFlags, SymbolKind, SymbolScope,
};

#[test]
fn coff_x86_64_tls() {
    let mut object =
        write::Object::new(BinaryFormat::Coff, Architecture::X86_64, Endianness::Little);

    let section = object.section_id(write::StandardSection::Tls);
    let symbol = object.add_symbol(write::Symbol {
        name: b"tls1".to_vec(),
        value: 0,
        size: 0,
        kind: SymbolKind::Tls,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Undefined,
        flags: SymbolFlags::None,
    });
    object.add_symbol_data(symbol, section, &[1; 30], 4);

    let bytes = object.write().unwrap();

    //std::fs::write(&"tls.o", &bytes).unwrap();

    let object = read::File::parse(&*bytes).unwrap();
    assert_eq!(object.format(), BinaryFormat::Coff);
    assert_eq!(object.architecture(), Architecture::X86_64);

    let mut sections = object.sections();

    let section = sections.next().unwrap();
    println!("{:?}", section);
    let tls_index = section.index();
    assert_eq!(section.name(), Ok(".tls$"));
    assert_eq!(section.kind(), SectionKind::Data);
    assert_eq!(section.size(), 30);
    assert_eq!(section.data().unwrap(), &[1; 30]);

    let mut symbols = object.symbols();

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok("tls1"));
    assert_eq!(symbol.kind(), SymbolKind::Data);
    assert_eq!(symbol.section_index(), Some(tls_index));
    assert_eq!(symbol.scope(), SymbolScope::Linkage);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());
}

#[test]
fn elf_x86_64_tls() {
    let mut object =
        write::Object::new(BinaryFormat::Elf, Architecture::X86_64, Endianness::Little);

    let section = object.section_id(write::StandardSection::Tls);
    let symbol = object.add_symbol(write::Symbol {
        name: b"tls1".to_vec(),
        value: 0,
        size: 0,
        kind: SymbolKind::Tls,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Undefined,
        flags: SymbolFlags::None,
    });
    object.add_symbol_data(symbol, section, &[1; 30], 4);

    let section = object.section_id(write::StandardSection::UninitializedTls);
    let symbol = object.add_symbol(write::Symbol {
        name: b"tls2".to_vec(),
        value: 0,
        size: 0,
        kind: SymbolKind::Tls,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Undefined,
        flags: SymbolFlags::None,
    });
    object.add_symbol_bss(symbol, section, 31, 4);

    let bytes = object.write().unwrap();

    //std::fs::write(&"tls.o", &bytes).unwrap();

    let object = read::File::parse(&*bytes).unwrap();
    assert_eq!(object.format(), BinaryFormat::Elf);
    assert_eq!(object.architecture(), Architecture::X86_64);

    let mut sections = object.sections();

    let section = sections.next().unwrap();
    println!("{:?}", section);
    let tdata_index = section.index();
    assert_eq!(section.name(), Ok(".tdata"));
    assert_eq!(section.kind(), SectionKind::Tls);
    assert_eq!(section.size(), 30);
    assert_eq!(section.data().unwrap(), &[1; 30]);

    let section = sections.next().unwrap();
    println!("{:?}", section);
    let tbss_index = section.index();
    assert_eq!(section.name(), Ok(".tbss"));
    assert_eq!(section.kind(), SectionKind::UninitializedTls);
    assert_eq!(section.size(), 31);
    assert_eq!(section.data().unwrap(), &[]);

    let mut symbols = object.symbols();

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok("tls1"));
    assert_eq!(symbol.kind(), SymbolKind::Tls);
    assert_eq!(symbol.section_index(), Some(tdata_index));
    assert_eq!(symbol.scope(), SymbolScope::Linkage);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());
    assert_eq!(symbol.size(), 30);

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok("tls2"));
    assert_eq!(symbol.kind(), SymbolKind::Tls);
    assert_eq!(symbol.section_index(), Some(tbss_index));
    assert_eq!(symbol.scope(), SymbolScope::Linkage);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());
    assert_eq!(symbol.size(), 31);
}

#[test]
fn macho_x86_64_tls() {
    let mut object = write::Object::new(
        BinaryFormat::MachO,
        Architecture::X86_64,
        Endianness::Little,
    );

    let section = object.section_id(write::StandardSection::Tls);
    let symbol = object.add_symbol(write::Symbol {
        name: b"tls1".to_vec(),
        value: 0,
        size: 0,
        kind: SymbolKind::Tls,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Undefined,
        flags: SymbolFlags::None,
    });
    object.add_symbol_data(symbol, section, &[1; 30], 4);

    let section = object.section_id(write::StandardSection::UninitializedTls);
    let symbol = object.add_symbol(write::Symbol {
        name: b"tls2".to_vec(),
        value: 0,
        size: 0,
        kind: SymbolKind::Tls,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Undefined,
        flags: SymbolFlags::None,
    });
    object.add_symbol_bss(symbol, section, 31, 4);

    let bytes = object.write().unwrap();

    //std::fs::write(&"tls.o", &bytes).unwrap();

    let object = read::File::parse(&*bytes).unwrap();
    assert_eq!(object.format(), BinaryFormat::MachO);
    assert_eq!(object.architecture(), Architecture::X86_64);

    let mut sections = object.sections();

    let thread_data = sections.next().unwrap();
    println!("{:?}", thread_data);
    let thread_data_index = thread_data.index();
    assert_eq!(thread_data.name(), Ok("__thread_data"));
    assert_eq!(thread_data.segment_name(), Ok(Some("__DATA")));
    assert_eq!(thread_data.kind(), SectionKind::Tls);
    assert_eq!(thread_data.size(), 30);
    assert_eq!(thread_data.data().unwrap(), &[1; 30]);

    let thread_vars = sections.next().unwrap();
    println!("{:?}", thread_vars);
    let thread_vars_index = thread_vars.index();
    assert_eq!(thread_vars.name(), Ok("__thread_vars"));
    assert_eq!(thread_vars.segment_name(), Ok(Some("__DATA")));
    assert_eq!(thread_vars.kind(), SectionKind::TlsVariables);
    assert_eq!(thread_vars.size(), 2 * 3 * 8);
    assert_eq!(thread_vars.data().unwrap(), &[0; 48]);

    let thread_bss = sections.next().unwrap();
    println!("{:?}", thread_bss);
    let thread_bss_index = thread_bss.index();
    assert_eq!(thread_bss.name(), Ok("__thread_bss"));
    assert_eq!(thread_bss.segment_name(), Ok(Some("__DATA")));
    assert_eq!(thread_bss.kind(), SectionKind::UninitializedTls);
    assert_eq!(thread_bss.size(), 31);
    assert_eq!(thread_bss.data(), Ok(&[][..]));

    let mut symbols = object.symbols();

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    let tls1_init_symbol = symbol.index();
    assert_eq!(symbol.name(), Ok("_tls1$tlv$init"));
    assert_eq!(symbol.kind(), SymbolKind::Tls);
    assert_eq!(symbol.section_index(), Some(thread_data_index));
    assert_eq!(symbol.scope(), SymbolScope::Compilation);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    let tls2_init_symbol = symbol.index();
    assert_eq!(symbol.name(), Ok("_tls2$tlv$init"));
    assert_eq!(symbol.kind(), SymbolKind::Tls);
    assert_eq!(symbol.section_index(), Some(thread_bss_index));
    assert_eq!(symbol.scope(), SymbolScope::Compilation);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok("_tls1"));
    assert_eq!(symbol.kind(), SymbolKind::Tls);
    assert_eq!(symbol.section_index(), Some(thread_vars_index));
    assert_eq!(symbol.scope(), SymbolScope::Linkage);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok("_tls2"));
    assert_eq!(symbol.kind(), SymbolKind::Tls);
    assert_eq!(symbol.section_index(), Some(thread_vars_index));
    assert_eq!(symbol.scope(), SymbolScope::Linkage);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    let tlv_bootstrap_symbol = symbol.index();
    assert_eq!(symbol.name(), Ok("__tlv_bootstrap"));
    assert_eq!(symbol.kind(), SymbolKind::Unknown);
    assert_eq!(symbol.section_index(), None);
    assert_eq!(symbol.scope(), SymbolScope::Unknown);
    assert!(!symbol.is_weak());
    assert!(symbol.is_undefined());

    let mut relocations = thread_vars.relocations();

    let (offset, relocation) = relocations.next().unwrap();
    println!("{:?}", relocation);
    assert_eq!(offset, 40);
    assert_eq!(relocation.kind(), RelocationKind::Absolute);
    assert_eq!(relocation.encoding(), RelocationEncoding::Generic);
    assert_eq!(relocation.size(), 64);
    assert_eq!(
        relocation.target(),
        read::RelocationTarget::Symbol(tls2_init_symbol)
    );
    assert_eq!(relocation.addend(), 0);

    let (offset, relocation) = relocations.next().unwrap();
    println!("{:?}", relocation);
    assert_eq!(offset, 24);
    assert_eq!(relocation.kind(), RelocationKind::Absolute);
    assert_eq!(relocation.encoding(), RelocationEncoding::Generic);
    assert_eq!(relocation.size(), 64);
    assert_eq!(
        relocation.target(),
        read::RelocationTarget::Symbol(tlv_bootstrap_symbol)
    );
    assert_eq!(relocation.addend(), 0);

    let (offset, relocation) = relocations.next().unwrap();
    println!("{:?}", relocation);
    assert_eq!(offset, 16);
    assert_eq!(relocation.kind(), RelocationKind::Absolute);
    assert_eq!(relocation.encoding(), RelocationEncoding::Generic);
    assert_eq!(relocation.size(), 64);
    assert_eq!(
        relocation.target(),
        read::RelocationTarget::Symbol(tls1_init_symbol)
    );
    assert_eq!(relocation.addend(), 0);

    let (offset, relocation) = relocations.next().unwrap();
    println!("{:?}", relocation);
    assert_eq!(offset, 0);
    assert_eq!(relocation.kind(), RelocationKind::Absolute);
    assert_eq!(relocation.encoding(), RelocationEncoding::Generic);
    assert_eq!(relocation.size(), 64);
    assert_eq!(
        relocation.target(),
        read::RelocationTarget::Symbol(tlv_bootstrap_symbol)
    );
    assert_eq!(relocation.addend(), 0);
}
