use object::read::{Object, ObjectSection};
use object::{read, write};
use object::{
    Architecture, BinaryFormat, Endianness, RelocationEncoding, RelocationFlags, RelocationKind,
    SymbolFlags, SymbolKind, SymbolScope,
};

#[test]
fn reloc_overflow() {
    let mut object =
        write::Object::new(BinaryFormat::Coff, Architecture::X86_64, Endianness::Little);
    let text = object.section_id(write::StandardSection::Text);
    object.append_section_data(text, &[0; 4], 4);
    let symbol = object.add_symbol(write::Symbol {
        name: b"f".to_vec(),
        value: 0,
        size: 4,
        kind: SymbolKind::Text,
        scope: SymbolScope::Linkage,
        weak: false,
        section: write::SymbolSection::Section(text),
        flags: SymbolFlags::None,
    });
    for i in 0..0x10000 {
        object
            .add_relocation(
                text,
                write::Relocation {
                    offset: i,
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

    //std::fs::write(&"reloc_overflow.o", &bytes).unwrap();

    let object = read::File::parse(&*bytes).unwrap();
    assert_eq!(object.format(), BinaryFormat::Coff);
    assert_eq!(object.architecture(), Architecture::X86_64);

    let section = object.sections().next().unwrap();
    assert_eq!(section.name(), Ok(".text"));

    let mut i = 0;
    for (offset, _relocation) in section.relocations() {
        assert_eq!(offset, i);
        i += 1;
    }
    assert_eq!(i, 0x10000);
}
