pub type time_t = i64;

pub type Elf_Addr = crate::Elf64_Addr;
pub type Elf_Half = crate::Elf64_Half;
pub type Elf_Phdr = crate::Elf64_Phdr;

s! {
    pub struct Elf64_Phdr {
        pub p_type: crate::Elf64_Word,
        pub p_flags: crate::Elf64_Word,
        pub p_offset: crate::Elf64_Off,
        pub p_vaddr: crate::Elf64_Addr,
        pub p_paddr: crate::Elf64_Addr,
        pub p_filesz: crate::Elf64_Xword,
        pub p_memsz: crate::Elf64_Xword,
        pub p_align: crate::Elf64_Xword,
    }
}
