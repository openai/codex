pub type time_t = i32;

pub type Elf_Addr = crate::Elf32_Addr;
pub type Elf_Half = crate::Elf32_Half;
pub type Elf_Phdr = crate::Elf32_Phdr;

s! {
    pub struct Elf32_Phdr {
        pub p_type: crate::Elf32_Word,
        pub p_offset: crate::Elf32_Off,
        pub p_vaddr: crate::Elf32_Addr,
        pub p_paddr: crate::Elf32_Addr,
        pub p_filesz: crate::Elf32_Word,
        pub p_memsz: crate::Elf32_Word,
        pub p_flags: crate::Elf32_Word,
        pub p_align: crate::Elf32_Word,
    }
}
