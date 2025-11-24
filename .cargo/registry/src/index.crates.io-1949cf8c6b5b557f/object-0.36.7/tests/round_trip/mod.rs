#![cfg(all(feature = "read", feature = "write"))]

use object::read::{Object, ObjectSection, ObjectSymbol};
use object::{read, write, SectionIndex, SubArchitecture};
use object::{
    Architecture, BinaryFormat, Endianness, RelocationEncoding, RelocationFlags, RelocationKind,
    SectionKind, SymbolFlags, SymbolKind, SymbolScope, SymbolSection,
};

mod bss;
mod coff;
mod comdat;
mod common;
mod elf;
mod macho;
mod section_flags;
mod tls;

#[test]
fn coff_any() {
    for (arch, sub_arch) in [
        (Architecture::Aarch64, None),
        (Architecture::Aarch64, Some(SubArchitecture::Arm64EC)),
        (Architecture::Arm, None),
        (Architecture::I386, None),
        (Architecture::X86_64, None),
    ]
    .iter()
    .copied()
    {
        let mut object = write::Object::new(BinaryFormat::Coff, arch, Endianness::Little);
        object.set_sub_architecture(sub_arch);

        object.add_file_symbol(b"file.c".to_vec());

        let text = object.section_id(write::StandardSection::Text);
        object.append_section_data(text, &[1; 30], 4);

        let func1_offset = object.append_section_data(text, &[1; 30], 4);
        assert_eq!(func1_offset, 32);
        let func1_symbol = object.add_symbol(write::Symbol {
            name: b"func1".to_vec(),
            value: func1_offset,
            size: 32,
            kind: SymbolKind::Text,
            scope: SymbolScope::Linkage,
            weak: false,
            section: write::SymbolSection::Section(text),
            flags: SymbolFlags::None,
        });
        let func2_offset = object.append_section_data(text, &[1; 30], 4);
        assert_eq!(func2_offset, 64);
        object.add_symbol(write::Symbol {
            name: b"func2_long".to_vec(),
            value: func2_offset,
            size: 32,
            kind: SymbolKind::Text,
            scope: SymbolScope::Linkage,
            weak: false,
            section: write::SymbolSection::Section(text),
            flags: SymbolFlags::None,
        });
        object
            .add_relocation(
                text,
                write::Relocation {
                    offset: 8,
                    symbol: func1_symbol,
                    addend: 0,
                    flags: RelocationFlags::Generic {
                        kind: RelocationKind::Absolute,
                        encoding: RelocationEncoding::Generic,
                        size: arch.address_size().unwrap().bytes() * 8,
                    },
                },
            )
            .unwrap();

        let bytes = object.write().unwrap();
        let object = read::File::parse(&*bytes).unwrap();
        assert_eq!(object.format(), BinaryFormat::Coff);
        assert_eq!(object.architecture(), arch);
        assert_eq!(object.sub_architecture(), sub_arch);
        assert_eq!(object.endianness(), Endianness::Little);

        let mut sections = object.sections();

        let text = sections.next().unwrap();
        println!("{:?}", text);
        let text_index = text.index();
        assert_eq!(text.name(), Ok(".text"));
        assert_eq!(text.kind(), SectionKind::Text);
        assert_eq!(text.address(), 0);
        assert_eq!(text.size(), 94);
        assert_eq!(&text.data().unwrap()[..30], &[1; 30]);
        assert_eq!(&text.data().unwrap()[32..62], &[1; 30]);

        let mut symbols = object.symbols();

        let symbol = symbols.next().unwrap();
        println!("{:?}", symbol);
        assert_eq!(symbol.name(), Ok("file.c"));
        assert_eq!(symbol.address(), 0);
        assert_eq!(symbol.kind(), SymbolKind::File);
        assert_eq!(symbol.section(), SymbolSection::None);
        assert_eq!(symbol.scope(), SymbolScope::Compilation);
        assert!(!symbol.is_weak());

        let decorated_name = |name: &str| {
            if arch == Architecture::I386 {
                format!("_{name}")
            } else {
                name.to_owned()
            }
        };

        let symbol = symbols.next().unwrap();
        println!("{:?}", symbol);
        let func1_symbol = symbol.index();
        assert_eq!(symbol.name(), Ok(decorated_name("func1").as_str()));
        assert_eq!(symbol.address(), func1_offset);
        assert_eq!(symbol.kind(), SymbolKind::Text);
        assert_eq!(symbol.section_index(), Some(text_index));
        assert_eq!(symbol.scope(), SymbolScope::Linkage);
        assert!(!symbol.is_weak());
        assert!(!symbol.is_undefined());

        let symbol = symbols.next().unwrap();
        println!("{:?}", symbol);
        assert_eq!(symbol.name(), Ok(decorated_name("func2_long").as_str()));
        assert_eq!(symbol.address(), func2_offset);
        assert_eq!(symbol.kind(), SymbolKind::Text);
        assert_eq!(symbol.section_index(), Some(text_index));
        assert_eq!(symbol.scope(), SymbolScope::Linkage);
        assert!(!symbol.is_weak());
        assert!(!symbol.is_undefined());

        let mut relocations = text.relocations();

        let (offset, relocation) = relocations.next().unwrap();
        println!("{:?}", relocation);
        assert_eq!(offset, 8);
        assert_eq!(relocation.kind(), RelocationKind::Absolute);
        assert_eq!(relocation.encoding(), RelocationEncoding::Generic);
        assert_eq!(relocation.size(), arch.address_size().unwrap().bytes() * 8);
        assert_eq!(
            relocation.target(),
            read::RelocationTarget::Symbol(func1_symbol)
        );
        assert_eq!(relocation.addend(), 0);

        let map = object.symbol_map();
        let symbol = map.get(func1_offset + 1).unwrap();
        assert_eq!(symbol.address(), func1_offset);
        assert_eq!(symbol.name(), decorated_name("func1"));
        assert_eq!(map.get(func1_offset - 1), None);
    }
}

#[test]
fn elf_x86_64() {
    let mut object =
        write::Object::new(BinaryFormat::Elf, Architecture::X86_64, Endianness::Little);

    object.add_file_symbol(b"file.c".to_vec());

    let text = object.section_id(write::StandardSection::Text);
    object.append_section_data(text, &[1; 30], 4);

    let func1_offset = object.append_section_data(text, &[1; 30], 4);
    assert_eq!(func1_offset, 32);
    let func1_symbol = object.add_symbol(write::Symbol {
        name: b"func1".to_vec(),
        value: func1_offset,
        size: 32,
        kind: SymbolKind::Text,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Section(text),
        flags: SymbolFlags::None,
    });
    object
        .add_relocation(
            text,
            write::Relocation {
                offset: 8,
                symbol: func1_symbol,
                addend: 0,
                flags: RelocationFlags::Generic {
                    kind: RelocationKind::Absolute,
                    encoding: RelocationEncoding::Generic,
                    size: 64,
                },
            },
        )
        .unwrap();

    let bytes = object.write().unwrap();
    let object = read::File::parse(&*bytes).unwrap();
    assert_eq!(object.format(), BinaryFormat::Elf);
    assert_eq!(object.architecture(), Architecture::X86_64);
    assert_eq!(object.endianness(), Endianness::Little);

    let mut sections = object.sections();

    let text = sections.next().unwrap();
    println!("{:?}", text);
    let text_index = text.index();
    assert_eq!(text.name(), Ok(".text"));
    assert_eq!(text.kind(), SectionKind::Text);
    assert_eq!(text.address(), 0);
    assert_eq!(text.size(), 62);
    assert_eq!(&text.data().unwrap()[..30], &[1; 30]);
    assert_eq!(&text.data().unwrap()[32..62], &[1; 30]);

    let mut symbols = object.symbols();

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    assert_eq!(symbol.name(), Ok("file.c"));
    assert_eq!(symbol.address(), 0);
    assert_eq!(symbol.kind(), SymbolKind::File);
    assert_eq!(symbol.section(), SymbolSection::None);
    assert_eq!(symbol.scope(), SymbolScope::Compilation);
    assert!(!symbol.is_weak());

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    let func1_symbol = symbol.index();
    assert_eq!(symbol.name(), Ok("func1"));
    assert_eq!(symbol.address(), func1_offset);
    assert_eq!(symbol.kind(), SymbolKind::Text);
    assert_eq!(symbol.section_index(), Some(text_index));
    assert_eq!(symbol.scope(), SymbolScope::Linkage);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());

    let mut relocations = text.relocations();

    let (offset, relocation) = relocations.next().unwrap();
    println!("{:?}", relocation);
    assert_eq!(offset, 8);
    assert_eq!(relocation.kind(), RelocationKind::Absolute);
    assert_eq!(relocation.encoding(), RelocationEncoding::Generic);
    assert_eq!(relocation.size(), 64);
    assert_eq!(
        relocation.target(),
        read::RelocationTarget::Symbol(func1_symbol)
    );
    assert_eq!(relocation.addend(), 0);

    let map = object.symbol_map();
    let symbol = map.get(func1_offset + 1).unwrap();
    assert_eq!(symbol.address(), func1_offset);
    assert_eq!(symbol.name(), "func1");
    assert_eq!(map.get(func1_offset - 1), None);
}

#[test]
fn elf_any() {
    for (arch, endian) in [
        (Architecture::Aarch64, Endianness::Little),
        (Architecture::Aarch64_Ilp32, Endianness::Little),
        (Architecture::Arm, Endianness::Little),
        (Architecture::Avr, Endianness::Little),
        (Architecture::Bpf, Endianness::Little),
        (Architecture::Csky, Endianness::Little),
        (Architecture::E2K32, Endianness::Little),
        (Architecture::E2K64, Endianness::Little),
        (Architecture::I386, Endianness::Little),
        (Architecture::X86_64, Endianness::Little),
        (Architecture::X86_64_X32, Endianness::Little),
        (Architecture::Hexagon, Endianness::Little),
        (Architecture::LoongArch64, Endianness::Little),
        (Architecture::M68k, Endianness::Big),
        (Architecture::Mips, Endianness::Little),
        (Architecture::Mips64, Endianness::Little),
        (Architecture::Mips64_N32, Endianness::Little),
        (Architecture::Msp430, Endianness::Little),
        (Architecture::PowerPc, Endianness::Big),
        (Architecture::PowerPc64, Endianness::Big),
        (Architecture::Riscv32, Endianness::Little),
        (Architecture::Riscv64, Endianness::Little),
        (Architecture::S390x, Endianness::Big),
        (Architecture::Sbf, Endianness::Little),
        (Architecture::Sparc, Endianness::Big),
        (Architecture::Sparc32Plus, Endianness::Big),
        (Architecture::Sparc64, Endianness::Big),
        (Architecture::Xtensa, Endianness::Little),
    ]
    .iter()
    .copied()
    {
        let mut object = write::Object::new(BinaryFormat::Elf, arch, endian);

        let section = object.section_id(write::StandardSection::Data);
        object.append_section_data(section, &[1; 30], 4);
        let symbol = object.section_symbol(section);

        object
            .add_relocation(
                section,
                write::Relocation {
                    offset: 8,
                    symbol,
                    addend: 0,
                    flags: RelocationFlags::Generic {
                        kind: RelocationKind::Absolute,
                        encoding: RelocationEncoding::Generic,
                        size: 32,
                    },
                },
            )
            .unwrap();
        if arch.address_size().unwrap().bytes() >= 8 {
            object
                .add_relocation(
                    section,
                    write::Relocation {
                        offset: 16,
                        symbol,
                        addend: 0,
                        flags: RelocationFlags::Generic {
                            kind: RelocationKind::Absolute,
                            encoding: RelocationEncoding::Generic,
                            size: 64,
                        },
                    },
                )
                .unwrap();
        }

        let bytes = object.write().unwrap();
        let object = read::File::parse(&*bytes).unwrap();
        println!("{:?}", object.architecture());
        assert_eq!(object.format(), BinaryFormat::Elf);
        assert_eq!(object.architecture(), arch);
        assert_eq!(object.endianness(), endian);

        let mut sections = object.sections();

        let data = sections.next().unwrap();
        println!("{:?}", data);
        assert_eq!(data.name(), Ok(".data"));
        assert_eq!(data.kind(), SectionKind::Data);

        let mut relocations = data.relocations();

        let (offset, relocation) = relocations.next().unwrap();
        println!("{:?}", relocation);
        assert_eq!(offset, 8);
        assert_eq!(relocation.kind(), RelocationKind::Absolute);
        assert_eq!(relocation.encoding(), RelocationEncoding::Generic);
        assert_eq!(relocation.size(), 32);
        assert_eq!(relocation.addend(), 0);

        if arch.address_size().unwrap().bytes() >= 8 {
            let (offset, relocation) = relocations.next().unwrap();
            println!("{:?}", relocation);
            assert_eq!(offset, 16);
            assert_eq!(relocation.kind(), RelocationKind::Absolute);
            assert_eq!(relocation.encoding(), RelocationEncoding::Generic);
            assert_eq!(relocation.size(), 64);
            assert_eq!(relocation.addend(), 0);
        }
    }
}

#[test]
fn macho_x86_64() {
    let mut object = write::Object::new(
        BinaryFormat::MachO,
        Architecture::X86_64,
        Endianness::Little,
    );

    object.add_file_symbol(b"file.c".to_vec());

    let text = object.section_id(write::StandardSection::Text);
    object.append_section_data(text, &[1; 30], 4);

    let func1_offset = object.append_section_data(text, &[1; 30], 4);
    assert_eq!(func1_offset, 32);
    let func1_symbol = object.add_symbol(write::Symbol {
        name: b"func1".to_vec(),
        value: func1_offset,
        size: 32,
        kind: SymbolKind::Text,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Section(text),
        flags: SymbolFlags::None,
    });
    object
        .add_relocation(
            text,
            write::Relocation {
                offset: 8,
                symbol: func1_symbol,
                addend: 0,
                flags: RelocationFlags::Generic {
                    kind: RelocationKind::Absolute,
                    encoding: RelocationEncoding::Generic,
                    size: 64,
                },
            },
        )
        .unwrap();
    object
        .add_relocation(
            text,
            write::Relocation {
                offset: 16,
                symbol: func1_symbol,
                addend: -4,
                flags: RelocationFlags::Generic {
                    kind: RelocationKind::Relative,
                    encoding: RelocationEncoding::Generic,
                    size: 32,
                },
            },
        )
        .unwrap();

    let bytes = object.write().unwrap();
    let object = read::File::parse(&*bytes).unwrap();
    assert_eq!(object.format(), BinaryFormat::MachO);
    assert_eq!(object.architecture(), Architecture::X86_64);
    assert_eq!(object.endianness(), Endianness::Little);

    let mut sections = object.sections();

    let text = sections.next().unwrap();
    println!("{:?}", text);
    let text_index = text.index();
    assert_eq!(text.name(), Ok("__text"));
    assert_eq!(text.segment_name(), Ok(Some("__TEXT")));
    assert_eq!(text.kind(), SectionKind::Text);
    assert_eq!(text.address(), 0);
    assert_eq!(text.size(), 62);
    assert_eq!(&text.data().unwrap()[..30], &[1; 30]);
    assert_eq!(&text.data().unwrap()[32..62], &[1; 30]);

    let mut symbols = object.symbols();

    let symbol = symbols.next().unwrap();
    println!("{:?}", symbol);
    let func1_symbol = symbol.index();
    assert_eq!(symbol.name(), Ok("_func1"));
    assert_eq!(symbol.address(), func1_offset);
    assert_eq!(symbol.kind(), SymbolKind::Text);
    assert_eq!(symbol.section_index(), Some(text_index));
    assert_eq!(symbol.scope(), SymbolScope::Linkage);
    assert!(!symbol.is_weak());
    assert!(!symbol.is_undefined());

    let mut relocations = text.relocations();

    let (offset, relocation) = relocations.next().unwrap();
    println!("{:?}", relocation);
    assert_eq!(offset, 16);
    assert_eq!(relocation.kind(), RelocationKind::Relative);
    assert_eq!(relocation.encoding(), RelocationEncoding::X86RipRelative);
    assert_eq!(relocation.size(), 32);
    assert_eq!(
        relocation.target(),
        read::RelocationTarget::Symbol(func1_symbol)
    );
    assert_eq!(relocation.addend(), -4);

    let (offset, relocation) = relocations.next().unwrap();
    println!("{:?}", relocation);
    assert_eq!(offset, 8);
    assert_eq!(relocation.kind(), RelocationKind::Absolute);
    assert_eq!(relocation.encoding(), RelocationEncoding::Generic);
    assert_eq!(relocation.size(), 64);
    assert_eq!(
        relocation.target(),
        read::RelocationTarget::Symbol(func1_symbol)
    );
    assert_eq!(relocation.addend(), 0);

    let map = object.symbol_map();
    let symbol = map.get(func1_offset + 1).unwrap();
    assert_eq!(symbol.address(), func1_offset);
    assert_eq!(symbol.name(), "_func1");
    assert_eq!(map.get(func1_offset - 1), None);
}

#[test]
fn macho_any() {
    for (arch, subarch, endian) in [
        (Architecture::Aarch64, None, Endianness::Little),
        (
            Architecture::Aarch64,
            Some(SubArchitecture::Arm64E),
            Endianness::Little,
        ),
        (Architecture::Aarch64_Ilp32, None, Endianness::Little),
        /* TODO:
        (Architecture::Arm, None, Endianness::Little),
        */
        (Architecture::I386, None, Endianness::Little),
        (Architecture::X86_64, None, Endianness::Little),
        /* TODO:
        (Architecture::PowerPc, None, Endianness::Big),
        (Architecture::PowerPc64, None, Endianness::Big),
        */
    ]
    .iter()
    .copied()
    {
        let mut object = write::Object::new(BinaryFormat::MachO, arch, endian);
        object.set_sub_architecture(subarch);

        let section = object.section_id(write::StandardSection::Data);
        object.append_section_data(section, &[1; 30], 4);
        let symbol = object.section_symbol(section);

        object
            .add_relocation(
                section,
                write::Relocation {
                    offset: 8,
                    symbol,
                    addend: 0,
                    flags: RelocationFlags::Generic {
                        kind: RelocationKind::Absolute,
                        encoding: RelocationEncoding::Generic,
                        size: 32,
                    },
                },
            )
            .unwrap();
        if arch.address_size().unwrap().bytes() >= 8 {
            object
                .add_relocation(
                    section,
                    write::Relocation {
                        offset: 16,
                        symbol,
                        addend: 0,
                        flags: RelocationFlags::Generic {
                            kind: RelocationKind::Absolute,
                            encoding: RelocationEncoding::Generic,
                            size: 64,
                        },
                    },
                )
                .unwrap();
        }

        let bytes = object.write().unwrap();
        let object = read::File::parse(&*bytes).unwrap();
        println!("{:?}", object.architecture());
        assert_eq!(object.format(), BinaryFormat::MachO);
        assert_eq!(object.architecture(), arch);
        assert_eq!(object.sub_architecture(), subarch);
        assert_eq!(object.endianness(), endian);

        let mut sections = object.sections();

        let data = sections.next().unwrap();
        println!("{:?}", data);
        assert_eq!(data.segment_name(), Ok(Some("__DATA")));
        assert_eq!(data.name(), Ok("__data"));
        assert_eq!(data.kind(), SectionKind::Data);

        let mut relocations = data.relocations();

        if arch.address_size().unwrap().bytes() >= 8 {
            let (offset, relocation) = relocations.next().unwrap();
            println!("{:?}", relocation);
            assert_eq!(offset, 16);
            assert_eq!(relocation.kind(), RelocationKind::Absolute);
            assert_eq!(relocation.encoding(), RelocationEncoding::Generic);
            assert_eq!(relocation.size(), 64);
            assert_eq!(relocation.addend(), 0);
        }

        let (offset, relocation) = relocations.next().unwrap();
        println!("{:?}", relocation);
        assert_eq!(offset, 8);
        assert_eq!(relocation.kind(), RelocationKind::Absolute);
        assert_eq!(relocation.encoding(), RelocationEncoding::Generic);
        assert_eq!(relocation.size(), 32);
        assert_eq!(relocation.addend(), 0);
    }
}

#[cfg(feature = "xcoff")]
#[test]
fn xcoff_powerpc() {
    for arch in [Architecture::PowerPc, Architecture::PowerPc64] {
        let mut object = write::Object::new(BinaryFormat::Xcoff, arch, Endianness::Big);

        object.add_file_symbol(b"file.c".to_vec());

        let text = object.section_id(write::StandardSection::Text);
        object.append_section_data(text, &[1; 30], 4);

        let func1_offset = object.append_section_data(text, &[1; 30], 4);
        assert_eq!(func1_offset, 32);
        let func1_symbol = object.add_symbol(write::Symbol {
            name: b"func1".to_vec(),
            value: func1_offset,
            size: 32,
            kind: SymbolKind::Text,
            scope: SymbolScope::Linkage,
            weak: false,
            section: write::SymbolSection::Section(text),
            flags: SymbolFlags::None,
        });

        object
            .add_relocation(
                text,
                write::Relocation {
                    offset: 8,
                    symbol: func1_symbol,
                    addend: 0,
                    flags: RelocationFlags::Generic {
                        kind: RelocationKind::Absolute,
                        encoding: RelocationEncoding::Generic,
                        size: 64,
                    },
                },
            )
            .unwrap();

        let bytes = object.write().unwrap();
        let object = read::File::parse(&*bytes).unwrap();
        assert_eq!(object.format(), BinaryFormat::Xcoff);
        assert_eq!(object.architecture(), arch);
        assert_eq!(object.endianness(), Endianness::Big);

        let mut sections = object.sections();

        let text = sections.next().unwrap();
        println!("{:?}", text);
        let text_index = text.index().0;
        assert_eq!(text.name(), Ok(".text"));
        assert_eq!(text.kind(), SectionKind::Text);
        assert_eq!(text.address(), 0);
        assert_eq!(text.size(), 62);
        assert_eq!(&text.data().unwrap()[..30], &[1; 30]);
        assert_eq!(&text.data().unwrap()[32..62], &[1; 30]);

        let mut symbols = object.symbols();

        let mut symbol = symbols.next().unwrap();
        println!("{:?}", symbol);
        assert_eq!(symbol.name(), Ok("file.c"));
        assert_eq!(symbol.address(), 0);
        assert_eq!(symbol.kind(), SymbolKind::File);
        assert_eq!(symbol.section_index(), None);
        assert_eq!(symbol.scope(), SymbolScope::Compilation);
        assert!(!symbol.is_weak());
        assert!(!symbol.is_undefined());

        symbol = symbols.next().unwrap();
        println!("{:?}", symbol);
        let func1_symbol = symbol.index();
        assert_eq!(symbol.name(), Ok("func1"));
        assert_eq!(symbol.address(), func1_offset);
        assert_eq!(symbol.kind(), SymbolKind::Text);
        assert_eq!(symbol.section_index(), Some(SectionIndex(text_index)));
        assert_eq!(symbol.scope(), SymbolScope::Linkage);
        assert!(!symbol.is_weak());
        assert!(!symbol.is_undefined());

        let mut relocations = text.relocations();

        let (offset, relocation) = relocations.next().unwrap();
        println!("{:?}", relocation);
        assert_eq!(offset, 8);
        assert_eq!(relocation.kind(), RelocationKind::Absolute);
        assert_eq!(relocation.encoding(), RelocationEncoding::Generic);
        assert_eq!(relocation.size(), 64);
        assert_eq!(
            relocation.target(),
            read::RelocationTarget::Symbol(func1_symbol)
        );
        assert_eq!(relocation.addend(), 0);
    }
}
