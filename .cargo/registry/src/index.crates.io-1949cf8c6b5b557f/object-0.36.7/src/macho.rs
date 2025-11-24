//! Mach-O definitions.
//!
//! These definitions are independent of read/write support, although we do implement
//! some traits useful for those.
//!
//! This module is based heavily on header files from MacOSX11.1.sdk.

#![allow(missing_docs)]

use crate::endian::{BigEndian, Endian, U64Bytes, U16, U32, U64};
use crate::pod::Pod;

// Definitions from "/usr/include/mach/machine.h".

/*
 * Capability bits used in the definition of cpu_type.
 */

/// mask for architecture bits
pub const CPU_ARCH_MASK: u32 = 0xff00_0000;
/// 64 bit ABI
pub const CPU_ARCH_ABI64: u32 = 0x0100_0000;
/// ABI for 64-bit hardware with 32-bit types; LP32
pub const CPU_ARCH_ABI64_32: u32 = 0x0200_0000;

/*
 *	Machine types known by all.
 */

pub const CPU_TYPE_ANY: u32 = !0;

pub const CPU_TYPE_VAX: u32 = 1;
pub const CPU_TYPE_MC680X0: u32 = 6;
pub const CPU_TYPE_X86: u32 = 7;
pub const CPU_TYPE_X86_64: u32 = CPU_TYPE_X86 | CPU_ARCH_ABI64;
pub const CPU_TYPE_MIPS: u32 = 8;
pub const CPU_TYPE_MC98000: u32 = 10;
pub const CPU_TYPE_HPPA: u32 = 11;
pub const CPU_TYPE_ARM: u32 = 12;
pub const CPU_TYPE_ARM64: u32 = CPU_TYPE_ARM | CPU_ARCH_ABI64;
pub const CPU_TYPE_ARM64_32: u32 = CPU_TYPE_ARM | CPU_ARCH_ABI64_32;
pub const CPU_TYPE_MC88000: u32 = 13;
pub const CPU_TYPE_SPARC: u32 = 14;
pub const CPU_TYPE_I860: u32 = 15;
pub const CPU_TYPE_ALPHA: u32 = 16;
pub const CPU_TYPE_POWERPC: u32 = 18;
pub const CPU_TYPE_POWERPC64: u32 = CPU_TYPE_POWERPC | CPU_ARCH_ABI64;

/*
 * Capability bits used in the definition of cpu_subtype.
 */
/// mask for feature flags
pub const CPU_SUBTYPE_MASK: u32 = 0xff00_0000;
/// 64 bit libraries
pub const CPU_SUBTYPE_LIB64: u32 = 0x8000_0000;
/// pointer authentication with versioned ABI
pub const CPU_SUBTYPE_PTRAUTH_ABI: u32 = 0x8000_0000;

/// When selecting a slice, ANY will pick the slice with the best
/// grading for the selected cpu_type_t, unlike the "ALL" subtypes,
/// which are the slices that can run on any hardware for that cpu type.
pub const CPU_SUBTYPE_ANY: u32 = !0;

/*
 *	Object files that are hand-crafted to run on any
 *	implementation of an architecture are tagged with
 *	CPU_SUBTYPE_MULTIPLE.  This functions essentially the same as
 *	the "ALL" subtype of an architecture except that it allows us
 *	to easily find object files that may need to be modified
 *	whenever a new implementation of an architecture comes out.
 *
 *	It is the responsibility of the implementor to make sure the
 *	software handles unsupported implementations elegantly.
 */
pub const CPU_SUBTYPE_MULTIPLE: u32 = !0;
pub const CPU_SUBTYPE_LITTLE_ENDIAN: u32 = 0;
pub const CPU_SUBTYPE_BIG_ENDIAN: u32 = 1;

/*
 *	VAX subtypes (these do *not* necessary conform to the actual cpu
 *	ID assigned by DEC available via the SID register).
 */

pub const CPU_SUBTYPE_VAX_ALL: u32 = 0;
pub const CPU_SUBTYPE_VAX780: u32 = 1;
pub const CPU_SUBTYPE_VAX785: u32 = 2;
pub const CPU_SUBTYPE_VAX750: u32 = 3;
pub const CPU_SUBTYPE_VAX730: u32 = 4;
pub const CPU_SUBTYPE_UVAXI: u32 = 5;
pub const CPU_SUBTYPE_UVAXII: u32 = 6;
pub const CPU_SUBTYPE_VAX8200: u32 = 7;
pub const CPU_SUBTYPE_VAX8500: u32 = 8;
pub const CPU_SUBTYPE_VAX8600: u32 = 9;
pub const CPU_SUBTYPE_VAX8650: u32 = 10;
pub const CPU_SUBTYPE_VAX8800: u32 = 11;
pub const CPU_SUBTYPE_UVAXIII: u32 = 12;

/*
 *      680x0 subtypes
 *
 * The subtype definitions here are unusual for historical reasons.
 * NeXT used to consider 68030 code as generic 68000 code.  For
 * backwards compatibility:
 *
 *	CPU_SUBTYPE_MC68030 symbol has been preserved for source code
 *	compatibility.
 *
 *	CPU_SUBTYPE_MC680x0_ALL has been defined to be the same
 *	subtype as CPU_SUBTYPE_MC68030 for binary comatability.
 *
 *	CPU_SUBTYPE_MC68030_ONLY has been added to allow new object
 *	files to be tagged as containing 68030-specific instructions.
 */

pub const CPU_SUBTYPE_MC680X0_ALL: u32 = 1;
// compat
pub const CPU_SUBTYPE_MC68030: u32 = 1;
pub const CPU_SUBTYPE_MC68040: u32 = 2;
pub const CPU_SUBTYPE_MC68030_ONLY: u32 = 3;

/*
 *	I386 subtypes
 */

#[inline]
pub const fn cpu_subtype_intel(f: u32, m: u32) -> u32 {
    f + (m << 4)
}

pub const CPU_SUBTYPE_I386_ALL: u32 = cpu_subtype_intel(3, 0);
pub const CPU_SUBTYPE_386: u32 = cpu_subtype_intel(3, 0);
pub const CPU_SUBTYPE_486: u32 = cpu_subtype_intel(4, 0);
pub const CPU_SUBTYPE_486SX: u32 = cpu_subtype_intel(4, 8);
pub const CPU_SUBTYPE_586: u32 = cpu_subtype_intel(5, 0);
pub const CPU_SUBTYPE_PENT: u32 = cpu_subtype_intel(5, 0);
pub const CPU_SUBTYPE_PENTPRO: u32 = cpu_subtype_intel(6, 1);
pub const CPU_SUBTYPE_PENTII_M3: u32 = cpu_subtype_intel(6, 3);
pub const CPU_SUBTYPE_PENTII_M5: u32 = cpu_subtype_intel(6, 5);
pub const CPU_SUBTYPE_CELERON: u32 = cpu_subtype_intel(7, 6);
pub const CPU_SUBTYPE_CELERON_MOBILE: u32 = cpu_subtype_intel(7, 7);
pub const CPU_SUBTYPE_PENTIUM_3: u32 = cpu_subtype_intel(8, 0);
pub const CPU_SUBTYPE_PENTIUM_3_M: u32 = cpu_subtype_intel(8, 1);
pub const CPU_SUBTYPE_PENTIUM_3_XEON: u32 = cpu_subtype_intel(8, 2);
pub const CPU_SUBTYPE_PENTIUM_M: u32 = cpu_subtype_intel(9, 0);
pub const CPU_SUBTYPE_PENTIUM_4: u32 = cpu_subtype_intel(10, 0);
pub const CPU_SUBTYPE_PENTIUM_4_M: u32 = cpu_subtype_intel(10, 1);
pub const CPU_SUBTYPE_ITANIUM: u32 = cpu_subtype_intel(11, 0);
pub const CPU_SUBTYPE_ITANIUM_2: u32 = cpu_subtype_intel(11, 1);
pub const CPU_SUBTYPE_XEON: u32 = cpu_subtype_intel(12, 0);
pub const CPU_SUBTYPE_XEON_MP: u32 = cpu_subtype_intel(12, 1);

#[inline]
pub const fn cpu_subtype_intel_family(x: u32) -> u32 {
    x & 15
}
pub const CPU_SUBTYPE_INTEL_FAMILY_MAX: u32 = 15;

#[inline]
pub const fn cpu_subtype_intel_model(x: u32) -> u32 {
    x >> 4
}
pub const CPU_SUBTYPE_INTEL_MODEL_ALL: u32 = 0;

/*
 *	X86 subtypes.
 */

pub const CPU_SUBTYPE_X86_ALL: u32 = 3;
pub const CPU_SUBTYPE_X86_64_ALL: u32 = 3;
pub const CPU_SUBTYPE_X86_ARCH1: u32 = 4;
/// Haswell feature subset
pub const CPU_SUBTYPE_X86_64_H: u32 = 8;

/*
 *	Mips subtypes.
 */

pub const CPU_SUBTYPE_MIPS_ALL: u32 = 0;
pub const CPU_SUBTYPE_MIPS_R2300: u32 = 1;
pub const CPU_SUBTYPE_MIPS_R2600: u32 = 2;
pub const CPU_SUBTYPE_MIPS_R2800: u32 = 3;
/// pmax
pub const CPU_SUBTYPE_MIPS_R2000A: u32 = 4;
pub const CPU_SUBTYPE_MIPS_R2000: u32 = 5;
/// 3max
pub const CPU_SUBTYPE_MIPS_R3000A: u32 = 6;
pub const CPU_SUBTYPE_MIPS_R3000: u32 = 7;

/*
 *	MC98000 (PowerPC) subtypes
 */
pub const CPU_SUBTYPE_MC98000_ALL: u32 = 0;
pub const CPU_SUBTYPE_MC98601: u32 = 1;

/*
 *	HPPA subtypes for Hewlett-Packard HP-PA family of
 *	risc processors. Port by NeXT to 700 series.
 */

pub const CPU_SUBTYPE_HPPA_ALL: u32 = 0;
pub const CPU_SUBTYPE_HPPA_7100LC: u32 = 1;

/*
 *	MC88000 subtypes.
 */
pub const CPU_SUBTYPE_MC88000_ALL: u32 = 0;
pub const CPU_SUBTYPE_MC88100: u32 = 1;
pub const CPU_SUBTYPE_MC88110: u32 = 2;

/*
 *	SPARC subtypes
 */
pub const CPU_SUBTYPE_SPARC_ALL: u32 = 0;

/*
 *	I860 subtypes
 */
pub const CPU_SUBTYPE_I860_ALL: u32 = 0;
pub const CPU_SUBTYPE_I860_860: u32 = 1;

/*
 *	PowerPC subtypes
 */
pub const CPU_SUBTYPE_POWERPC_ALL: u32 = 0;
pub const CPU_SUBTYPE_POWERPC_601: u32 = 1;
pub const CPU_SUBTYPE_POWERPC_602: u32 = 2;
pub const CPU_SUBTYPE_POWERPC_603: u32 = 3;
pub const CPU_SUBTYPE_POWERPC_603E: u32 = 4;
pub const CPU_SUBTYPE_POWERPC_603EV: u32 = 5;
pub const CPU_SUBTYPE_POWERPC_604: u32 = 6;
pub const CPU_SUBTYPE_POWERPC_604E: u32 = 7;
pub const CPU_SUBTYPE_POWERPC_620: u32 = 8;
pub const CPU_SUBTYPE_POWERPC_750: u32 = 9;
pub const CPU_SUBTYPE_POWERPC_7400: u32 = 10;
pub const CPU_SUBTYPE_POWERPC_7450: u32 = 11;
pub const CPU_SUBTYPE_POWERPC_970: u32 = 100;

/*
 *	ARM subtypes
 */
pub const CPU_SUBTYPE_ARM_ALL: u32 = 0;
pub const CPU_SUBTYPE_ARM_V4T: u32 = 5;
pub const CPU_SUBTYPE_ARM_V6: u32 = 6;
pub const CPU_SUBTYPE_ARM_V5TEJ: u32 = 7;
pub const CPU_SUBTYPE_ARM_XSCALE: u32 = 8;
/// ARMv7-A and ARMv7-R
pub const CPU_SUBTYPE_ARM_V7: u32 = 9;
/// Cortex A9
pub const CPU_SUBTYPE_ARM_V7F: u32 = 10;
/// Swift
pub const CPU_SUBTYPE_ARM_V7S: u32 = 11;
pub const CPU_SUBTYPE_ARM_V7K: u32 = 12;
pub const CPU_SUBTYPE_ARM_V8: u32 = 13;
/// Not meant to be run under xnu
pub const CPU_SUBTYPE_ARM_V6M: u32 = 14;
/// Not meant to be run under xnu
pub const CPU_SUBTYPE_ARM_V7M: u32 = 15;
/// Not meant to be run under xnu
pub const CPU_SUBTYPE_ARM_V7EM: u32 = 16;
/// Not meant to be run under xnu
pub const CPU_SUBTYPE_ARM_V8M: u32 = 17;

/*
 *  ARM64 subtypes
 */
pub const CPU_SUBTYPE_ARM64_ALL: u32 = 0;
pub const CPU_SUBTYPE_ARM64_V8: u32 = 1;
pub const CPU_SUBTYPE_ARM64E: u32 = 2;

/*
 *  ARM64_32 subtypes
 */
pub const CPU_SUBTYPE_ARM64_32_ALL: u32 = 0;
pub const CPU_SUBTYPE_ARM64_32_V8: u32 = 1;

// Definitions from "/usr/include/mach/vm_prot.h".

/// read permission
pub const VM_PROT_READ: u32 = 0x01;
/// write permission
pub const VM_PROT_WRITE: u32 = 0x02;
/// execute permission
pub const VM_PROT_EXECUTE: u32 = 0x04;

// Definitions from https://opensource.apple.com/source/dyld/dyld-210.2.3/launch-cache/dyld_cache_format.h.auto.html

/// The dyld cache header.
/// Corresponds to struct dyld_cache_header from dyld_cache_format.h.
/// This header has grown over time. Only the fields up to and including dyld_base_address
/// are guaranteed to be present. For all other fields, check the header size before
/// accessing the field. The header size is stored in mapping_offset; the mappings start
/// right after the theader.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DyldCacheHeader<E: Endian> {
    /// e.g. "dyld_v0    i386"
    pub magic: [u8; 16],
    /// file offset to first dyld_cache_mapping_info
    pub mapping_offset: U32<E>, // offset: 0x10
    /// number of dyld_cache_mapping_info entries
    pub mapping_count: U32<E>, // offset: 0x14
    /// file offset to first dyld_cache_image_info
    pub images_offset: U32<E>, // offset: 0x18
    /// number of dyld_cache_image_info entries
    pub images_count: U32<E>, // offset: 0x1c
    /// base address of dyld when cache was built
    pub dyld_base_address: U64<E>, // offset: 0x20
    reserved1: [u8; 32], // offset: 0x28
    /// file offset of where local symbols are stored
    pub local_symbols_offset: U64<E>, // offset: 0x48
    /// size of local symbols information
    pub local_symbols_size: U64<E>, // offset: 0x50
    /// unique value for each shared cache file
    pub uuid: [u8; 16], // offset: 0x58
    reserved2: [u8; 32], // offset: 0x68
    reserved3: [u8; 32], // offset: 0x88
    reserved4: [u8; 32], // offset: 0xa8
    reserved5: [u8; 32], // offset: 0xc8
    reserved6: [u8; 32], // offset: 0xe8
    reserved7: [u8; 32], // offset: 0x108
    reserved8: [u8; 32], // offset: 0x128
    reserved9: [u8; 32], // offset: 0x148
    reserved10: [u8; 32], // offset: 0x168
    /// file offset to first dyld_subcache_info
    pub subcaches_offset: U32<E>, // offset: 0x188
    /// number of dyld_subcache_info entries
    pub subcaches_count: U32<E>, // offset: 0x18c
    /// the UUID of the .symbols subcache
    pub symbols_subcache_uuid: [u8; 16], // offset: 0x190
    reserved11: [u8; 32], // offset: 0x1a0
    /// file offset to first dyld_cache_image_info
    /// Use this  instead of images_offset if mapping_offset is at least 0x1c4.
    pub images_across_all_subcaches_offset: U32<E>, // offset: 0x1c0
    /// number of dyld_cache_image_info entries
    /// Use this  instead of images_count if mapping_offset is at least 0x1c4.
    pub images_across_all_subcaches_count: U32<E>, // offset: 0x1c4
}

/// Corresponds to struct dyld_cache_mapping_info from dyld_cache_format.h.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DyldCacheMappingInfo<E: Endian> {
    pub address: U64<E>,
    pub size: U64<E>,
    pub file_offset: U64<E>,
    pub max_prot: U32<E>,
    pub init_prot: U32<E>,
}

/// Corresponds to struct dyld_cache_image_info from dyld_cache_format.h.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DyldCacheImageInfo<E: Endian> {
    pub address: U64<E>,
    pub mod_time: U64<E>,
    pub inode: U64<E>,
    pub path_file_offset: U32<E>,
    pub pad: U32<E>,
}

/// Added in dyld-940, which shipped with macOS 12 / iOS 15.
/// Originally called `dyld_subcache_entry`, renamed to `dyld_subcache_entry_v1`
/// in dyld-1042.1.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DyldSubCacheEntryV1<E: Endian> {
    /// The UUID of this subcache.
    pub uuid: [u8; 16],
    /// The offset of this subcache from the main cache base address.
    pub cache_vm_offset: U64<E>,
}

/// Added in dyld-1042.1, which shipped with macOS 13 / iOS 16.
/// Called `dyld_subcache_entry` as of dyld-1042.1.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DyldSubCacheEntryV2<E: Endian> {
    /// The UUID of this subcache.
    pub uuid: [u8; 16],
    /// The offset of this subcache from the main cache base address.
    pub cache_vm_offset: U64<E>,
    /// The file name suffix of the subCache file, e.g. ".25.data" or ".03.development".
    pub file_suffix: [u8; 32],
}

// Definitions from "/usr/include/mach-o/loader.h".

/*
 * This header file describes the structures of the file format for "fat"
 * architecture specific file (wrapper design).  At the beginning of the file
 * there is one `FatHeader` structure followed by a number of `FatArch*`
 * structures.  For each architecture in the file, specified by a pair of
 * cputype and cpusubtype, the `FatHeader` describes the file offset, file
 * size and alignment in the file of the architecture specific member.
 * The padded bytes in the file to place each member on it's specific alignment
 * are defined to be read as zeros and can be left as "holes" if the file system
 * can support them as long as they read as zeros.
 *
 * All structures defined here are always written and read to/from disk
 * in big-endian order.
 */

pub const FAT_MAGIC: u32 = 0xcafe_babe;
/// NXSwapLong(FAT_MAGIC)
pub const FAT_CIGAM: u32 = 0xbeba_feca;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FatHeader {
    /// FAT_MAGIC or FAT_MAGIC_64
    pub magic: U32<BigEndian>,
    /// number of structs that follow
    pub nfat_arch: U32<BigEndian>,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FatArch32 {
    /// cpu specifier (int)
    pub cputype: U32<BigEndian>,
    /// machine specifier (int)
    pub cpusubtype: U32<BigEndian>,
    /// file offset to this object file
    pub offset: U32<BigEndian>,
    /// size of this object file
    pub size: U32<BigEndian>,
    /// alignment as a power of 2
    pub align: U32<BigEndian>,
}

/*
 * The support for the 64-bit fat file format described here is a work in
 * progress and not yet fully supported in all the Apple Developer Tools.
 *
 * When a slice is greater than 4mb or an offset to a slice is greater than 4mb
 * then the 64-bit fat file format is used.
 */
pub const FAT_MAGIC_64: u32 = 0xcafe_babf;
/// NXSwapLong(FAT_MAGIC_64)
pub const FAT_CIGAM_64: u32 = 0xbfba_feca;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FatArch64 {
    /// cpu specifier (int)
    pub cputype: U32<BigEndian>,
    /// machine specifier (int)
    pub cpusubtype: U32<BigEndian>,
    /// file offset to this object file
    pub offset: U64<BigEndian>,
    /// size of this object file
    pub size: U64<BigEndian>,
    /// alignment as a power of 2
    pub align: U32<BigEndian>,
    /// reserved
    pub reserved: U32<BigEndian>,
}

// Definitions from "/usr/include/mach-o/loader.h".

/// The 32-bit mach header.
///
/// Appears at the very beginning of the object file for 32-bit architectures.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MachHeader32<E: Endian> {
    /// mach magic number identifier
    pub magic: U32<BigEndian>,
    /// cpu specifier
    pub cputype: U32<E>,
    /// machine specifier
    pub cpusubtype: U32<E>,
    /// type of file
    pub filetype: U32<E>,
    /// number of load commands
    pub ncmds: U32<E>,
    /// the size of all the load commands
    pub sizeofcmds: U32<E>,
    /// flags
    pub flags: U32<E>,
}

// Values for `MachHeader32::magic`.
/// the mach magic number
pub const MH_MAGIC: u32 = 0xfeed_face;
/// NXSwapInt(MH_MAGIC)
pub const MH_CIGAM: u32 = 0xcefa_edfe;

/// The 64-bit mach header.
///
/// Appears at the very beginning of object files for 64-bit architectures.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MachHeader64<E: Endian> {
    /// mach magic number identifier
    pub magic: U32<BigEndian>,
    /// cpu specifier
    pub cputype: U32<E>,
    /// machine specifier
    pub cpusubtype: U32<E>,
    /// type of file
    pub filetype: U32<E>,
    /// number of load commands
    pub ncmds: U32<E>,
    /// the size of all the load commands
    pub sizeofcmds: U32<E>,
    /// flags
    pub flags: U32<E>,
    /// reserved
    pub reserved: U32<E>,
}

// Values for `MachHeader64::magic`.
/// the 64-bit mach magic number
pub const MH_MAGIC_64: u32 = 0xfeed_facf;
/// NXSwapInt(MH_MAGIC_64)
pub const MH_CIGAM_64: u32 = 0xcffa_edfe;

/*
 * The layout of the file depends on the filetype.  For all but the MH_OBJECT
 * file type the segments are padded out and aligned on a segment alignment
 * boundary for efficient demand pageing.  The MH_EXECUTE, MH_FVMLIB, MH_DYLIB,
 * MH_DYLINKER and MH_BUNDLE file types also have the headers included as part
 * of their first segment.
 *
 * The file type MH_OBJECT is a compact format intended as output of the
 * assembler and input (and possibly output) of the link editor (the .o
 * format).  All sections are in one unnamed segment with no segment padding.
 * This format is used as an executable format when the file is so small the
 * segment padding greatly increases its size.
 *
 * The file type MH_PRELOAD is an executable format intended for things that
 * are not executed under the kernel (proms, stand alones, kernels, etc).  The
 * format can be executed under the kernel but may demand paged it and not
 * preload it before execution.
 *
 * A core file is in MH_CORE format and can be any in an arbritray legal
 * Mach-O file.
 */

// Values for `MachHeader*::filetype`.
/// relocatable object file
pub const MH_OBJECT: u32 = 0x1;
/// demand paged executable file
pub const MH_EXECUTE: u32 = 0x2;
/// fixed VM shared library file
pub const MH_FVMLIB: u32 = 0x3;
/// core file
pub const MH_CORE: u32 = 0x4;
/// preloaded executable file
pub const MH_PRELOAD: u32 = 0x5;
/// dynamically bound shared library
pub const MH_DYLIB: u32 = 0x6;
/// dynamic link editor
pub const MH_DYLINKER: u32 = 0x7;
/// dynamically bound bundle file
pub const MH_BUNDLE: u32 = 0x8;
/// shared library stub for static linking only, no section contents
pub const MH_DYLIB_STUB: u32 = 0x9;
/// companion file with only debug sections
pub const MH_DSYM: u32 = 0xa;
/// x86_64 kexts
pub const MH_KEXT_BUNDLE: u32 = 0xb;
/// set of mach-o's
pub const MH_FILESET: u32 = 0xc;

// Values for `MachHeader*::flags`.
/// the object file has no undefined references
pub const MH_NOUNDEFS: u32 = 0x1;
/// the object file is the output of an incremental link against a base file and can't be link edited again
pub const MH_INCRLINK: u32 = 0x2;
/// the object file is input for the dynamic linker and can't be statically link edited again
pub const MH_DYLDLINK: u32 = 0x4;
/// the object file's undefined references are bound by the dynamic linker when loaded.
pub const MH_BINDATLOAD: u32 = 0x8;
/// the file has its dynamic undefined references prebound.
pub const MH_PREBOUND: u32 = 0x10;
/// the file has its read-only and read-write segments split
pub const MH_SPLIT_SEGS: u32 = 0x20;
/// the shared library init routine is to be run lazily via catching memory faults to its writeable segments (obsolete)
pub const MH_LAZY_INIT: u32 = 0x40;
/// the image is using two-level name space bindings
pub const MH_TWOLEVEL: u32 = 0x80;
/// the executable is forcing all images to use flat name space bindings
pub const MH_FORCE_FLAT: u32 = 0x100;
/// this umbrella guarantees no multiple definitions of symbols in its sub-images so the two-level namespace hints can always be used.
pub const MH_NOMULTIDEFS: u32 = 0x200;
/// do not have dyld notify the prebinding agent about this executable
pub const MH_NOFIXPREBINDING: u32 = 0x400;
/// the binary is not prebound but can have its prebinding redone. only used when MH_PREBOUND is not set.
pub const MH_PREBINDABLE: u32 = 0x800;
/// indicates that this binary binds to all two-level namespace modules of its dependent libraries. only used when MH_PREBINDABLE and MH_TWOLEVEL are both set.
pub const MH_ALLMODSBOUND: u32 = 0x1000;
/// safe to divide up the sections into sub-sections via symbols for dead code stripping
pub const MH_SUBSECTIONS_VIA_SYMBOLS: u32 = 0x2000;
/// the binary has been canonicalized via the unprebind operation
pub const MH_CANONICAL: u32 = 0x4000;
/// the final linked image contains external weak symbols
pub const MH_WEAK_DEFINES: u32 = 0x8000;
/// the final linked image uses weak symbols
pub const MH_BINDS_TO_WEAK: u32 = 0x10000;
/// When this bit is set, all stacks in the task will be given stack execution privilege.  Only used in MH_EXECUTE filetypes.
pub const MH_ALLOW_STACK_EXECUTION: u32 = 0x20000;
/// When this bit is set, the binary declares it is safe for use in processes with uid zero
pub const MH_ROOT_SAFE: u32 = 0x40000;
/// When this bit is set, the binary declares it is safe for use in processes when issetugid() is true
pub const MH_SETUID_SAFE: u32 = 0x80000;
/// When this bit is set on a dylib, the static linker does not need to examine dependent dylibs to see if any are re-exported
pub const MH_NO_REEXPORTED_DYLIBS: u32 = 0x10_0000;
/// When this bit is set, the OS will load the main executable at a random address.  Only used in MH_EXECUTE filetypes.
pub const MH_PIE: u32 = 0x20_0000;
/// Only for use on dylibs.  When linking against a dylib that has this bit set, the static linker will automatically not create a LC_LOAD_DYLIB load command to the dylib if no symbols are being referenced from the dylib.
pub const MH_DEAD_STRIPPABLE_DYLIB: u32 = 0x40_0000;
/// Contains a section of type S_THREAD_LOCAL_VARIABLES
pub const MH_HAS_TLV_DESCRIPTORS: u32 = 0x80_0000;
/// When this bit is set, the OS will run the main executable with a non-executable heap even on platforms (e.g. i386) that don't require it. Only used in MH_EXECUTE filetypes.
pub const MH_NO_HEAP_EXECUTION: u32 = 0x100_0000;
/// The code was linked for use in an application extension.
pub const MH_APP_EXTENSION_SAFE: u32 = 0x0200_0000;
/// The external symbols listed in the nlist symbol table do not include all the symbols listed in the dyld info.
pub const MH_NLIST_OUTOFSYNC_WITH_DYLDINFO: u32 = 0x0400_0000;
/// Allow LC_MIN_VERSION_MACOS and LC_BUILD_VERSION load commands with
/// the platforms macOS, iOSMac, iOSSimulator, tvOSSimulator and watchOSSimulator.
pub const MH_SIM_SUPPORT: u32 = 0x0800_0000;
/// Only for use on dylibs. When this bit is set, the dylib is part of the dyld
/// shared cache, rather than loose in the filesystem.
pub const MH_DYLIB_IN_CACHE: u32 = 0x8000_0000;

/// Common fields at the start of every load command.
///
/// The load commands directly follow the mach_header.  The total size of all
/// of the commands is given by the sizeofcmds field in the mach_header.  All
/// load commands must have as their first two fields `cmd` and `cmdsize`.  The `cmd`
/// field is filled in with a constant for that command type.  Each command type
/// has a structure specifically for it.  The `cmdsize` field is the size in bytes
/// of the particular load command structure plus anything that follows it that
/// is a part of the load command (i.e. section structures, strings, etc.).  To
/// advance to the next load command the `cmdsize` can be added to the offset or
/// pointer of the current load command.  The `cmdsize` for 32-bit architectures
/// MUST be a multiple of 4 bytes and for 64-bit architectures MUST be a multiple
/// of 8 bytes (these are forever the maximum alignment of any load commands).
/// The padded bytes must be zero.  All tables in the object file must also
/// follow these rules so the file can be memory mapped.  Otherwise the pointers
/// to these tables will not work well or at all on some machines.  With all
/// padding zeroed like objects will compare byte for byte.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct LoadCommand<E: Endian> {
    /// Type of load command.
    ///
    /// One of the `LC_*` constants.
    pub cmd: U32<E>,
    /// Total size of command in bytes.
    pub cmdsize: U32<E>,
}

/*
 * After MacOS X 10.1 when a new load command is added that is required to be
 * understood by the dynamic linker for the image to execute properly the
 * LC_REQ_DYLD bit will be or'ed into the load command constant.  If the dynamic
 * linker sees such a load command it it does not understand will issue a
 * "unknown load command required for execution" error and refuse to use the
 * image.  Other load commands without this bit that are not understood will
 * simply be ignored.
 */
pub const LC_REQ_DYLD: u32 = 0x8000_0000;

/* Constants for the cmd field of all load commands, the type */
/// segment of this file to be mapped
pub const LC_SEGMENT: u32 = 0x1;
/// link-edit stab symbol table info
pub const LC_SYMTAB: u32 = 0x2;
/// link-edit gdb symbol table info (obsolete)
pub const LC_SYMSEG: u32 = 0x3;
/// thread
pub const LC_THREAD: u32 = 0x4;
/// unix thread (includes a stack)
pub const LC_UNIXTHREAD: u32 = 0x5;
/// load a specified fixed VM shared library
pub const LC_LOADFVMLIB: u32 = 0x6;
/// fixed VM shared library identification
pub const LC_IDFVMLIB: u32 = 0x7;
/// object identification info (obsolete)
pub const LC_IDENT: u32 = 0x8;
/// fixed VM file inclusion (internal use)
pub const LC_FVMFILE: u32 = 0x9;
/// prepage command (internal use)
pub const LC_PREPAGE: u32 = 0xa;
/// dynamic link-edit symbol table info
pub const LC_DYSYMTAB: u32 = 0xb;
/// load a dynamically linked shared library
pub const LC_LOAD_DYLIB: u32 = 0xc;
/// dynamically linked shared lib ident
pub const LC_ID_DYLIB: u32 = 0xd;
/// load a dynamic linker
pub const LC_LOAD_DYLINKER: u32 = 0xe;
/// dynamic linker identification
pub const LC_ID_DYLINKER: u32 = 0xf;
/// modules prebound for a dynamically linked shared library
pub const LC_PREBOUND_DYLIB: u32 = 0x10;
/// image routines
pub const LC_ROUTINES: u32 = 0x11;
/// sub framework
pub const LC_SUB_FRAMEWORK: u32 = 0x12;
/// sub umbrella
pub const LC_SUB_UMBRELLA: u32 = 0x13;
/// sub client
pub const LC_SUB_CLIENT: u32 = 0x14;
/// sub library
pub const LC_SUB_LIBRARY: u32 = 0x15;
/// two-level namespace lookup hints
pub const LC_TWOLEVEL_HINTS: u32 = 0x16;
/// prebind checksum
pub const LC_PREBIND_CKSUM: u32 = 0x17;
/// load a dynamically linked shared library that is allowed to be missing
/// (all symbols are weak imported).
pub const LC_LOAD_WEAK_DYLIB: u32 = 0x18 | LC_REQ_DYLD;
/// 64-bit segment of this file to be mapped
pub const LC_SEGMENT_64: u32 = 0x19;
/// 64-bit image routines
pub const LC_ROUTINES_64: u32 = 0x1a;
/// the uuid
pub const LC_UUID: u32 = 0x1b;
/// runpath additions
pub const LC_RPATH: u32 = 0x1c | LC_REQ_DYLD;
/// local of code signature
pub const LC_CODE_SIGNATURE: u32 = 0x1d;
/// local of info to split segments
pub const LC_SEGMENT_SPLIT_INFO: u32 = 0x1e;
/// load and re-export dylib
pub const LC_REEXPORT_DYLIB: u32 = 0x1f | LC_REQ_DYLD;
/// delay load of dylib until first use
pub const LC_LAZY_LOAD_DYLIB: u32 = 0x20;
/// encrypted segment information
pub const LC_ENCRYPTION_INFO: u32 = 0x21;
/// compressed dyld information
pub const LC_DYLD_INFO: u32 = 0x22;
/// compressed dyld information only
pub const LC_DYLD_INFO_ONLY: u32 = 0x22 | LC_REQ_DYLD;
/// load upward dylib
pub const LC_LOAD_UPWARD_DYLIB: u32 = 0x23 | LC_REQ_DYLD;
/// build for MacOSX min OS version
pub const LC_VERSION_MIN_MACOSX: u32 = 0x24;
/// build for iPhoneOS min OS version
pub const LC_VERSION_MIN_IPHONEOS: u32 = 0x25;
/// compressed table of function start addresses
pub const LC_FUNCTION_STARTS: u32 = 0x26;
/// string for dyld to treat like environment variable
pub const LC_DYLD_ENVIRONMENT: u32 = 0x27;
/// replacement for LC_UNIXTHREAD
pub const LC_MAIN: u32 = 0x28 | LC_REQ_DYLD;
/// table of non-instructions in __text
pub const LC_DATA_IN_CODE: u32 = 0x29;
/// source version used to build binary
pub const LC_SOURCE_VERSION: u32 = 0x2A;
/// Code signing DRs copied from linked dylibs
pub const LC_DYLIB_CODE_SIGN_DRS: u32 = 0x2B;
/// 64-bit encrypted segment information
pub const LC_ENCRYPTION_INFO_64: u32 = 0x2C;
/// linker options in MH_OBJECT files
pub const LC_LINKER_OPTION: u32 = 0x2D;
/// optimization hints in MH_OBJECT files
pub const LC_LINKER_OPTIMIZATION_HINT: u32 = 0x2E;
/// build for AppleTV min OS version
pub const LC_VERSION_MIN_TVOS: u32 = 0x2F;
/// build for Watch min OS version
pub const LC_VERSION_MIN_WATCHOS: u32 = 0x30;
/// arbitrary data included within a Mach-O file
pub const LC_NOTE: u32 = 0x31;
/// build for platform min OS version
pub const LC_BUILD_VERSION: u32 = 0x32;
/// used with `LinkeditDataCommand`, payload is trie
pub const LC_DYLD_EXPORTS_TRIE: u32 = 0x33 | LC_REQ_DYLD;
/// used with `LinkeditDataCommand`
pub const LC_DYLD_CHAINED_FIXUPS: u32 = 0x34 | LC_REQ_DYLD;
/// used with `FilesetEntryCommand`
pub const LC_FILESET_ENTRY: u32 = 0x35 | LC_REQ_DYLD;

/// A variable length string in a load command.
///
/// The strings are stored just after the load command structure and
/// the offset is from the start of the load command structure.  The size
/// of the string is reflected in the `cmdsize` field of the load command.
/// Once again any padded bytes to bring the `cmdsize` field to a multiple
/// of 4 bytes must be zero.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct LcStr<E: Endian> {
    /// offset to the string
    pub offset: U32<E>,
}

/// 32-bit segment load command.
///
/// The segment load command indicates that a part of this file is to be
/// mapped into the task's address space.  The size of this segment in memory,
/// vmsize, maybe equal to or larger than the amount to map from this file,
/// filesize.  The file is mapped starting at fileoff to the beginning of
/// the segment in memory, vmaddr.  The rest of the memory of the segment,
/// if any, is allocated zero fill on demand.  The segment's maximum virtual
/// memory protection and initial virtual memory protection are specified
/// by the maxprot and initprot fields.  If the segment has sections then the
/// `Section32` structures directly follow the segment command and their size is
/// reflected in `cmdsize`.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SegmentCommand32<E: Endian> {
    /// LC_SEGMENT
    pub cmd: U32<E>,
    /// includes sizeof section structs
    pub cmdsize: U32<E>,
    /// segment name
    pub segname: [u8; 16],
    /// memory address of this segment
    pub vmaddr: U32<E>,
    /// memory size of this segment
    pub vmsize: U32<E>,
    /// file offset of this segment
    pub fileoff: U32<E>,
    /// amount to map from the file
    pub filesize: U32<E>,
    /// maximum VM protection
    pub maxprot: U32<E>,
    /// initial VM protection
    pub initprot: U32<E>,
    /// number of sections in segment
    pub nsects: U32<E>,
    /// flags
    pub flags: U32<E>,
}

/// 64-bit segment load command.
///
/// The 64-bit segment load command indicates that a part of this file is to be
/// mapped into a 64-bit task's address space.  If the 64-bit segment has
/// sections then `Section64` structures directly follow the 64-bit segment
/// command and their size is reflected in `cmdsize`.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SegmentCommand64<E: Endian> {
    /// LC_SEGMENT_64
    pub cmd: U32<E>,
    /// includes sizeof section_64 structs
    pub cmdsize: U32<E>,
    /// segment name
    pub segname: [u8; 16],
    /// memory address of this segment
    pub vmaddr: U64<E>,
    /// memory size of this segment
    pub vmsize: U64<E>,
    /// file offset of this segment
    pub fileoff: U64<E>,
    /// amount to map from the file
    pub filesize: U64<E>,
    /// maximum VM protection
    pub maxprot: U32<E>,
    /// initial VM protection
    pub initprot: U32<E>,
    /// number of sections in segment
    pub nsects: U32<E>,
    /// flags
    pub flags: U32<E>,
}

// Values for `SegmentCommand*::flags`.
/// the file contents for this segment is for the high part of the VM space, the low part is zero filled (for stacks in core files)
pub const SG_HIGHVM: u32 = 0x1;
/// this segment is the VM that is allocated by a fixed VM library, for overlap checking in the link editor
pub const SG_FVMLIB: u32 = 0x2;
/// this segment has nothing that was relocated in it and nothing relocated to it, that is it maybe safely replaced without relocation
pub const SG_NORELOC: u32 = 0x4;
/// This segment is protected.  If the segment starts at file offset 0, the first page of the segment is not protected.  All other pages of the segment are protected.
pub const SG_PROTECTED_VERSION_1: u32 = 0x8;
/// This segment is made read-only after fixups
pub const SG_READ_ONLY: u32 = 0x10;

/*
 * A segment is made up of zero or more sections.  Non-MH_OBJECT files have
 * all of their segments with the proper sections in each, and padded to the
 * specified segment alignment when produced by the link editor.  The first
 * segment of a MH_EXECUTE and MH_FVMLIB format file contains the mach_header
 * and load commands of the object file before its first section.  The zero
 * fill sections are always last in their segment (in all formats).  This
 * allows the zeroed segment padding to be mapped into memory where zero fill
 * sections might be. The gigabyte zero fill sections, those with the section
 * type S_GB_ZEROFILL, can only be in a segment with sections of this type.
 * These segments are then placed after all other segments.
 *
 * The MH_OBJECT format has all of its sections in one segment for
 * compactness.  There is no padding to a specified segment boundary and the
 * mach_header and load commands are not part of the segment.
 *
 * Sections with the same section name, sectname, going into the same segment,
 * segname, are combined by the link editor.  The resulting section is aligned
 * to the maximum alignment of the combined sections and is the new section's
 * alignment.  The combined sections are aligned to their original alignment in
 * the combined section.  Any padded bytes to get the specified alignment are
 * zeroed.
 *
 * The format of the relocation entries referenced by the reloff and nreloc
 * fields of the section structure for mach object files is described in the
 * header file <reloc.h>.
 */
/// 32-bit section.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Section32<E: Endian> {
    /// name of this section
    pub sectname: [u8; 16],
    /// segment this section goes in
    pub segname: [u8; 16],
    /// memory address of this section
    pub addr: U32<E>,
    /// size in bytes of this section
    pub size: U32<E>,
    /// file offset of this section
    pub offset: U32<E>,
    /// section alignment (power of 2)
    pub align: U32<E>,
    /// file offset of relocation entries
    pub reloff: U32<E>,
    /// number of relocation entries
    pub nreloc: U32<E>,
    /// flags (section type and attributes)
    pub flags: U32<E>,
    /// reserved (for offset or index)
    pub reserved1: U32<E>,
    /// reserved (for count or sizeof)
    pub reserved2: U32<E>,
}

/// 64-bit section.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Section64<E: Endian> {
    /// name of this section
    pub sectname: [u8; 16],
    /// segment this section goes in
    pub segname: [u8; 16],
    /// memory address of this section
    pub addr: U64<E>,
    /// size in bytes of this section
    pub size: U64<E>,
    /// file offset of this section
    pub offset: U32<E>,
    /// section alignment (power of 2)
    pub align: U32<E>,
    /// file offset of relocation entries
    pub reloff: U32<E>,
    /// number of relocation entries
    pub nreloc: U32<E>,
    /// flags (section type and attributes)
    pub flags: U32<E>,
    /// reserved (for offset or index)
    pub reserved1: U32<E>,
    /// reserved (for count or sizeof)
    pub reserved2: U32<E>,
    /// reserved
    pub reserved3: U32<E>,
}

/*
 * The flags field of a section structure is separated into two parts a section
 * type and section attributes.  The section types are mutually exclusive (it
 * can only have one type) but the section attributes are not (it may have more
 * than one attribute).
 */
/// 256 section types
pub const SECTION_TYPE: u32 = 0x0000_00ff;
/// 24 section attributes
pub const SECTION_ATTRIBUTES: u32 = 0xffff_ff00;

/* Constants for the type of a section */
/// regular section
pub const S_REGULAR: u32 = 0x0;
/// zero fill on demand section
pub const S_ZEROFILL: u32 = 0x1;
/// section with only literal C strings
pub const S_CSTRING_LITERALS: u32 = 0x2;
/// section with only 4 byte literals
pub const S_4BYTE_LITERALS: u32 = 0x3;
/// section with only 8 byte literals
pub const S_8BYTE_LITERALS: u32 = 0x4;
/// section with only pointers to literals
pub const S_LITERAL_POINTERS: u32 = 0x5;
/*
 * For the two types of symbol pointers sections and the symbol stubs section
 * they have indirect symbol table entries.  For each of the entries in the
 * section the indirect symbol table entries, in corresponding order in the
 * indirect symbol table, start at the index stored in the reserved1 field
 * of the section structure.  Since the indirect symbol table entries
 * correspond to the entries in the section the number of indirect symbol table
 * entries is inferred from the size of the section divided by the size of the
 * entries in the section.  For symbol pointers sections the size of the entries
 * in the section is 4 bytes and for symbol stubs sections the byte size of the
 * stubs is stored in the reserved2 field of the section structure.
 */
/// section with only non-lazy symbol pointers
pub const S_NON_LAZY_SYMBOL_POINTERS: u32 = 0x6;
/// section with only lazy symbol pointers
pub const S_LAZY_SYMBOL_POINTERS: u32 = 0x7;
/// section with only symbol stubs, byte size of stub in the reserved2 field
pub const S_SYMBOL_STUBS: u32 = 0x8;
/// section with only function pointers for initialization
pub const S_MOD_INIT_FUNC_POINTERS: u32 = 0x9;
/// section with only function pointers for termination
pub const S_MOD_TERM_FUNC_POINTERS: u32 = 0xa;
/// section contains symbols that are to be coalesced
pub const S_COALESCED: u32 = 0xb;
/// zero fill on demand section (that can be larger than 4 gigabytes)
pub const S_GB_ZEROFILL: u32 = 0xc;
/// section with only pairs of function pointers for interposing
pub const S_INTERPOSING: u32 = 0xd;
/// section with only 16 byte literals
pub const S_16BYTE_LITERALS: u32 = 0xe;
/// section contains DTrace Object Format
pub const S_DTRACE_DOF: u32 = 0xf;
/// section with only lazy symbol pointers to lazy loaded dylibs
pub const S_LAZY_DYLIB_SYMBOL_POINTERS: u32 = 0x10;
/*
 * Section types to support thread local variables
 */
/// template of initial values for TLVs
pub const S_THREAD_LOCAL_REGULAR: u32 = 0x11;
/// template of initial values for TLVs
pub const S_THREAD_LOCAL_ZEROFILL: u32 = 0x12;
/// TLV descriptors
pub const S_THREAD_LOCAL_VARIABLES: u32 = 0x13;
/// pointers to TLV descriptors
pub const S_THREAD_LOCAL_VARIABLE_POINTERS: u32 = 0x14;
/// functions to call to initialize TLV values
pub const S_THREAD_LOCAL_INIT_FUNCTION_POINTERS: u32 = 0x15;
/// 32-bit offsets to initializers
pub const S_INIT_FUNC_OFFSETS: u32 = 0x16;

/*
 * Constants for the section attributes part of the flags field of a section
 * structure.
 */
/// User setable attributes
pub const SECTION_ATTRIBUTES_USR: u32 = 0xff00_0000;
/// section contains only true machine instructions
pub const S_ATTR_PURE_INSTRUCTIONS: u32 = 0x8000_0000;
/// section contains coalesced symbols that are not to be in a ranlib table of contents
pub const S_ATTR_NO_TOC: u32 = 0x4000_0000;
/// ok to strip static symbols in this section in files with the MH_DYLDLINK flag
pub const S_ATTR_STRIP_STATIC_SYMS: u32 = 0x2000_0000;
/// no dead stripping
pub const S_ATTR_NO_DEAD_STRIP: u32 = 0x1000_0000;
/// blocks are live if they reference live blocks
pub const S_ATTR_LIVE_SUPPORT: u32 = 0x0800_0000;
/// Used with i386 code stubs written on by dyld
pub const S_ATTR_SELF_MODIFYING_CODE: u32 = 0x0400_0000;
/*
 * If a segment contains any sections marked with S_ATTR_DEBUG then all
 * sections in that segment must have this attribute.  No section other than
 * a section marked with this attribute may reference the contents of this
 * section.  A section with this attribute may contain no symbols and must have
 * a section type S_REGULAR.  The static linker will not copy section contents
 * from sections with this attribute into its output file.  These sections
 * generally contain DWARF debugging info.
 */
/// a debug section
pub const S_ATTR_DEBUG: u32 = 0x0200_0000;
/// system setable attributes
pub const SECTION_ATTRIBUTES_SYS: u32 = 0x00ff_ff00;
/// section contains some machine instructions
pub const S_ATTR_SOME_INSTRUCTIONS: u32 = 0x0000_0400;
/// section has external relocation entries
pub const S_ATTR_EXT_RELOC: u32 = 0x0000_0200;
/// section has local relocation entries
pub const S_ATTR_LOC_RELOC: u32 = 0x0000_0100;

/*
 * The names of segments and sections in them are mostly meaningless to the
 * link-editor.  But there are few things to support traditional UNIX
 * executables that require the link-editor and assembler to use some names
 * agreed upon by convention.
 *
 * The initial protection of the "__TEXT" segment has write protection turned
 * off (not writeable).
 *
 * The link-editor will allocate common symbols at the end of the "__common"
 * section in the "__DATA" segment.  It will create the section and segment
 * if needed.
 */

/* The currently known segment names and the section names in those segments */

/// the pagezero segment which has no protections and catches NULL references for MH_EXECUTE files
pub const SEG_PAGEZERO: &str = "__PAGEZERO";

/// the tradition UNIX text segment
pub const SEG_TEXT: &str = "__TEXT";
/// the real text part of the text section no headers, and no padding
pub const SECT_TEXT: &str = "__text";
/// the fvmlib initialization section
pub const SECT_FVMLIB_INIT0: &str = "__fvmlib_init0";
/// the section following the fvmlib initialization section
pub const SECT_FVMLIB_INIT1: &str = "__fvmlib_init1";

/// the tradition UNIX data segment
pub const SEG_DATA: &str = "__DATA";
/// the real initialized data section no padding, no bss overlap
pub const SECT_DATA: &str = "__data";
/// the real uninitialized data section no padding
pub const SECT_BSS: &str = "__bss";
/// the section common symbols are allocated in by the link editor
pub const SECT_COMMON: &str = "__common";

/// objective-C runtime segment
pub const SEG_OBJC: &str = "__OBJC";
/// symbol table
pub const SECT_OBJC_SYMBOLS: &str = "__symbol_table";
/// module information
pub const SECT_OBJC_MODULES: &str = "__module_info";
/// string table
pub const SECT_OBJC_STRINGS: &str = "__selector_strs";
/// string table
pub const SECT_OBJC_REFS: &str = "__selector_refs";

/// the icon segment
pub const SEG_ICON: &str = "__ICON";
/// the icon headers
pub const SECT_ICON_HEADER: &str = "__header";
/// the icons in tiff format
pub const SECT_ICON_TIFF: &str = "__tiff";

/// the segment containing all structs created and maintained by the link editor.  Created with -seglinkedit option to ld(1) for MH_EXECUTE and FVMLIB file types only
pub const SEG_LINKEDIT: &str = "__LINKEDIT";

/// the segment overlapping with linkedit containing linking information
pub const SEG_LINKINFO: &str = "__LINKINFO";

/// the unix stack segment
pub const SEG_UNIXSTACK: &str = "__UNIXSTACK";

/// the segment for the self (dyld) modifying code stubs that has read, write and execute permissions
pub const SEG_IMPORT: &str = "__IMPORT";

/*
 * Fixed virtual memory shared libraries are identified by two things.  The
 * target pathname (the name of the library as found for execution), and the
 * minor version number.  The address of where the headers are loaded is in
 * header_addr. (THIS IS OBSOLETE and no longer supported).
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Fvmlib<E: Endian> {
    /// library's target pathname
    pub name: LcStr<E>,
    /// library's minor version number
    pub minor_version: U32<E>,
    /// library's header address
    pub header_addr: U32<E>,
}

/*
 * A fixed virtual shared library (filetype == MH_FVMLIB in the mach header)
 * contains a `FvmlibCommand` (cmd == LC_IDFVMLIB) to identify the library.
 * An object that uses a fixed virtual shared library also contains a
 * `FvmlibCommand` (cmd == LC_LOADFVMLIB) for each library it uses.
 * (THIS IS OBSOLETE and no longer supported).
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FvmlibCommand<E: Endian> {
    /// LC_IDFVMLIB or LC_LOADFVMLIB
    pub cmd: U32<E>,
    /// includes pathname string
    pub cmdsize: U32<E>,
    /// the library identification
    pub fvmlib: Fvmlib<E>,
}

/*
 * Dynamically linked shared libraries are identified by two things.  The
 * pathname (the name of the library as found for execution), and the
 * compatibility version number.  The pathname must match and the compatibility
 * number in the user of the library must be greater than or equal to the
 * library being used.  The time stamp is used to record the time a library was
 * built and copied into user so it can be use to determined if the library used
 * at runtime is exactly the same as used to built the program.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Dylib<E: Endian> {
    /// library's path name
    pub name: LcStr<E>,
    /// library's build time stamp
    pub timestamp: U32<E>,
    /// library's current version number
    pub current_version: U32<E>,
    /// library's compatibility vers number
    pub compatibility_version: U32<E>,
}

/*
 * A dynamically linked shared library (filetype == MH_DYLIB in the mach header)
 * contains a `DylibCommand` (cmd == LC_ID_DYLIB) to identify the library.
 * An object that uses a dynamically linked shared library also contains a
 * `DylibCommand` (cmd == LC_LOAD_DYLIB, LC_LOAD_WEAK_DYLIB, or
 * LC_REEXPORT_DYLIB) for each library it uses.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DylibCommand<E: Endian> {
    /// LC_ID_DYLIB, LC_LOAD_{,WEAK_}DYLIB, LC_REEXPORT_DYLIB
    pub cmd: U32<E>,
    /// includes pathname string
    pub cmdsize: U32<E>,
    /// the library identification
    pub dylib: Dylib<E>,
}

/*
 * A dynamically linked shared library may be a subframework of an umbrella
 * framework.  If so it will be linked with "-umbrella umbrella_name" where
 * Where "umbrella_name" is the name of the umbrella framework. A subframework
 * can only be linked against by its umbrella framework or other subframeworks
 * that are part of the same umbrella framework.  Otherwise the static link
 * editor produces an error and states to link against the umbrella framework.
 * The name of the umbrella framework for subframeworks is recorded in the
 * following structure.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SubFrameworkCommand<E: Endian> {
    /// LC_SUB_FRAMEWORK
    pub cmd: U32<E>,
    /// includes umbrella string
    pub cmdsize: U32<E>,
    /// the umbrella framework name
    pub umbrella: LcStr<E>,
}

/*
 * For dynamically linked shared libraries that are subframework of an umbrella
 * framework they can allow clients other than the umbrella framework or other
 * subframeworks in the same umbrella framework.  To do this the subframework
 * is built with "-allowable_client client_name" and an LC_SUB_CLIENT load
 * command is created for each -allowable_client flag.  The client_name is
 * usually a framework name.  It can also be a name used for bundles clients
 * where the bundle is built with "-client_name client_name".
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SubClientCommand<E: Endian> {
    /// LC_SUB_CLIENT
    pub cmd: U32<E>,
    /// includes client string
    pub cmdsize: U32<E>,
    /// the client name
    pub client: LcStr<E>,
}

/*
 * A dynamically linked shared library may be a sub_umbrella of an umbrella
 * framework.  If so it will be linked with "-sub_umbrella umbrella_name" where
 * Where "umbrella_name" is the name of the sub_umbrella framework.  When
 * statically linking when -twolevel_namespace is in effect a twolevel namespace
 * umbrella framework will only cause its subframeworks and those frameworks
 * listed as sub_umbrella frameworks to be implicited linked in.  Any other
 * dependent dynamic libraries will not be linked it when -twolevel_namespace
 * is in effect.  The primary library recorded by the static linker when
 * resolving a symbol in these libraries will be the umbrella framework.
 * Zero or more sub_umbrella frameworks may be use by an umbrella framework.
 * The name of a sub_umbrella framework is recorded in the following structure.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SubUmbrellaCommand<E: Endian> {
    /// LC_SUB_UMBRELLA
    pub cmd: U32<E>,
    /// includes sub_umbrella string
    pub cmdsize: U32<E>,
    /// the sub_umbrella framework name
    pub sub_umbrella: LcStr<E>,
}

/*
 * A dynamically linked shared library may be a sub_library of another shared
 * library.  If so it will be linked with "-sub_library library_name" where
 * Where "library_name" is the name of the sub_library shared library.  When
 * statically linking when -twolevel_namespace is in effect a twolevel namespace
 * shared library will only cause its subframeworks and those frameworks
 * listed as sub_umbrella frameworks and libraries listed as sub_libraries to
 * be implicited linked in.  Any other dependent dynamic libraries will not be
 * linked it when -twolevel_namespace is in effect.  The primary library
 * recorded by the static linker when resolving a symbol in these libraries
 * will be the umbrella framework (or dynamic library). Zero or more sub_library
 * shared libraries may be use by an umbrella framework or (or dynamic library).
 * The name of a sub_library framework is recorded in the following structure.
 * For example /usr/lib/libobjc_profile.A.dylib would be recorded as "libobjc".
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SubLibraryCommand<E: Endian> {
    /// LC_SUB_LIBRARY
    pub cmd: U32<E>,
    /// includes sub_library string
    pub cmdsize: U32<E>,
    /// the sub_library name
    pub sub_library: LcStr<E>,
}

/*
 * A program (filetype == MH_EXECUTE) that is
 * prebound to its dynamic libraries has one of these for each library that
 * the static linker used in prebinding.  It contains a bit vector for the
 * modules in the library.  The bits indicate which modules are bound (1) and
 * which are not (0) from the library.  The bit for module 0 is the low bit
 * of the first byte.  So the bit for the Nth module is:
 * (linked_modules[N/8] >> N%8) & 1
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PreboundDylibCommand<E: Endian> {
    /// LC_PREBOUND_DYLIB
    pub cmd: U32<E>,
    /// includes strings
    pub cmdsize: U32<E>,
    /// library's path name
    pub name: LcStr<E>,
    /// number of modules in library
    pub nmodules: U32<E>,
    /// bit vector of linked modules
    pub linked_modules: LcStr<E>,
}

/*
 * A program that uses a dynamic linker contains a `DylinkerCommand` to identify
 * the name of the dynamic linker (LC_LOAD_DYLINKER).  And a dynamic linker
 * contains a `DylinkerCommand` to identify the dynamic linker (LC_ID_DYLINKER).
 * A file can have at most one of these.
 * This struct is also used for the LC_DYLD_ENVIRONMENT load command and
 * contains string for dyld to treat like environment variable.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DylinkerCommand<E: Endian> {
    /// LC_ID_DYLINKER, LC_LOAD_DYLINKER or LC_DYLD_ENVIRONMENT
    pub cmd: U32<E>,
    /// includes pathname string
    pub cmdsize: U32<E>,
    /// dynamic linker's path name
    pub name: LcStr<E>,
}

/*
 * Thread commands contain machine-specific data structures suitable for
 * use in the thread state primitives.  The machine specific data structures
 * follow the struct `ThreadCommand` as follows.
 * Each flavor of machine specific data structure is preceded by an uint32_t
 * constant for the flavor of that data structure, an uint32_t that is the
 * count of uint32_t's of the size of the state data structure and then
 * the state data structure follows.  This triple may be repeated for many
 * flavors.  The constants for the flavors, counts and state data structure
 * definitions are expected to be in the header file <machine/thread_status.h>.
 * These machine specific data structures sizes must be multiples of
 * 4 bytes.  The `cmdsize` reflects the total size of the `ThreadCommand`
 * and all of the sizes of the constants for the flavors, counts and state
 * data structures.
 *
 * For executable objects that are unix processes there will be one
 * `ThreadCommand` (cmd == LC_UNIXTHREAD) created for it by the link-editor.
 * This is the same as a LC_THREAD, except that a stack is automatically
 * created (based on the shell's limit for the stack size).  Command arguments
 * and environment variables are copied onto that stack.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ThreadCommand<E: Endian> {
    /// LC_THREAD or  LC_UNIXTHREAD
    pub cmd: U32<E>,
    /// total size of this command
    pub cmdsize: U32<E>,
    /* uint32_t flavor		   flavor of thread state */
    /* uint32_t count		   count of uint32_t's in thread state */
    /* struct XXX_thread_state state   thread state for this flavor */
    /* ... */
}

/*
 * The routines command contains the address of the dynamic shared library
 * initialization routine and an index into the module table for the module
 * that defines the routine.  Before any modules are used from the library the
 * dynamic linker fully binds the module that defines the initialization routine
 * and then calls it.  This gets called before any module initialization
 * routines (used for C++ static constructors) in the library.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct RoutinesCommand32<E: Endian> {
    /* for 32-bit architectures */
    /// LC_ROUTINES
    pub cmd: U32<E>,
    /// total size of this command
    pub cmdsize: U32<E>,
    /// address of initialization routine
    pub init_address: U32<E>,
    /// index into the module table that the init routine is defined in
    pub init_module: U32<E>,
    pub reserved1: U32<E>,
    pub reserved2: U32<E>,
    pub reserved3: U32<E>,
    pub reserved4: U32<E>,
    pub reserved5: U32<E>,
    pub reserved6: U32<E>,
}

/*
 * The 64-bit routines command.  Same use as above.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct RoutinesCommand64<E: Endian> {
    /* for 64-bit architectures */
    /// LC_ROUTINES_64
    pub cmd: U32<E>,
    /// total size of this command
    pub cmdsize: U32<E>,
    /// address of initialization routine
    pub init_address: U64<E>,
    /// index into the module table that the init routine is defined in
    pub init_module: U64<E>,
    pub reserved1: U64<E>,
    pub reserved2: U64<E>,
    pub reserved3: U64<E>,
    pub reserved4: U64<E>,
    pub reserved5: U64<E>,
    pub reserved6: U64<E>,
}

/*
 * The `SymtabCommand` contains the offsets and sizes of the link-edit 4.3BSD
 * "stab" style symbol table information as described in the header files
 * <nlist.h> and <stab.h>.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SymtabCommand<E: Endian> {
    /// LC_SYMTAB
    pub cmd: U32<E>,
    /// sizeof(struct SymtabCommand)
    pub cmdsize: U32<E>,
    /// symbol table offset
    pub symoff: U32<E>,
    /// number of symbol table entries
    pub nsyms: U32<E>,
    /// string table offset
    pub stroff: U32<E>,
    /// string table size in bytes
    pub strsize: U32<E>,
}

/*
 * This is the second set of the symbolic information which is used to support
 * the data structures for the dynamically link editor.
 *
 * The original set of symbolic information in the `SymtabCommand` which contains
 * the symbol and string tables must also be present when this load command is
 * present.  When this load command is present the symbol table is organized
 * into three groups of symbols:
 *	local symbols (static and debugging symbols) - grouped by module
 *	defined external symbols - grouped by module (sorted by name if not lib)
 *	undefined external symbols (sorted by name if MH_BINDATLOAD is not set,
 *	     			    and in order the were seen by the static
 *				    linker if MH_BINDATLOAD is set)
 * In this load command there are offsets and counts to each of the three groups
 * of symbols.
 *
 * This load command contains a the offsets and sizes of the following new
 * symbolic information tables:
 *	table of contents
 *	module table
 *	reference symbol table
 *	indirect symbol table
 * The first three tables above (the table of contents, module table and
 * reference symbol table) are only present if the file is a dynamically linked
 * shared library.  For executable and object modules, which are files
 * containing only one module, the information that would be in these three
 * tables is determined as follows:
 * 	table of contents - the defined external symbols are sorted by name
 *	module table - the file contains only one module so everything in the
 *		       file is part of the module.
 *	reference symbol table - is the defined and undefined external symbols
 *
 * For dynamically linked shared library files this load command also contains
 * offsets and sizes to the pool of relocation entries for all sections
 * separated into two groups:
 *	external relocation entries
 *	local relocation entries
 * For executable and object modules the relocation entries continue to hang
 * off the section structures.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DysymtabCommand<E: Endian> {
    /// LC_DYSYMTAB
    pub cmd: U32<E>,
    /// sizeof(struct DysymtabCommand)
    pub cmdsize: U32<E>,

    /*
     * The symbols indicated by symoff and nsyms of the LC_SYMTAB load command
     * are grouped into the following three groups:
     *    local symbols (further grouped by the module they are from)
     *    defined external symbols (further grouped by the module they are from)
     *    undefined symbols
     *
     * The local symbols are used only for debugging.  The dynamic binding
     * process may have to use them to indicate to the debugger the local
     * symbols for a module that is being bound.
     *
     * The last two groups are used by the dynamic binding process to do the
     * binding (indirectly through the module table and the reference symbol
     * table when this is a dynamically linked shared library file).
     */
    /// index to local symbols
    pub ilocalsym: U32<E>,
    /// number of local symbols
    pub nlocalsym: U32<E>,

    /// index to externally defined symbols
    pub iextdefsym: U32<E>,
    /// number of externally defined symbols
    pub nextdefsym: U32<E>,

    /// index to undefined symbols
    pub iundefsym: U32<E>,
    /// number of undefined symbols
    pub nundefsym: U32<E>,

    /*
     * For the for the dynamic binding process to find which module a symbol
     * is defined in the table of contents is used (analogous to the ranlib
     * structure in an archive) which maps defined external symbols to modules
     * they are defined in.  This exists only in a dynamically linked shared
     * library file.  For executable and object modules the defined external
     * symbols are sorted by name and is use as the table of contents.
     */
    /// file offset to table of contents
    pub tocoff: U32<E>,
    /// number of entries in table of contents
    pub ntoc: U32<E>,

    /*
     * To support dynamic binding of "modules" (whole object files) the symbol
     * table must reflect the modules that the file was created from.  This is
     * done by having a module table that has indexes and counts into the merged
     * tables for each module.  The module structure that these two entries
     * refer to is described below.  This exists only in a dynamically linked
     * shared library file.  For executable and object modules the file only
     * contains one module so everything in the file belongs to the module.
     */
    /// file offset to module table
    pub modtaboff: U32<E>,
    /// number of module table entries
    pub nmodtab: U32<E>,

    /*
     * To support dynamic module binding the module structure for each module
     * indicates the external references (defined and undefined) each module
     * makes.  For each module there is an offset and a count into the
     * reference symbol table for the symbols that the module references.
     * This exists only in a dynamically linked shared library file.  For
     * executable and object modules the defined external symbols and the
     * undefined external symbols indicates the external references.
     */
    /// offset to referenced symbol table
    pub extrefsymoff: U32<E>,
    /// number of referenced symbol table entries
    pub nextrefsyms: U32<E>,

    /*
     * The sections that contain "symbol pointers" and "routine stubs" have
     * indexes and (implied counts based on the size of the section and fixed
     * size of the entry) into the "indirect symbol" table for each pointer
     * and stub.  For every section of these two types the index into the
     * indirect symbol table is stored in the section header in the field
     * reserved1.  An indirect symbol table entry is simply a 32bit index into
     * the symbol table to the symbol that the pointer or stub is referring to.
     * The indirect symbol table is ordered to match the entries in the section.
     */
    /// file offset to the indirect symbol table
    pub indirectsymoff: U32<E>,
    /// number of indirect symbol table entries
    pub nindirectsyms: U32<E>,

    /*
     * To support relocating an individual module in a library file quickly the
     * external relocation entries for each module in the library need to be
     * accessed efficiently.  Since the relocation entries can't be accessed
     * through the section headers for a library file they are separated into
     * groups of local and external entries further grouped by module.  In this
     * case the presents of this load command who's extreloff, nextrel,
     * locreloff and nlocrel fields are non-zero indicates that the relocation
     * entries of non-merged sections are not referenced through the section
     * structures (and the reloff and nreloc fields in the section headers are
     * set to zero).
     *
     * Since the relocation entries are not accessed through the section headers
     * this requires the r_address field to be something other than a section
     * offset to identify the item to be relocated.  In this case r_address is
     * set to the offset from the vmaddr of the first LC_SEGMENT command.
     * For MH_SPLIT_SEGS images r_address is set to the the offset from the
     * vmaddr of the first read-write LC_SEGMENT command.
     *
     * The relocation entries are grouped by module and the module table
     * entries have indexes and counts into them for the group of external
     * relocation entries for that the module.
     *
     * For sections that are merged across modules there must not be any
     * remaining external relocation entries for them (for merged sections
     * remaining relocation entries must be local).
     */
    /// offset to external relocation entries
    pub extreloff: U32<E>,
    /// number of external relocation entries
    pub nextrel: U32<E>,

    /*
     * All the local relocation entries are grouped together (they are not
     * grouped by their module since they are only used if the object is moved
     * from it statically link edited address).
     */
    /// offset to local relocation entries
    pub locreloff: U32<E>,
    /// number of local relocation entries
    pub nlocrel: U32<E>,
}

/*
 * An indirect symbol table entry is simply a 32bit index into the symbol table
 * to the symbol that the pointer or stub is referring to.  Unless it is for a
 * non-lazy symbol pointer section for a defined symbol which strip(1) as
 * removed.  In which case it has the value INDIRECT_SYMBOL_LOCAL.  If the
 * symbol was also absolute INDIRECT_SYMBOL_ABS is or'ed with that.
 */
pub const INDIRECT_SYMBOL_LOCAL: u32 = 0x8000_0000;
pub const INDIRECT_SYMBOL_ABS: u32 = 0x4000_0000;

/* a table of contents entry */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DylibTableOfContents<E: Endian> {
    /// the defined external symbol (index into the symbol table)
    pub symbol_index: U32<E>,
    /// index into the module table this symbol is defined in
    pub module_index: U32<E>,
}

/* a module table entry */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DylibModule32<E: Endian> {
    /// the module name (index into string table)
    pub module_name: U32<E>,

    /// index into externally defined symbols
    pub iextdefsym: U32<E>,
    /// number of externally defined symbols
    pub nextdefsym: U32<E>,
    /// index into reference symbol table
    pub irefsym: U32<E>,
    /// number of reference symbol table entries
    pub nrefsym: U32<E>,
    /// index into symbols for local symbols
    pub ilocalsym: U32<E>,
    /// number of local symbols
    pub nlocalsym: U32<E>,

    /// index into external relocation entries
    pub iextrel: U32<E>,
    /// number of external relocation entries
    pub nextrel: U32<E>,

    /// low 16 bits are the index into the init section, high 16 bits are the index into the term section
    pub iinit_iterm: U32<E>,
    /// low 16 bits are the number of init section entries, high 16 bits are the number of term section entries
    pub ninit_nterm: U32<E>,

    /// for this module address of the start of the (__OBJC,__module_info) section
    pub objc_module_info_addr: U32<E>,
    /// for this module size of the (__OBJC,__module_info) section
    pub objc_module_info_size: U32<E>,
}

/* a 64-bit module table entry */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DylibModule64<E: Endian> {
    /// the module name (index into string table)
    pub module_name: U32<E>,

    /// index into externally defined symbols
    pub iextdefsym: U32<E>,
    /// number of externally defined symbols
    pub nextdefsym: U32<E>,
    /// index into reference symbol table
    pub irefsym: U32<E>,
    /// number of reference symbol table entries
    pub nrefsym: U32<E>,
    /// index into symbols for local symbols
    pub ilocalsym: U32<E>,
    /// number of local symbols
    pub nlocalsym: U32<E>,

    /// index into external relocation entries
    pub iextrel: U32<E>,
    /// number of external relocation entries
    pub nextrel: U32<E>,

    /// low 16 bits are the index into the init section, high 16 bits are the index into the term section
    pub iinit_iterm: U32<E>,
    /// low 16 bits are the number of init section entries, high 16 bits are the number of term section entries
    pub ninit_nterm: U32<E>,

    /// for this module size of the (__OBJC,__module_info) section
    pub objc_module_info_size: U32<E>,
    /// for this module address of the start of the (__OBJC,__module_info) section
    pub objc_module_info_addr: U64<E>,
}

/*
 * The entries in the reference symbol table are used when loading the module
 * (both by the static and dynamic link editors) and if the module is unloaded
 * or replaced.  Therefore all external symbols (defined and undefined) are
 * listed in the module's reference table.  The flags describe the type of
 * reference that is being made.  The constants for the flags are defined in
 * <mach-o/nlist.h> as they are also used for symbol table entries.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DylibReference<E: Endian> {
    /* TODO:
    uint32_t isym:24,		/* index into the symbol table */
              flags:8;	/* flags to indicate the type of reference */
    */
    pub bitfield: U32<E>,
}

/*
 * The TwolevelHintsCommand contains the offset and number of hints in the
 * two-level namespace lookup hints table.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct TwolevelHintsCommand<E: Endian> {
    /// LC_TWOLEVEL_HINTS
    pub cmd: U32<E>,
    /// sizeof(struct TwolevelHintsCommand)
    pub cmdsize: U32<E>,
    /// offset to the hint table
    pub offset: U32<E>,
    /// number of hints in the hint table
    pub nhints: U32<E>,
}

/*
 * The entries in the two-level namespace lookup hints table are TwolevelHint
 * structs.  These provide hints to the dynamic link editor where to start
 * looking for an undefined symbol in a two-level namespace image.  The
 * isub_image field is an index into the sub-images (sub-frameworks and
 * sub-umbrellas list) that made up the two-level image that the undefined
 * symbol was found in when it was built by the static link editor.  If
 * isub-image is 0 the the symbol is expected to be defined in library and not
 * in the sub-images.  If isub-image is non-zero it is an index into the array
 * of sub-images for the umbrella with the first index in the sub-images being
 * 1. The array of sub-images is the ordered list of sub-images of the umbrella
 * that would be searched for a symbol that has the umbrella recorded as its
 * primary library.  The table of contents index is an index into the
 * library's table of contents.  This is used as the starting point of the
 * binary search or a directed linear search.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct TwolevelHint<E: Endian> {
    /* TODO:
    uint32_t
    isub_image:8,	/* index into the sub images */
    itoc:24;	/* index into the table of contents */
    */
    pub bitfield: U32<E>,
}

/*
 * The PrebindCksumCommand contains the value of the original check sum for
 * prebound files or zero.  When a prebound file is first created or modified
 * for other than updating its prebinding information the value of the check sum
 * is set to zero.  When the file has it prebinding re-done and if the value of
 * the check sum is zero the original check sum is calculated and stored in
 * cksum field of this load command in the output file.  If when the prebinding
 * is re-done and the cksum field is non-zero it is left unchanged from the
 * input file.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PrebindCksumCommand<E: Endian> {
    /// LC_PREBIND_CKSUM
    pub cmd: U32<E>,
    /// sizeof(struct PrebindCksumCommand)
    pub cmdsize: U32<E>,
    /// the check sum or zero
    pub cksum: U32<E>,
}

/*
 * The uuid load command contains a single 128-bit unique random number that
 * identifies an object produced by the static link editor.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct UuidCommand<E: Endian> {
    /// LC_UUID
    pub cmd: U32<E>,
    /// sizeof(struct UuidCommand)
    pub cmdsize: U32<E>,
    /// the 128-bit uuid
    pub uuid: [u8; 16],
}

/*
 * The RpathCommand contains a path which at runtime should be added to
 * the current run path used to find @rpath prefixed dylibs.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct RpathCommand<E: Endian> {
    /// LC_RPATH
    pub cmd: U32<E>,
    /// includes string
    pub cmdsize: U32<E>,
    /// path to add to run path
    pub path: LcStr<E>,
}

/*
 * The LinkeditDataCommand contains the offsets and sizes of a blob
 * of data in the __LINKEDIT segment.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct LinkeditDataCommand<E: Endian> {
    /// `LC_CODE_SIGNATURE`, `LC_SEGMENT_SPLIT_INFO`, `LC_FUNCTION_STARTS`,
    /// `LC_DATA_IN_CODE`, `LC_DYLIB_CODE_SIGN_DRS`, `LC_LINKER_OPTIMIZATION_HINT`,
    /// `LC_DYLD_EXPORTS_TRIE`, or `LC_DYLD_CHAINED_FIXUPS`.
    pub cmd: U32<E>,
    /// sizeof(struct LinkeditDataCommand)
    pub cmdsize: U32<E>,
    /// file offset of data in __LINKEDIT segment
    pub dataoff: U32<E>,
    /// file size of data in __LINKEDIT segment
    pub datasize: U32<E>,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FilesetEntryCommand<E: Endian> {
    // LC_FILESET_ENTRY
    pub cmd: U32<E>,
    /// includes id string
    pub cmdsize: U32<E>,
    /// memory address of the dylib
    pub vmaddr: U64<E>,
    /// file offset of the dylib
    pub fileoff: U64<E>,
    /// contained entry id
    pub entry_id: LcStr<E>,
    /// entry_id is 32-bits long, so this is the reserved padding
    pub reserved: U32<E>,
}

/*
 * The EncryptionInfoCommand32 contains the file offset and size of an
 * of an encrypted segment.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct EncryptionInfoCommand32<E: Endian> {
    /// LC_ENCRYPTION_INFO
    pub cmd: U32<E>,
    /// sizeof(struct EncryptionInfoCommand32)
    pub cmdsize: U32<E>,
    /// file offset of encrypted range
    pub cryptoff: U32<E>,
    /// file size of encrypted range
    pub cryptsize: U32<E>,
    /// which enryption system, 0 means not-encrypted yet
    pub cryptid: U32<E>,
}

/*
 * The EncryptionInfoCommand64 contains the file offset and size of an
 * of an encrypted segment (for use in x86_64 targets).
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct EncryptionInfoCommand64<E: Endian> {
    /// LC_ENCRYPTION_INFO_64
    pub cmd: U32<E>,
    /// sizeof(struct EncryptionInfoCommand64)
    pub cmdsize: U32<E>,
    /// file offset of encrypted range
    pub cryptoff: U32<E>,
    /// file size of encrypted range
    pub cryptsize: U32<E>,
    /// which enryption system, 0 means not-encrypted yet
    pub cryptid: U32<E>,
    /// padding to make this struct's size a multiple of 8 bytes
    pub pad: U32<E>,
}

/*
 * The VersionMinCommand contains the min OS version on which this
 * binary was built to run.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct VersionMinCommand<E: Endian> {
    /// LC_VERSION_MIN_MACOSX or LC_VERSION_MIN_IPHONEOS or LC_VERSION_MIN_WATCHOS or LC_VERSION_MIN_TVOS
    pub cmd: U32<E>,
    /// sizeof(struct VersionMinCommand)
    pub cmdsize: U32<E>,
    /// X.Y.Z is encoded in nibbles xxxx.yy.zz
    pub version: U32<E>,
    /// X.Y.Z is encoded in nibbles xxxx.yy.zz
    pub sdk: U32<E>,
}

/*
 * The BuildVersionCommand contains the min OS version on which this
 * binary was built to run for its platform.  The list of known platforms and
 * tool values following it.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct BuildVersionCommand<E: Endian> {
    /// LC_BUILD_VERSION
    pub cmd: U32<E>,
    /// sizeof(struct BuildVersionCommand) plus ntools * sizeof(struct BuildToolVersion)
    pub cmdsize: U32<E>,
    /// platform
    pub platform: U32<E>,
    /// X.Y.Z is encoded in nibbles xxxx.yy.zz
    pub minos: U32<E>,
    /// X.Y.Z is encoded in nibbles xxxx.yy.zz
    pub sdk: U32<E>,
    /// number of tool entries following this
    pub ntools: U32<E>,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct BuildToolVersion<E: Endian> {
    /// enum for the tool
    pub tool: U32<E>,
    /// version number of the tool
    pub version: U32<E>,
}

/* Known values for the platform field above. */
pub const PLATFORM_MACOS: u32 = 1;
pub const PLATFORM_IOS: u32 = 2;
pub const PLATFORM_TVOS: u32 = 3;
pub const PLATFORM_WATCHOS: u32 = 4;
pub const PLATFORM_BRIDGEOS: u32 = 5;
pub const PLATFORM_MACCATALYST: u32 = 6;
pub const PLATFORM_IOSSIMULATOR: u32 = 7;
pub const PLATFORM_TVOSSIMULATOR: u32 = 8;
pub const PLATFORM_WATCHOSSIMULATOR: u32 = 9;
pub const PLATFORM_DRIVERKIT: u32 = 10;
pub const PLATFORM_XROS: u32 = 11;
pub const PLATFORM_XROSSIMULATOR: u32 = 12;

/* Known values for the tool field above. */
pub const TOOL_CLANG: u32 = 1;
pub const TOOL_SWIFT: u32 = 2;
pub const TOOL_LD: u32 = 3;

/*
 * The DyldInfoCommand contains the file offsets and sizes of
 * the new compressed form of the information dyld needs to
 * load the image.  This information is used by dyld on Mac OS X
 * 10.6 and later.  All information pointed to by this command
 * is encoded using byte streams, so no endian swapping is needed
 * to interpret it.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DyldInfoCommand<E: Endian> {
    /// LC_DYLD_INFO or LC_DYLD_INFO_ONLY
    pub cmd: U32<E>,
    /// sizeof(struct DyldInfoCommand)
    pub cmdsize: U32<E>,

    /*
     * Dyld rebases an image whenever dyld loads it at an address different
     * from its preferred address.  The rebase information is a stream
     * of byte sized opcodes whose symbolic names start with REBASE_OPCODE_.
     * Conceptually the rebase information is a table of tuples:
     *    <seg-index, seg-offset, type>
     * The opcodes are a compressed way to encode the table by only
     * encoding when a column changes.  In addition simple patterns
     * like "every n'th offset for m times" can be encoded in a few
     * bytes.
     */
    /// file offset to rebase info
    pub rebase_off: U32<E>,
    /// size of rebase info
    pub rebase_size: U32<E>,

    /*
     * Dyld binds an image during the loading process, if the image
     * requires any pointers to be initialized to symbols in other images.
     * The bind information is a stream of byte sized
     * opcodes whose symbolic names start with BIND_OPCODE_.
     * Conceptually the bind information is a table of tuples:
     *    <seg-index, seg-offset, type, symbol-library-ordinal, symbol-name, addend>
     * The opcodes are a compressed way to encode the table by only
     * encoding when a column changes.  In addition simple patterns
     * like for runs of pointers initialized to the same value can be
     * encoded in a few bytes.
     */
    /// file offset to binding info
    pub bind_off: U32<E>,
    /// size of binding info
    pub bind_size: U32<E>,

    /*
     * Some C++ programs require dyld to unique symbols so that all
     * images in the process use the same copy of some code/data.
     * This step is done after binding. The content of the weak_bind
     * info is an opcode stream like the bind_info.  But it is sorted
     * alphabetically by symbol name.  This enable dyld to walk
     * all images with weak binding information in order and look
     * for collisions.  If there are no collisions, dyld does
     * no updating.  That means that some fixups are also encoded
     * in the bind_info.  For instance, all calls to "operator new"
     * are first bound to libstdc++.dylib using the information
     * in bind_info.  Then if some image overrides operator new
     * that is detected when the weak_bind information is processed
     * and the call to operator new is then rebound.
     */
    /// file offset to weak binding info
    pub weak_bind_off: U32<E>,
    /// size of weak binding info
    pub weak_bind_size: U32<E>,

    /*
     * Some uses of external symbols do not need to be bound immediately.
     * Instead they can be lazily bound on first use.  The lazy_bind
     * are contains a stream of BIND opcodes to bind all lazy symbols.
     * Normal use is that dyld ignores the lazy_bind section when
     * loading an image.  Instead the static linker arranged for the
     * lazy pointer to initially point to a helper function which
     * pushes the offset into the lazy_bind area for the symbol
     * needing to be bound, then jumps to dyld which simply adds
     * the offset to lazy_bind_off to get the information on what
     * to bind.
     */
    /// file offset to lazy binding info
    pub lazy_bind_off: U32<E>,
    /// size of lazy binding infs
    pub lazy_bind_size: U32<E>,

    /*
     * The symbols exported by a dylib are encoded in a trie.  This
     * is a compact representation that factors out common prefixes.
     * It also reduces LINKEDIT pages in RAM because it encodes all
     * information (name, address, flags) in one small, contiguous range.
     * The export area is a stream of nodes.  The first node sequentially
     * is the start node for the trie.
     *
     * Nodes for a symbol start with a uleb128 that is the length of
     * the exported symbol information for the string so far.
     * If there is no exported symbol, the node starts with a zero byte.
     * If there is exported info, it follows the length.
     *
     * First is a uleb128 containing flags. Normally, it is followed by
     * a uleb128 encoded offset which is location of the content named
     * by the symbol from the mach_header for the image.  If the flags
     * is EXPORT_SYMBOL_FLAGS_REEXPORT, then following the flags is
     * a uleb128 encoded library ordinal, then a zero terminated
     * UTF8 string.  If the string is zero length, then the symbol
     * is re-export from the specified dylib with the same name.
     * If the flags is EXPORT_SYMBOL_FLAGS_STUB_AND_RESOLVER, then following
     * the flags is two uleb128s: the stub offset and the resolver offset.
     * The stub is used by non-lazy pointers.  The resolver is used
     * by lazy pointers and must be called to get the actual address to use.
     *
     * After the optional exported symbol information is a byte of
     * how many edges (0-255) that this node has leaving it,
     * followed by each edge.
     * Each edge is a zero terminated UTF8 of the addition chars
     * in the symbol, followed by a uleb128 offset for the node that
     * edge points to.
     *
     */
    /// file offset to lazy binding info
    pub export_off: U32<E>,
    /// size of lazy binding infs
    pub export_size: U32<E>,
}

/*
 * The following are used to encode rebasing information
 */
pub const REBASE_TYPE_POINTER: u8 = 1;
pub const REBASE_TYPE_TEXT_ABSOLUTE32: u8 = 2;
pub const REBASE_TYPE_TEXT_PCREL32: u8 = 3;

pub const REBASE_OPCODE_MASK: u8 = 0xF0;
pub const REBASE_IMMEDIATE_MASK: u8 = 0x0F;
pub const REBASE_OPCODE_DONE: u8 = 0x00;
pub const REBASE_OPCODE_SET_TYPE_IMM: u8 = 0x10;
pub const REBASE_OPCODE_SET_SEGMENT_AND_OFFSET_ULEB: u8 = 0x20;
pub const REBASE_OPCODE_ADD_ADDR_ULEB: u8 = 0x30;
pub const REBASE_OPCODE_ADD_ADDR_IMM_SCALED: u8 = 0x40;
pub const REBASE_OPCODE_DO_REBASE_IMM_TIMES: u8 = 0x50;
pub const REBASE_OPCODE_DO_REBASE_ULEB_TIMES: u8 = 0x60;
pub const REBASE_OPCODE_DO_REBASE_ADD_ADDR_ULEB: u8 = 0x70;
pub const REBASE_OPCODE_DO_REBASE_ULEB_TIMES_SKIPPING_ULEB: u8 = 0x80;

/*
 * The following are used to encode binding information
 */
pub const BIND_TYPE_POINTER: u8 = 1;
pub const BIND_TYPE_TEXT_ABSOLUTE32: u8 = 2;
pub const BIND_TYPE_TEXT_PCREL32: u8 = 3;

pub const BIND_SPECIAL_DYLIB_SELF: i8 = 0;
pub const BIND_SPECIAL_DYLIB_MAIN_EXECUTABLE: i8 = -1;
pub const BIND_SPECIAL_DYLIB_FLAT_LOOKUP: i8 = -2;
pub const BIND_SPECIAL_DYLIB_WEAK_LOOKUP: i8 = -3;

pub const BIND_SYMBOL_FLAGS_WEAK_IMPORT: u8 = 0x1;
pub const BIND_SYMBOL_FLAGS_NON_WEAK_DEFINITION: u8 = 0x8;

pub const BIND_OPCODE_MASK: u8 = 0xF0;
pub const BIND_IMMEDIATE_MASK: u8 = 0x0F;
pub const BIND_OPCODE_DONE: u8 = 0x00;
pub const BIND_OPCODE_SET_DYLIB_ORDINAL_IMM: u8 = 0x10;
pub const BIND_OPCODE_SET_DYLIB_ORDINAL_ULEB: u8 = 0x20;
pub const BIND_OPCODE_SET_DYLIB_SPECIAL_IMM: u8 = 0x30;
pub const BIND_OPCODE_SET_SYMBOL_TRAILING_FLAGS_IMM: u8 = 0x40;
pub const BIND_OPCODE_SET_TYPE_IMM: u8 = 0x50;
pub const BIND_OPCODE_SET_ADDEND_SLEB: u8 = 0x60;
pub const BIND_OPCODE_SET_SEGMENT_AND_OFFSET_ULEB: u8 = 0x70;
pub const BIND_OPCODE_ADD_ADDR_ULEB: u8 = 0x80;
pub const BIND_OPCODE_DO_BIND: u8 = 0x90;
pub const BIND_OPCODE_DO_BIND_ADD_ADDR_ULEB: u8 = 0xA0;
pub const BIND_OPCODE_DO_BIND_ADD_ADDR_IMM_SCALED: u8 = 0xB0;
pub const BIND_OPCODE_DO_BIND_ULEB_TIMES_SKIPPING_ULEB: u8 = 0xC0;
pub const BIND_OPCODE_THREADED: u8 = 0xD0;
pub const BIND_SUBOPCODE_THREADED_SET_BIND_ORDINAL_TABLE_SIZE_ULEB: u8 = 0x00;
pub const BIND_SUBOPCODE_THREADED_APPLY: u8 = 0x01;

/*
 * The following are used on the flags byte of a terminal node
 * in the export information.
 */
pub const EXPORT_SYMBOL_FLAGS_KIND_MASK: u32 = 0x03;
pub const EXPORT_SYMBOL_FLAGS_KIND_REGULAR: u32 = 0x00;
pub const EXPORT_SYMBOL_FLAGS_KIND_THREAD_LOCAL: u32 = 0x01;
pub const EXPORT_SYMBOL_FLAGS_KIND_ABSOLUTE: u32 = 0x02;
pub const EXPORT_SYMBOL_FLAGS_WEAK_DEFINITION: u32 = 0x04;
pub const EXPORT_SYMBOL_FLAGS_REEXPORT: u32 = 0x08;
pub const EXPORT_SYMBOL_FLAGS_STUB_AND_RESOLVER: u32 = 0x10;

/*
 * The LinkerOptionCommand contains linker options embedded in object files.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct LinkerOptionCommand<E: Endian> {
    /// LC_LINKER_OPTION only used in MH_OBJECT filetypes
    pub cmd: U32<E>,
    pub cmdsize: U32<E>,
    /// number of strings
    pub count: U32<E>,
    /* concatenation of zero terminated UTF8 strings.
    Zero filled at end to align */
}

/*
 * The SymsegCommand contains the offset and size of the GNU style
 * symbol table information as described in the header file <symseg.h>.
 * The symbol roots of the symbol segments must also be aligned properly
 * in the file.  So the requirement of keeping the offsets aligned to a
 * multiple of a 4 bytes translates to the length field of the symbol
 * roots also being a multiple of a long.  Also the padding must again be
 * zeroed. (THIS IS OBSOLETE and no longer supported).
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SymsegCommand<E: Endian> {
    /// LC_SYMSEG
    pub cmd: U32<E>,
    /// sizeof(struct SymsegCommand)
    pub cmdsize: U32<E>,
    /// symbol segment offset
    pub offset: U32<E>,
    /// symbol segment size in bytes
    pub size: U32<E>,
}

/*
 * The IdentCommand contains a free format string table following the
 * IdentCommand structure.  The strings are null terminated and the size of
 * the command is padded out with zero bytes to a multiple of 4 bytes/
 * (THIS IS OBSOLETE and no longer supported).
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IdentCommand<E: Endian> {
    /// LC_IDENT
    pub cmd: U32<E>,
    /// strings that follow this command
    pub cmdsize: U32<E>,
}

/*
 * The FvmfileCommand contains a reference to a file to be loaded at the
 * specified virtual address.  (Presently, this command is reserved for
 * internal use.  The kernel ignores this command when loading a program into
 * memory).
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FvmfileCommand<E: Endian> {
    /// LC_FVMFILE
    pub cmd: U32<E>,
    /// includes pathname string
    pub cmdsize: U32<E>,
    /// files pathname
    pub name: LcStr<E>,
    /// files virtual address
    pub header_addr: U32<E>,
}

/*
 * The EntryPointCommand is a replacement for thread_command.
 * It is used for main executables to specify the location (file offset)
 * of main().  If -stack_size was used at link time, the stacksize
 * field will contain the stack size need for the main thread.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct EntryPointCommand<E: Endian> {
    /// LC_MAIN only used in MH_EXECUTE filetypes
    pub cmd: U32<E>,
    /// 24
    pub cmdsize: U32<E>,
    /// file (__TEXT) offset of main()
    pub entryoff: U64<E>,
    /// if not zero, initial stack size
    pub stacksize: U64<E>,
}

/*
 * The SourceVersionCommand is an optional load command containing
 * the version of the sources used to build the binary.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SourceVersionCommand<E: Endian> {
    /// LC_SOURCE_VERSION
    pub cmd: U32<E>,
    /// 16
    pub cmdsize: U32<E>,
    /// A.B.C.D.E packed as a24.b10.c10.d10.e10
    pub version: U64<E>,
}

/*
 * The LC_DATA_IN_CODE load commands uses a LinkeditDataCommand
 * to point to an array of DataInCodeEntry entries. Each entry
 * describes a range of data in a code section.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DataInCodeEntry<E: Endian> {
    /// from mach_header to start of data range
    pub offset: U32<E>,
    /// number of bytes in data range
    pub length: U16<E>,
    /// a DICE_KIND_* value
    pub kind: U16<E>,
}
pub const DICE_KIND_DATA: u32 = 0x0001;
pub const DICE_KIND_JUMP_TABLE8: u32 = 0x0002;
pub const DICE_KIND_JUMP_TABLE16: u32 = 0x0003;
pub const DICE_KIND_JUMP_TABLE32: u32 = 0x0004;
pub const DICE_KIND_ABS_JUMP_TABLE32: u32 = 0x0005;

/*
 * Sections of type S_THREAD_LOCAL_VARIABLES contain an array
 * of TlvDescriptor structures.
 */
/* TODO:
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct TlvDescriptor<E: Endian>
{
    void*		(*thunk)(struct TlvDescriptor*);
    unsigned long	key;
    unsigned long	offset;
}
*/

/*
 * LC_NOTE commands describe a region of arbitrary data included in a Mach-O
 * file.  Its initial use is to record extra data in MH_CORE files.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct NoteCommand<E: Endian> {
    /// LC_NOTE
    pub cmd: U32<E>,
    /// sizeof(struct NoteCommand)
    pub cmdsize: U32<E>,
    /// owner name for this LC_NOTE
    pub data_owner: [u8; 16],
    /// file offset of this data
    pub offset: U64<E>,
    /// length of data region
    pub size: U64<E>,
}

// Definitions from "/usr/include/mach-o/nlist.h".

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Nlist32<E: Endian> {
    /// index into the string table
    pub n_strx: U32<E>,
    /// type flag, see below
    pub n_type: u8,
    /// section number or NO_SECT
    pub n_sect: u8,
    /// see <mach-o/stab.h>
    pub n_desc: U16<E>,
    /// value of this symbol (or stab offset)
    pub n_value: U32<E>,
}

/*
 * This is the symbol table entry structure for 64-bit architectures.
 */
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Nlist64<E: Endian> {
    /// index into the string table
    pub n_strx: U32<E>,
    /// type flag, see below
    pub n_type: u8,
    /// section number or NO_SECT
    pub n_sect: u8,
    /// see <mach-o/stab.h>
    pub n_desc: U16<E>,
    /// value of this symbol (or stab offset)
    // Note: 4 byte alignment has been observed in practice.
    pub n_value: U64Bytes<E>,
}

/*
 * Symbols with a index into the string table of zero (n_un.n_strx == 0) are
 * defined to have a null, "", name.  Therefore all string indexes to non null
 * names must not have a zero string index.  This is bit historical information
 * that has never been well documented.
 */

/*
 * The n_type field really contains four fields:
 *	unsigned char N_STAB:3,
 *		      N_PEXT:1,
 *		      N_TYPE:3,
 *		      N_EXT:1;
 * which are used via the following masks.
 */
/// if any of these bits set, a symbolic debugging entry
pub const N_STAB: u8 = 0xe0;
/// private external symbol bit
pub const N_PEXT: u8 = 0x10;
/// mask for the type bits
pub const N_TYPE: u8 = 0x0e;
/// external symbol bit, set for external symbols
pub const N_EXT: u8 = 0x01;

/*
 * Only symbolic debugging entries have some of the N_STAB bits set and if any
 * of these bits are set then it is a symbolic debugging entry (a stab).  In
 * which case then the values of the n_type field (the entire field) are given
 * in <mach-o/stab.h>
 */

/*
 * Values for N_TYPE bits of the n_type field.
 */
/// undefined, n_sect == NO_SECT
pub const N_UNDF: u8 = 0x0;
/// absolute, n_sect == NO_SECT
pub const N_ABS: u8 = 0x2;
/// defined in section number n_sect
pub const N_SECT: u8 = 0xe;
/// prebound undefined (defined in a dylib)
pub const N_PBUD: u8 = 0xc;
/// indirect
pub const N_INDR: u8 = 0xa;

/*
 * If the type is N_INDR then the symbol is defined to be the same as another
 * symbol.  In this case the n_value field is an index into the string table
 * of the other symbol's name.  When the other symbol is defined then they both
 * take on the defined type and value.
 */

/*
 * If the type is N_SECT then the n_sect field contains an ordinal of the
 * section the symbol is defined in.  The sections are numbered from 1 and
 * refer to sections in order they appear in the load commands for the file
 * they are in.  This means the same ordinal may very well refer to different
 * sections in different files.
 *
 * The n_value field for all symbol table entries (including N_STAB's) gets
 * updated by the link editor based on the value of it's n_sect field and where
 * the section n_sect references gets relocated.  If the value of the n_sect
 * field is NO_SECT then it's n_value field is not changed by the link editor.
 */
/// symbol is not in any section
pub const NO_SECT: u8 = 0;
/// 1 thru 255 inclusive
pub const MAX_SECT: u8 = 255;

/*
 * Common symbols are represented by undefined (N_UNDF) external (N_EXT) types
 * who's values (n_value) are non-zero.  In which case the value of the n_value
 * field is the size (in bytes) of the common symbol.  The n_sect field is set
 * to NO_SECT.  The alignment of a common symbol may be set as a power of 2
 * between 2^1 and 2^15 as part of the n_desc field using the macros below. If
 * the alignment is not set (a value of zero) then natural alignment based on
 * the size is used.
 */
/* TODO:
#define GET_COMM_ALIGN(n_desc) (((n_desc) >> 8) & 0x0f)
#define SET_COMM_ALIGN(n_desc,align) \
    (n_desc) = (((n_desc) & 0xf0ff) | (((align) & 0x0f) << 8))
 */

/*
 * To support the lazy binding of undefined symbols in the dynamic link-editor,
 * the undefined symbols in the symbol table (the nlist structures) are marked
 * with the indication if the undefined reference is a lazy reference or
 * non-lazy reference.  If both a non-lazy reference and a lazy reference is
 * made to the same symbol the non-lazy reference takes precedence.  A reference
 * is lazy only when all references to that symbol are made through a symbol
 * pointer in a lazy symbol pointer section.
 *
 * The implementation of marking nlist structures in the symbol table for
 * undefined symbols will be to use some of the bits of the n_desc field as a
 * reference type.  The mask REFERENCE_TYPE will be applied to the n_desc field
 * of an nlist structure for an undefined symbol to determine the type of
 * undefined reference (lazy or non-lazy).
 *
 * The constants for the REFERENCE FLAGS are propagated to the reference table
 * in a shared library file.  In that case the constant for a defined symbol,
 * REFERENCE_FLAG_DEFINED, is also used.
 */
/* Reference type bits of the n_desc field of undefined symbols */
pub const REFERENCE_TYPE: u16 = 0x7;
/* types of references */
pub const REFERENCE_FLAG_UNDEFINED_NON_LAZY: u16 = 0;
pub const REFERENCE_FLAG_UNDEFINED_LAZY: u16 = 1;
pub const REFERENCE_FLAG_DEFINED: u16 = 2;
pub const REFERENCE_FLAG_PRIVATE_DEFINED: u16 = 3;
pub const REFERENCE_FLAG_PRIVATE_UNDEFINED_NON_LAZY: u16 = 4;
pub const REFERENCE_FLAG_PRIVATE_UNDEFINED_LAZY: u16 = 5;

/*
 * To simplify stripping of objects that use are used with the dynamic link
 * editor, the static link editor marks the symbols defined an object that are
 * referenced by a dynamically bound object (dynamic shared libraries, bundles).
 * With this marking strip knows not to strip these symbols.
 */
pub const REFERENCED_DYNAMICALLY: u16 = 0x0010;

/*
 * For images created by the static link editor with the -twolevel_namespace
 * option in effect the flags field of the mach header is marked with
 * MH_TWOLEVEL.  And the binding of the undefined references of the image are
 * determined by the static link editor.  Which library an undefined symbol is
 * bound to is recorded by the static linker in the high 8 bits of the n_desc
 * field using the SET_LIBRARY_ORDINAL macro below.  The ordinal recorded
 * references the libraries listed in the Mach-O's LC_LOAD_DYLIB,
 * LC_LOAD_WEAK_DYLIB, LC_REEXPORT_DYLIB, LC_LOAD_UPWARD_DYLIB, and
 * LC_LAZY_LOAD_DYLIB, etc. load commands in the order they appear in the
 * headers.   The library ordinals start from 1.
 * For a dynamic library that is built as a two-level namespace image the
 * undefined references from module defined in another use the same nlist struct
 * an in that case SELF_LIBRARY_ORDINAL is used as the library ordinal.  For
 * defined symbols in all images they also must have the library ordinal set to
 * SELF_LIBRARY_ORDINAL.  The EXECUTABLE_ORDINAL refers to the executable
 * image for references from plugins that refer to the executable that loads
 * them.
 *
 * The DYNAMIC_LOOKUP_ORDINAL is for undefined symbols in a two-level namespace
 * image that are looked up by the dynamic linker with flat namespace semantics.
 * This ordinal was added as a feature in Mac OS X 10.3 by reducing the
 * value of MAX_LIBRARY_ORDINAL by one.  So it is legal for existing binaries
 * or binaries built with older tools to have 0xfe (254) dynamic libraries.  In
 * this case the ordinal value 0xfe (254) must be treated as a library ordinal
 * for compatibility.
 */
/* TODO:
#define GET_LIBRARY_ORDINAL(n_desc) (((n_desc) >> 8) & 0xff)
#define SET_LIBRARY_ORDINAL(n_desc,ordinal) \
    (n_desc) = (((n_desc) & 0x00ff) | (((ordinal) & 0xff) << 8))
 */
pub const SELF_LIBRARY_ORDINAL: u8 = 0x0;
pub const MAX_LIBRARY_ORDINAL: u8 = 0xfd;
pub const DYNAMIC_LOOKUP_ORDINAL: u8 = 0xfe;
pub const EXECUTABLE_ORDINAL: u8 = 0xff;

/*
 * The bit 0x0020 of the n_desc field is used for two non-overlapping purposes
 * and has two different symbolic names, N_NO_DEAD_STRIP and N_DESC_DISCARDED.
 */

/*
 * The N_NO_DEAD_STRIP bit of the n_desc field only ever appears in a
 * relocatable .o file (MH_OBJECT filetype). And is used to indicate to the
 * static link editor it is never to dead strip the symbol.
 */
/// symbol is not to be dead stripped
pub const N_NO_DEAD_STRIP: u16 = 0x0020;

/*
 * The N_DESC_DISCARDED bit of the n_desc field never appears in linked image.
 * But is used in very rare cases by the dynamic link editor to mark an in
 * memory symbol as discared and longer used for linking.
 */
/// symbol is discarded
pub const N_DESC_DISCARDED: u16 = 0x0020;

/*
 * The N_WEAK_REF bit of the n_desc field indicates to the dynamic linker that
 * the undefined symbol is allowed to be missing and is to have the address of
 * zero when missing.
 */
/// symbol is weak referenced
pub const N_WEAK_REF: u16 = 0x0040;

/*
 * The N_WEAK_DEF bit of the n_desc field indicates to the static and dynamic
 * linkers that the symbol definition is weak, allowing a non-weak symbol to
 * also be used which causes the weak definition to be discared.  Currently this
 * is only supported for symbols in coalesced sections.
 */
/// coalesced symbol is a weak definition
pub const N_WEAK_DEF: u16 = 0x0080;

/*
 * The N_REF_TO_WEAK bit of the n_desc field indicates to the dynamic linker
 * that the undefined symbol should be resolved using flat namespace searching.
 */
/// reference to a weak symbol
pub const N_REF_TO_WEAK: u16 = 0x0080;

/*
 * The N_ARM_THUMB_DEF bit of the n_desc field indicates that the symbol is
 * a definition of a Thumb function.
 */
/// symbol is a Thumb function (ARM)
pub const N_ARM_THUMB_DEF: u16 = 0x0008;

/*
 * The N_SYMBOL_RESOLVER bit of the n_desc field indicates that the
 * that the function is actually a resolver function and should
 * be called to get the address of the real function to use.
 * This bit is only available in .o files (MH_OBJECT filetype)
 */
pub const N_SYMBOL_RESOLVER: u16 = 0x0100;

/*
 * The N_ALT_ENTRY bit of the n_desc field indicates that the
 * symbol is pinned to the previous content.
 */
pub const N_ALT_ENTRY: u16 = 0x0200;

// Definitions from "/usr/include/mach-o/stab.h".

/*
 * This file gives definitions supplementing <nlist.h> for permanent symbol
 * table entries of Mach-O files.  Modified from the BSD definitions.  The
 * modifications from the original definitions were changing what the values of
 * what was the n_other field (an unused field) which is now the n_sect field.
 * These modifications are required to support symbols in an arbitrary number of
 * sections not just the three sections (text, data and bss) in a BSD file.
 * The values of the defined constants have NOT been changed.
 *
 * These must have one of the N_STAB bits on.  The n_value fields are subject
 * to relocation according to the value of their n_sect field.  So for types
 * that refer to things in sections the n_sect field must be filled in with the
 * proper section ordinal.  For types that are not to have their n_value field
 * relocatated the n_sect field must be NO_SECT.
 */

/*
 * Symbolic debugger symbols.  The comments give the conventional use for
 *
 * 	.stabs "n_name", n_type, n_sect, n_desc, n_value
 *
 * where n_type is the defined constant and not listed in the comment.  Other
 * fields not listed are zero. n_sect is the section ordinal the entry is
 * referring to.
 */
/// global symbol: name,,NO_SECT,type,0
pub const N_GSYM: u8 = 0x20;
/// procedure name (f77 kludge): name,,NO_SECT,0,0
pub const N_FNAME: u8 = 0x22;
/// procedure: name,,n_sect,linenumber,address
pub const N_FUN: u8 = 0x24;
/// static symbol: name,,n_sect,type,address
pub const N_STSYM: u8 = 0x26;
/// .lcomm symbol: name,,n_sect,type,address
pub const N_LCSYM: u8 = 0x28;
/// begin nsect sym: 0,,n_sect,0,address
pub const N_BNSYM: u8 = 0x2e;
/// AST file path: name,,NO_SECT,0,0
pub const N_AST: u8 = 0x32;
/// emitted with gcc2_compiled and in gcc source
pub const N_OPT: u8 = 0x3c;
/// register sym: name,,NO_SECT,type,register
pub const N_RSYM: u8 = 0x40;
/// src line: 0,,n_sect,linenumber,address
pub const N_SLINE: u8 = 0x44;
/// end nsect sym: 0,,n_sect,0,address
pub const N_ENSYM: u8 = 0x4e;
/// structure elt: name,,NO_SECT,type,struct_offset
pub const N_SSYM: u8 = 0x60;
/// source file name: name,,n_sect,0,address
pub const N_SO: u8 = 0x64;
/// object file name: name,,0,0,st_mtime
pub const N_OSO: u8 = 0x66;
/// local sym: name,,NO_SECT,type,offset
pub const N_LSYM: u8 = 0x80;
/// include file beginning: name,,NO_SECT,0,sum
pub const N_BINCL: u8 = 0x82;
/// #included file name: name,,n_sect,0,address
pub const N_SOL: u8 = 0x84;
/// compiler parameters: name,,NO_SECT,0,0
pub const N_PARAMS: u8 = 0x86;
/// compiler version: name,,NO_SECT,0,0
pub const N_VERSION: u8 = 0x88;
/// compiler -O level: name,,NO_SECT,0,0
pub const N_OLEVEL: u8 = 0x8A;
/// parameter: name,,NO_SECT,type,offset
pub const N_PSYM: u8 = 0xa0;
/// include file end: name,,NO_SECT,0,0
pub const N_EINCL: u8 = 0xa2;
/// alternate entry: name,,n_sect,linenumber,address
pub const N_ENTRY: u8 = 0xa4;
/// left bracket: 0,,NO_SECT,nesting level,address
pub const N_LBRAC: u8 = 0xc0;
/// deleted include file: name,,NO_SECT,0,sum
pub const N_EXCL: u8 = 0xc2;
/// right bracket: 0,,NO_SECT,nesting level,address
pub const N_RBRAC: u8 = 0xe0;
/// begin common: name,,NO_SECT,0,0
pub const N_BCOMM: u8 = 0xe2;
/// end common: name,,n_sect,0,0
pub const N_ECOMM: u8 = 0xe4;
/// end common (local name): 0,,n_sect,0,address
pub const N_ECOML: u8 = 0xe8;
/// second stab entry with length information
pub const N_LENG: u8 = 0xfe;

/*
 * for the berkeley pascal compiler, pc(1):
 */
/// global pascal symbol: name,,NO_SECT,subtype,line
pub const N_PC: u8 = 0x30;

// Definitions from "/usr/include/mach-o/reloc.h".

/// A relocation entry.
///
/// Mach-O relocations have plain and scattered variants, with the
/// meaning of the fields depending on the variant.
///
/// This type provides functions for determining whether the relocation
/// is scattered, and for accessing the fields of each variant.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Relocation<E: Endian> {
    pub r_word0: U32<E>,
    pub r_word1: U32<E>,
}

impl<E: Endian> Relocation<E> {
    /// Determine whether this is a scattered relocation.
    #[inline]
    pub fn r_scattered(self, endian: E, cputype: u32) -> bool {
        if cputype == CPU_TYPE_X86_64 {
            false
        } else {
            self.r_word0.get(endian) & R_SCATTERED != 0
        }
    }

    /// Return the fields of a plain relocation.
    pub fn info(self, endian: E) -> RelocationInfo {
        let r_address = self.r_word0.get(endian);
        let r_word1 = self.r_word1.get(endian);
        if endian.is_little_endian() {
            RelocationInfo {
                r_address,
                r_symbolnum: r_word1 & 0x00ff_ffff,
                r_pcrel: ((r_word1 >> 24) & 0x1) != 0,
                r_length: ((r_word1 >> 25) & 0x3) as u8,
                r_extern: ((r_word1 >> 27) & 0x1) != 0,
                r_type: (r_word1 >> 28) as u8,
            }
        } else {
            RelocationInfo {
                r_address,
                r_symbolnum: r_word1 >> 8,
                r_pcrel: ((r_word1 >> 7) & 0x1) != 0,
                r_length: ((r_word1 >> 5) & 0x3) as u8,
                r_extern: ((r_word1 >> 4) & 0x1) != 0,
                r_type: (r_word1 & 0xf) as u8,
            }
        }
    }

    /// Return the fields of a scattered relocation.
    pub fn scattered_info(self, endian: E) -> ScatteredRelocationInfo {
        let r_word0 = self.r_word0.get(endian);
        let r_value = self.r_word1.get(endian);
        ScatteredRelocationInfo {
            r_address: r_word0 & 0x00ff_ffff,
            r_type: ((r_word0 >> 24) & 0xf) as u8,
            r_length: ((r_word0 >> 28) & 0x3) as u8,
            r_pcrel: ((r_word0 >> 30) & 0x1) != 0,
            r_value,
        }
    }
}

/*
 * Format of a relocation entry of a Mach-O file.  Modified from the 4.3BSD
 * format.  The modifications from the original format were changing the value
 * of the r_symbolnum field for "local" (r_extern == 0) relocation entries.
 * This modification is required to support symbols in an arbitrary number of
 * sections not just the three sections (text, data and bss) in a 4.3BSD file.
 * Also the last 4 bits have had the r_type tag added to them.
 */

#[derive(Debug, Clone, Copy)]
pub struct RelocationInfo {
    /// offset in the section to what is being relocated
    pub r_address: u32,
    /// symbol index if r_extern == 1 or section ordinal if r_extern == 0
    pub r_symbolnum: u32,
    /// was relocated pc relative already
    pub r_pcrel: bool,
    /// 0=byte, 1=word, 2=long, 3=quad
    pub r_length: u8,
    /// does not include value of sym referenced
    pub r_extern: bool,
    /// if not 0, machine specific relocation type
    pub r_type: u8,
}

impl RelocationInfo {
    /// Combine the fields into a `Relocation`.
    pub fn relocation<E: Endian>(self, endian: E) -> Relocation<E> {
        let r_word0 = U32::new(endian, self.r_address);
        let r_word1 = U32::new(
            endian,
            if endian.is_little_endian() {
                self.r_symbolnum & 0x00ff_ffff
                    | u32::from(self.r_pcrel) << 24
                    | u32::from(self.r_length & 0x3) << 25
                    | u32::from(self.r_extern) << 27
                    | u32::from(self.r_type) << 28
            } else {
                self.r_symbolnum >> 8
                    | u32::from(self.r_pcrel) << 7
                    | u32::from(self.r_length & 0x3) << 5
                    | u32::from(self.r_extern) << 4
                    | u32::from(self.r_type) & 0xf
            },
        );
        Relocation { r_word0, r_word1 }
    }
}

/// absolute relocation type for Mach-O files
pub const R_ABS: u8 = 0;

/*
 * The r_address is not really the address as it's name indicates but an offset.
 * In 4.3BSD a.out objects this offset is from the start of the "segment" for
 * which relocation entry is for (text or data).  For Mach-O object files it is
 * also an offset but from the start of the "section" for which the relocation
 * entry is for.  See comments in <mach-o/loader.h> about the r_address feild
 * in images for used with the dynamic linker.
 *
 * In 4.3BSD a.out objects if r_extern is zero then r_symbolnum is an ordinal
 * for the segment the symbol being relocated is in.  These ordinals are the
 * symbol types N_TEXT, N_DATA, N_BSS or N_ABS.  In Mach-O object files these
 * ordinals refer to the sections in the object file in the order their section
 * structures appear in the headers of the object file they are in.  The first
 * section has the ordinal 1, the second 2, and so on.  This means that the
 * same ordinal in two different object files could refer to two different
 * sections.  And further could have still different ordinals when combined
 * by the link-editor.  The value R_ABS is used for relocation entries for
 * absolute symbols which need no further relocation.
 */

/*
 * For RISC machines some of the references are split across two instructions
 * and the instruction does not contain the complete value of the reference.
 * In these cases a second, or paired relocation entry, follows each of these
 * relocation entries, using a PAIR r_type, which contains the other part of the
 * reference not contained in the instruction.  This other part is stored in the
 * pair's r_address field.  The exact number of bits of the other part of the
 * reference store in the r_address field is dependent on the particular
 * relocation type for the particular architecture.
 */

/*
 * To make scattered loading by the link editor work correctly "local"
 * relocation entries can't be used when the item to be relocated is the value
 * of a symbol plus an offset (where the resulting expression is outside the
 * block the link editor is moving, a blocks are divided at symbol addresses).
 * In this case. where the item is a symbol value plus offset, the link editor
 * needs to know more than just the section the symbol was defined.  What is
 * needed is the actual value of the symbol without the offset so it can do the
 * relocation correctly based on where the value of the symbol got relocated to
 * not the value of the expression (with the offset added to the symbol value).
 * So for the NeXT 2.0 release no "local" relocation entries are ever used when
 * there is a non-zero offset added to a symbol.  The "external" and "local"
 * relocation entries remain unchanged.
 *
 * The implementation is quite messy given the compatibility with the existing
 * relocation entry format.  The ASSUMPTION is that a section will never be
 * bigger than 2**24 - 1 (0x00ffffff or 16,777,215) bytes.  This assumption
 * allows the r_address (which is really an offset) to fit in 24 bits and high
 * bit of the r_address field in the relocation_info structure to indicate
 * it is really a scattered_relocation_info structure.  Since these are only
 * used in places where "local" relocation entries are used and not where
 * "external" relocation entries are used the r_extern field has been removed.
 *
 * For scattered loading to work on a RISC machine where some of the references
 * are split across two instructions the link editor needs to be assured that
 * each reference has a unique 32 bit reference (that more than one reference is
 * NOT sharing the same high 16 bits for example) so it move each referenced
 * item independent of each other.  Some compilers guarantees this but the
 * compilers don't so scattered loading can be done on those that do guarantee
 * this.
 */

/// Bit set in `Relocation::r_word0` for scattered relocations.
pub const R_SCATTERED: u32 = 0x8000_0000;

#[derive(Debug, Clone, Copy)]
pub struct ScatteredRelocationInfo {
    /// offset in the section to what is being relocated
    pub r_address: u32,
    /// if not 0, machine specific relocation type
    pub r_type: u8,
    /// 0=byte, 1=word, 2=long, 3=quad
    pub r_length: u8,
    /// was relocated pc relative already
    pub r_pcrel: bool,
    /// the value the item to be relocated is referring to (without any offset added)
    pub r_value: u32,
}

impl ScatteredRelocationInfo {
    /// Combine the fields into a `Relocation`.
    pub fn relocation<E: Endian>(self, endian: E) -> Relocation<E> {
        let r_word0 = U32::new(
            endian,
            self.r_address & 0x00ff_ffff
                | u32::from(self.r_type & 0xf) << 24
                | u32::from(self.r_length & 0x3) << 28
                | u32::from(self.r_pcrel) << 30
                | R_SCATTERED,
        );
        let r_word1 = U32::new(endian, self.r_value);
        Relocation { r_word0, r_word1 }
    }
}

/*
 * Relocation types used in a generic implementation.  Relocation entries for
 * normal things use the generic relocation as described above and their r_type
 * is GENERIC_RELOC_VANILLA (a value of zero).
 *
 * Another type of generic relocation, GENERIC_RELOC_SECTDIFF, is to support
 * the difference of two symbols defined in different sections.  That is the
 * expression "symbol1 - symbol2 + constant" is a relocatable expression when
 * both symbols are defined in some section.  For this type of relocation the
 * both relocations entries are scattered relocation entries.  The value of
 * symbol1 is stored in the first relocation entry's r_value field and the
 * value of symbol2 is stored in the pair's r_value field.
 *
 * A special case for a prebound lazy pointer is needed to beable to set the
 * value of the lazy pointer back to its non-prebound state.  This is done
 * using the GENERIC_RELOC_PB_LA_PTR r_type.  This is a scattered relocation
 * entry where the r_value feild is the value of the lazy pointer not prebound.
 */
/// generic relocation as described above
pub const GENERIC_RELOC_VANILLA: u8 = 0;
/// Only follows a GENERIC_RELOC_SECTDIFF
pub const GENERIC_RELOC_PAIR: u8 = 1;
pub const GENERIC_RELOC_SECTDIFF: u8 = 2;
/// prebound lazy pointer
pub const GENERIC_RELOC_PB_LA_PTR: u8 = 3;
pub const GENERIC_RELOC_LOCAL_SECTDIFF: u8 = 4;
/// thread local variables
pub const GENERIC_RELOC_TLV: u8 = 5;

// Definitions from "/usr/include/mach-o/arm/reloc.h".

/*
 * Relocation types used in the arm implementation.  Relocation entries for
 * things other than instructions use the same generic relocation as described
 * in <mach-o/reloc.h> and their r_type is ARM_RELOC_VANILLA, one of the
 * *_SECTDIFF or the *_PB_LA_PTR types.  The rest of the relocation types are
 * for instructions.  Since they are for instructions the r_address field
 * indicates the 32 bit instruction that the relocation is to be performed on.
 */
/// generic relocation as described above
pub const ARM_RELOC_VANILLA: u8 = 0;
/// the second relocation entry of a pair
pub const ARM_RELOC_PAIR: u8 = 1;
/// a PAIR follows with subtract symbol value
pub const ARM_RELOC_SECTDIFF: u8 = 2;
/// like ARM_RELOC_SECTDIFF, but the symbol referenced was local.
pub const ARM_RELOC_LOCAL_SECTDIFF: u8 = 3;
/// prebound lazy pointer
pub const ARM_RELOC_PB_LA_PTR: u8 = 4;
/// 24 bit branch displacement (to a word address)
pub const ARM_RELOC_BR24: u8 = 5;
/// 22 bit branch displacement (to a half-word address)
pub const ARM_THUMB_RELOC_BR22: u8 = 6;
/// obsolete - a thumb 32-bit branch instruction possibly needing page-spanning branch workaround
pub const ARM_THUMB_32BIT_BRANCH: u8 = 7;

/*
 * For these two r_type relocations they always have a pair following them
 * and the r_length bits are used differently.  The encoding of the
 * r_length is as follows:
 * low bit of r_length:
 *  0 - :lower16: for movw instructions
 *  1 - :upper16: for movt instructions
 * high bit of r_length:
 *  0 - arm instructions
 *  1 - thumb instructions
 * the other half of the relocated expression is in the following pair
 * relocation entry in the the low 16 bits of r_address field.
 */
pub const ARM_RELOC_HALF: u8 = 8;
pub const ARM_RELOC_HALF_SECTDIFF: u8 = 9;

// Definitions from "/usr/include/mach-o/arm64/reloc.h".

/*
 * Relocation types used in the arm64 implementation.
 */
/// for pointers
pub const ARM64_RELOC_UNSIGNED: u8 = 0;
/// must be followed by a ARM64_RELOC_UNSIGNED
pub const ARM64_RELOC_SUBTRACTOR: u8 = 1;
/// a B/BL instruction with 26-bit displacement
pub const ARM64_RELOC_BRANCH26: u8 = 2;
/// pc-rel distance to page of target
pub const ARM64_RELOC_PAGE21: u8 = 3;
/// offset within page, scaled by r_length
pub const ARM64_RELOC_PAGEOFF12: u8 = 4;
/// pc-rel distance to page of GOT slot
pub const ARM64_RELOC_GOT_LOAD_PAGE21: u8 = 5;
/// offset within page of GOT slot, scaled by r_length
pub const ARM64_RELOC_GOT_LOAD_PAGEOFF12: u8 = 6;
/// for pointers to GOT slots
pub const ARM64_RELOC_POINTER_TO_GOT: u8 = 7;
/// pc-rel distance to page of TLVP slot
pub const ARM64_RELOC_TLVP_LOAD_PAGE21: u8 = 8;
/// offset within page of TLVP slot, scaled by r_length
pub const ARM64_RELOC_TLVP_LOAD_PAGEOFF12: u8 = 9;
/// must be followed by PAGE21 or PAGEOFF12
pub const ARM64_RELOC_ADDEND: u8 = 10;

// An arm64e authenticated pointer.
//
// Represents a pointer to a symbol (like ARM64_RELOC_UNSIGNED).
// Additionally, the resulting pointer is signed.  The signature is
// specified in the target location: the addend is restricted to the lower
// 32 bits (instead of the full 64 bits for ARM64_RELOC_UNSIGNED):
//
//   |63|62|61-51|50-49|  48  |47     -     32|31  -  0|
//   | 1| 0|  0  | key | addr | discriminator | addend |
//
// The key is one of:
//   IA: 00 IB: 01
//   DA: 10 DB: 11
//
// The discriminator field is used as extra signature diversification.
//
// The addr field indicates whether the target address should be blended
// into the discriminator.
//
pub const ARM64_RELOC_AUTHENTICATED_POINTER: u8 = 11;

// Definitions from "/usr/include/mach-o/ppc/reloc.h".

/*
 * Relocation types used in the ppc implementation.  Relocation entries for
 * things other than instructions use the same generic relocation as described
 * above and their r_type is RELOC_VANILLA.  The rest of the relocation types
 * are for instructions.  Since they are for instructions the r_address field
 * indicates the 32 bit instruction that the relocation is to be performed on.
 * The fields r_pcrel and r_length are ignored for non-RELOC_VANILLA r_types
 * except for PPC_RELOC_BR14.
 *
 * For PPC_RELOC_BR14 if the r_length is the unused value 3, then the branch was
 * statically predicted setting or clearing the Y-bit based on the sign of the
 * displacement or the opcode.  If this is the case the static linker must flip
 * the value of the Y-bit if the sign of the displacement changes for non-branch
 * always conditions.
 */
/// generic relocation as described above
pub const PPC_RELOC_VANILLA: u8 = 0;
/// the second relocation entry of a pair
pub const PPC_RELOC_PAIR: u8 = 1;
/// 14 bit branch displacement (to a word address)
pub const PPC_RELOC_BR14: u8 = 2;
/// 24 bit branch displacement (to a word address)
pub const PPC_RELOC_BR24: u8 = 3;
/// a PAIR follows with the low half
pub const PPC_RELOC_HI16: u8 = 4;
/// a PAIR follows with the high half
pub const PPC_RELOC_LO16: u8 = 5;
/// Same as the RELOC_HI16 except the low 16 bits and the high 16 bits are added together
/// with the low 16 bits sign extended first.  This means if bit 15 of the low 16 bits is
/// set the high 16 bits stored in the instruction will be adjusted.
pub const PPC_RELOC_HA16: u8 = 6;
/// Same as the LO16 except that the low 2 bits are not stored in the instruction and are
/// always zero.  This is used in double word load/store instructions.
pub const PPC_RELOC_LO14: u8 = 7;
/// a PAIR follows with subtract symbol value
pub const PPC_RELOC_SECTDIFF: u8 = 8;
/// prebound lazy pointer
pub const PPC_RELOC_PB_LA_PTR: u8 = 9;
/// section difference forms of above.  a PAIR
pub const PPC_RELOC_HI16_SECTDIFF: u8 = 10;
/// follows these with subtract symbol value
pub const PPC_RELOC_LO16_SECTDIFF: u8 = 11;
pub const PPC_RELOC_HA16_SECTDIFF: u8 = 12;
pub const PPC_RELOC_JBSR: u8 = 13;
pub const PPC_RELOC_LO14_SECTDIFF: u8 = 14;
/// like PPC_RELOC_SECTDIFF, but the symbol referenced was local.
pub const PPC_RELOC_LOCAL_SECTDIFF: u8 = 15;

// Definitions from "/usr/include/mach-o/x86_64/reloc.h".

/*
 * Relocations for x86_64 are a bit different than for other architectures in
 * Mach-O: Scattered relocations are not used.  Almost all relocations produced
 * by the compiler are external relocations.  An external relocation has the
 * r_extern bit set to 1 and the r_symbolnum field contains the symbol table
 * index of the target label.
 *
 * When the assembler is generating relocations, if the target label is a local
 * label (begins with 'L'), then the previous non-local label in the same
 * section is used as the target of the external relocation.  An addend is used
 * with the distance from that non-local label to the target label.  Only when
 * there is no previous non-local label in the section is an internal
 * relocation used.
 *
 * The addend (i.e. the 4 in _foo+4) is encoded in the instruction (Mach-O does
 * not have RELA relocations).  For PC-relative relocations, the addend is
 * stored directly in the instruction.  This is different from other Mach-O
 * architectures, which encode the addend minus the current section offset.
 *
 * The relocation types are:
 *
 * 	X86_64_RELOC_UNSIGNED	// for absolute addresses
 * 	X86_64_RELOC_SIGNED		// for signed 32-bit displacement
 * 	X86_64_RELOC_BRANCH		// a CALL/JMP instruction with 32-bit displacement
 * 	X86_64_RELOC_GOT_LOAD	// a MOVQ load of a GOT entry
 * 	X86_64_RELOC_GOT		// other GOT references
 * 	X86_64_RELOC_SUBTRACTOR	// must be followed by a X86_64_RELOC_UNSIGNED
 *
 * The following are sample assembly instructions, followed by the relocation
 * and section content they generate in an object file:
 *
 * 	call _foo
 * 		r_type=X86_64_RELOC_BRANCH, r_length=2, r_extern=1, r_pcrel=1, r_symbolnum=_foo
 * 		E8 00 00 00 00
 *
 * 	call _foo+4
 * 		r_type=X86_64_RELOC_BRANCH, r_length=2, r_extern=1, r_pcrel=1, r_symbolnum=_foo
 * 		E8 04 00 00 00
 *
 * 	movq _foo@GOTPCREL(%rip), %rax
 * 		r_type=X86_64_RELOC_GOT_LOAD, r_length=2, r_extern=1, r_pcrel=1, r_symbolnum=_foo
 * 		48 8B 05 00 00 00 00
 *
 * 	pushq _foo@GOTPCREL(%rip)
 * 		r_type=X86_64_RELOC_GOT, r_length=2, r_extern=1, r_pcrel=1, r_symbolnum=_foo
 * 		FF 35 00 00 00 00
 *
 * 	movl _foo(%rip), %eax
 * 		r_type=X86_64_RELOC_SIGNED, r_length=2, r_extern=1, r_pcrel=1, r_symbolnum=_foo
 * 		8B 05 00 00 00 00
 *
 * 	movl _foo+4(%rip), %eax
 * 		r_type=X86_64_RELOC_SIGNED, r_length=2, r_extern=1, r_pcrel=1, r_symbolnum=_foo
 * 		8B 05 04 00 00 00
 *
 * 	movb  $0x12, _foo(%rip)
 * 		r_type=X86_64_RELOC_SIGNED, r_length=2, r_extern=1, r_pcrel=1, r_symbolnum=_foo
 * 		C6 05 FF FF FF FF 12
 *
 * 	movl  $0x12345678, _foo(%rip)
 * 		r_type=X86_64_RELOC_SIGNED, r_length=2, r_extern=1, r_pcrel=1, r_symbolnum=_foo
 * 		C7 05 FC FF FF FF 78 56 34 12
 *
 * 	.quad _foo
 * 		r_type=X86_64_RELOC_UNSIGNED, r_length=3, r_extern=1, r_pcrel=0, r_symbolnum=_foo
 * 		00 00 00 00 00 00 00 00
 *
 * 	.quad _foo+4
 * 		r_type=X86_64_RELOC_UNSIGNED, r_length=3, r_extern=1, r_pcrel=0, r_symbolnum=_foo
 * 		04 00 00 00 00 00 00 00
 *
 * 	.quad _foo - _bar
 * 		r_type=X86_64_RELOC_SUBTRACTOR, r_length=3, r_extern=1, r_pcrel=0, r_symbolnum=_bar
 * 		r_type=X86_64_RELOC_UNSIGNED, r_length=3, r_extern=1, r_pcrel=0, r_symbolnum=_foo
 * 		00 00 00 00 00 00 00 00
 *
 * 	.quad _foo - _bar + 4
 * 		r_type=X86_64_RELOC_SUBTRACTOR, r_length=3, r_extern=1, r_pcrel=0, r_symbolnum=_bar
 * 		r_type=X86_64_RELOC_UNSIGNED, r_length=3, r_extern=1, r_pcrel=0, r_symbolnum=_foo
 * 		04 00 00 00 00 00 00 00
 *
 * 	.long _foo - _bar
 * 		r_type=X86_64_RELOC_SUBTRACTOR, r_length=2, r_extern=1, r_pcrel=0, r_symbolnum=_bar
 * 		r_type=X86_64_RELOC_UNSIGNED, r_length=2, r_extern=1, r_pcrel=0, r_symbolnum=_foo
 * 		00 00 00 00
 *
 * 	lea L1(%rip), %rax
 * 		r_type=X86_64_RELOC_SIGNED, r_length=2, r_extern=1, r_pcrel=1, r_symbolnum=_prev
 * 		48 8d 05 12 00 00 00
 * 		// assumes _prev is the first non-local label 0x12 bytes before L1
 *
 * 	lea L0(%rip), %rax
 * 		r_type=X86_64_RELOC_SIGNED, r_length=2, r_extern=0, r_pcrel=1, r_symbolnum=3
 * 		48 8d 05 56 00 00 00
 *		// assumes L0 is in third section and there is no previous non-local label.
 *		// The rip-relative-offset of 0x00000056 is L0-address_of_next_instruction.
 *		// address_of_next_instruction is the address of the relocation + 4.
 *
 *     add     $6,L0(%rip)
 *             r_type=X86_64_RELOC_SIGNED_1, r_length=2, r_extern=0, r_pcrel=1, r_symbolnum=3
 *		83 05 18 00 00 00 06
 *		// assumes L0 is in third section and there is no previous non-local label.
 *		// The rip-relative-offset of 0x00000018 is L0-address_of_next_instruction.
 *		// address_of_next_instruction is the address of the relocation + 4 + 1.
 *		// The +1 comes from SIGNED_1.  This is used because the relocation is not
 *		// at the end of the instruction.
 *
 * 	.quad L1
 * 		r_type=X86_64_RELOC_UNSIGNED, r_length=3, r_extern=1, r_pcrel=0, r_symbolnum=_prev
 * 		12 00 00 00 00 00 00 00
 * 		// assumes _prev is the first non-local label 0x12 bytes before L1
 *
 * 	.quad L0
 * 		r_type=X86_64_RELOC_UNSIGNED, r_length=3, r_extern=0, r_pcrel=0, r_symbolnum=3
 * 		56 00 00 00 00 00 00 00
 * 		// assumes L0 is in third section, has an address of 0x00000056 in .o
 * 		// file, and there is no previous non-local label
 *
 * 	.quad _foo - .
 * 		r_type=X86_64_RELOC_SUBTRACTOR, r_length=3, r_extern=1, r_pcrel=0, r_symbolnum=_prev
 * 		r_type=X86_64_RELOC_UNSIGNED, r_length=3, r_extern=1, r_pcrel=0, r_symbolnum=_foo
 * 		EE FF FF FF FF FF FF FF
 * 		// assumes _prev is the first non-local label 0x12 bytes before this
 * 		// .quad
 *
 * 	.quad _foo - L1
 * 		r_type=X86_64_RELOC_SUBTRACTOR, r_length=3, r_extern=1, r_pcrel=0, r_symbolnum=_prev
 * 		r_type=X86_64_RELOC_UNSIGNED, r_length=3, r_extern=1, r_pcrel=0, r_symbolnum=_foo
 * 		EE FF FF FF FF FF FF FF
 * 		// assumes _prev is the first non-local label 0x12 bytes before L1
 *
 * 	.quad L1 - _prev
 * 		// No relocations.  This is an assembly time constant.
 * 		12 00 00 00 00 00 00 00
 * 		// assumes _prev is the first non-local label 0x12 bytes before L1
 *
 *
 *
 * In final linked images, there are only two valid relocation kinds:
 *
 *     r_type=X86_64_RELOC_UNSIGNED, r_length=3, r_pcrel=0, r_extern=1, r_symbolnum=sym_index
 *	This tells dyld to add the address of a symbol to a pointer sized (8-byte)
 *  piece of data (i.e on disk the 8-byte piece of data contains the addend). The
 *  r_symbolnum contains the index into the symbol table of the target symbol.
 *
 *     r_type=X86_64_RELOC_UNSIGNED, r_length=3, r_pcrel=0, r_extern=0, r_symbolnum=0
 * This tells dyld to adjust the pointer sized (8-byte) piece of data by the amount
 * the containing image was loaded from its base address (e.g. slide).
 *
 */
/// for absolute addresses
pub const X86_64_RELOC_UNSIGNED: u8 = 0;
/// for signed 32-bit displacement
pub const X86_64_RELOC_SIGNED: u8 = 1;
/// a CALL/JMP instruction with 32-bit displacement
pub const X86_64_RELOC_BRANCH: u8 = 2;
/// a MOVQ load of a GOT entry
pub const X86_64_RELOC_GOT_LOAD: u8 = 3;
/// other GOT references
pub const X86_64_RELOC_GOT: u8 = 4;
/// must be followed by a X86_64_RELOC_UNSIGNED
pub const X86_64_RELOC_SUBTRACTOR: u8 = 5;
/// for signed 32-bit displacement with a -1 addend
pub const X86_64_RELOC_SIGNED_1: u8 = 6;
/// for signed 32-bit displacement with a -2 addend
pub const X86_64_RELOC_SIGNED_2: u8 = 7;
/// for signed 32-bit displacement with a -4 addend
pub const X86_64_RELOC_SIGNED_4: u8 = 8;
/// for thread local variables
pub const X86_64_RELOC_TLV: u8 = 9;

unsafe_impl_pod!(FatHeader, FatArch32, FatArch64,);
unsafe_impl_endian_pod!(
    DyldCacheHeader,
    DyldCacheMappingInfo,
    DyldCacheImageInfo,
    DyldSubCacheEntryV1,
    DyldSubCacheEntryV2,
    MachHeader32,
    MachHeader64,
    LoadCommand,
    LcStr,
    SegmentCommand32,
    SegmentCommand64,
    Section32,
    Section64,
    Fvmlib,
    FvmlibCommand,
    Dylib,
    DylibCommand,
    SubFrameworkCommand,
    SubClientCommand,
    SubUmbrellaCommand,
    SubLibraryCommand,
    PreboundDylibCommand,
    DylinkerCommand,
    ThreadCommand,
    RoutinesCommand32,
    RoutinesCommand64,
    SymtabCommand,
    DysymtabCommand,
    DylibTableOfContents,
    DylibModule32,
    DylibModule64,
    DylibReference,
    TwolevelHintsCommand,
    TwolevelHint,
    PrebindCksumCommand,
    UuidCommand,
    RpathCommand,
    LinkeditDataCommand,
    FilesetEntryCommand,
    EncryptionInfoCommand32,
    EncryptionInfoCommand64,
    VersionMinCommand,
    BuildVersionCommand,
    BuildToolVersion,
    DyldInfoCommand,
    LinkerOptionCommand,
    SymsegCommand,
    IdentCommand,
    FvmfileCommand,
    EntryPointCommand,
    SourceVersionCommand,
    DataInCodeEntry,
    //TlvDescriptor,
    NoteCommand,
    Nlist32,
    Nlist64,
    Relocation,
);
