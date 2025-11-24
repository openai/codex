//! The ELF ABI. 🧝
//!
//! This module is not as comprehensive as bindgened [`elf_uapi`] and provides only types for target
//! pointer width: instead of [`elf32_phdr`] and [`elf64_phdr`] there's only [`Elf_Phdr`].
//!
//! [`elf_uapi`]: super::elf_uapi
//! [`elf32_phdr`]: super::elf_uapi::elf32_phdr
//! [`elf64_phdr`]: super::elf_uapi::elf64_phdr

pub const SELFMAG: usize = 4;
pub const ELFMAG: [u8; SELFMAG] = [0x7f, b'E', b'L', b'F'];
pub const EI_CLASS: usize = 4;
pub const EI_DATA: usize = 5;
pub const EI_VERSION: usize = 6;
pub const EI_OSABI: usize = 7;
pub const EI_ABIVERSION: usize = 8;
pub const EV_CURRENT: u8 = 1;
#[cfg(target_pointer_width = "32")]
pub const ELFCLASS: u8 = 1; // ELFCLASS32
#[cfg(target_pointer_width = "64")]
pub const ELFCLASS: u8 = 2; // ELFCLASS64
#[cfg(target_endian = "little")]
pub const ELFDATA: u8 = 1; // ELFDATA2LSB
#[cfg(target_endian = "big")]
pub const ELFDATA: u8 = 2; // ELFDATA2MSB
pub const ELFOSABI_SYSV: u8 = 0;
pub const ELFOSABI_LINUX: u8 = 3;
// At present all of our supported platforms use 0.
pub const ELFABIVERSION: u8 = 0;
pub const ET_DYN: u16 = 3;
pub const EI_NIDENT: usize = 16;
pub const SHN_UNDEF: u16 = 0;
pub const SHN_ABS: u16 = 0xfff1;
pub const PN_XNUM: u16 = 0xffff;
pub const PT_LOAD: u32 = 1;
pub const PT_DYNAMIC: u32 = 2;
pub const PT_INTERP: u32 = 3;
pub const PT_PHDR: u32 = 6;
pub const PT_TLS: u32 = 7;
pub const PT_GNU_STACK: u32 = 0x6474_e551;
pub const PT_GNU_RELRO: u32 = 0x6474_e552;
pub const PF_X: u32 = 1;
pub const PF_W: u32 = 2;
pub const PF_R: u32 = 4;
pub const DT_NULL: usize = 0;
pub const DT_HASH: usize = 4;
pub const DT_STRTAB: usize = 5;
pub const DT_SYMTAB: usize = 6;
pub const DT_RELA: usize = 7;
pub const DT_RELASZ: usize = 8;
pub const DT_RELAENT: usize = 9;
pub const DT_REL: usize = 17;
pub const DT_RELSZ: usize = 18;
pub const DT_RELENT: usize = 19;
pub const DT_SYMENT: usize = 11;
pub const DT_GNU_HASH: usize = 0x6fff_fef5;
pub const DT_VERSYM: usize = 0x6fff_fff0;
pub const DT_VERDEF: usize = 0x6fff_fffc;
pub const STB_WEAK: u8 = 2;
pub const STB_GLOBAL: u8 = 1;
pub const STT_NOTYPE: u8 = 0;
pub const STT_FUNC: u8 = 2;
pub const STN_UNDEF: u32 = 0;
pub const VER_FLG_BASE: u16 = 0x1;
pub const VER_DEF_CURRENT: u16 = 1;
pub const STV_DEFAULT: u8 = 0;
#[cfg(target_arch = "arm")]
pub const EM_CURRENT: u16 = 40; // EM_ARM
#[cfg(target_arch = "x86")]
pub const EM_CURRENT: u16 = 3; // EM_386
#[cfg(target_arch = "powerpc64")]
pub const EM_CURRENT: u16 = 21; // EM_PPC64
#[cfg(any(
    target_arch = "mips",
    target_arch = "mips32r6",
    target_arch = "mips64",
    target_arch = "mips64r6"
))]
pub const EM_CURRENT: u16 = 8; // EM_MIPS
#[cfg(target_arch = "x86_64")]
pub const EM_CURRENT: u16 = 62; // EM_X86_64
#[cfg(target_arch = "aarch64")]
pub const EM_CURRENT: u16 = 183; // EM_AARCH64
#[cfg(target_arch = "riscv64")]
pub const EM_CURRENT: u16 = 243; // EM_RISCV

#[inline]
pub const fn ELF_ST_VISIBILITY(o: u8) -> u8 {
    o & 0x03
}

#[inline]
pub const fn ELF_ST_BIND(val: u8) -> u8 {
    val >> 4
}

#[inline]
pub const fn ELF_ST_TYPE(val: u8) -> u8 {
    val & 0xf
}

#[repr(C)]
pub struct Elf_Ehdr {
    pub e_ident: [u8; EI_NIDENT],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: usize,
    pub e_phoff: usize,
    pub e_shoff: usize,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

#[cfg(target_pointer_width = "32")]
#[repr(C)]
pub struct Elf_Phdr {
    pub p_type: u32,
    pub p_offset: usize,
    pub p_vaddr: usize,
    pub p_paddr: usize,
    pub p_filesz: usize,
    pub p_memsz: usize,
    pub p_flags: u32,
    pub p_align: usize,
}

#[cfg(target_pointer_width = "64")]
#[repr(C)]
pub struct Elf_Phdr {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: usize,
    pub p_vaddr: usize,
    pub p_paddr: usize,
    pub p_filesz: usize,
    pub p_memsz: usize,
    pub p_align: usize,
}

#[cfg(target_pointer_width = "32")]
#[repr(C)]
pub struct Elf_Sym {
    pub st_name: u32,
    pub st_value: usize,
    pub st_size: usize,
    pub st_info: u8,
    pub st_other: u8,
    pub st_shndx: u16,
}

#[cfg(target_pointer_width = "64")]
#[repr(C)]
pub struct Elf_Sym {
    pub st_name: u32,
    pub st_info: u8,
    pub st_other: u8,
    pub st_shndx: u16,
    pub st_value: usize,
    pub st_size: usize,
}

#[repr(C)]
pub struct Elf_Verdef {
    pub vd_version: u16,
    pub vd_flags: u16,
    pub vd_ndx: u16,
    pub vd_cnt: u16,
    pub vd_hash: u32,
    pub vd_aux: u32,
    pub vd_next: u32,
}

#[repr(C)]
pub struct Elf_Verdaux {
    pub vda_name: u32,
    pub _vda_next: u32,
}

#[cfg(target_pointer_width = "32")]
#[repr(C)]
#[derive(Copy, Clone)]
pub struct Elf_Dyn {
    pub d_tag: usize,
    pub d_un: Elf_Dyn_Union,
}

#[cfg(target_pointer_width = "32")]
#[repr(C)]
#[derive(Copy, Clone)]
pub union Elf_Dyn_Union {
    pub d_val: u32,
    pub d_ptr: usize,
}

#[cfg(target_pointer_width = "64")]
#[repr(C)]
#[derive(Copy, Clone)]
pub struct Elf_Dyn {
    pub d_tag: usize,
    pub d_un: Elf_Dyn_Union,
}

#[cfg(target_pointer_width = "64")]
#[repr(C)]
#[derive(Copy, Clone)]
pub union Elf_Dyn_Union {
    pub d_val: u64,
    pub d_ptr: usize,
}

#[cfg(target_pointer_width = "32")]
#[repr(C)]
pub struct Elf_Rela {
    pub r_offset: usize,
    pub r_info: u32,
    pub r_addend: usize,
}

#[cfg(target_pointer_width = "64")]
#[repr(C)]
pub struct Elf_Rela {
    pub r_offset: usize,
    pub r_info: u64,
    pub r_addend: usize,
}

impl Elf_Rela {
    #[inline]
    pub fn type_(&self) -> u32 {
        #[cfg(target_pointer_width = "32")]
        {
            self.r_info & 0xff
        }
        #[cfg(target_pointer_width = "64")]
        {
            (self.r_info & 0xffff_ffff) as u32
        }
    }
}

#[cfg(target_pointer_width = "32")]
#[repr(C)]
pub struct Elf_Rel {
    pub r_offset: usize,
    pub r_info: u32,
}

#[cfg(target_pointer_width = "64")]
#[repr(C)]
pub struct Elf_Rel {
    pub r_offset: usize,
    pub r_info: u64,
}

impl Elf_Rel {
    #[inline]
    pub fn type_(&self) -> u32 {
        #[cfg(target_pointer_width = "32")]
        {
            self.r_info & 0xff
        }
        #[cfg(target_pointer_width = "64")]
        {
            (self.r_info & 0xffff_ffff) as u32
        }
    }
}

#[cfg(target_arch = "x86_64")]
pub const R_RELATIVE: u32 = 8; // `R_X86_64_RELATIVE`
#[cfg(target_arch = "x86")]
pub const R_RELATIVE: u32 = 8; // `R_386_RELATIVE`
#[cfg(target_arch = "aarch64")]
pub const R_RELATIVE: u32 = 1027; // `R_AARCH64_RELATIVE`
#[cfg(target_arch = "riscv64")]
pub const R_RELATIVE: u32 = 3; // `R_RISCV_RELATIVE`
#[cfg(target_arch = "arm")]
pub const R_RELATIVE: u32 = 23; // `R_ARM_RELATIVE`

#[repr(C)]
#[derive(Clone)]
pub struct Elf_auxv_t {
    pub a_type: usize,

    // Some of the values in the auxv array are pointers, so we make `a_val` a
    // pointer, in order to preserve their provenance. For the values which are
    // integers, we cast this to `usize`.
    pub a_val: *mut crate::ctypes::c_void,
}
