#![cfg(all(feature = "read", feature = "write"))]

use object::pe;
use object::read::{Object, ObjectComdat, ObjectSection, ObjectSymbol};
use object::{read, write};
use object::{
    Architecture, BinaryFormat, ComdatKind, Endianness, SectionKind, SymbolFlags, SymbolKind,
    SymbolScope,
};

#[test]
fn coff_x86_64_comdat() {
    let mut object =
        write::Object::new(BinaryFormat::Coff, Architecture::X86_64, Endianness::Little);

    let section1 = object.add_subsection(write::StandardSection::Text, b"s1");
    let offset = object.append_section_data(section1, &[0, 1, 2, 3], 4);
    object.section_symbol(section1);
    let section2 = object.add_subsection(write::StandardSection::Data, b"s1");
    object.append_section_data(section2, &[0, 1, 2, 3], 4);
    object.section_symbol(section2);

    let symbol = object.add_symbol(write::Symbol {
        name: b"s1".to_vec(),
        value: offset,
        size: 4,
        kind: SymbolKind::Data,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Section(section1),
        flags: SymbolFlags::None,
    });

    object.add_comdat(write::Comdat {
        kind: ComdatKind::NoDuplicates,
        symbol,
        sections: vec![section1, section2],
    });

    let bytes = object.write().unwrap();

    //std::fs::write(&"comdat.o", &bytes).unwrap();

    let object = read::File::parse(&*bytes).unwrap();
    assert_eq!(object.format(), BinaryFormat::Coff);
    assert_eq!(object.architecture(), Architecture::X86_64);

    let mut sections = object.sections();

    let section1 = sections.next().unwrap();
    println!("{:?}", section1);
    let section1_index = section1.index();
    assert_eq!(section1.name(), Ok(".text$s1"));
    assert_eq!(section1.kind(), SectionKind::Text);
    assert_eq!(section1.address(), 0);
    assert_eq!(section1.size(), 4);

    let section2 = sections.next().unwrap();
    println!("{:?}", section2);
    let section2_index = section2.index();
    assert_eq!(section2.name(), Ok(".data$s1"));
    assert_eq!(section2.kind(), SectionKind::Data);
    assert_eq!(section2.address(), 0);
    assert_eq!(section2.size(), 4);

    let mut symbols = object.symbols();

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok(".text$s1"));
    assert_eq!(symbol.kind(), SymbolKind::Section);
    assert_eq!(
        symbol.section(),
        read::SymbolSection::Section(section1.index())
    );
    assert_eq!(
        symbol.flags(),
        SymbolFlags::CoffSection {
            selection: pe::IMAGE_COMDAT_SELECT_NODUPLICATES,
            associative_section: None
        }
    );

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok(".data$s1"));
    assert_eq!(symbol.kind(), SymbolKind::Section);
    assert_eq!(
        symbol.section(),
        read::SymbolSection::Section(section2.index())
    );
    assert_eq!(
        symbol.flags(),
        SymbolFlags::CoffSection {
            selection: pe::IMAGE_COMDAT_SELECT_ASSOCIATIVE,
            associative_section: Some(section1_index)
        }
    );

    let symbol = symbols.next().unwrap();
    let symbol_index = symbol.index();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok("s1"));
    assert_eq!(symbol.kind(), SymbolKind::Data);
    assert_eq!(
        symbol.section(),
        read::SymbolSection::Section(section1.index())
    );
    assert_eq!(symbol.scope(), SymbolScope::Linkage);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());
    assert_eq!(symbol.address(), 0);

    let symbol = symbols.next();
    assert!(symbol.is_none(), "unexpected symbol {:?}", symbol);

    let mut comdats = object.comdats();

    let comdat = comdats.next().unwrap();
    println!("{:?}", comdat);
    assert_eq!(comdat.kind(), ComdatKind::NoDuplicates);
    assert_eq!(comdat.symbol(), symbol_index);

    let mut comdat_sections = comdat.sections();
    assert_eq!(comdat_sections.next(), Some(section1_index));
    assert_eq!(comdat_sections.next(), Some(section2_index));
    assert_eq!(comdat_sections.next(), None);
}

#[test]
fn elf_x86_64_comdat() {
    let mut object =
        write::Object::new(BinaryFormat::Elf, Architecture::X86_64, Endianness::Little);

    let section1 = object.add_subsection(write::StandardSection::Text, b"s1");
    let offset = object.append_section_data(section1, &[0, 1, 2, 3], 4);
    let section2 = object.add_subsection(write::StandardSection::Data, b"s1");
    object.append_section_data(section2, &[0, 1, 2, 3], 4);

    let symbol = object.add_symbol(write::Symbol {
        name: b"s1".to_vec(),
        value: offset,
        size: 4,
        kind: SymbolKind::Data,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Section(section1),
        flags: SymbolFlags::None,
    });

    object.add_comdat(write::Comdat {
        kind: ComdatKind::Any,
        symbol,
        sections: vec![section1, section2],
    });

    let bytes = object.write().unwrap();

    //std::fs::write(&"comdat.o", &bytes).unwrap();

    let object = read::File::parse(&*bytes).unwrap();
    assert_eq!(object.format(), BinaryFormat::Elf);
    assert_eq!(object.architecture(), Architecture::X86_64);

    let mut sections = object.sections();

    let section = sections.next().unwrap();
    println!("{:?}", section);
    assert_eq!(section.name(), Ok(".group"));

    let section1 = sections.next().unwrap();
    println!("{:?}", section1);
    let section1_index = section1.index();
    assert_eq!(section1.name(), Ok(".text.s1"));
    assert_eq!(section1.kind(), SectionKind::Text);
    assert_eq!(section1.address(), 0);
    assert_eq!(section1.size(), 4);

    let section2 = sections.next().unwrap();
    println!("{:?}", section2);
    let section2_index = section2.index();
    assert_eq!(section2.name(), Ok(".data.s1"));
    assert_eq!(section2.kind(), SectionKind::Data);
    assert_eq!(section2.address(), 0);
    assert_eq!(section2.size(), 4);

    let mut symbols = object.symbols();

    let symbol = symbols.next().unwrap();
    let symbol_index = symbol.index();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok("s1"));
    assert_eq!(symbol.kind(), SymbolKind::Data);
    assert_eq!(
        symbol.section(),
        read::SymbolSection::Section(section1.index())
    );
    assert_eq!(symbol.scope(), SymbolScope::Linkage);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());
    assert_eq!(symbol.address(), 0);

    let symbol = symbols.next();
    assert!(symbol.is_none(), "unexpected symbol {:?}", symbol);

    let mut comdats = object.comdats();

    let comdat = comdats.next().unwrap();
    println!("{:?}", comdat);
    assert_eq!(comdat.kind(), ComdatKind::Any);
    assert_eq!(comdat.symbol(), symbol_index);

    let mut comdat_sections = comdat.sections();
    assert_eq!(comdat_sections.next(), Some(section1_index));
    assert_eq!(comdat_sections.next(), Some(section2_index));
    assert_eq!(comdat_sections.next(), None);
}
