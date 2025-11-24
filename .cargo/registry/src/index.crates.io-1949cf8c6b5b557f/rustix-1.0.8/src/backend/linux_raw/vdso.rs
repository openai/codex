//! Parse the Linux vDSO.
//!
//! The following code is transliterated from
//! tools/testing/selftests/vDSO/parse_vdso.c in Linux 6.13, which is licensed
//! with Creative Commons Zero License, version 1.0,
//! available at <https://creativecommons.org/publicdomain/zero/1.0/legalcode>
//!
//! It also incorporates the patch at:
//! <https://git.kernel.org/pub/scm/linux/kernel/git/shuah/linux-kselftest.git/commit/tools/testing/selftests/vDSO?h=next&id=01587d80b04f29747b6fd6d766c3bfa632f14eb0>,
//! with changes to fix the pointer arithmetic on s390x.
//!
//! # Safety
//!
//! Parsing the vDSO involves a lot of raw pointer manipulation. This
//! implementation follows Linux's reference implementation, and adds several
//! additional safety checks.
#![allow(unsafe_code)]

use super::c;
use crate::ffi::CStr;
use crate::utils::check_raw_pointer;
use core::ffi::c_void;
use core::mem::size_of;
use core::ptr::{null, null_mut};
use linux_raw_sys::elf::*;

#[cfg(target_arch = "s390x")]
type ElfHashEntry = u64;
#[cfg(not(target_arch = "s390x"))]
type ElfHashEntry = u32;

pub(super) struct Vdso {
    // Load information
    load_addr: *const Elf_Ehdr,
    load_end: *const c_void, // the end of the `PT_LOAD` segment
    pv_offset: usize,        // recorded paddr - recorded vaddr

    // Symbol table
    symtab: *const Elf_Sym,
    symstrings: *const u8,
    gnu_hash: *const u32,
    bucket: *const ElfHashEntry,
    chain: *const ElfHashEntry,
    nbucket: ElfHashEntry,
    //nchain: ElfHashEntry,

    // Version table
    versym: *const u16,
    verdef: *const Elf_Verdef,
}

/// Straight from the ELF specification…and then tweaked slightly, in order to
/// avoid a few clang warnings.
/// (And then translated to Rust).
fn elf_hash(name: &CStr) -> u32 {
    let mut h: u32 = 0;
    for b in name.to_bytes() {
        h = (h << 4).wrapping_add(u32::from(*b));
        let g = h & 0xf000_0000;
        if g != 0 {
            h ^= g >> 24;
        }
        h &= !g;
    }
    h
}

fn gnu_hash(name: &CStr) -> u32 {
    let mut h: u32 = 5381;
    for s in name.to_bytes() {
        h = h
            .wrapping_add(h.wrapping_mul(32))
            .wrapping_add(u32::from(*s));
    }
    h
}

/// Create a `Vdso` value by parsing the vDSO at the `sysinfo_ehdr` address.
fn init_from_sysinfo_ehdr() -> Option<Vdso> {
    // SAFETY: The auxv initialization code does extensive checks to ensure
    // that the value we get really is an `AT_SYSINFO_EHDR` value from the
    // kernel.
    unsafe {
        let hdr = super::param::auxv::sysinfo_ehdr();

        // If the platform doesn't provide a `AT_SYSINFO_EHDR`, we can't locate
        // the vDSO.
        if hdr.is_null() {
            return None;
        }

        let mut vdso = Vdso {
            load_addr: hdr,
            load_end: hdr.cast(),
            pv_offset: 0,
            symtab: null(),
            symstrings: null(),
            gnu_hash: null(),
            bucket: null(),
            chain: null(),
            nbucket: 0,
            //nchain: 0,
            versym: null(),
            verdef: null(),
        };

        let hdr = &*hdr;
        let pt = check_raw_pointer::<Elf_Phdr>(vdso.base_plus(hdr.e_phoff)? as *mut _)?.as_ptr();
        let mut dyn_: *const Elf_Dyn = null();
        let mut num_dyn = 0;

        // We need two things from the segment table: the load offset
        // and the dynamic table.
        let mut found_vaddr = false;
        for i in 0..hdr.e_phnum {
            let phdr = &*pt.add(i as usize);
            if phdr.p_type == PT_LOAD && !found_vaddr {
                // The segment should be readable and executable, because it
                // contains the symbol table and the function bodies.
                if phdr.p_flags & (PF_R | PF_X) != (PF_R | PF_X) {
                    return None;
                }
                found_vaddr = true;
                vdso.load_end = vdso.base_plus(phdr.p_offset.checked_add(phdr.p_memsz)?)?;
                vdso.pv_offset = phdr.p_offset.wrapping_sub(phdr.p_vaddr);
            } else if phdr.p_type == PT_DYNAMIC {
                // If `p_offset` is zero, it's more likely that we're looking
                // at memory that has been zeroed than that the kernel has
                // somehow aliased the `Ehdr` and the `Elf_Dyn` array.
                if phdr.p_offset < size_of::<Elf_Ehdr>() {
                    return None;
                }

                dyn_ = check_raw_pointer::<Elf_Dyn>(vdso.base_plus(phdr.p_offset)? as *mut _)?
                    .as_ptr();
                num_dyn = phdr.p_memsz / size_of::<Elf_Dyn>();
            } else if phdr.p_type == PT_INTERP || phdr.p_type == PT_GNU_RELRO {
                // Don't trust any ELF image that has an “interpreter” or
                // that uses RELRO, which is likely to be a user ELF image
                // rather and not the kernel vDSO.
                return None;
            }
        }

        if !found_vaddr || dyn_.is_null() {
            return None; // Failed
        }

        // Fish out the useful bits of the dynamic table.
        let mut hash: *const ElfHashEntry = null();
        vdso.symstrings = null();
        vdso.symtab = null();
        vdso.versym = null();
        vdso.verdef = null();
        let mut i = 0;
        loop {
            if i == num_dyn {
                return None;
            }
            let d = &*dyn_.add(i);
            match d.d_tag {
                DT_STRTAB => {
                    vdso.symstrings =
                        check_raw_pointer::<u8>(vdso.addr_from_elf(d.d_un.d_ptr)? as *mut _)?
                            .as_ptr();
                }
                DT_SYMTAB => {
                    vdso.symtab =
                        check_raw_pointer::<Elf_Sym>(vdso.addr_from_elf(d.d_un.d_ptr)? as *mut _)?
                            .as_ptr();
                }
                DT_HASH => {
                    hash = check_raw_pointer::<ElfHashEntry>(
                        vdso.addr_from_elf(d.d_un.d_ptr)? as *mut _
                    )?
                    .as_ptr();
                }
                DT_GNU_HASH => {
                    vdso.gnu_hash =
                        check_raw_pointer::<u32>(vdso.addr_from_elf(d.d_un.d_ptr)? as *mut _)?
                            .as_ptr()
                }
                DT_VERSYM => {
                    vdso.versym =
                        check_raw_pointer::<u16>(vdso.addr_from_elf(d.d_un.d_ptr)? as *mut _)?
                            .as_ptr();
                }
                DT_VERDEF => {
                    vdso.verdef = check_raw_pointer::<Elf_Verdef>(
                        vdso.addr_from_elf(d.d_un.d_ptr)? as *mut _,
                    )?
                    .as_ptr();
                }
                DT_SYMENT => {
                    if d.d_un.d_ptr != size_of::<Elf_Sym>() {
                        return None; // Failed
                    }
                }
                DT_NULL => break,
                _ => {}
            }
            i = i.checked_add(1)?;
        }
        // `check_raw_pointer` will have checked these pointers for null,
        // however they could still be null if the expected dynamic table
        // entries are absent.
        if vdso.symstrings.is_null()
            || vdso.symtab.is_null()
            || (hash.is_null() && vdso.gnu_hash.is_null())
        {
            return None; // Failed
        }

        if vdso.verdef.is_null() {
            vdso.versym = null();
        }

        // Parse the hash table header.
        if !vdso.gnu_hash.is_null() {
            vdso.nbucket = ElfHashEntry::from(*vdso.gnu_hash);
            // The bucket array is located after the header (4 uint32) and the
            // bloom filter (size_t array of gnu_hash[2] elements).
            vdso.bucket = vdso
                .gnu_hash
                .add(4)
                .add(size_of::<c::size_t>() / 4 * *vdso.gnu_hash.add(2) as usize)
                .cast();
        } else {
            vdso.nbucket = *hash.add(0);
            //vdso.nchain = *hash.add(1);
            vdso.bucket = hash.add(2);
            vdso.chain = hash.add(vdso.nbucket as usize + 2);
        }

        // That's all we need.
        Some(vdso)
    }
}

impl Vdso {
    /// Parse the vDSO.
    ///
    /// Returns `None` if the vDSO can't be located or if it doesn't conform to
    /// our expectations.
    #[inline]
    pub(super) fn new() -> Option<Self> {
        init_from_sysinfo_ehdr()
    }

    /// Check the version for a symbol.
    ///
    /// # Safety
    ///
    /// The raw pointers inside `self` must be valid.
    unsafe fn match_version(&self, mut ver: u16, name: &CStr, hash: u32) -> bool {
        // This is a helper function to check if the version indexed by
        // ver matches name (which hashes to hash).
        //
        // The version definition table is a mess, and I don't know how
        // to do this in better than linear time without allocating memory
        // to build an index. I also don't know why the table has
        // variable size entries in the first place.
        //
        // For added fun, I can't find a comprehensible specification of how
        // to parse all the weird flags in the table.
        //
        // So I just parse the whole table every time.

        // First step: find the version definition
        ver &= 0x7fff; // Apparently bit 15 means "hidden"
        let mut def = self.verdef;
        loop {
            if (*def).vd_version != VER_DEF_CURRENT {
                return false; // Failed
            }

            if ((*def).vd_flags & VER_FLG_BASE) == 0 && ((*def).vd_ndx & 0x7fff) == ver {
                break;
            }

            if (*def).vd_next == 0 {
                return false; // No definition.
            }

            def = def
                .cast::<u8>()
                .add((*def).vd_next as usize)
                .cast::<Elf_Verdef>();
        }

        // Now figure out whether it matches.
        let aux = &*(def.cast::<u8>())
            .add((*def).vd_aux as usize)
            .cast::<Elf_Verdaux>();
        (*def).vd_hash == hash
            && (name == CStr::from_ptr(self.symstrings.add(aux.vda_name as usize).cast()))
    }

    /// Check to see if the symbol is the one we're looking for.
    ///
    /// # Safety
    ///
    /// The raw pointers inside `self` must be valid.
    unsafe fn check_sym(
        &self,
        sym: &Elf_Sym,
        i: ElfHashEntry,
        name: &CStr,
        version: &CStr,
        ver_hash: u32,
    ) -> bool {
        // Check for a defined global or weak function w/ right name.
        //
        // Accept `STT_NOTYPE` in addition to `STT_FUNC` for the symbol
        // type, for compatibility with some versions of Linux on
        // PowerPC64. See [this commit] in Linux for more background.
        //
        // [this commit]: https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/commit/tools/testing/selftests/vDSO/parse_vdso.c?id=0161bd38c24312853ed5ae9a425a1c41c4ac674a
        if ELF_ST_TYPE(sym.st_info) != STT_FUNC && ELF_ST_TYPE(sym.st_info) != STT_NOTYPE {
            return false;
        }
        if ELF_ST_BIND(sym.st_info) != STB_GLOBAL && ELF_ST_BIND(sym.st_info) != STB_WEAK {
            return false;
        }
        if name != CStr::from_ptr(self.symstrings.add(sym.st_name as usize).cast()) {
            return false;
        }

        // Check symbol version.
        if !self.versym.is_null()
            && !self.match_version(*self.versym.add(i as usize), version, ver_hash)
        {
            return false;
        }

        true
    }

    /// Look up a symbol in the vDSO.
    pub(super) fn sym(&self, version: &CStr, name: &CStr) -> *mut c::c_void {
        let ver_hash = elf_hash(version);

        // SAFETY: The pointers in `self` must be valid.
        unsafe {
            if !self.gnu_hash.is_null() {
                let mut h1: u32 = gnu_hash(name);

                // Changes to fix the pointer arithmetic on s390x: cast
                // `self.bucket` to `*const u32` here, because even though
                // s390x's `ElfHashEntry` is 64-bit for `DT_HASH` tables,
                // it uses 32-bit entries for `DT_GNU_HASH` tables.
                let mut i = *self
                    .bucket
                    .cast::<u32>()
                    .add((ElfHashEntry::from(h1) % self.nbucket) as usize);
                if i == 0 {
                    return null_mut();
                }
                h1 |= 1;
                // Changes to fix the pointer arithmetic on s390x: As above,
                // cast `self.bucket` to `*const u32`.
                let mut hashval = self
                    .bucket
                    .cast::<u32>()
                    .add(self.nbucket as usize)
                    .add((i - *self.gnu_hash.add(1)) as usize);
                loop {
                    let sym: &Elf_Sym = &*self.symtab.add(i as usize);
                    let h2 = *hashval;
                    hashval = hashval.add(1);
                    if h1 == (h2 | 1)
                        && self.check_sym(sym, ElfHashEntry::from(i), name, version, ver_hash)
                    {
                        let sum = self.addr_from_elf(sym.st_value).unwrap();
                        assert!(
                            sum as usize >= self.load_addr as usize
                                && sum as usize <= self.load_end as usize
                        );
                        return sum as *mut c::c_void;
                    }
                    if (h2 & 1) != 0 {
                        break;
                    }
                    i += 1;
                }
            } else {
                let mut i = *self
                    .bucket
                    .add((ElfHashEntry::from(elf_hash(name)) % self.nbucket) as usize);
                while i != 0 {
                    let sym: &Elf_Sym = &*self.symtab.add(i as usize);
                    if sym.st_shndx != SHN_UNDEF && self.check_sym(sym, i, name, version, ver_hash)
                    {
                        let sum = self.addr_from_elf(sym.st_value).unwrap();
                        assert!(
                            sum as usize >= self.load_addr as usize
                                && sum as usize <= self.load_end as usize
                        );
                        return sum as *mut c::c_void;
                    }
                    i = *self.chain.add(i as usize);
                }
            }
        }

        null_mut()
    }

    /// Add the given address to the vDSO base address.
    unsafe fn base_plus(&self, offset: usize) -> Option<*const c_void> {
        // Check for overflow.
        let _ = (self.load_addr as usize).checked_add(offset)?;
        // Add the offset to the base.
        Some(self.load_addr.cast::<u8>().add(offset).cast())
    }

    /// Translate an ELF-address-space address into a usable virtual address.
    unsafe fn addr_from_elf(&self, elf_addr: usize) -> Option<*const c_void> {
        self.base_plus(elf_addr.wrapping_add(self.pv_offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Disable on MIPS since QEMU on MIPS doesn't provide a vDSO.
    #[cfg(linux_raw)]
    #[test]
    #[cfg_attr(any(target_arch = "mips", target_arch = "mips64"), ignore)]
    #[allow(unused_variables)]
    fn test_vdso() {
        let vdso = Vdso::new().unwrap();
        assert!(!vdso.symtab.is_null());
        assert!(!vdso.symstrings.is_null());

        {
            #[cfg(target_arch = "x86_64")]
            let ptr = vdso.sym(cstr!("LINUX_2.6"), cstr!("__vdso_clock_gettime"));
            #[cfg(target_arch = "arm")]
            let ptr = vdso.sym(cstr!("LINUX_2.6"), cstr!("__vdso_clock_gettime64"));
            #[cfg(target_arch = "aarch64")]
            let ptr = vdso.sym(cstr!("LINUX_2.6.39"), cstr!("__kernel_clock_gettime"));
            #[cfg(target_arch = "x86")]
            let ptr = vdso.sym(cstr!("LINUX_2.6"), cstr!("__vdso_clock_gettime64"));
            #[cfg(target_arch = "riscv64")]
            let ptr = vdso.sym(cstr!("LINUX_4.15"), cstr!("__vdso_clock_gettime"));
            #[cfg(target_arch = "powerpc")]
            let _ptr = vdso.sym(cstr!("LINUX_5.11"), cstr!("__kernel_clock_gettime64"));
            #[cfg(target_arch = "powerpc64")]
            let ptr = vdso.sym(cstr!("LINUX_2.6.15"), cstr!("__kernel_clock_gettime"));
            #[cfg(target_arch = "s390x")]
            let ptr = vdso.sym(cstr!("LINUX_2.6.29"), cstr!("__kernel_clock_gettime"));
            #[cfg(any(target_arch = "mips", target_arch = "mips32r6"))]
            let ptr = vdso.sym(cstr!("LINUX_2.6"), cstr!("__vdso_clock_gettime64"));
            #[cfg(any(target_arch = "mips64", target_arch = "mips64r6"))]
            let ptr = vdso.sym(cstr!("LINUX_2.6"), cstr!("__vdso_clock_gettime"));

            // On PowerPC, "__kernel_clock_gettime64" isn't available in
            // Linux < 5.11.
            // On x86, "__vdso_clock_gettime64" isn't available in
            // Linux < 5.3.
            #[cfg(not(any(target_arch = "powerpc", target_arch = "x86")))]
            assert!(!ptr.is_null());
        }

        {
            #[cfg(target_arch = "x86_64")]
            let ptr = vdso.sym(cstr!("LINUX_2.6"), cstr!("__vdso_clock_getres"));
            #[cfg(target_arch = "arm")]
            let ptr = vdso.sym(cstr!("LINUX_2.6"), cstr!("__vdso_clock_getres"));
            #[cfg(target_arch = "aarch64")]
            let ptr = vdso.sym(cstr!("LINUX_2.6.39"), cstr!("__kernel_clock_getres"));
            #[cfg(target_arch = "x86")]
            let ptr = vdso.sym(cstr!("LINUX_2.6"), cstr!("__vdso_clock_getres"));
            #[cfg(target_arch = "riscv64")]
            let ptr = vdso.sym(cstr!("LINUX_4.15"), cstr!("__vdso_clock_getres"));
            #[cfg(any(target_arch = "powerpc", target_arch = "powerpc64"))]
            let ptr = vdso.sym(cstr!("LINUX_2.6.15"), cstr!("__kernel_clock_getres"));
            #[cfg(target_arch = "s390x")]
            let ptr = vdso.sym(cstr!("LINUX_2.6.29"), cstr!("__kernel_clock_getres"));
            #[cfg(any(target_arch = "mips", target_arch = "mips32r6"))]
            let ptr = vdso.sym(cstr!("LINUX_2.6"), cstr!("__vdso_clock_getres"));
            #[cfg(any(target_arch = "mips64", target_arch = "mips64r6"))]
            let ptr = vdso.sym(cstr!("LINUX_2.6"), cstr!("__vdso_clock_getres"));

            // Some versions of Linux appear to lack "__vdso_clock_getres" on x86.
            #[cfg(not(target_arch = "x86"))]
            assert!(!ptr.is_null());
        }

        {
            #[cfg(target_arch = "x86_64")]
            let ptr = vdso.sym(cstr!("LINUX_2.6"), cstr!("__vdso_gettimeofday"));
            #[cfg(target_arch = "arm")]
            let ptr = vdso.sym(cstr!("LINUX_2.6"), cstr!("__vdso_gettimeofday"));
            #[cfg(target_arch = "aarch64")]
            let ptr = vdso.sym(cstr!("LINUX_2.6.39"), cstr!("__kernel_gettimeofday"));
            #[cfg(target_arch = "x86")]
            let ptr = vdso.sym(cstr!("LINUX_2.6"), cstr!("__vdso_gettimeofday"));
            #[cfg(target_arch = "riscv64")]
            let ptr = vdso.sym(cstr!("LINUX_4.15"), cstr!("__vdso_gettimeofday"));
            #[cfg(any(target_arch = "powerpc", target_arch = "powerpc64"))]
            let ptr = vdso.sym(cstr!("LINUX_2.6.15"), cstr!("__kernel_gettimeofday"));
            #[cfg(target_arch = "s390x")]
            let ptr = vdso.sym(cstr!("LINUX_2.6.29"), cstr!("__kernel_gettimeofday"));
            #[cfg(any(target_arch = "mips", target_arch = "mips32r6"))]
            let ptr = vdso.sym(cstr!("LINUX_2.6"), cstr!("__vdso_gettimeofday"));
            #[cfg(any(target_arch = "mips64", target_arch = "mips64r6"))]
            let ptr = vdso.sym(cstr!("LINUX_2.6"), cstr!("__vdso_gettimeofday"));

            // Some versions of Linux appear to lack "__vdso_gettimeofday" on x86.
            #[cfg(not(target_arch = "x86"))]
            assert!(!ptr.is_null());
        }

        #[cfg(any(
            target_arch = "x86_64",
            target_arch = "x86",
            target_arch = "riscv64",
            target_arch = "powerpc",
            target_arch = "powerpc64",
            target_arch = "s390x",
        ))]
        {
            #[cfg(target_arch = "x86_64")]
            let ptr = vdso.sym(cstr!("LINUX_2.6"), cstr!("__vdso_getcpu"));
            #[cfg(target_arch = "x86")]
            let ptr = vdso.sym(cstr!("LINUX_2.6"), cstr!("__vdso_getcpu"));
            #[cfg(target_arch = "riscv64")]
            let ptr = vdso.sym(cstr!("LINUX_4.15"), cstr!("__vdso_getcpu"));
            #[cfg(target_arch = "powerpc")]
            let ptr = vdso.sym(cstr!("LINUX_2.6.15"), cstr!("__kernel_getcpu"));
            #[cfg(target_arch = "powerpc64")]
            let ptr = vdso.sym(cstr!("LINUX_2.6.15"), cstr!("__kernel_getcpu"));
            #[cfg(target_arch = "s390x")]
            let ptr = vdso.sym(cstr!("LINUX_2.6.29"), cstr!("__kernel_getcpu"));

            // On PowerPC, "__kernel_getcpu" isn't available in 32-bit kernels.
            // Some versions of Linux appear to lack "__vdso_getcpu" on x86.
            #[cfg(not(any(target_arch = "powerpc", target_arch = "x86")))]
            assert!(!ptr.is_null());
        }
    }
}
