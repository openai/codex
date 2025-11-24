use crate::prelude::*;

pub type Elf32_Addr = c_ulong;
pub type Elf32_Half = c_ushort;
pub type Elf32_Off = c_ulong;
pub type Elf32_Sword = c_long;
pub type Elf32_Word = c_ulong;
pub type Elf32_Lword = c_ulonglong;
pub type Elf32_Phdr = __c_anonymous_Elf32_Phdr;

s! {
    pub struct __c_anonymous_Elf32_Phdr {
        pub p_type: Elf32_Word,
        pub p_offset: Elf32_Off,
        pub p_vaddr: Elf32_Addr,
        pub p_paddr: Elf32_Addr,
        pub p_filesz: Elf32_Word,
        pub p_memsz: Elf32_Word,
        pub p_flags: Elf32_Word,
        pub p_align: Elf32_Word,
    }

    pub struct dl_phdr_info {
        pub dlpi_addr: Elf32_Addr,
        pub dlpi_name: *const c_char,
        pub dlpi_phdr: *const Elf32_Phdr,
        pub dlpi_phnum: Elf32_Half,
        pub dlpi_adds: c_ulonglong,
        pub dlpi_subs: c_ulonglong,
    }
}
