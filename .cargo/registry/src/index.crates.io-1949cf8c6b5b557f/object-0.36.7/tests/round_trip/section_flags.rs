#![cfg(all(feature = "read", feature = "write"))]

use object::read::{Object, ObjectSection};
use object::{read, write};
use object::{Architecture, BinaryFormat, Endianness, SectionFlags, SectionKind};

#[test]
fn coff_x86_64_section_flags() {
    let mut object =
        write::Object::new(BinaryFormat::Coff, Architecture::X86_64, Endianness::Little);

    let section = object.add_section(Vec::new(), b".text".to_vec(), SectionKind::Text);
    object.section_mut(section).flags = SectionFlags::Coff {
        characteristics: object::pe::IMAGE_SCN_MEM_WRITE,
    };

    let bytes = object.write().unwrap();

    let object = read::File::parse(&*bytes).unwrap();
    assert_eq!(object.format(), BinaryFormat::Coff);
    assert_eq!(object.architecture(), Architecture::X86_64);

    let mut sections = object.sections();
    let section = sections.next().unwrap();
    assert_eq!(section.name(), Ok(".text"));
    assert_eq!(
        section.flags(),
        SectionFlags::Coff {
            characteristics: object::pe::IMAGE_SCN_MEM_WRITE | object::pe::IMAGE_SCN_ALIGN_1BYTES,
        }
    );
}

#[test]
fn elf_x86_64_section_flags() {
    let mut object =
        write::Object::new(BinaryFormat::Elf, Architecture::X86_64, Endianness::Little);

    let section = object.add_section(Vec::new(), b".text".to_vec(), SectionKind::Text);
    object.section_mut(section).flags = SectionFlags::Elf {
        sh_flags: object::elf::SHF_WRITE.into(),
    };

    let bytes = object.write().unwrap();

    let object = read::File::parse(&*bytes).unwrap();
    assert_eq!(object.format(), BinaryFormat::Elf);
    assert_eq!(object.architecture(), Architecture::X86_64);

    let mut sections = object.sections();
    let section = sections.next().unwrap();
    assert_eq!(section.name(), Ok(".text"));
    assert_eq!(
        section.flags(),
        SectionFlags::Elf {
            sh_flags: object::elf::SHF_WRITE.into(),
        }
    );
}

#[test]
fn macho_x86_64_section_flags() {
    let mut object = write::Object::new(
        BinaryFormat::MachO,
        Architecture::X86_64,
        Endianness::Little,
    );

    let section = object.add_section(Vec::new(), b".text".to_vec(), SectionKind::Text);
    object.section_mut(section).flags = SectionFlags::MachO {
        flags: object::macho::S_ATTR_SELF_MODIFYING_CODE,
    };

    let bytes = object.write().unwrap();

    let object = read::File::parse(&*bytes).unwrap();
    assert_eq!(object.format(), BinaryFormat::MachO);
    assert_eq!(object.architecture(), Architecture::X86_64);

    let mut sections = object.sections();
    let section = sections.next().unwrap();
    assert_eq!(section.name(), Ok(".text"));
    assert_eq!(
        section.flags(),
        SectionFlags::MachO {
            flags: object::macho::S_ATTR_SELF_MODIFYING_CODE,
        }
    );
}
