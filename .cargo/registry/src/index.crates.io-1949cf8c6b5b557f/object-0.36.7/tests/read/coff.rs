use object::{pe, read, Object, ObjectSection};
use std::fs;
use std::path::PathBuf;

#[cfg(feature = "coff")]
#[test]
fn coff_extended_relocations() {
    let path_to_obj: PathBuf = ["testfiles", "coff", "relocs_overflow.o"].iter().collect();
    let contents = fs::read(path_to_obj).expect("Could not read relocs_overflow.o");
    let file =
        read::coff::CoffFile::<_>::parse(&contents[..]).expect("Could not parse relocs_overflow.o");
    let code_section = file
        .section_by_name(".text")
        .expect("Could not find .text section in relocs_overflow.o");
    match code_section.flags() {
        object::SectionFlags::Coff { characteristics } => {
            assert!(characteristics & pe::IMAGE_SCN_LNK_NRELOC_OVFL != 0)
        }
        _ => panic!("Invalid section flags flavour."),
    };
    let relocations = code_section.relocations().collect::<Vec<_>>();
    assert_eq!(relocations.len(), 65536);
}
