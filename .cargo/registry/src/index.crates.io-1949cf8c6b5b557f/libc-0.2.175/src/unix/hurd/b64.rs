use crate::prelude::*;

pub type __int64_t = c_long;
pub type __uint64_t = c_ulong;

pub type int_fast16_t = c_long;
pub type int_fast32_t = c_long;
pub type int_fast64_t = c_long;
pub type uint_fast16_t = c_ulong;
pub type uint_fast32_t = c_ulong;
pub type uint_fast64_t = c_ulong;

pub type __quad_t = c_long;
pub type __u_quad_t = c_ulong;
pub type __intmax_t = c_long;
pub type __uintmax_t = c_ulong;

pub type __squad_type = c_long;
pub type __uquad_type = c_ulong;
pub type __sword_type = c_long;
pub type __uword_type = c_ulong;
pub type __slong32_type = c_int;
pub type __ulong32_type = c_uint;
pub type __s64_type = c_long;
pub type __u64_type = c_ulong;

pub type __ipc_pid_t = c_int;

pub type Elf64_Half = u16;
pub type Elf64_Word = u32;
pub type Elf64_Off = u64;
pub type Elf64_Addr = u64;
pub type Elf64_Xword = u64;
pub type Elf64_Sxword = i64;
pub type Elf64_Section = u16;

pub type Elf_Addr = crate::Elf64_Addr;
pub type Elf_Half = crate::Elf64_Half;
pub type Elf_Ehdr = crate::Elf64_Ehdr;
pub type Elf_Phdr = crate::Elf64_Phdr;
pub type Elf_Shdr = crate::Elf64_Shdr;
pub type Elf_Sym = crate::Elf64_Sym;

s! {
    pub struct Elf64_Ehdr {
        pub e_ident: [c_uchar; 16],
        pub e_type: Elf64_Half,
        pub e_machine: Elf64_Half,
        pub e_version: Elf64_Word,
        pub e_entry: Elf64_Addr,
        pub e_phoff: Elf64_Off,
        pub e_shoff: Elf64_Off,
        pub e_flags: Elf64_Word,
        pub e_ehsize: Elf64_Half,
        pub e_phentsize: Elf64_Half,
        pub e_phnum: Elf64_Half,
        pub e_shentsize: Elf64_Half,
        pub e_shnum: Elf64_Half,
        pub e_shstrndx: Elf64_Half,
    }

    pub struct Elf64_Shdr {
        pub sh_name: Elf64_Word,
        pub sh_type: Elf64_Word,
        pub sh_flags: Elf64_Xword,
        pub sh_addr: Elf64_Addr,
        pub sh_offset: Elf64_Off,
        pub sh_size: Elf64_Xword,
        pub sh_link: Elf64_Word,
        pub sh_info: Elf64_Word,
        pub sh_addralign: Elf64_Xword,
        pub sh_entsize: Elf64_Xword,
    }

    pub struct Elf64_Sym {
        pub st_name: Elf64_Word,
        pub st_info: c_uchar,
        pub st_other: c_uchar,
        pub st_shndx: Elf64_Section,
        pub st_value: Elf64_Addr,
        pub st_size: Elf64_Xword,
    }

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
