//! ELF definitions.
//!
//! These definitions are independent of read/write support, although we do implement
//! some traits useful for those.
//!
//! This module is the equivalent of /usr/include/elf.h, and is based heavily on it.

#![allow(missing_docs)]
#![allow(clippy::identity_op)]

use crate::endian::{Endian, U32Bytes, U64Bytes, I32, I64, U16, U32, U64};
use crate::pod::Pod;

/// The header at the start of every 32-bit ELF file.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FileHeader32<E: Endian> {
    /// Magic number and other information.
    pub e_ident: Ident,
    /// Object file type. One of the `ET_*` constants.
    pub e_type: U16<E>,
    /// Architecture. One of the `EM_*` constants.
    pub e_machine: U16<E>,
    /// Object file version. Must be `EV_CURRENT`.
    pub e_version: U32<E>,
    /// Entry point virtual address.
    pub e_entry: U32<E>,
    /// Program header table file offset.
    pub e_phoff: U32<E>,
    /// Section header table file offset.
    pub e_shoff: U32<E>,
    /// Processor-specific flags.
    ///
    /// A combination of the `EF_*` constants.
    pub e_flags: U32<E>,
    /// Size in bytes of this header.
    pub e_ehsize: U16<E>,
    /// Program header table entry size.
    pub e_phentsize: U16<E>,
    /// Program header table entry count.
    ///
    /// If the count is greater than or equal to `PN_XNUM` then this field is set to
    /// `PN_XNUM` and the count is stored in the `sh_info` field of section 0.
    pub e_phnum: U16<E>,
    /// Section header table entry size.
    pub e_shentsize: U16<E>,
    /// Section header table entry count.
    ///
    /// If the count is greater than or equal to `SHN_LORESERVE` then this field is set to
    /// `0` and the count is stored in the `sh_size` field of section 0.
    /// first section header.
    pub e_shnum: U16<E>,
    /// Section header string table index.
    ///
    /// If the index is greater than or equal to `SHN_LORESERVE` then this field is set to
    /// `SHN_XINDEX` and the index is stored in the `sh_link` field of section 0.
    pub e_shstrndx: U16<E>,
}

/// The header at the start of every 64-bit ELF file.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FileHeader64<E: Endian> {
    /// Magic number and other information.
    pub e_ident: Ident,
    /// Object file type. One of the `ET_*` constants.
    pub e_type: U16<E>,
    /// Architecture. One of the `EM_*` constants.
    pub e_machine: U16<E>,
    /// Object file version. Must be `EV_CURRENT`.
    pub e_version: U32<E>,
    /// Entry point virtual address.
    pub e_entry: U64<E>,
    /// Program header table file offset.
    pub e_phoff: U64<E>,
    /// Section header table file offset.
    pub e_shoff: U64<E>,
    /// Processor-specific flags.
    ///
    /// A combination of the `EF_*` constants.
    pub e_flags: U32<E>,
    /// Size in bytes of this header.
    pub e_ehsize: U16<E>,
    /// Program header table entry size.
    pub e_phentsize: U16<E>,
    /// Program header table entry count.
    ///
    /// If the count is greater than or equal to `PN_XNUM` then this field is set to
    /// `PN_XNUM` and the count is stored in the `sh_info` field of section 0.
    pub e_phnum: U16<E>,
    /// Section header table entry size.
    pub e_shentsize: U16<E>,
    /// Section header table entry count.
    ///
    /// If the count is greater than or equal to `SHN_LORESERVE` then this field is set to
    /// `0` and the count is stored in the `sh_size` field of section 0.
    /// first section header.
    pub e_shnum: U16<E>,
    /// Section header string table index.
    ///
    /// If the index is greater than or equal to `SHN_LORESERVE` then this field is set to
    /// `SHN_XINDEX` and the index is stored in the `sh_link` field of section 0.
    pub e_shstrndx: U16<E>,
}

/// Magic number and other information.
///
/// Contained in the file header.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Ident {
    /// Magic number. Must be `ELFMAG`.
    pub magic: [u8; 4],
    /// File class. One of the `ELFCLASS*` constants.
    pub class: u8,
    /// Data encoding. One of the `ELFDATA*` constants.
    pub data: u8,
    /// ELF version. Must be `EV_CURRENT`.
    pub version: u8,
    /// OS ABI identification. One of the `ELFOSABI*` constants.
    pub os_abi: u8,
    /// ABI version.
    ///
    /// The meaning of this field depends on the `os_abi` value.
    pub abi_version: u8,
    /// Padding bytes.
    pub padding: [u8; 7],
}

/// File identification bytes stored in `Ident::magic`.
pub const ELFMAG: [u8; 4] = [0x7f, b'E', b'L', b'F'];

// Values for `Ident::class`.
/// Invalid class.
pub const ELFCLASSNONE: u8 = 0;
/// 32-bit object.
pub const ELFCLASS32: u8 = 1;
/// 64-bit object.
pub const ELFCLASS64: u8 = 2;

// Values for `Ident::data`.
/// Invalid data encoding.
pub const ELFDATANONE: u8 = 0;
/// 2's complement, little endian.
pub const ELFDATA2LSB: u8 = 1;
/// 2's complement, big endian.
pub const ELFDATA2MSB: u8 = 2;

// Values for `Ident::os_abi`.
/// UNIX System V ABI.
pub const ELFOSABI_NONE: u8 = 0;
/// UNIX System V ABI.
///
/// Alias.
pub const ELFOSABI_SYSV: u8 = 0;
/// HP-UX.
pub const ELFOSABI_HPUX: u8 = 1;
/// NetBSD.
pub const ELFOSABI_NETBSD: u8 = 2;
/// Object uses GNU ELF extensions.
pub const ELFOSABI_GNU: u8 = 3;
/// Object uses GNU ELF extensions.
///
/// Compatibility alias.
pub const ELFOSABI_LINUX: u8 = ELFOSABI_GNU;
/// GNU/Hurd.
pub const ELFOSABI_HURD: u8 = 4;
/// Sun Solaris.
pub const ELFOSABI_SOLARIS: u8 = 6;
/// IBM AIX.
pub const ELFOSABI_AIX: u8 = 7;
/// SGI Irix.
pub const ELFOSABI_IRIX: u8 = 8;
/// FreeBSD.
pub const ELFOSABI_FREEBSD: u8 = 9;
/// Compaq TRU64 UNIX.
pub const ELFOSABI_TRU64: u8 = 10;
/// Novell Modesto.
pub const ELFOSABI_MODESTO: u8 = 11;
/// OpenBSD.
pub const ELFOSABI_OPENBSD: u8 = 12;
/// OpenVMS.
pub const ELFOSABI_OPENVMS: u8 = 13;
/// Hewlett-Packard Non-Stop Kernel.
pub const ELFOSABI_NSK: u8 = 14;
/// AROS
pub const ELFOSABI_AROS: u8 = 15;
/// FenixOS
pub const ELFOSABI_FENIXOS: u8 = 16;
/// Nuxi CloudABI
pub const ELFOSABI_CLOUDABI: u8 = 17;
/// ARM EABI.
pub const ELFOSABI_ARM_AEABI: u8 = 64;
/// ARM.
pub const ELFOSABI_ARM: u8 = 97;
/// Standalone (embedded) application.
pub const ELFOSABI_STANDALONE: u8 = 255;

// Values for `FileHeader*::e_type`.
/// No file type.
pub const ET_NONE: u16 = 0;
/// Relocatable file.
pub const ET_REL: u16 = 1;
/// Executable file.
pub const ET_EXEC: u16 = 2;
/// Shared object file.
pub const ET_DYN: u16 = 3;
/// Core file.
pub const ET_CORE: u16 = 4;
/// OS-specific range start.
pub const ET_LOOS: u16 = 0xfe00;
/// OS-specific range end.
pub const ET_HIOS: u16 = 0xfeff;
/// Processor-specific range start.
pub const ET_LOPROC: u16 = 0xff00;
/// Processor-specific range end.
pub const ET_HIPROC: u16 = 0xffff;

// Values for `FileHeader*::e_machine`.
/// No machine
pub const EM_NONE: u16 = 0;
/// AT&T WE 32100
pub const EM_M32: u16 = 1;
/// SUN SPARC
pub const EM_SPARC: u16 = 2;
/// Intel 80386
pub const EM_386: u16 = 3;
/// Motorola m68k family
pub const EM_68K: u16 = 4;
/// Motorola m88k family
pub const EM_88K: u16 = 5;
/// Intel MCU
pub const EM_IAMCU: u16 = 6;
/// Intel 80860
pub const EM_860: u16 = 7;
/// MIPS R3000 big-endian
pub const EM_MIPS: u16 = 8;
/// IBM System/370
pub const EM_S370: u16 = 9;
/// MIPS R3000 little-endian
pub const EM_MIPS_RS3_LE: u16 = 10;
/// HPPA
pub const EM_PARISC: u16 = 15;
/// Fujitsu VPP500
pub const EM_VPP500: u16 = 17;
/// Sun's "v8plus"
pub const EM_SPARC32PLUS: u16 = 18;
/// Intel 80960
pub const EM_960: u16 = 19;
/// PowerPC
pub const EM_PPC: u16 = 20;
/// PowerPC 64-bit
pub const EM_PPC64: u16 = 21;
/// IBM S390
pub const EM_S390: u16 = 22;
/// IBM SPU/SPC
pub const EM_SPU: u16 = 23;
/// NEC V800 series
pub const EM_V800: u16 = 36;
/// Fujitsu FR20
pub const EM_FR20: u16 = 37;
/// TRW RH-32
pub const EM_RH32: u16 = 38;
/// Motorola RCE
pub const EM_RCE: u16 = 39;
/// ARM
pub const EM_ARM: u16 = 40;
/// Digital Alpha
pub const EM_FAKE_ALPHA: u16 = 41;
/// Hitachi SH
pub const EM_SH: u16 = 42;
/// SPARC v9 64-bit
pub const EM_SPARCV9: u16 = 43;
/// Siemens Tricore
pub const EM_TRICORE: u16 = 44;
/// Argonaut RISC Core
pub const EM_ARC: u16 = 45;
/// Hitachi H8/300
pub const EM_H8_300: u16 = 46;
/// Hitachi H8/300H
pub const EM_H8_300H: u16 = 47;
/// Hitachi H8S
pub const EM_H8S: u16 = 48;
/// Hitachi H8/500
pub const EM_H8_500: u16 = 49;
/// Intel Merced
pub const EM_IA_64: u16 = 50;
/// Stanford MIPS-X
pub const EM_MIPS_X: u16 = 51;
/// Motorola Coldfire
pub const EM_COLDFIRE: u16 = 52;
/// Motorola M68HC12
pub const EM_68HC12: u16 = 53;
/// Fujitsu MMA Multimedia Accelerator
pub const EM_MMA: u16 = 54;
/// Siemens PCP
pub const EM_PCP: u16 = 55;
/// Sony nCPU embeeded RISC
pub const EM_NCPU: u16 = 56;
/// Denso NDR1 microprocessor
pub const EM_NDR1: u16 = 57;
/// Motorola Start*Core processor
pub const EM_STARCORE: u16 = 58;
/// Toyota ME16 processor
pub const EM_ME16: u16 = 59;
/// STMicroelectronic ST100 processor
pub const EM_ST100: u16 = 60;
/// Advanced Logic Corp. Tinyj emb.fam
pub const EM_TINYJ: u16 = 61;
/// AMD x86-64 architecture
pub const EM_X86_64: u16 = 62;
/// Sony DSP Processor
pub const EM_PDSP: u16 = 63;
/// Digital PDP-10
pub const EM_PDP10: u16 = 64;
/// Digital PDP-11
pub const EM_PDP11: u16 = 65;
/// Siemens FX66 microcontroller
pub const EM_FX66: u16 = 66;
/// STMicroelectronics ST9+ 8/16 mc
pub const EM_ST9PLUS: u16 = 67;
/// STmicroelectronics ST7 8 bit mc
pub const EM_ST7: u16 = 68;
/// Motorola MC68HC16 microcontroller
pub const EM_68HC16: u16 = 69;
/// Motorola MC68HC11 microcontroller
pub const EM_68HC11: u16 = 70;
/// Motorola MC68HC08 microcontroller
pub const EM_68HC08: u16 = 71;
/// Motorola MC68HC05 microcontroller
pub const EM_68HC05: u16 = 72;
/// Silicon Graphics SVx
pub const EM_SVX: u16 = 73;
/// STMicroelectronics ST19 8 bit mc
pub const EM_ST19: u16 = 74;
/// Digital VAX
pub const EM_VAX: u16 = 75;
/// Axis Communications 32-bit emb.proc
pub const EM_CRIS: u16 = 76;
/// Infineon Technologies 32-bit emb.proc
pub const EM_JAVELIN: u16 = 77;
/// Element 14 64-bit DSP Processor
pub const EM_FIREPATH: u16 = 78;
/// LSI Logic 16-bit DSP Processor
pub const EM_ZSP: u16 = 79;
/// Donald Knuth's educational 64-bit proc
pub const EM_MMIX: u16 = 80;
/// Harvard University machine-independent object files
pub const EM_HUANY: u16 = 81;
/// SiTera Prism
pub const EM_PRISM: u16 = 82;
/// Atmel AVR 8-bit microcontroller
pub const EM_AVR: u16 = 83;
/// Fujitsu FR30
pub const EM_FR30: u16 = 84;
/// Mitsubishi D10V
pub const EM_D10V: u16 = 85;
/// Mitsubishi D30V
pub const EM_D30V: u16 = 86;
/// NEC v850
pub const EM_V850: u16 = 87;
/// Mitsubishi M32R
pub const EM_M32R: u16 = 88;
/// Matsushita MN10300
pub const EM_MN10300: u16 = 89;
/// Matsushita MN10200
pub const EM_MN10200: u16 = 90;
/// picoJava
pub const EM_PJ: u16 = 91;
/// OpenRISC 32-bit embedded processor
pub const EM_OPENRISC: u16 = 92;
/// ARC International ARCompact
pub const EM_ARC_COMPACT: u16 = 93;
/// Tensilica Xtensa Architecture
pub const EM_XTENSA: u16 = 94;
/// Alphamosaic VideoCore
pub const EM_VIDEOCORE: u16 = 95;
/// Thompson Multimedia General Purpose Proc
pub const EM_TMM_GPP: u16 = 96;
/// National Semi. 32000
pub const EM_NS32K: u16 = 97;
/// Tenor Network TPC
pub const EM_TPC: u16 = 98;
/// Trebia SNP 1000
pub const EM_SNP1K: u16 = 99;
/// STMicroelectronics ST200
pub const EM_ST200: u16 = 100;
/// Ubicom IP2xxx
pub const EM_IP2K: u16 = 101;
/// MAX processor
pub const EM_MAX: u16 = 102;
/// National Semi. CompactRISC
pub const EM_CR: u16 = 103;
/// Fujitsu F2MC16
pub const EM_F2MC16: u16 = 104;
/// Texas Instruments msp430
pub const EM_MSP430: u16 = 105;
/// Analog Devices Blackfin DSP
pub const EM_BLACKFIN: u16 = 106;
/// Seiko Epson S1C33 family
pub const EM_SE_C33: u16 = 107;
/// Sharp embedded microprocessor
pub const EM_SEP: u16 = 108;
/// Arca RISC
pub const EM_ARCA: u16 = 109;
/// PKU-Unity & MPRC Peking Uni. mc series
pub const EM_UNICORE: u16 = 110;
/// eXcess configurable cpu
pub const EM_EXCESS: u16 = 111;
/// Icera Semi. Deep Execution Processor
pub const EM_DXP: u16 = 112;
/// Altera Nios II
pub const EM_ALTERA_NIOS2: u16 = 113;
/// National Semi. CompactRISC CRX
pub const EM_CRX: u16 = 114;
/// Motorola XGATE
pub const EM_XGATE: u16 = 115;
/// Infineon C16x/XC16x
pub const EM_C166: u16 = 116;
/// Renesas M16C
pub const EM_M16C: u16 = 117;
/// Microchip Technology dsPIC30F
pub const EM_DSPIC30F: u16 = 118;
/// Freescale Communication Engine RISC
pub const EM_CE: u16 = 119;
/// Renesas M32C
pub const EM_M32C: u16 = 120;
/// Altium TSK3000
pub const EM_TSK3000: u16 = 131;
/// Freescale RS08
pub const EM_RS08: u16 = 132;
/// Analog Devices SHARC family
pub const EM_SHARC: u16 = 133;
/// Cyan Technology eCOG2
pub const EM_ECOG2: u16 = 134;
/// Sunplus S+core7 RISC
pub const EM_SCORE7: u16 = 135;
/// New Japan Radio (NJR) 24-bit DSP
pub const EM_DSP24: u16 = 136;
/// Broadcom VideoCore III
pub const EM_VIDEOCORE3: u16 = 137;
/// RISC for Lattice FPGA
pub const EM_LATTICEMICO32: u16 = 138;
/// Seiko Epson C17
pub const EM_SE_C17: u16 = 139;
/// Texas Instruments TMS320C6000 DSP
pub const EM_TI_C6000: u16 = 140;
/// Texas Instruments TMS320C2000 DSP
pub const EM_TI_C2000: u16 = 141;
/// Texas Instruments TMS320C55x DSP
pub const EM_TI_C5500: u16 = 142;
/// Texas Instruments App. Specific RISC
pub const EM_TI_ARP32: u16 = 143;
/// Texas Instruments Prog. Realtime Unit
pub const EM_TI_PRU: u16 = 144;
/// STMicroelectronics 64bit VLIW DSP
pub const EM_MMDSP_PLUS: u16 = 160;
/// Cypress M8C
pub const EM_CYPRESS_M8C: u16 = 161;
/// Renesas R32C
pub const EM_R32C: u16 = 162;
/// NXP Semi. TriMedia
pub const EM_TRIMEDIA: u16 = 163;
/// QUALCOMM Hexagon
pub const EM_HEXAGON: u16 = 164;
/// Intel 8051 and variants
pub const EM_8051: u16 = 165;
/// STMicroelectronics STxP7x
pub const EM_STXP7X: u16 = 166;
/// Andes Tech. compact code emb. RISC
pub const EM_NDS32: u16 = 167;
/// Cyan Technology eCOG1X
pub const EM_ECOG1X: u16 = 168;
/// Dallas Semi. MAXQ30 mc
pub const EM_MAXQ30: u16 = 169;
/// New Japan Radio (NJR) 16-bit DSP
pub const EM_XIMO16: u16 = 170;
/// M2000 Reconfigurable RISC
pub const EM_MANIK: u16 = 171;
/// Cray NV2 vector architecture
pub const EM_CRAYNV2: u16 = 172;
/// Renesas RX
pub const EM_RX: u16 = 173;
/// Imagination Tech. META
pub const EM_METAG: u16 = 174;
/// MCST Elbrus
pub const EM_MCST_ELBRUS: u16 = 175;
/// Cyan Technology eCOG16
pub const EM_ECOG16: u16 = 176;
/// National Semi. CompactRISC CR16
pub const EM_CR16: u16 = 177;
/// Freescale Extended Time Processing Unit
pub const EM_ETPU: u16 = 178;
/// Infineon Tech. SLE9X
pub const EM_SLE9X: u16 = 179;
/// Intel L10M
pub const EM_L10M: u16 = 180;
/// Intel K10M
pub const EM_K10M: u16 = 181;
/// ARM AARCH64
pub const EM_AARCH64: u16 = 183;
/// Amtel 32-bit microprocessor
pub const EM_AVR32: u16 = 185;
/// STMicroelectronics STM8
pub const EM_STM8: u16 = 186;
/// Tileta TILE64
pub const EM_TILE64: u16 = 187;
/// Tilera TILEPro
pub const EM_TILEPRO: u16 = 188;
/// Xilinx MicroBlaze
pub const EM_MICROBLAZE: u16 = 189;
/// NVIDIA CUDA
pub const EM_CUDA: u16 = 190;
/// Tilera TILE-Gx
pub const EM_TILEGX: u16 = 191;
/// CloudShield
pub const EM_CLOUDSHIELD: u16 = 192;
/// KIPO-KAIST Core-A 1st gen.
pub const EM_COREA_1ST: u16 = 193;
/// KIPO-KAIST Core-A 2nd gen.
pub const EM_COREA_2ND: u16 = 194;
/// Synopsys ARCompact V2
pub const EM_ARC_COMPACT2: u16 = 195;
/// Open8 RISC
pub const EM_OPEN8: u16 = 196;
/// Renesas RL78
pub const EM_RL78: u16 = 197;
/// Broadcom VideoCore V
pub const EM_VIDEOCORE5: u16 = 198;
/// Renesas 78KOR
pub const EM_78KOR: u16 = 199;
/// Freescale 56800EX DSC
pub const EM_56800EX: u16 = 200;
/// Beyond BA1
pub const EM_BA1: u16 = 201;
/// Beyond BA2
pub const EM_BA2: u16 = 202;
/// XMOS xCORE
pub const EM_XCORE: u16 = 203;
/// Microchip 8-bit PIC(r)
pub const EM_MCHP_PIC: u16 = 204;
/// KM211 KM32
pub const EM_KM32: u16 = 210;
/// KM211 KMX32
pub const EM_KMX32: u16 = 211;
/// KM211 KMX16
pub const EM_EMX16: u16 = 212;
/// KM211 KMX8
pub const EM_EMX8: u16 = 213;
/// KM211 KVARC
pub const EM_KVARC: u16 = 214;
/// Paneve CDP
pub const EM_CDP: u16 = 215;
/// Cognitive Smart Memory Processor
pub const EM_COGE: u16 = 216;
/// Bluechip CoolEngine
pub const EM_COOL: u16 = 217;
/// Nanoradio Optimized RISC
pub const EM_NORC: u16 = 218;
/// CSR Kalimba
pub const EM_CSR_KALIMBA: u16 = 219;
/// Zilog Z80
pub const EM_Z80: u16 = 220;
/// Controls and Data Services VISIUMcore
pub const EM_VISIUM: u16 = 221;
/// FTDI Chip FT32
pub const EM_FT32: u16 = 222;
/// Moxie processor
pub const EM_MOXIE: u16 = 223;
/// AMD GPU
pub const EM_AMDGPU: u16 = 224;
/// RISC-V
pub const EM_RISCV: u16 = 243;
/// Linux BPF -- in-kernel virtual machine
pub const EM_BPF: u16 = 247;
/// C-SKY
pub const EM_CSKY: u16 = 252;
/// Loongson LoongArch
pub const EM_LOONGARCH: u16 = 258;
/// Solana Binary Format
pub const EM_SBF: u16 = 263;
/// Digital Alpha
pub const EM_ALPHA: u16 = 0x9026;

// Values for `FileHeader*::e_version` and `Ident::version`.
/// Invalid ELF version.
pub const EV_NONE: u8 = 0;
/// Current ELF version.
pub const EV_CURRENT: u8 = 1;

/// Section header.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SectionHeader32<E: Endian> {
    /// Section name.
    ///
    /// This is an offset into the section header string table.
    pub sh_name: U32<E>,
    /// Section type. One of the `SHT_*` constants.
    pub sh_type: U32<E>,
    /// Section flags. A combination of the `SHF_*` constants.
    pub sh_flags: U32<E>,
    /// Section virtual address at execution.
    pub sh_addr: U32<E>,
    /// Section file offset.
    pub sh_offset: U32<E>,
    /// Section size in bytes.
    pub sh_size: U32<E>,
    /// Link to another section.
    ///
    /// The section relationship depends on the `sh_type` value.
    pub sh_link: U32<E>,
    /// Additional section information.
    ///
    /// The meaning of this field depends on the `sh_type` value.
    pub sh_info: U32<E>,
    /// Section alignment.
    pub sh_addralign: U32<E>,
    /// Entry size if the section holds a table.
    pub sh_entsize: U32<E>,
}

/// Section header.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SectionHeader64<E: Endian> {
    /// Section name.
    ///
    /// This is an offset into the section header string table.
    pub sh_name: U32<E>,
    /// Section type. One of the `SHT_*` constants.
    pub sh_type: U32<E>,
    /// Section flags. A combination of the `SHF_*` constants.
    pub sh_flags: U64<E>,
    /// Section virtual address at execution.
    pub sh_addr: U64<E>,
    /// Section file offset.
    pub sh_offset: U64<E>,
    /// Section size in bytes.
    pub sh_size: U64<E>,
    /// Link to another section.
    ///
    /// The section relationship depends on the `sh_type` value.
    pub sh_link: U32<E>,
    /// Additional section information.
    ///
    /// The meaning of this field depends on the `sh_type` value.
    pub sh_info: U32<E>,
    /// Section alignment.
    pub sh_addralign: U64<E>,
    /// Entry size if the section holds a table.
    pub sh_entsize: U64<E>,
}

// Special values for section indices.
/// Undefined section.
pub const SHN_UNDEF: u16 = 0;
/// OS-specific range start.
/// Start of reserved section indices.
pub const SHN_LORESERVE: u16 = 0xff00;
/// Start of processor-specific section indices.
pub const SHN_LOPROC: u16 = 0xff00;
/// End of processor-specific section indices.
pub const SHN_HIPROC: u16 = 0xff1f;
/// Start of OS-specific section indices.
pub const SHN_LOOS: u16 = 0xff20;
/// End of OS-specific section indices.
pub const SHN_HIOS: u16 = 0xff3f;
/// Associated symbol is absolute.
pub const SHN_ABS: u16 = 0xfff1;
/// Associated symbol is common.
pub const SHN_COMMON: u16 = 0xfff2;
/// Section index is in the `SHT_SYMTAB_SHNDX` section.
pub const SHN_XINDEX: u16 = 0xffff;
/// End of reserved section indices.
pub const SHN_HIRESERVE: u16 = 0xffff;

// Values for `SectionHeader*::sh_type`.
/// Section header table entry is unused.
pub const SHT_NULL: u32 = 0;
/// Program data.
pub const SHT_PROGBITS: u32 = 1;
/// Symbol table.
pub const SHT_SYMTAB: u32 = 2;
/// String table.
pub const SHT_STRTAB: u32 = 3;
/// Relocation entries with explicit addends.
pub const SHT_RELA: u32 = 4;
/// Symbol hash table.
pub const SHT_HASH: u32 = 5;
/// Dynamic linking information.
pub const SHT_DYNAMIC: u32 = 6;
/// Notes.
pub const SHT_NOTE: u32 = 7;
/// Program space with no data (bss).
pub const SHT_NOBITS: u32 = 8;
/// Relocation entries without explicit addends.
pub const SHT_REL: u32 = 9;
/// Reserved section type.
pub const SHT_SHLIB: u32 = 10;
/// Dynamic linker symbol table.
pub const SHT_DYNSYM: u32 = 11;
/// Array of constructors.
pub const SHT_INIT_ARRAY: u32 = 14;
/// Array of destructors.
pub const SHT_FINI_ARRAY: u32 = 15;
/// Array of pre-constructors.
pub const SHT_PREINIT_ARRAY: u32 = 16;
/// Section group.
pub const SHT_GROUP: u32 = 17;
/// Extended section indices for a symbol table.
pub const SHT_SYMTAB_SHNDX: u32 = 18;
/// Relocation entries; only offsets.
pub const SHT_RELR: u32 = 19;
/// Start of OS-specific section types.
pub const SHT_LOOS: u32 = 0x6000_0000;
/// LLVM-style dependent libraries.
pub const SHT_LLVM_DEPENDENT_LIBRARIES: u32 = 0x6fff4c04;
/// Object attributes.
pub const SHT_GNU_ATTRIBUTES: u32 = 0x6fff_fff5;
/// GNU-style hash table.
pub const SHT_GNU_HASH: u32 = 0x6fff_fff6;
/// Prelink library list
pub const SHT_GNU_LIBLIST: u32 = 0x6fff_fff7;
/// Checksum for DSO content.
pub const SHT_CHECKSUM: u32 = 0x6fff_fff8;
/// Sun-specific low bound.
pub const SHT_LOSUNW: u32 = 0x6fff_fffa;
#[allow(non_upper_case_globals)]
pub const SHT_SUNW_move: u32 = 0x6fff_fffa;
pub const SHT_SUNW_COMDAT: u32 = 0x6fff_fffb;
#[allow(non_upper_case_globals)]
pub const SHT_SUNW_syminfo: u32 = 0x6fff_fffc;
/// Version definition section.
#[allow(non_upper_case_globals)]
pub const SHT_GNU_VERDEF: u32 = 0x6fff_fffd;
/// Version needs section.
#[allow(non_upper_case_globals)]
pub const SHT_GNU_VERNEED: u32 = 0x6fff_fffe;
/// Version symbol table.
#[allow(non_upper_case_globals)]
pub const SHT_GNU_VERSYM: u32 = 0x6fff_ffff;
/// Sun-specific high bound.
pub const SHT_HISUNW: u32 = 0x6fff_ffff;
/// End of OS-specific section types.
pub const SHT_HIOS: u32 = 0x6fff_ffff;
/// Start of processor-specific section types.
pub const SHT_LOPROC: u32 = 0x7000_0000;
/// End of processor-specific section types.
pub const SHT_HIPROC: u32 = 0x7fff_ffff;
/// Start of application-specific section types.
pub const SHT_LOUSER: u32 = 0x8000_0000;
/// End of application-specific section types.
pub const SHT_HIUSER: u32 = 0x8fff_ffff;

// Values for `SectionHeader*::sh_flags`.
/// Section is writable.
pub const SHF_WRITE: u32 = 1 << 0;
/// Section occupies memory during execution.
pub const SHF_ALLOC: u32 = 1 << 1;
/// Section is executable.
pub const SHF_EXECINSTR: u32 = 1 << 2;
/// Section may be be merged to eliminate duplication.
pub const SHF_MERGE: u32 = 1 << 4;
/// Section contains nul-terminated strings.
pub const SHF_STRINGS: u32 = 1 << 5;
/// The `sh_info` field contains a section header table index.
pub const SHF_INFO_LINK: u32 = 1 << 6;
/// Section has special ordering requirements when combining sections.
pub const SHF_LINK_ORDER: u32 = 1 << 7;
/// Section requires special OS-specific handling.
pub const SHF_OS_NONCONFORMING: u32 = 1 << 8;
/// Section is a member of a group.
pub const SHF_GROUP: u32 = 1 << 9;
/// Section holds thread-local storage.
pub const SHF_TLS: u32 = 1 << 10;
/// Section is compressed.
///
/// Compressed sections begin with one of the `CompressionHeader*` headers.
pub const SHF_COMPRESSED: u32 = 1 << 11;
/// OS-specific section flags.
pub const SHF_MASKOS: u32 = 0x0ff0_0000;
/// Section should not be garbage collected by the linker.
pub const SHF_GNU_RETAIN: u32 = 1 << 21;
/// Mbind section.
pub const SHF_GNU_MBIND: u32 = 1 << 24;
/// Processor-specific section flags.
pub const SHF_MASKPROC: u32 = 0xf000_0000;
/// This section is excluded from the final executable or shared library.
pub const SHF_EXCLUDE: u32 = 0x8000_0000;

/// Section compression header.
///
/// Used when `SHF_COMPRESSED` is set.
///
/// Note: this type currently allows for misaligned headers, but that may be
/// changed in a future version.
#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct CompressionHeader32<E: Endian> {
    /// Compression format. One of the `ELFCOMPRESS_*` values.
    pub ch_type: U32Bytes<E>,
    /// Uncompressed data size.
    pub ch_size: U32Bytes<E>,
    /// Uncompressed data alignment.
    pub ch_addralign: U32Bytes<E>,
}

/// Section compression header.
///
/// Used when `SHF_COMPRESSED` is set.
///
/// Note: this type currently allows for misaligned headers, but that may be
/// changed in a future version.
#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct CompressionHeader64<E: Endian> {
    /// Compression format. One of the `ELFCOMPRESS_*` values.
    pub ch_type: U32Bytes<E>,
    /// Reserved.
    pub ch_reserved: U32Bytes<E>,
    /// Uncompressed data size.
    pub ch_size: U64Bytes<E>,
    /// Uncompressed data alignment.
    pub ch_addralign: U64Bytes<E>,
}

/// ZLIB/DEFLATE algorithm.
pub const ELFCOMPRESS_ZLIB: u32 = 1;
/// Zstandard algorithm.
pub const ELFCOMPRESS_ZSTD: u32 = 2;
/// Start of OS-specific compression types.
pub const ELFCOMPRESS_LOOS: u32 = 0x6000_0000;
/// End of OS-specific compression types.
pub const ELFCOMPRESS_HIOS: u32 = 0x6fff_ffff;
/// Start of processor-specific compression types.
pub const ELFCOMPRESS_LOPROC: u32 = 0x7000_0000;
/// End of processor-specific compression types.
pub const ELFCOMPRESS_HIPROC: u32 = 0x7fff_ffff;

// Values for the flag entry for section groups.
/// Mark group as COMDAT.
pub const GRP_COMDAT: u32 = 1;

/// Symbol table entry.
#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct Sym32<E: Endian> {
    /// Symbol name.
    ///
    /// This is an offset into the symbol string table.
    pub st_name: U32<E>,
    /// Symbol value.
    pub st_value: U32<E>,
    /// Symbol size.
    pub st_size: U32<E>,
    /// Symbol type and binding.
    ///
    /// Use the `st_type` and `st_bind` methods to access this value.
    pub st_info: u8,
    /// Symbol visibility.
    ///
    /// Use the `st_visibility` method to access this value.
    pub st_other: u8,
    /// Section index or one of the `SHN_*` values.
    pub st_shndx: U16<E>,
}

impl<E: Endian> Sym32<E> {
    /// Get the `st_bind` component of the `st_info` field.
    #[inline]
    pub fn st_bind(&self) -> u8 {
        self.st_info >> 4
    }

    /// Get the `st_type` component of the `st_info` field.
    #[inline]
    pub fn st_type(&self) -> u8 {
        self.st_info & 0xf
    }

    /// Set the `st_info` field given the `st_bind` and `st_type` components.
    #[inline]
    pub fn set_st_info(&mut self, st_bind: u8, st_type: u8) {
        self.st_info = (st_bind << 4) + (st_type & 0xf);
    }

    /// Get the `st_visibility` component of the `st_info` field.
    #[inline]
    pub fn st_visibility(&self) -> u8 {
        self.st_other & 0x3
    }
}

/// Symbol table entry.
#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct Sym64<E: Endian> {
    /// Symbol name.
    ///
    /// This is an offset into the symbol string table.
    pub st_name: U32<E>,
    /// Symbol type and binding.
    ///
    /// Use the `st_bind` and `st_type` methods to access this value.
    pub st_info: u8,
    /// Symbol visibility.
    ///
    /// Use the `st_visibility` method to access this value.
    pub st_other: u8,
    /// Section index or one of the `SHN_*` values.
    pub st_shndx: U16<E>,
    /// Symbol value.
    pub st_value: U64<E>,
    /// Symbol size.
    pub st_size: U64<E>,
}

impl<E: Endian> Sym64<E> {
    /// Get the `st_bind` component of the `st_info` field.
    #[inline]
    pub fn st_bind(&self) -> u8 {
        self.st_info >> 4
    }

    /// Get the `st_type` component of the `st_info` field.
    #[inline]
    pub fn st_type(&self) -> u8 {
        self.st_info & 0xf
    }

    /// Set the `st_info` field given the `st_bind` and `st_type` components.
    #[inline]
    pub fn set_st_info(&mut self, st_bind: u8, st_type: u8) {
        self.st_info = (st_bind << 4) + (st_type & 0xf);
    }

    /// Get the `st_visibility` component of the `st_info` field.
    #[inline]
    pub fn st_visibility(&self) -> u8 {
        self.st_other & 0x3
    }
}

/// Additional information about a `Sym32`.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Syminfo32<E: Endian> {
    /// Direct bindings, symbol bound to.
    pub si_boundto: U16<E>,
    /// Per symbol flags.
    pub si_flags: U16<E>,
}

/// Additional information about a `Sym64`.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Syminfo64<E: Endian> {
    /// Direct bindings, symbol bound to.
    pub si_boundto: U16<E>,
    /// Per symbol flags.
    pub si_flags: U16<E>,
}

// Values for `Syminfo*::si_boundto`.
/// Symbol bound to self
pub const SYMINFO_BT_SELF: u16 = 0xffff;
/// Symbol bound to parent
pub const SYMINFO_BT_PARENT: u16 = 0xfffe;
/// Beginning of reserved entries
pub const SYMINFO_BT_LOWRESERVE: u16 = 0xff00;

// Values for `Syminfo*::si_flags`.
/// Direct bound symbol
pub const SYMINFO_FLG_DIRECT: u16 = 0x0001;
/// Pass-thru symbol for translator
pub const SYMINFO_FLG_PASSTHRU: u16 = 0x0002;
/// Symbol is a copy-reloc
pub const SYMINFO_FLG_COPY: u16 = 0x0004;
/// Symbol bound to object to be lazy loaded
pub const SYMINFO_FLG_LAZYLOAD: u16 = 0x0008;

// Syminfo version values.
pub const SYMINFO_NONE: u16 = 0;
pub const SYMINFO_CURRENT: u16 = 1;
pub const SYMINFO_NUM: u16 = 2;

// Values for bind component of `Sym*::st_info`.
/// Local symbol.
pub const STB_LOCAL: u8 = 0;
/// Global symbol.
pub const STB_GLOBAL: u8 = 1;
/// Weak symbol.
pub const STB_WEAK: u8 = 2;
/// Start of OS-specific symbol binding.
pub const STB_LOOS: u8 = 10;
/// Unique symbol.
pub const STB_GNU_UNIQUE: u8 = 10;
/// End of OS-specific symbol binding.
pub const STB_HIOS: u8 = 12;
/// Start of processor-specific symbol binding.
pub const STB_LOPROC: u8 = 13;
/// End of processor-specific symbol binding.
pub const STB_HIPROC: u8 = 15;

// Values for type component of `Sym*::st_info`.
/// Symbol type is unspecified.
pub const STT_NOTYPE: u8 = 0;
/// Symbol is a data object.
pub const STT_OBJECT: u8 = 1;
/// Symbol is a code object.
pub const STT_FUNC: u8 = 2;
/// Symbol is associated with a section.
pub const STT_SECTION: u8 = 3;
/// Symbol's name is a file name.
pub const STT_FILE: u8 = 4;
/// Symbol is a common data object.
pub const STT_COMMON: u8 = 5;
/// Symbol is a thread-local storage object.
pub const STT_TLS: u8 = 6;
/// Start of OS-specific symbol types.
pub const STT_LOOS: u8 = 10;
/// Symbol is an indirect code object.
pub const STT_GNU_IFUNC: u8 = 10;
/// End of OS-specific symbol types.
pub const STT_HIOS: u8 = 12;
/// Start of processor-specific symbol types.
pub const STT_LOPROC: u8 = 13;
/// End of processor-specific symbol types.
pub const STT_HIPROC: u8 = 15;

// Values for visibility component of `Symbol*::st_other`.
/// Default symbol visibility rules.
pub const STV_DEFAULT: u8 = 0;
/// Processor specific hidden class.
pub const STV_INTERNAL: u8 = 1;
/// Symbol is not visible to other components.
pub const STV_HIDDEN: u8 = 2;
/// Symbol is visible to other components, but is not preemptible.
pub const STV_PROTECTED: u8 = 3;

/// Relocation table entry without explicit addend.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Rel32<E: Endian> {
    /// Relocation address.
    pub r_offset: U32<E>,
    /// Relocation type and symbol index.
    pub r_info: U32<E>,
}

impl<E: Endian> Rel32<E> {
    /// Get the `r_sym` component of the `r_info` field.
    #[inline]
    pub fn r_sym(&self, endian: E) -> u32 {
        self.r_info.get(endian) >> 8
    }

    /// Get the `r_type` component of the `r_info` field.
    #[inline]
    pub fn r_type(&self, endian: E) -> u32 {
        self.r_info.get(endian) & 0xff
    }

    /// Calculate the `r_info` field given the `r_sym` and `r_type` components.
    pub fn r_info(endian: E, r_sym: u32, r_type: u8) -> U32<E> {
        U32::new(endian, (r_sym << 8) | u32::from(r_type))
    }

    /// Set the `r_info` field given the `r_sym` and `r_type` components.
    pub fn set_r_info(&mut self, endian: E, r_sym: u32, r_type: u8) {
        self.r_info = Self::r_info(endian, r_sym, r_type)
    }
}

/// Relocation table entry with explicit addend.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Rela32<E: Endian> {
    /// Relocation address.
    pub r_offset: U32<E>,
    /// Relocation type and symbol index.
    pub r_info: U32<E>,
    /// Explicit addend.
    pub r_addend: I32<E>,
}

impl<E: Endian> Rela32<E> {
    /// Get the `r_sym` component of the `r_info` field.
    #[inline]
    pub fn r_sym(&self, endian: E) -> u32 {
        self.r_info.get(endian) >> 8
    }

    /// Get the `r_type` component of the `r_info` field.
    #[inline]
    pub fn r_type(&self, endian: E) -> u32 {
        self.r_info.get(endian) & 0xff
    }

    /// Calculate the `r_info` field given the `r_sym` and `r_type` components.
    pub fn r_info(endian: E, r_sym: u32, r_type: u8) -> U32<E> {
        U32::new(endian, (r_sym << 8) | u32::from(r_type))
    }

    /// Set the `r_info` field given the `r_sym` and `r_type` components.
    pub fn set_r_info(&mut self, endian: E, r_sym: u32, r_type: u8) {
        self.r_info = Self::r_info(endian, r_sym, r_type)
    }
}

impl<E: Endian> From<Rel32<E>> for Rela32<E> {
    fn from(rel: Rel32<E>) -> Self {
        Rela32 {
            r_offset: rel.r_offset,
            r_info: rel.r_info,
            r_addend: I32::default(),
        }
    }
}

/// Relocation table entry without explicit addend.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Rel64<E: Endian> {
    /// Relocation address.
    pub r_offset: U64<E>,
    /// Relocation type and symbol index.
    pub r_info: U64<E>,
}

impl<E: Endian> Rel64<E> {
    /// Get the `r_sym` component of the `r_info` field.
    #[inline]
    pub fn r_sym(&self, endian: E) -> u32 {
        (self.r_info.get(endian) >> 32) as u32
    }

    /// Get the `r_type` component of the `r_info` field.
    #[inline]
    pub fn r_type(&self, endian: E) -> u32 {
        (self.r_info.get(endian) & 0xffff_ffff) as u32
    }

    /// Calculate the `r_info` field given the `r_sym` and `r_type` components.
    pub fn r_info(endian: E, r_sym: u32, r_type: u32) -> U64<E> {
        U64::new(endian, (u64::from(r_sym) << 32) | u64::from(r_type))
    }

    /// Set the `r_info` field given the `r_sym` and `r_type` components.
    pub fn set_r_info(&mut self, endian: E, r_sym: u32, r_type: u32) {
        self.r_info = Self::r_info(endian, r_sym, r_type)
    }
}

impl<E: Endian> From<Rel64<E>> for Rela64<E> {
    fn from(rel: Rel64<E>) -> Self {
        Rela64 {
            r_offset: rel.r_offset,
            r_info: rel.r_info,
            r_addend: I64::default(),
        }
    }
}

/// Relocation table entry with explicit addend.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Rela64<E: Endian> {
    /// Relocation address.
    pub r_offset: U64<E>,
    /// Relocation type and symbol index.
    pub r_info: U64<E>,
    /// Explicit addend.
    pub r_addend: I64<E>,
}

impl<E: Endian> Rela64<E> {
    pub(crate) fn get_r_info(&self, endian: E, is_mips64el: bool) -> u64 {
        let mut t = self.r_info.get(endian);
        if is_mips64el {
            t = (t << 32)
                | ((t >> 8) & 0xff000000)
                | ((t >> 24) & 0x00ff0000)
                | ((t >> 40) & 0x0000ff00)
                | ((t >> 56) & 0x000000ff);
        }
        t
    }

    /// Get the `r_sym` component of the `r_info` field.
    #[inline]
    pub fn r_sym(&self, endian: E, is_mips64el: bool) -> u32 {
        (self.get_r_info(endian, is_mips64el) >> 32) as u32
    }

    /// Get the `r_type` component of the `r_info` field.
    #[inline]
    pub fn r_type(&self, endian: E, is_mips64el: bool) -> u32 {
        (self.get_r_info(endian, is_mips64el) & 0xffff_ffff) as u32
    }

    /// Calculate the `r_info` field given the `r_sym` and `r_type` components.
    pub fn r_info(endian: E, is_mips64el: bool, r_sym: u32, r_type: u32) -> U64<E> {
        let mut t = (u64::from(r_sym) << 32) | u64::from(r_type);
        if is_mips64el {
            t = (t >> 32)
                | ((t & 0xff000000) << 8)
                | ((t & 0x00ff0000) << 24)
                | ((t & 0x0000ff00) << 40)
                | ((t & 0x000000ff) << 56);
        }
        U64::new(endian, t)
    }

    /// Set the `r_info` field given the `r_sym` and `r_type` components.
    pub fn set_r_info(&mut self, endian: E, is_mips64el: bool, r_sym: u32, r_type: u32) {
        self.r_info = Self::r_info(endian, is_mips64el, r_sym, r_type);
    }
}

/// 32-bit relative relocation table entry.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Relr32<E: Endian>(pub U32<E>);

/// 64-bit relative relocation table entry.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Relr64<E: Endian>(pub U64<E>);

/// Program segment header.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ProgramHeader32<E: Endian> {
    /// Segment type. One of the `PT_*` constants.
    pub p_type: U32<E>,
    /// Segment file offset.
    pub p_offset: U32<E>,
    /// Segment virtual address.
    pub p_vaddr: U32<E>,
    /// Segment physical address.
    pub p_paddr: U32<E>,
    /// Segment size in the file.
    pub p_filesz: U32<E>,
    /// Segment size in memory.
    pub p_memsz: U32<E>,
    /// Segment flags. A combination of the `PF_*` constants.
    pub p_flags: U32<E>,
    /// Segment alignment.
    pub p_align: U32<E>,
}

/// Program segment header.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ProgramHeader64<E: Endian> {
    /// Segment type. One of the `PT_*` constants.
    pub p_type: U32<E>,
    /// Segment flags. A combination of the `PF_*` constants.
    pub p_flags: U32<E>,
    /// Segment file offset.
    pub p_offset: U64<E>,
    /// Segment virtual address.
    pub p_vaddr: U64<E>,
    /// Segment physical address.
    pub p_paddr: U64<E>,
    /// Segment size in the file.
    pub p_filesz: U64<E>,
    /// Segment size in memory.
    pub p_memsz: U64<E>,
    /// Segment alignment.
    pub p_align: U64<E>,
}

/// Special value for `FileHeader*::e_phnum`.
///
/// This indicates that the real number of program headers is too large to fit into e_phnum.
/// Instead the real value is in the field `sh_info` of section 0.
pub const PN_XNUM: u16 = 0xffff;

// Values for `ProgramHeader*::p_type`.
/// Program header table entry is unused.
pub const PT_NULL: u32 = 0;
/// Loadable program segment.
pub const PT_LOAD: u32 = 1;
/// Dynamic linking information.
pub const PT_DYNAMIC: u32 = 2;
/// Program interpreter.
pub const PT_INTERP: u32 = 3;
/// Auxiliary information.
pub const PT_NOTE: u32 = 4;
/// Reserved.
pub const PT_SHLIB: u32 = 5;
/// Segment contains the program header table.
pub const PT_PHDR: u32 = 6;
/// Thread-local storage segment.
pub const PT_TLS: u32 = 7;
/// Start of OS-specific segment types.
pub const PT_LOOS: u32 = 0x6000_0000;
/// GCC `.eh_frame_hdr` segment.
pub const PT_GNU_EH_FRAME: u32 = 0x6474_e550;
/// Indicates stack executability.
pub const PT_GNU_STACK: u32 = 0x6474_e551;
/// Read-only after relocation.
pub const PT_GNU_RELRO: u32 = 0x6474_e552;
/// Segment containing `.note.gnu.property` section.
pub const PT_GNU_PROPERTY: u32 = 0x6474_e553;
/// End of OS-specific segment types.
pub const PT_HIOS: u32 = 0x6fff_ffff;
/// Start of processor-specific segment types.
pub const PT_LOPROC: u32 = 0x7000_0000;
/// End of processor-specific segment types.
pub const PT_HIPROC: u32 = 0x7fff_ffff;

// Values for `ProgramHeader*::p_flags`.
/// Segment is executable.
pub const PF_X: u32 = 1 << 0;
/// Segment is writable.
pub const PF_W: u32 = 1 << 1;
/// Segment is readable.
pub const PF_R: u32 = 1 << 2;
/// OS-specific segment flags.
pub const PF_MASKOS: u32 = 0x0ff0_0000;
/// Processor-specific segment flags.
pub const PF_MASKPROC: u32 = 0xf000_0000;

/// Note name for core files.
pub const ELF_NOTE_CORE: &[u8] = b"CORE";
/// Note name for linux core files.
///
/// Notes in linux core files may also use `ELF_NOTE_CORE`.
pub const ELF_NOTE_LINUX: &[u8] = b"LINUX";

// Values for `NoteHeader*::n_type` in core files.
//
/// Contains copy of prstatus struct.
pub const NT_PRSTATUS: u32 = 1;
/// Contains copy of fpregset struct.
pub const NT_PRFPREG: u32 = 2;
/// Contains copy of fpregset struct.
pub const NT_FPREGSET: u32 = 2;
/// Contains copy of prpsinfo struct.
pub const NT_PRPSINFO: u32 = 3;
/// Contains copy of prxregset struct.
pub const NT_PRXREG: u32 = 4;
/// Contains copy of task structure.
pub const NT_TASKSTRUCT: u32 = 4;
/// String from sysinfo(SI_PLATFORM).
pub const NT_PLATFORM: u32 = 5;
/// Contains copy of auxv array.
pub const NT_AUXV: u32 = 6;
/// Contains copy of gwindows struct.
pub const NT_GWINDOWS: u32 = 7;
/// Contains copy of asrset struct.
pub const NT_ASRS: u32 = 8;
/// Contains copy of pstatus struct.
pub const NT_PSTATUS: u32 = 10;
/// Contains copy of psinfo struct.
pub const NT_PSINFO: u32 = 13;
/// Contains copy of prcred struct.
pub const NT_PRCRED: u32 = 14;
/// Contains copy of utsname struct.
pub const NT_UTSNAME: u32 = 15;
/// Contains copy of lwpstatus struct.
pub const NT_LWPSTATUS: u32 = 16;
/// Contains copy of lwpinfo struct.
pub const NT_LWPSINFO: u32 = 17;
/// Contains copy of fprxregset struct.
pub const NT_PRFPXREG: u32 = 20;
/// Contains copy of siginfo_t, size might increase.
pub const NT_SIGINFO: u32 = 0x5349_4749;
/// Contains information about mapped files.
pub const NT_FILE: u32 = 0x4649_4c45;
/// Contains copy of user_fxsr_struct.
pub const NT_PRXFPREG: u32 = 0x46e6_2b7f;
/// PowerPC Altivec/VMX registers.
pub const NT_PPC_VMX: u32 = 0x100;
/// PowerPC SPE/EVR registers.
pub const NT_PPC_SPE: u32 = 0x101;
/// PowerPC VSX registers.
pub const NT_PPC_VSX: u32 = 0x102;
/// Target Address Register.
pub const NT_PPC_TAR: u32 = 0x103;
/// Program Priority Register.
pub const NT_PPC_PPR: u32 = 0x104;
/// Data Stream Control Register.
pub const NT_PPC_DSCR: u32 = 0x105;
/// Event Based Branch Registers.
pub const NT_PPC_EBB: u32 = 0x106;
/// Performance Monitor Registers.
pub const NT_PPC_PMU: u32 = 0x107;
/// TM checkpointed GPR Registers.
pub const NT_PPC_TM_CGPR: u32 = 0x108;
/// TM checkpointed FPR Registers.
pub const NT_PPC_TM_CFPR: u32 = 0x109;
/// TM checkpointed VMX Registers.
pub const NT_PPC_TM_CVMX: u32 = 0x10a;
/// TM checkpointed VSX Registers.
pub const NT_PPC_TM_CVSX: u32 = 0x10b;
/// TM Special Purpose Registers.
pub const NT_PPC_TM_SPR: u32 = 0x10c;
/// TM checkpointed Target Address Register.
pub const NT_PPC_TM_CTAR: u32 = 0x10d;
/// TM checkpointed Program Priority Register.
pub const NT_PPC_TM_CPPR: u32 = 0x10e;
/// TM checkpointed Data Stream Control Register.
pub const NT_PPC_TM_CDSCR: u32 = 0x10f;
/// Memory Protection Keys registers.
pub const NT_PPC_PKEY: u32 = 0x110;
/// i386 TLS slots (struct user_desc).
pub const NT_386_TLS: u32 = 0x200;
/// x86 io permission bitmap (1=deny).
pub const NT_386_IOPERM: u32 = 0x201;
/// x86 extended state using xsave.
pub const NT_X86_XSTATE: u32 = 0x202;
/// s390 upper register halves.
pub const NT_S390_HIGH_GPRS: u32 = 0x300;
/// s390 timer register.
pub const NT_S390_TIMER: u32 = 0x301;
/// s390 TOD clock comparator register.
pub const NT_S390_TODCMP: u32 = 0x302;
/// s390 TOD programmable register.
pub const NT_S390_TODPREG: u32 = 0x303;
/// s390 control registers.
pub const NT_S390_CTRS: u32 = 0x304;
/// s390 prefix register.
pub const NT_S390_PREFIX: u32 = 0x305;
/// s390 breaking event address.
pub const NT_S390_LAST_BREAK: u32 = 0x306;
/// s390 system call restart data.
pub const NT_S390_SYSTEM_CALL: u32 = 0x307;
/// s390 transaction diagnostic block.
pub const NT_S390_TDB: u32 = 0x308;
/// s390 vector registers 0-15 upper half.
pub const NT_S390_VXRS_LOW: u32 = 0x309;
/// s390 vector registers 16-31.
pub const NT_S390_VXRS_HIGH: u32 = 0x30a;
/// s390 guarded storage registers.
pub const NT_S390_GS_CB: u32 = 0x30b;
/// s390 guarded storage broadcast control block.
pub const NT_S390_GS_BC: u32 = 0x30c;
/// s390 runtime instrumentation.
pub const NT_S390_RI_CB: u32 = 0x30d;
/// ARM VFP/NEON registers.
pub const NT_ARM_VFP: u32 = 0x400;
/// ARM TLS register.
pub const NT_ARM_TLS: u32 = 0x401;
/// ARM hardware breakpoint registers.
pub const NT_ARM_HW_BREAK: u32 = 0x402;
/// ARM hardware watchpoint registers.
pub const NT_ARM_HW_WATCH: u32 = 0x403;
/// ARM system call number.
pub const NT_ARM_SYSTEM_CALL: u32 = 0x404;
/// ARM Scalable Vector Extension registers.
pub const NT_ARM_SVE: u32 = 0x405;
/// Vmcore Device Dump Note.
pub const NT_VMCOREDD: u32 = 0x700;
/// MIPS DSP ASE registers.
pub const NT_MIPS_DSP: u32 = 0x800;
/// MIPS floating-point mode.
pub const NT_MIPS_FP_MODE: u32 = 0x801;

/// Note type for version string.
///
/// This note may appear in object files.
///
/// It must be handled as a special case because it has no descriptor, and instead
/// uses the note name as the version string.
pub const NT_VERSION: u32 = 1;

/// Dynamic section entry.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Dyn32<E: Endian> {
    /// Dynamic entry type.
    pub d_tag: U32<E>,
    /// Value (integer or address).
    pub d_val: U32<E>,
}

/// Dynamic section entry.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Dyn64<E: Endian> {
    /// Dynamic entry type.
    pub d_tag: U64<E>,
    /// Value (integer or address).
    pub d_val: U64<E>,
}

// Values for `Dyn*::d_tag`.

/// Marks end of dynamic section
pub const DT_NULL: u32 = 0;
/// Name of needed library
pub const DT_NEEDED: u32 = 1;
/// Size in bytes of PLT relocs
pub const DT_PLTRELSZ: u32 = 2;
/// Processor defined value
pub const DT_PLTGOT: u32 = 3;
/// Address of symbol hash table
pub const DT_HASH: u32 = 4;
/// Address of string table
pub const DT_STRTAB: u32 = 5;
/// Address of symbol table
pub const DT_SYMTAB: u32 = 6;
/// Address of Rela relocs
pub const DT_RELA: u32 = 7;
/// Total size of Rela relocs
pub const DT_RELASZ: u32 = 8;
/// Size of one Rela reloc
pub const DT_RELAENT: u32 = 9;
/// Size of string table
pub const DT_STRSZ: u32 = 10;
/// Size of one symbol table entry
pub const DT_SYMENT: u32 = 11;
/// Address of init function
pub const DT_INIT: u32 = 12;
/// Address of termination function
pub const DT_FINI: u32 = 13;
/// Name of shared object
pub const DT_SONAME: u32 = 14;
/// Library search path (deprecated)
pub const DT_RPATH: u32 = 15;
/// Start symbol search here
pub const DT_SYMBOLIC: u32 = 16;
/// Address of Rel relocs
pub const DT_REL: u32 = 17;
/// Total size of Rel relocs
pub const DT_RELSZ: u32 = 18;
/// Size of one Rel reloc
pub const DT_RELENT: u32 = 19;
/// Type of reloc in PLT
pub const DT_PLTREL: u32 = 20;
/// For debugging; unspecified
pub const DT_DEBUG: u32 = 21;
/// Reloc might modify .text
pub const DT_TEXTREL: u32 = 22;
/// Address of PLT relocs
pub const DT_JMPREL: u32 = 23;
/// Process relocations of object
pub const DT_BIND_NOW: u32 = 24;
/// Array with addresses of init fct
pub const DT_INIT_ARRAY: u32 = 25;
/// Array with addresses of fini fct
pub const DT_FINI_ARRAY: u32 = 26;
/// Size in bytes of DT_INIT_ARRAY
pub const DT_INIT_ARRAYSZ: u32 = 27;
/// Size in bytes of DT_FINI_ARRAY
pub const DT_FINI_ARRAYSZ: u32 = 28;
/// Library search path
pub const DT_RUNPATH: u32 = 29;
/// Flags for the object being loaded
pub const DT_FLAGS: u32 = 30;
/// Start of encoded range
pub const DT_ENCODING: u32 = 32;
/// Array with addresses of preinit fct
pub const DT_PREINIT_ARRAY: u32 = 32;
/// size in bytes of DT_PREINIT_ARRAY
pub const DT_PREINIT_ARRAYSZ: u32 = 33;
/// Address of SYMTAB_SHNDX section
pub const DT_SYMTAB_SHNDX: u32 = 34;
/// Start of OS-specific
pub const DT_LOOS: u32 = 0x6000_000d;
/// End of OS-specific
pub const DT_HIOS: u32 = 0x6fff_f000;
/// Start of processor-specific
pub const DT_LOPROC: u32 = 0x7000_0000;
/// End of processor-specific
pub const DT_HIPROC: u32 = 0x7fff_ffff;

// `DT_*` entries between `DT_VALRNGHI` & `DT_VALRNGLO` use `d_val` as a value.
pub const DT_VALRNGLO: u32 = 0x6fff_fd00;
/// Prelinking timestamp
pub const DT_GNU_PRELINKED: u32 = 0x6fff_fdf5;
/// Size of conflict section
pub const DT_GNU_CONFLICTSZ: u32 = 0x6fff_fdf6;
/// Size of library list
pub const DT_GNU_LIBLISTSZ: u32 = 0x6fff_fdf7;
pub const DT_CHECKSUM: u32 = 0x6fff_fdf8;
pub const DT_PLTPADSZ: u32 = 0x6fff_fdf9;
pub const DT_MOVEENT: u32 = 0x6fff_fdfa;
pub const DT_MOVESZ: u32 = 0x6fff_fdfb;
/// Feature selection (DTF_*).
pub const DT_FEATURE_1: u32 = 0x6fff_fdfc;
/// Flags for DT_* entries, affecting the following DT_* entry.
pub const DT_POSFLAG_1: u32 = 0x6fff_fdfd;
/// Size of syminfo table (in bytes)
pub const DT_SYMINSZ: u32 = 0x6fff_fdfe;
/// Entry size of syminfo
pub const DT_SYMINENT: u32 = 0x6fff_fdff;
pub const DT_VALRNGHI: u32 = 0x6fff_fdff;

// `DT_*` entries between `DT_ADDRRNGHI` & `DT_ADDRRNGLO` use `d_val` as an address.
//
// If any adjustment is made to the ELF object after it has been
// built these entries will need to be adjusted.
pub const DT_ADDRRNGLO: u32 = 0x6fff_fe00;
/// GNU-style hash table.
pub const DT_GNU_HASH: u32 = 0x6fff_fef5;
pub const DT_TLSDESC_PLT: u32 = 0x6fff_fef6;
pub const DT_TLSDESC_GOT: u32 = 0x6fff_fef7;
/// Start of conflict section
pub const DT_GNU_CONFLICT: u32 = 0x6fff_fef8;
/// Library list
pub const DT_GNU_LIBLIST: u32 = 0x6fff_fef9;
/// Configuration information.
pub const DT_CONFIG: u32 = 0x6fff_fefa;
/// Dependency auditing.
pub const DT_DEPAUDIT: u32 = 0x6fff_fefb;
/// Object auditing.
pub const DT_AUDIT: u32 = 0x6fff_fefc;
/// PLT padding.
pub const DT_PLTPAD: u32 = 0x6fff_fefd;
/// Move table.
pub const DT_MOVETAB: u32 = 0x6fff_fefe;
/// Syminfo table.
pub const DT_SYMINFO: u32 = 0x6fff_feff;
pub const DT_ADDRRNGHI: u32 = 0x6fff_feff;

// The versioning entry types.  The next are defined as part of the
// GNU extension.
pub const DT_VERSYM: u32 = 0x6fff_fff0;
pub const DT_RELACOUNT: u32 = 0x6fff_fff9;
pub const DT_RELCOUNT: u32 = 0x6fff_fffa;
/// State flags, see DF_1_* below.
pub const DT_FLAGS_1: u32 = 0x6fff_fffb;
/// Address of version definition table
pub const DT_VERDEF: u32 = 0x6fff_fffc;
/// Number of version definitions
pub const DT_VERDEFNUM: u32 = 0x6fff_fffd;
/// Address of table with needed versions
pub const DT_VERNEED: u32 = 0x6fff_fffe;
/// Number of needed versions
pub const DT_VERNEEDNUM: u32 = 0x6fff_ffff;

// Machine-independent extensions in the "processor-specific" range.
/// Shared object to load before self
pub const DT_AUXILIARY: u32 = 0x7fff_fffd;
/// Shared object to get values from
pub const DT_FILTER: u32 = 0x7fff_ffff;

// Values of `Dyn*::d_val` in the `DT_FLAGS` entry.
/// Object may use DF_ORIGIN
pub const DF_ORIGIN: u32 = 0x0000_0001;
/// Symbol resolutions starts here
pub const DF_SYMBOLIC: u32 = 0x0000_0002;
/// Object contains text relocations
pub const DF_TEXTREL: u32 = 0x0000_0004;
/// No lazy binding for this object
pub const DF_BIND_NOW: u32 = 0x0000_0008;
/// Module uses the static TLS model
pub const DF_STATIC_TLS: u32 = 0x0000_0010;

// Values of `Dyn*::d_val` in the `DT_FLAGS_1` entry.
/// Set RTLD_NOW for this object.
pub const DF_1_NOW: u32 = 0x0000_0001;
/// Set RTLD_GLOBAL for this object.
pub const DF_1_GLOBAL: u32 = 0x0000_0002;
/// Set RTLD_GROUP for this object.
pub const DF_1_GROUP: u32 = 0x0000_0004;
/// Set RTLD_NODELETE for this object.
pub const DF_1_NODELETE: u32 = 0x0000_0008;
/// Trigger filtee loading at runtime.
pub const DF_1_LOADFLTR: u32 = 0x0000_0010;
/// Set RTLD_INITFIRST for this object.
pub const DF_1_INITFIRST: u32 = 0x0000_0020;
/// Set RTLD_NOOPEN for this object.
pub const DF_1_NOOPEN: u32 = 0x0000_0040;
/// $ORIGIN must be handled.
pub const DF_1_ORIGIN: u32 = 0x0000_0080;
/// Direct binding enabled.
pub const DF_1_DIRECT: u32 = 0x0000_0100;
pub const DF_1_TRANS: u32 = 0x0000_0200;
/// Object is used to interpose.
pub const DF_1_INTERPOSE: u32 = 0x0000_0400;
/// Ignore default lib search path.
pub const DF_1_NODEFLIB: u32 = 0x0000_0800;
/// Object can't be dldump'ed.
pub const DF_1_NODUMP: u32 = 0x0000_1000;
/// Configuration alternative created.
pub const DF_1_CONFALT: u32 = 0x0000_2000;
/// Filtee terminates filters search.
pub const DF_1_ENDFILTEE: u32 = 0x0000_4000;
/// Disp reloc applied at build time.
pub const DF_1_DISPRELDNE: u32 = 0x0000_8000;
/// Disp reloc applied at run-time.
pub const DF_1_DISPRELPND: u32 = 0x0001_0000;
/// Object has no-direct binding.
pub const DF_1_NODIRECT: u32 = 0x0002_0000;
pub const DF_1_IGNMULDEF: u32 = 0x0004_0000;
pub const DF_1_NOKSYMS: u32 = 0x0008_0000;
pub const DF_1_NOHDR: u32 = 0x0010_0000;
/// Object is modified after built.
pub const DF_1_EDITED: u32 = 0x0020_0000;
pub const DF_1_NORELOC: u32 = 0x0040_0000;
/// Object has individual interposers.
pub const DF_1_SYMINTPOSE: u32 = 0x0080_0000;
/// Global auditing required.
pub const DF_1_GLOBAUDIT: u32 = 0x0100_0000;
/// Singleton symbols are used.
pub const DF_1_SINGLETON: u32 = 0x0200_0000;
pub const DF_1_STUB: u32 = 0x0400_0000;
pub const DF_1_PIE: u32 = 0x0800_0000;

/// Version symbol information
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Versym<E: Endian>(pub U16<E>);

/// Symbol is hidden.
pub const VERSYM_HIDDEN: u16 = 0x8000;
/// Symbol version index.
pub const VERSYM_VERSION: u16 = 0x7fff;

/// Version definition sections
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Verdef<E: Endian> {
    /// Version revision
    pub vd_version: U16<E>,
    /// Version information
    pub vd_flags: U16<E>,
    /// Version Index
    pub vd_ndx: U16<E>,
    /// Number of associated aux entries
    pub vd_cnt: U16<E>,
    /// Version name hash value
    pub vd_hash: U32<E>,
    /// Offset in bytes to verdaux array
    pub vd_aux: U32<E>,
    /// Offset in bytes to next verdef entry
    pub vd_next: U32<E>,
}

// Legal values for vd_version (version revision).
/// No version
pub const VER_DEF_NONE: u16 = 0;
/// Current version
pub const VER_DEF_CURRENT: u16 = 1;

// Legal values for vd_flags (version information flags).
/// Version definition of file itself
pub const VER_FLG_BASE: u16 = 0x1;
// Legal values for vd_flags and vna_flags (version information flags).
/// Weak version identifier
pub const VER_FLG_WEAK: u16 = 0x2;

// Versym symbol index values.
/// Symbol is local.
pub const VER_NDX_LOCAL: u16 = 0;
/// Symbol is global.
pub const VER_NDX_GLOBAL: u16 = 1;

/// Auxiliary version information.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Verdaux<E: Endian> {
    /// Version or dependency names
    pub vda_name: U32<E>,
    /// Offset in bytes to next verdaux
    pub vda_next: U32<E>,
}

/// Version dependency.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Verneed<E: Endian> {
    /// Version of structure
    pub vn_version: U16<E>,
    /// Number of associated aux entries
    pub vn_cnt: U16<E>,
    /// Offset of filename for this dependency
    pub vn_file: U32<E>,
    /// Offset in bytes to vernaux array
    pub vn_aux: U32<E>,
    /// Offset in bytes to next verneed entry
    pub vn_next: U32<E>,
}

// Legal values for vn_version (version revision).
/// No version
pub const VER_NEED_NONE: u16 = 0;
/// Current version
pub const VER_NEED_CURRENT: u16 = 1;

/// Auxiliary needed version information.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Vernaux<E: Endian> {
    /// Hash value of dependency name
    pub vna_hash: U32<E>,
    /// Dependency specific information
    pub vna_flags: U16<E>,
    /// Version Index
    pub vna_other: U16<E>,
    /// Dependency name string offset
    pub vna_name: U32<E>,
    /// Offset in bytes to next vernaux entry
    pub vna_next: U32<E>,
}

// TODO: Elf*_auxv_t, AT_*

/// Note section entry header.
///
/// A note consists of a header followed by a variable length name and descriptor.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct NoteHeader32<E: Endian> {
    /// Length of the note's name.
    ///
    /// Some known names are defined by the `ELF_NOTE_*` constants.
    pub n_namesz: U32<E>,
    /// Length of the note's descriptor.
    ///
    /// The content of the descriptor depends on the note name and type.
    pub n_descsz: U32<E>,
    /// Type of the note.
    ///
    /// One of the `NT_*` constants. The note name determines which
    /// `NT_*` constants are valid.
    pub n_type: U32<E>,
}

/// Note section entry header.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct NoteHeader64<E: Endian> {
    /// Length of the note's name.
    ///
    /// Some known names are defined by the `ELF_NOTE_*` constants.
    pub n_namesz: U32<E>,
    /// Length of the note's descriptor.
    ///
    /// The content of the descriptor depends on the note name and type.
    pub n_descsz: U32<E>,
    /// Type of the note.
    ///
    /// One of the `NT_*` constants. The note name determines which
    /// `NT_*` constants are valid.
    pub n_type: U32<E>,
}

/// Solaris entries in the note section have this name.
pub const ELF_NOTE_SOLARIS: &[u8] = b"SUNW Solaris";

// Values for `n_type` when the name is `ELF_NOTE_SOLARIS`.
/// Desired pagesize for the binary.
pub const NT_SOLARIS_PAGESIZE_HINT: u32 = 1;

/// GNU entries in the note section have this name.
pub const ELF_NOTE_GNU: &[u8] = b"GNU";

/// Go entries in the note section have this name.
// See https://go-review.googlesource.com/9520 and https://go-review.googlesource.com/10704.
pub const ELF_NOTE_GO: &[u8] = b"Go";

// Note types for `ELF_NOTE_GNU`.

/// ABI information.
///
/// The descriptor consists of words:
/// - word 0: OS descriptor
/// - word 1: major version of the ABI
/// - word 2: minor version of the ABI
/// - word 3: subminor version of the ABI
pub const NT_GNU_ABI_TAG: u32 = 1;

/// OS descriptor for `NT_GNU_ABI_TAG`.
pub const ELF_NOTE_OS_LINUX: u32 = 0;
/// OS descriptor for `NT_GNU_ABI_TAG`.
pub const ELF_NOTE_OS_GNU: u32 = 1;
/// OS descriptor for `NT_GNU_ABI_TAG`.
pub const ELF_NOTE_OS_SOLARIS2: u32 = 2;
/// OS descriptor for `NT_GNU_ABI_TAG`.
pub const ELF_NOTE_OS_FREEBSD: u32 = 3;

/// Synthetic hwcap information.
///
/// The descriptor begins with two words:
/// - word 0: number of entries
/// - word 1: bitmask of enabled entries
///
/// Then follow variable-length entries, one byte followed by a
/// '\0'-terminated hwcap name string.  The byte gives the bit
/// number to test if enabled, (1U << bit) & bitmask.  */
pub const NT_GNU_HWCAP: u32 = 2;

/// Build ID bits as generated by `ld --build-id`.
///
/// The descriptor consists of any nonzero number of bytes.
pub const NT_GNU_BUILD_ID: u32 = 3;

/// Build ID bits as generated by Go's gc compiler.
///
/// The descriptor consists of any nonzero number of bytes.
// See https://go-review.googlesource.com/10707.
pub const NT_GO_BUILD_ID: u32 = 4;

/// Version note generated by GNU gold containing a version string.
pub const NT_GNU_GOLD_VERSION: u32 = 4;

/// Program property.
pub const NT_GNU_PROPERTY_TYPE_0: u32 = 5;

// Values used in GNU .note.gnu.property notes (NT_GNU_PROPERTY_TYPE_0).

/// Stack size.
pub const GNU_PROPERTY_STACK_SIZE: u32 = 1;
/// No copy relocation on protected data symbol.
pub const GNU_PROPERTY_NO_COPY_ON_PROTECTED: u32 = 2;

// A 4-byte unsigned integer property: A bit is set if it is set in all
// relocatable inputs.
pub const GNU_PROPERTY_UINT32_AND_LO: u32 = 0xb0000000;
pub const GNU_PROPERTY_UINT32_AND_HI: u32 = 0xb0007fff;

// A 4-byte unsigned integer property: A bit is set if it is set in any
// relocatable inputs.
pub const GNU_PROPERTY_UINT32_OR_LO: u32 = 0xb0008000;
pub const GNU_PROPERTY_UINT32_OR_HI: u32 = 0xb000ffff;

/// The needed properties by the object file.  */
pub const GNU_PROPERTY_1_NEEDED: u32 = GNU_PROPERTY_UINT32_OR_LO;

/// Set if the object file requires canonical function pointers and
/// cannot be used with copy relocation.
pub const GNU_PROPERTY_1_NEEDED_INDIRECT_EXTERN_ACCESS: u32 = 1 << 0;

/// Processor-specific semantics, lo
pub const GNU_PROPERTY_LOPROC: u32 = 0xc0000000;
/// Processor-specific semantics, hi
pub const GNU_PROPERTY_HIPROC: u32 = 0xdfffffff;
/// Application-specific semantics, lo
pub const GNU_PROPERTY_LOUSER: u32 = 0xe0000000;
/// Application-specific semantics, hi
pub const GNU_PROPERTY_HIUSER: u32 = 0xffffffff;

/// AArch64 specific GNU properties.
pub const GNU_PROPERTY_AARCH64_FEATURE_1_AND: u32 = 0xc0000000;
pub const GNU_PROPERTY_AARCH64_FEATURE_PAUTH: u32 = 0xc0000001;

pub const GNU_PROPERTY_AARCH64_FEATURE_1_BTI: u32 = 1 << 0;
pub const GNU_PROPERTY_AARCH64_FEATURE_1_PAC: u32 = 1 << 1;

// A 4-byte unsigned integer property: A bit is set if it is set in all
// relocatable inputs.
pub const GNU_PROPERTY_X86_UINT32_AND_LO: u32 = 0xc0000002;
pub const GNU_PROPERTY_X86_UINT32_AND_HI: u32 = 0xc0007fff;

// A 4-byte unsigned integer property: A bit is set if it is set in any
// relocatable inputs.
pub const GNU_PROPERTY_X86_UINT32_OR_LO: u32 = 0xc0008000;
pub const GNU_PROPERTY_X86_UINT32_OR_HI: u32 = 0xc000ffff;

// A 4-byte unsigned integer property: A bit is set if it is set in any
// relocatable inputs and the property is present in all relocatable
// inputs.
pub const GNU_PROPERTY_X86_UINT32_OR_AND_LO: u32 = 0xc0010000;
pub const GNU_PROPERTY_X86_UINT32_OR_AND_HI: u32 = 0xc0017fff;

/// The x86 instruction sets indicated by the corresponding bits are
/// used in program.  Their support in the hardware is optional.
pub const GNU_PROPERTY_X86_ISA_1_USED: u32 = 0xc0010002;
/// The x86 instruction sets indicated by the corresponding bits are
/// used in program and they must be supported by the hardware.
pub const GNU_PROPERTY_X86_ISA_1_NEEDED: u32 = 0xc0008002;
/// X86 processor-specific features used in program.
pub const GNU_PROPERTY_X86_FEATURE_1_AND: u32 = 0xc0000002;

/// GNU_PROPERTY_X86_ISA_1_BASELINE: CMOV, CX8 (cmpxchg8b), FPU (fld),
/// MMX, OSFXSR (fxsave), SCE (syscall), SSE and SSE2.
pub const GNU_PROPERTY_X86_ISA_1_BASELINE: u32 = 1 << 0;
/// GNU_PROPERTY_X86_ISA_1_V2: GNU_PROPERTY_X86_ISA_1_BASELINE,
/// CMPXCHG16B (cmpxchg16b), LAHF-SAHF (lahf), POPCNT (popcnt), SSE3,
/// SSSE3, SSE4.1 and SSE4.2.
pub const GNU_PROPERTY_X86_ISA_1_V2: u32 = 1 << 1;
/// GNU_PROPERTY_X86_ISA_1_V3: GNU_PROPERTY_X86_ISA_1_V2, AVX, AVX2, BMI1,
/// BMI2, F16C, FMA, LZCNT, MOVBE, XSAVE.
pub const GNU_PROPERTY_X86_ISA_1_V3: u32 = 1 << 2;
/// GNU_PROPERTY_X86_ISA_1_V4: GNU_PROPERTY_X86_ISA_1_V3, AVX512F,
/// AVX512BW, AVX512CD, AVX512DQ and AVX512VL.
pub const GNU_PROPERTY_X86_ISA_1_V4: u32 = 1 << 3;

/// This indicates that all executable sections are compatible with IBT.
pub const GNU_PROPERTY_X86_FEATURE_1_IBT: u32 = 1 << 0;
/// This indicates that all executable sections are compatible with SHSTK.
pub const GNU_PROPERTY_X86_FEATURE_1_SHSTK: u32 = 1 << 1;

// TODO: Elf*_Move

/// Header of `SHT_HASH` section.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct HashHeader<E: Endian> {
    /// The number of hash buckets.
    pub bucket_count: U32<E>,
    /// The number of chain values.
    pub chain_count: U32<E>,
    // Array of hash bucket start indices.
    // buckets: U32<E>[bucket_count]
    // Array of hash chain links. An index of 0 terminates the chain.
    // chains: U32<E>[chain_count]
}

/// Calculate the SysV hash for a symbol name.
///
/// Used for `SHT_HASH`.
pub fn hash(name: &[u8]) -> u32 {
    let mut hash = 0u32;
    for byte in name {
        hash = hash.wrapping_mul(16).wrapping_add(u32::from(*byte));
        hash ^= (hash >> 24) & 0xf0;
    }
    hash & 0xfff_ffff
}

/// Header of `SHT_GNU_HASH` section.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct GnuHashHeader<E: Endian> {
    /// The number of hash buckets.
    pub bucket_count: U32<E>,
    /// The symbol table index of the first symbol in the hash.
    pub symbol_base: U32<E>,
    /// The number of words in the bloom filter.
    ///
    /// Must be a non-zero power of 2.
    pub bloom_count: U32<E>,
    /// The bit shift count for the bloom filter.
    pub bloom_shift: U32<E>,
    // Array of bloom filter words.
    // bloom_filters: U32<E>[bloom_count] or U64<E>[bloom_count]
    // Array of hash bucket start indices.
    // buckets: U32<E>[bucket_count]
    // Array of hash values, one for each symbol starting at symbol_base.
    // values: U32<E>[symbol_count]
}

/// Calculate the GNU hash for a symbol name.
///
/// Used for `SHT_GNU_HASH`.
pub fn gnu_hash(name: &[u8]) -> u32 {
    let mut hash = 5381u32;
    for byte in name {
        hash = hash.wrapping_mul(33).wrapping_add(u32::from(*byte));
    }
    hash
}

// Motorola 68k specific definitions.

// m68k values for `Rel*::r_type`.

/// No reloc
pub const R_68K_NONE: u32 = 0;
/// Direct 32 bit
pub const R_68K_32: u32 = 1;
/// Direct 16 bit
pub const R_68K_16: u32 = 2;
/// Direct 8 bit
pub const R_68K_8: u32 = 3;
/// PC relative 32 bit
pub const R_68K_PC32: u32 = 4;
/// PC relative 16 bit
pub const R_68K_PC16: u32 = 5;
/// PC relative 8 bit
pub const R_68K_PC8: u32 = 6;
/// 32 bit PC relative GOT entry
pub const R_68K_GOT32: u32 = 7;
/// 16 bit PC relative GOT entry
pub const R_68K_GOT16: u32 = 8;
/// 8 bit PC relative GOT entry
pub const R_68K_GOT8: u32 = 9;
/// 32 bit GOT offset
pub const R_68K_GOT32O: u32 = 10;
/// 16 bit GOT offset
pub const R_68K_GOT16O: u32 = 11;
/// 8 bit GOT offset
pub const R_68K_GOT8O: u32 = 12;
/// 32 bit PC relative PLT address
pub const R_68K_PLT32: u32 = 13;
/// 16 bit PC relative PLT address
pub const R_68K_PLT16: u32 = 14;
/// 8 bit PC relative PLT address
pub const R_68K_PLT8: u32 = 15;
/// 32 bit PLT offset
pub const R_68K_PLT32O: u32 = 16;
/// 16 bit PLT offset
pub const R_68K_PLT16O: u32 = 17;
/// 8 bit PLT offset
pub const R_68K_PLT8O: u32 = 18;
/// Copy symbol at runtime
pub const R_68K_COPY: u32 = 19;
/// Create GOT entry
pub const R_68K_GLOB_DAT: u32 = 20;
/// Create PLT entry
pub const R_68K_JMP_SLOT: u32 = 21;
/// Adjust by program base
pub const R_68K_RELATIVE: u32 = 22;
/// 32 bit GOT offset for GD
pub const R_68K_TLS_GD32: u32 = 25;
/// 16 bit GOT offset for GD
pub const R_68K_TLS_GD16: u32 = 26;
/// 8 bit GOT offset for GD
pub const R_68K_TLS_GD8: u32 = 27;
/// 32 bit GOT offset for LDM
pub const R_68K_TLS_LDM32: u32 = 28;
/// 16 bit GOT offset for LDM
pub const R_68K_TLS_LDM16: u32 = 29;
/// 8 bit GOT offset for LDM
pub const R_68K_TLS_LDM8: u32 = 30;
/// 32 bit module-relative offset
pub const R_68K_TLS_LDO32: u32 = 31;
/// 16 bit module-relative offset
pub const R_68K_TLS_LDO16: u32 = 32;
/// 8 bit module-relative offset
pub const R_68K_TLS_LDO8: u32 = 33;
/// 32 bit GOT offset for IE
pub const R_68K_TLS_IE32: u32 = 34;
/// 16 bit GOT offset for IE
pub const R_68K_TLS_IE16: u32 = 35;
/// 8 bit GOT offset for IE
pub const R_68K_TLS_IE8: u32 = 36;
/// 32 bit offset relative to static TLS block
pub const R_68K_TLS_LE32: u32 = 37;
/// 16 bit offset relative to static TLS block
pub const R_68K_TLS_LE16: u32 = 38;
/// 8 bit offset relative to static TLS block
pub const R_68K_TLS_LE8: u32 = 39;
/// 32 bit module number
pub const R_68K_TLS_DTPMOD32: u32 = 40;
/// 32 bit module-relative offset
pub const R_68K_TLS_DTPREL32: u32 = 41;
/// 32 bit TP-relative offset
pub const R_68K_TLS_TPREL32: u32 = 42;

// Intel 80386 specific definitions.

// i386 values for `Rel*::r_type`.

/// No reloc
pub const R_386_NONE: u32 = 0;
/// Direct 32 bit
pub const R_386_32: u32 = 1;
/// PC relative 32 bit
pub const R_386_PC32: u32 = 2;
/// 32 bit GOT entry
pub const R_386_GOT32: u32 = 3;
/// 32 bit PLT address
pub const R_386_PLT32: u32 = 4;
/// Copy symbol at runtime
pub const R_386_COPY: u32 = 5;
/// Create GOT entry
pub const R_386_GLOB_DAT: u32 = 6;
/// Create PLT entry
pub const R_386_JMP_SLOT: u32 = 7;
/// Adjust by program base
pub const R_386_RELATIVE: u32 = 8;
/// 32 bit offset to GOT
pub const R_386_GOTOFF: u32 = 9;
/// 32 bit PC relative offset to GOT
pub const R_386_GOTPC: u32 = 10;
/// Direct 32 bit PLT address
pub const R_386_32PLT: u32 = 11;
/// Offset in static TLS block
pub const R_386_TLS_TPOFF: u32 = 14;
/// Address of GOT entry for static TLS block offset
pub const R_386_TLS_IE: u32 = 15;
/// GOT entry for static TLS block offset
pub const R_386_TLS_GOTIE: u32 = 16;
/// Offset relative to static TLS block
pub const R_386_TLS_LE: u32 = 17;
/// Direct 32 bit for GNU version of general dynamic thread local data
pub const R_386_TLS_GD: u32 = 18;
/// Direct 32 bit for GNU version of local dynamic thread local data in LE code
pub const R_386_TLS_LDM: u32 = 19;
/// Direct 16 bit
pub const R_386_16: u32 = 20;
/// PC relative 16 bit
pub const R_386_PC16: u32 = 21;
/// Direct 8 bit
pub const R_386_8: u32 = 22;
/// PC relative 8 bit
pub const R_386_PC8: u32 = 23;
/// Direct 32 bit for general dynamic thread local data
pub const R_386_TLS_GD_32: u32 = 24;
/// Tag for pushl in GD TLS code
pub const R_386_TLS_GD_PUSH: u32 = 25;
/// Relocation for call to __tls_get_addr()
pub const R_386_TLS_GD_CALL: u32 = 26;
/// Tag for popl in GD TLS code
pub const R_386_TLS_GD_POP: u32 = 27;
/// Direct 32 bit for local dynamic thread local data in LE code
pub const R_386_TLS_LDM_32: u32 = 28;
/// Tag for pushl in LDM TLS code
pub const R_386_TLS_LDM_PUSH: u32 = 29;
/// Relocation for call to __tls_get_addr() in LDM code
pub const R_386_TLS_LDM_CALL: u32 = 30;
/// Tag for popl in LDM TLS code
pub const R_386_TLS_LDM_POP: u32 = 31;
/// Offset relative to TLS block
pub const R_386_TLS_LDO_32: u32 = 32;
/// GOT entry for negated static TLS block offset
pub const R_386_TLS_IE_32: u32 = 33;
/// Negated offset relative to static TLS block
pub const R_386_TLS_LE_32: u32 = 34;
/// ID of module containing symbol
pub const R_386_TLS_DTPMOD32: u32 = 35;
/// Offset in TLS block
pub const R_386_TLS_DTPOFF32: u32 = 36;
/// Negated offset in static TLS block
pub const R_386_TLS_TPOFF32: u32 = 37;
/// 32-bit symbol size
pub const R_386_SIZE32: u32 = 38;
/// GOT offset for TLS descriptor.
pub const R_386_TLS_GOTDESC: u32 = 39;
/// Marker of call through TLS descriptor for relaxation.
pub const R_386_TLS_DESC_CALL: u32 = 40;
/// TLS descriptor containing pointer to code and to argument, returning the TLS offset for the symbol.
pub const R_386_TLS_DESC: u32 = 41;
/// Adjust indirectly by program base
pub const R_386_IRELATIVE: u32 = 42;
/// Load from 32 bit GOT entry, relaxable.
pub const R_386_GOT32X: u32 = 43;

// ADI SHARC specific definitions

// SHARC values for `Rel*::r_type`

/// 24-bit absolute address in bits 23:0 of a 48-bit instr
///
/// Targets:
///
/// * Type 25a (PC_DIRECT)
pub const R_SHARC_ADDR24_V3: u32 = 0x0b;

/// 32-bit absolute address in bits 31:0 of a 48-bit instr
///
/// Targets:
///
/// * Type 14a
/// * Type 14d
/// * Type 15a
/// * Type 16a
/// * Type 17a
/// * Type 18a
/// * Type 19a
pub const R_SHARC_ADDR32_V3: u32 = 0x0c;

/// 32-bit absolute address in bits 31:0 of a 32-bit data location
///
/// Represented with `RelocationEncoding::Generic`
pub const R_SHARC_ADDR_VAR_V3: u32 = 0x0d;

/// 6-bit PC-relative address in bits 32:27 of a 48-bit instr
///
/// Targets:
///
/// * Type 9a
/// * Type 10a
pub const R_SHARC_PCRSHORT_V3: u32 = 0x0e;

/// 24-bit PC-relative address in bits 23:0 of a 48-bit instr
///
/// Targets:
///
/// * Type 8a
/// * Type 12a (truncated to 23 bits after relocation)
/// * Type 13a (truncated to 23 bits after relocation)
/// * Type 25a (PC Relative)
pub const R_SHARC_PCRLONG_V3: u32 = 0x0f;

/// 6-bit absolute address in bits 32:27 of a 48-bit instr
///
/// Targets:
///
/// * Type 4a
/// * Type 4b
/// * Type 4d
pub const R_SHARC_DATA6_V3: u32 = 0x10;

/// 16-bit absolute address in bits 39:24 of a 48-bit instr
///
/// Targets:
///
/// * Type 12a
pub const R_SHARC_DATA16_V3: u32 = 0x11;

/// 6-bit absolute address into bits 16:11 of a 32-bit instr
///
/// Targets:
///
/// * Type 4b
pub const R_SHARC_DATA6_VISA_V3: u32 = 0x12;

/// 7-bit absolute address into bits 6:0 of a 32-bit instr
pub const R_SHARC_DATA7_VISA_V3: u32 = 0x13;

/// 16-bit absolute address into bits 15:0 of a 32-bit instr
pub const R_SHARC_DATA16_VISA_V3: u32 = 0x14;

/// 6-bit PC-relative address into bits 16:11 of a Type B
///
/// Targets:
///
/// * Type 9b
pub const R_SHARC_PCR6_VISA_V3: u32 = 0x17;

/// 16-bit absolute address into bits 15:0 of a 16-bit location.
///
/// Represented with `RelocationEncoding::Generic`
pub const R_SHARC_ADDR_VAR16_V3: u32 = 0x19;

pub const R_SHARC_CALC_PUSH_ADDR: u32 = 0xe0;
pub const R_SHARC_CALC_PUSH_ADDEND: u32 = 0xe1;
pub const R_SHARC_CALC_ADD: u32 = 0xe2;
pub const R_SHARC_CALC_SUB: u32 = 0xe3;
pub const R_SHARC_CALC_MUL: u32 = 0xe4;
pub const R_SHARC_CALC_DIV: u32 = 0xe5;
pub const R_SHARC_CALC_MOD: u32 = 0xe6;
pub const R_SHARC_CALC_LSHIFT: u32 = 0xe7;
pub const R_SHARC_CALC_RSHIFT: u32 = 0xe8;
pub const R_SHARC_CALC_AND: u32 = 0xe9;
pub const R_SHARC_CALC_OR: u32 = 0xea;
pub const R_SHARC_CALC_XOR: u32 = 0xeb;
pub const R_SHARC_CALC_PUSH_LEN: u32 = 0xec;
pub const R_SHARC_CALC_NOT: u32 = 0xf6;

// SHARC values for `SectionHeader*::sh_type`.

/// .adi.attributes
pub const SHT_SHARC_ADI_ATTRIBUTES: u32 = SHT_LOPROC + 0x2;

// SUN SPARC specific definitions.

// SPARC values for `st_type` component of `Sym*::st_info`.

/// Global register reserved to app.
pub const STT_SPARC_REGISTER: u8 = 13;

// SPARC values for `FileHeader64::e_flags`.

pub const EF_SPARCV9_MM: u32 = 3;
pub const EF_SPARCV9_TSO: u32 = 0;
pub const EF_SPARCV9_PSO: u32 = 1;
pub const EF_SPARCV9_RMO: u32 = 2;
/// little endian data
pub const EF_SPARC_LEDATA: u32 = 0x80_0000;
pub const EF_SPARC_EXT_MASK: u32 = 0xFF_FF00;
/// generic V8+ features
pub const EF_SPARC_32PLUS: u32 = 0x00_0100;
/// Sun UltraSPARC1 extensions
pub const EF_SPARC_SUN_US1: u32 = 0x00_0200;
/// HAL R1 extensions
pub const EF_SPARC_HAL_R1: u32 = 0x00_0400;
/// Sun UltraSPARCIII extensions
pub const EF_SPARC_SUN_US3: u32 = 0x00_0800;

// SPARC values for `Rel*::r_type`.

/// No reloc
pub const R_SPARC_NONE: u32 = 0;
/// Direct 8 bit
pub const R_SPARC_8: u32 = 1;
/// Direct 16 bit
pub const R_SPARC_16: u32 = 2;
/// Direct 32 bit
pub const R_SPARC_32: u32 = 3;
/// PC relative 8 bit
pub const R_SPARC_DISP8: u32 = 4;
/// PC relative 16 bit
pub const R_SPARC_DISP16: u32 = 5;
/// PC relative 32 bit
pub const R_SPARC_DISP32: u32 = 6;
/// PC relative 30 bit shifted
pub const R_SPARC_WDISP30: u32 = 7;
/// PC relative 22 bit shifted
pub const R_SPARC_WDISP22: u32 = 8;
/// High 22 bit
pub const R_SPARC_HI22: u32 = 9;
/// Direct 22 bit
pub const R_SPARC_22: u32 = 10;
/// Direct 13 bit
pub const R_SPARC_13: u32 = 11;
/// Truncated 10 bit
pub const R_SPARC_LO10: u32 = 12;
/// Truncated 10 bit GOT entry
pub const R_SPARC_GOT10: u32 = 13;
/// 13 bit GOT entry
pub const R_SPARC_GOT13: u32 = 14;
/// 22 bit GOT entry shifted
pub const R_SPARC_GOT22: u32 = 15;
/// PC relative 10 bit truncated
pub const R_SPARC_PC10: u32 = 16;
/// PC relative 22 bit shifted
pub const R_SPARC_PC22: u32 = 17;
/// 30 bit PC relative PLT address
pub const R_SPARC_WPLT30: u32 = 18;
/// Copy symbol at runtime
pub const R_SPARC_COPY: u32 = 19;
/// Create GOT entry
pub const R_SPARC_GLOB_DAT: u32 = 20;
/// Create PLT entry
pub const R_SPARC_JMP_SLOT: u32 = 21;
/// Adjust by program base
pub const R_SPARC_RELATIVE: u32 = 22;
/// Direct 32 bit unaligned
pub const R_SPARC_UA32: u32 = 23;

// Sparc64 values for `Rel*::r_type`.

/// Direct 32 bit ref to PLT entry
pub const R_SPARC_PLT32: u32 = 24;
/// High 22 bit PLT entry
pub const R_SPARC_HIPLT22: u32 = 25;
/// Truncated 10 bit PLT entry
pub const R_SPARC_LOPLT10: u32 = 26;
/// PC rel 32 bit ref to PLT entry
pub const R_SPARC_PCPLT32: u32 = 27;
/// PC rel high 22 bit PLT entry
pub const R_SPARC_PCPLT22: u32 = 28;
/// PC rel trunc 10 bit PLT entry
pub const R_SPARC_PCPLT10: u32 = 29;
/// Direct 10 bit
pub const R_SPARC_10: u32 = 30;
/// Direct 11 bit
pub const R_SPARC_11: u32 = 31;
/// Direct 64 bit
pub const R_SPARC_64: u32 = 32;
/// 10bit with secondary 13bit addend
pub const R_SPARC_OLO10: u32 = 33;
/// Top 22 bits of direct 64 bit
pub const R_SPARC_HH22: u32 = 34;
/// High middle 10 bits of ...
pub const R_SPARC_HM10: u32 = 35;
/// Low middle 22 bits of ...
pub const R_SPARC_LM22: u32 = 36;
/// Top 22 bits of pc rel 64 bit
pub const R_SPARC_PC_HH22: u32 = 37;
/// High middle 10 bit of ...
pub const R_SPARC_PC_HM10: u32 = 38;
/// Low miggle 22 bits of ...
pub const R_SPARC_PC_LM22: u32 = 39;
/// PC relative 16 bit shifted
pub const R_SPARC_WDISP16: u32 = 40;
/// PC relative 19 bit shifted
pub const R_SPARC_WDISP19: u32 = 41;
/// was part of v9 ABI but was removed
pub const R_SPARC_GLOB_JMP: u32 = 42;
/// Direct 7 bit
pub const R_SPARC_7: u32 = 43;
/// Direct 5 bit
pub const R_SPARC_5: u32 = 44;
/// Direct 6 bit
pub const R_SPARC_6: u32 = 45;
/// PC relative 64 bit
pub const R_SPARC_DISP64: u32 = 46;
/// Direct 64 bit ref to PLT entry
pub const R_SPARC_PLT64: u32 = 47;
/// High 22 bit complemented
pub const R_SPARC_HIX22: u32 = 48;
/// Truncated 11 bit complemented
pub const R_SPARC_LOX10: u32 = 49;
/// Direct high 12 of 44 bit
pub const R_SPARC_H44: u32 = 50;
/// Direct mid 22 of 44 bit
pub const R_SPARC_M44: u32 = 51;
/// Direct low 10 of 44 bit
pub const R_SPARC_L44: u32 = 52;
/// Global register usage
pub const R_SPARC_REGISTER: u32 = 53;
/// Direct 64 bit unaligned
pub const R_SPARC_UA64: u32 = 54;
/// Direct 16 bit unaligned
pub const R_SPARC_UA16: u32 = 55;
pub const R_SPARC_TLS_GD_HI22: u32 = 56;
pub const R_SPARC_TLS_GD_LO10: u32 = 57;
pub const R_SPARC_TLS_GD_ADD: u32 = 58;
pub const R_SPARC_TLS_GD_CALL: u32 = 59;
pub const R_SPARC_TLS_LDM_HI22: u32 = 60;
pub const R_SPARC_TLS_LDM_LO10: u32 = 61;
pub const R_SPARC_TLS_LDM_ADD: u32 = 62;
pub const R_SPARC_TLS_LDM_CALL: u32 = 63;
pub const R_SPARC_TLS_LDO_HIX22: u32 = 64;
pub const R_SPARC_TLS_LDO_LOX10: u32 = 65;
pub const R_SPARC_TLS_LDO_ADD: u32 = 66;
pub const R_SPARC_TLS_IE_HI22: u32 = 67;
pub const R_SPARC_TLS_IE_LO10: u32 = 68;
pub const R_SPARC_TLS_IE_LD: u32 = 69;
pub const R_SPARC_TLS_IE_LDX: u32 = 70;
pub const R_SPARC_TLS_IE_ADD: u32 = 71;
pub const R_SPARC_TLS_LE_HIX22: u32 = 72;
pub const R_SPARC_TLS_LE_LOX10: u32 = 73;
pub const R_SPARC_TLS_DTPMOD32: u32 = 74;
pub const R_SPARC_TLS_DTPMOD64: u32 = 75;
pub const R_SPARC_TLS_DTPOFF32: u32 = 76;
pub const R_SPARC_TLS_DTPOFF64: u32 = 77;
pub const R_SPARC_TLS_TPOFF32: u32 = 78;
pub const R_SPARC_TLS_TPOFF64: u32 = 79;
pub const R_SPARC_GOTDATA_HIX22: u32 = 80;
pub const R_SPARC_GOTDATA_LOX10: u32 = 81;
pub const R_SPARC_GOTDATA_OP_HIX22: u32 = 82;
pub const R_SPARC_GOTDATA_OP_LOX10: u32 = 83;
pub const R_SPARC_GOTDATA_OP: u32 = 84;
pub const R_SPARC_H34: u32 = 85;
pub const R_SPARC_SIZE32: u32 = 86;
pub const R_SPARC_SIZE64: u32 = 87;
pub const R_SPARC_WDISP10: u32 = 88;
pub const R_SPARC_JMP_IREL: u32 = 248;
pub const R_SPARC_IRELATIVE: u32 = 249;
pub const R_SPARC_GNU_VTINHERIT: u32 = 250;
pub const R_SPARC_GNU_VTENTRY: u32 = 251;
pub const R_SPARC_REV32: u32 = 252;

// Sparc64 values for `Dyn32::d_tag`.

pub const DT_SPARC_REGISTER: u32 = 0x7000_0001;

// MIPS R3000 specific definitions.

// MIPS values for `FileHeader32::e_flags`.

/// A .noreorder directive was used.
pub const EF_MIPS_NOREORDER: u32 = 1;
/// Contains PIC code.
pub const EF_MIPS_PIC: u32 = 2;
/// Uses PIC calling sequence.
pub const EF_MIPS_CPIC: u32 = 4;
pub const EF_MIPS_XGOT: u32 = 8;
pub const EF_MIPS_64BIT_WHIRL: u32 = 16;
pub const EF_MIPS_ABI2: u32 = 32;
pub const EF_MIPS_ABI_ON32: u32 = 64;
/// Uses FP64 (12 callee-saved).
pub const EF_MIPS_FP64: u32 = 512;
/// Uses IEEE 754-2008 NaN encoding.
pub const EF_MIPS_NAN2008: u32 = 1024;
/// MIPS architecture level.
pub const EF_MIPS_ARCH: u32 = 0xf000_0000;

/// The first MIPS 32 bit ABI
pub const EF_MIPS_ABI_O32: u32 = 0x0000_1000;
/// O32 ABI extended for 64-bit architectures
pub const EF_MIPS_ABI_O64: u32 = 0x0000_2000;
/// EABI in 32-bit mode
pub const EF_MIPS_ABI_EABI32: u32 = 0x0000_3000;
/// EABI in 64-bit mode
pub const EF_MIPS_ABI_EABI64: u32 = 0x0000_4000;
/// Mask for selecting EF_MIPS_ABI_ variant
pub const EF_MIPS_ABI: u32 = 0x0000_f000;

// Legal values for MIPS architecture level.

/// -mips1 code.
pub const EF_MIPS_ARCH_1: u32 = 0x0000_0000;
/// -mips2 code.
pub const EF_MIPS_ARCH_2: u32 = 0x1000_0000;
/// -mips3 code.
pub const EF_MIPS_ARCH_3: u32 = 0x2000_0000;
/// -mips4 code.
pub const EF_MIPS_ARCH_4: u32 = 0x3000_0000;
/// -mips5 code.
pub const EF_MIPS_ARCH_5: u32 = 0x4000_0000;
/// MIPS32 code.
pub const EF_MIPS_ARCH_32: u32 = 0x5000_0000;
/// MIPS64 code.
pub const EF_MIPS_ARCH_64: u32 = 0x6000_0000;
/// MIPS32r2 code.
pub const EF_MIPS_ARCH_32R2: u32 = 0x7000_0000;
/// MIPS64r2 code.
pub const EF_MIPS_ARCH_64R2: u32 = 0x8000_0000;
/// MIPS32r6 code
pub const EF_MIPS_ARCH_32R6: u32 = 0x9000_0000;
/// MIPS64r6 code
pub const EF_MIPS_ARCH_64R6: u32 = 0xa000_0000;

// MIPS values for `Sym32::st_shndx`.

/// Allocated common symbols.
pub const SHN_MIPS_ACOMMON: u16 = 0xff00;
/// Allocated test symbols.
pub const SHN_MIPS_TEXT: u16 = 0xff01;
/// Allocated data symbols.
pub const SHN_MIPS_DATA: u16 = 0xff02;
/// Small common symbols.
pub const SHN_MIPS_SCOMMON: u16 = 0xff03;
/// Small undefined symbols.
pub const SHN_MIPS_SUNDEFINED: u16 = 0xff04;

// MIPS values for `SectionHeader32::sh_type`.

/// Shared objects used in link.
pub const SHT_MIPS_LIBLIST: u32 = 0x7000_0000;
pub const SHT_MIPS_MSYM: u32 = 0x7000_0001;
/// Conflicting symbols.
pub const SHT_MIPS_CONFLICT: u32 = 0x7000_0002;
/// Global data area sizes.
pub const SHT_MIPS_GPTAB: u32 = 0x7000_0003;
/// Reserved for SGI/MIPS compilers
pub const SHT_MIPS_UCODE: u32 = 0x7000_0004;
/// MIPS ECOFF debugging info.
pub const SHT_MIPS_DEBUG: u32 = 0x7000_0005;
/// Register usage information.
pub const SHT_MIPS_REGINFO: u32 = 0x7000_0006;
pub const SHT_MIPS_PACKAGE: u32 = 0x7000_0007;
pub const SHT_MIPS_PACKSYM: u32 = 0x7000_0008;
pub const SHT_MIPS_RELD: u32 = 0x7000_0009;
pub const SHT_MIPS_IFACE: u32 = 0x7000_000b;
pub const SHT_MIPS_CONTENT: u32 = 0x7000_000c;
/// Miscellaneous options.
pub const SHT_MIPS_OPTIONS: u32 = 0x7000_000d;
pub const SHT_MIPS_SHDR: u32 = 0x7000_0010;
pub const SHT_MIPS_FDESC: u32 = 0x7000_0011;
pub const SHT_MIPS_EXTSYM: u32 = 0x7000_0012;
pub const SHT_MIPS_DENSE: u32 = 0x7000_0013;
pub const SHT_MIPS_PDESC: u32 = 0x7000_0014;
pub const SHT_MIPS_LOCSYM: u32 = 0x7000_0015;
pub const SHT_MIPS_AUXSYM: u32 = 0x7000_0016;
pub const SHT_MIPS_OPTSYM: u32 = 0x7000_0017;
pub const SHT_MIPS_LOCSTR: u32 = 0x7000_0018;
pub const SHT_MIPS_LINE: u32 = 0x7000_0019;
pub const SHT_MIPS_RFDESC: u32 = 0x7000_001a;
pub const SHT_MIPS_DELTASYM: u32 = 0x7000_001b;
pub const SHT_MIPS_DELTAINST: u32 = 0x7000_001c;
pub const SHT_MIPS_DELTACLASS: u32 = 0x7000_001d;
/// DWARF debugging information.
pub const SHT_MIPS_DWARF: u32 = 0x7000_001e;
pub const SHT_MIPS_DELTADECL: u32 = 0x7000_001f;
pub const SHT_MIPS_SYMBOL_LIB: u32 = 0x7000_0020;
/// Event section.
pub const SHT_MIPS_EVENTS: u32 = 0x7000_0021;
pub const SHT_MIPS_TRANSLATE: u32 = 0x7000_0022;
pub const SHT_MIPS_PIXIE: u32 = 0x7000_0023;
pub const SHT_MIPS_XLATE: u32 = 0x7000_0024;
pub const SHT_MIPS_XLATE_DEBUG: u32 = 0x7000_0025;
pub const SHT_MIPS_WHIRL: u32 = 0x7000_0026;
pub const SHT_MIPS_EH_REGION: u32 = 0x7000_0027;
pub const SHT_MIPS_XLATE_OLD: u32 = 0x7000_0028;
pub const SHT_MIPS_PDR_EXCEPTION: u32 = 0x7000_0029;

// MIPS values for `SectionHeader32::sh_flags`.

/// Must be in global data area.
pub const SHF_MIPS_GPREL: u32 = 0x1000_0000;
pub const SHF_MIPS_MERGE: u32 = 0x2000_0000;
pub const SHF_MIPS_ADDR: u32 = 0x4000_0000;
pub const SHF_MIPS_STRINGS: u32 = 0x8000_0000;
pub const SHF_MIPS_NOSTRIP: u32 = 0x0800_0000;
pub const SHF_MIPS_LOCAL: u32 = 0x0400_0000;
pub const SHF_MIPS_NAMES: u32 = 0x0200_0000;
pub const SHF_MIPS_NODUPE: u32 = 0x0100_0000;

// MIPS values for `Sym32::st_other`.

pub const STO_MIPS_PLT: u8 = 0x8;
/// Only valid for `STB_MIPS_SPLIT_COMMON`.
pub const STO_MIPS_SC_ALIGN_UNUSED: u8 = 0xff;

// MIPS values for `Sym32::st_info'.
pub const STB_MIPS_SPLIT_COMMON: u8 = 13;

// Entries found in sections of type `SHT_MIPS_GPTAB`.

// TODO: Elf32_gptab, Elf32_RegInfo, Elf_Options

// Values for `Elf_Options::kind`.

/// Undefined.
pub const ODK_NULL: u32 = 0;
/// Register usage information.
pub const ODK_REGINFO: u32 = 1;
/// Exception processing options.
pub const ODK_EXCEPTIONS: u32 = 2;
/// Section padding options.
pub const ODK_PAD: u32 = 3;
/// Hardware workarounds performed
pub const ODK_HWPATCH: u32 = 4;
/// record the fill value used by the linker.
pub const ODK_FILL: u32 = 5;
/// reserve space for desktop tools to write.
pub const ODK_TAGS: u32 = 6;
/// HW workarounds.  'AND' bits when merging.
pub const ODK_HWAND: u32 = 7;
/// HW workarounds.  'OR' bits when merging.
pub const ODK_HWOR: u32 = 8;

// Values for `Elf_Options::info` for `ODK_EXCEPTIONS` entries.

/// FPE's which MUST be enabled.
pub const OEX_FPU_MIN: u32 = 0x1f;
/// FPE's which MAY be enabled.
pub const OEX_FPU_MAX: u32 = 0x1f00;
/// page zero must be mapped.
pub const OEX_PAGE0: u32 = 0x10000;
/// Force sequential memory mode?
pub const OEX_SMM: u32 = 0x20000;
/// Force floating point debug mode?
pub const OEX_FPDBUG: u32 = 0x40000;
pub const OEX_PRECISEFP: u32 = OEX_FPDBUG;
/// Dismiss invalid address faults?
pub const OEX_DISMISS: u32 = 0x80000;

pub const OEX_FPU_INVAL: u32 = 0x10;
pub const OEX_FPU_DIV0: u32 = 0x08;
pub const OEX_FPU_OFLO: u32 = 0x04;
pub const OEX_FPU_UFLO: u32 = 0x02;
pub const OEX_FPU_INEX: u32 = 0x01;

// Masks for `Elf_Options::info` for an `ODK_HWPATCH` entry.  */
/// R4000 end-of-page patch.
pub const OHW_R4KEOP: u32 = 0x1;
/// may need R8000 prefetch patch.
pub const OHW_R8KPFETCH: u32 = 0x2;
/// R5000 end-of-page patch.
pub const OHW_R5KEOP: u32 = 0x4;
/// R5000 cvt.\[ds\].l bug.  clean=1.
pub const OHW_R5KCVTL: u32 = 0x8;

pub const OPAD_PREFIX: u32 = 0x1;
pub const OPAD_POSTFIX: u32 = 0x2;
pub const OPAD_SYMBOL: u32 = 0x4;

// Entries found in sections of type `SHT_MIPS_OPTIONS`.

// TODO: Elf_Options_Hw

// Masks for `ElfOptions::info` for `ODK_HWAND` and `ODK_HWOR` entries.

pub const OHWA0_R4KEOP_CHECKED: u32 = 0x0000_0001;
pub const OHWA1_R4KEOP_CLEAN: u32 = 0x0000_0002;

// MIPS values for `Rel*::r_type`.

/// No reloc
pub const R_MIPS_NONE: u32 = 0;
/// Direct 16 bit
pub const R_MIPS_16: u32 = 1;
/// Direct 32 bit
pub const R_MIPS_32: u32 = 2;
/// PC relative 32 bit
pub const R_MIPS_REL32: u32 = 3;
/// Direct 26 bit shifted
pub const R_MIPS_26: u32 = 4;
/// High 16 bit
pub const R_MIPS_HI16: u32 = 5;
/// Low 16 bit
pub const R_MIPS_LO16: u32 = 6;
/// GP relative 16 bit
pub const R_MIPS_GPREL16: u32 = 7;
/// 16 bit literal entry
pub const R_MIPS_LITERAL: u32 = 8;
/// 16 bit GOT entry
pub const R_MIPS_GOT16: u32 = 9;
/// PC relative 16 bit
pub const R_MIPS_PC16: u32 = 10;
/// 16 bit GOT entry for function
pub const R_MIPS_CALL16: u32 = 11;
/// GP relative 32 bit
pub const R_MIPS_GPREL32: u32 = 12;

pub const R_MIPS_SHIFT5: u32 = 16;
pub const R_MIPS_SHIFT6: u32 = 17;
pub const R_MIPS_64: u32 = 18;
pub const R_MIPS_GOT_DISP: u32 = 19;
pub const R_MIPS_GOT_PAGE: u32 = 20;
pub const R_MIPS_GOT_OFST: u32 = 21;
pub const R_MIPS_GOT_HI16: u32 = 22;
pub const R_MIPS_GOT_LO16: u32 = 23;
pub const R_MIPS_SUB: u32 = 24;
pub const R_MIPS_INSERT_A: u32 = 25;
pub const R_MIPS_INSERT_B: u32 = 26;
pub const R_MIPS_DELETE: u32 = 27;
pub const R_MIPS_HIGHER: u32 = 28;
pub const R_MIPS_HIGHEST: u32 = 29;
pub const R_MIPS_CALL_HI16: u32 = 30;
pub const R_MIPS_CALL_LO16: u32 = 31;
pub const R_MIPS_SCN_DISP: u32 = 32;
pub const R_MIPS_REL16: u32 = 33;
pub const R_MIPS_ADD_IMMEDIATE: u32 = 34;
pub const R_MIPS_PJUMP: u32 = 35;
pub const R_MIPS_RELGOT: u32 = 36;
pub const R_MIPS_JALR: u32 = 37;
/// Module number 32 bit
pub const R_MIPS_TLS_DTPMOD32: u32 = 38;
/// Module-relative offset 32 bit
pub const R_MIPS_TLS_DTPREL32: u32 = 39;
/// Module number 64 bit
pub const R_MIPS_TLS_DTPMOD64: u32 = 40;
/// Module-relative offset 64 bit
pub const R_MIPS_TLS_DTPREL64: u32 = 41;
/// 16 bit GOT offset for GD
pub const R_MIPS_TLS_GD: u32 = 42;
/// 16 bit GOT offset for LDM
pub const R_MIPS_TLS_LDM: u32 = 43;
/// Module-relative offset, high 16 bits
pub const R_MIPS_TLS_DTPREL_HI16: u32 = 44;
/// Module-relative offset, low 16 bits
pub const R_MIPS_TLS_DTPREL_LO16: u32 = 45;
/// 16 bit GOT offset for IE
pub const R_MIPS_TLS_GOTTPREL: u32 = 46;
/// TP-relative offset, 32 bit
pub const R_MIPS_TLS_TPREL32: u32 = 47;
/// TP-relative offset, 64 bit
pub const R_MIPS_TLS_TPREL64: u32 = 48;
/// TP-relative offset, high 16 bits
pub const R_MIPS_TLS_TPREL_HI16: u32 = 49;
/// TP-relative offset, low 16 bits
pub const R_MIPS_TLS_TPREL_LO16: u32 = 50;
pub const R_MIPS_GLOB_DAT: u32 = 51;
pub const R_MIPS_COPY: u32 = 126;
pub const R_MIPS_JUMP_SLOT: u32 = 127;

// MIPS values for `ProgramHeader32::p_type`.

/// Register usage information.
pub const PT_MIPS_REGINFO: u32 = 0x7000_0000;
/// Runtime procedure table.
pub const PT_MIPS_RTPROC: u32 = 0x7000_0001;
pub const PT_MIPS_OPTIONS: u32 = 0x7000_0002;
/// FP mode requirement.
pub const PT_MIPS_ABIFLAGS: u32 = 0x7000_0003;

// MIPS values for `ProgramHeader32::p_flags`.

pub const PF_MIPS_LOCAL: u32 = 0x1000_0000;

// MIPS values for `Dyn32::d_tag`.

/// Runtime linker interface version
pub const DT_MIPS_RLD_VERSION: u32 = 0x7000_0001;
/// Timestamp
pub const DT_MIPS_TIME_STAMP: u32 = 0x7000_0002;
/// Checksum
pub const DT_MIPS_ICHECKSUM: u32 = 0x7000_0003;
/// Version string (string tbl index)
pub const DT_MIPS_IVERSION: u32 = 0x7000_0004;
/// Flags
pub const DT_MIPS_FLAGS: u32 = 0x7000_0005;
/// Base address
pub const DT_MIPS_BASE_ADDRESS: u32 = 0x7000_0006;
pub const DT_MIPS_MSYM: u32 = 0x7000_0007;
/// Address of CONFLICT section
pub const DT_MIPS_CONFLICT: u32 = 0x7000_0008;
/// Address of LIBLIST section
pub const DT_MIPS_LIBLIST: u32 = 0x7000_0009;
/// Number of local GOT entries
pub const DT_MIPS_LOCAL_GOTNO: u32 = 0x7000_000a;
/// Number of CONFLICT entries
pub const DT_MIPS_CONFLICTNO: u32 = 0x7000_000b;
/// Number of LIBLIST entries
pub const DT_MIPS_LIBLISTNO: u32 = 0x7000_0010;
/// Number of DYNSYM entries
pub const DT_MIPS_SYMTABNO: u32 = 0x7000_0011;
/// First external DYNSYM
pub const DT_MIPS_UNREFEXTNO: u32 = 0x7000_0012;
/// First GOT entry in DYNSYM
pub const DT_MIPS_GOTSYM: u32 = 0x7000_0013;
/// Number of GOT page table entries
pub const DT_MIPS_HIPAGENO: u32 = 0x7000_0014;
/// Address of run time loader map.
pub const DT_MIPS_RLD_MAP: u32 = 0x7000_0016;
/// Delta C++ class definition.
pub const DT_MIPS_DELTA_CLASS: u32 = 0x7000_0017;
/// Number of entries in DT_MIPS_DELTA_CLASS.
pub const DT_MIPS_DELTA_CLASS_NO: u32 = 0x7000_0018;
/// Delta C++ class instances.
pub const DT_MIPS_DELTA_INSTANCE: u32 = 0x7000_0019;
/// Number of entries in DT_MIPS_DELTA_INSTANCE.
pub const DT_MIPS_DELTA_INSTANCE_NO: u32 = 0x7000_001a;
/// Delta relocations.
pub const DT_MIPS_DELTA_RELOC: u32 = 0x7000_001b;
/// Number of entries in DT_MIPS_DELTA_RELOC.
pub const DT_MIPS_DELTA_RELOC_NO: u32 = 0x7000_001c;
/// Delta symbols that Delta relocations refer to.
pub const DT_MIPS_DELTA_SYM: u32 = 0x7000_001d;
/// Number of entries in DT_MIPS_DELTA_SYM.
pub const DT_MIPS_DELTA_SYM_NO: u32 = 0x7000_001e;
/// Delta symbols that hold the class declaration.
pub const DT_MIPS_DELTA_CLASSSYM: u32 = 0x7000_0020;
/// Number of entries in DT_MIPS_DELTA_CLASSSYM.
pub const DT_MIPS_DELTA_CLASSSYM_NO: u32 = 0x7000_0021;
/// Flags indicating for C++ flavor.
pub const DT_MIPS_CXX_FLAGS: u32 = 0x7000_0022;
pub const DT_MIPS_PIXIE_INIT: u32 = 0x7000_0023;
pub const DT_MIPS_SYMBOL_LIB: u32 = 0x7000_0024;
pub const DT_MIPS_LOCALPAGE_GOTIDX: u32 = 0x7000_0025;
pub const DT_MIPS_LOCAL_GOTIDX: u32 = 0x7000_0026;
pub const DT_MIPS_HIDDEN_GOTIDX: u32 = 0x7000_0027;
pub const DT_MIPS_PROTECTED_GOTIDX: u32 = 0x7000_0028;
/// Address of .options.
pub const DT_MIPS_OPTIONS: u32 = 0x7000_0029;
/// Address of .interface.
pub const DT_MIPS_INTERFACE: u32 = 0x7000_002a;
pub const DT_MIPS_DYNSTR_ALIGN: u32 = 0x7000_002b;
/// Size of the .interface section.
pub const DT_MIPS_INTERFACE_SIZE: u32 = 0x7000_002c;
/// Address of rld_text_rsolve function stored in GOT.
pub const DT_MIPS_RLD_TEXT_RESOLVE_ADDR: u32 = 0x7000_002d;
/// Default suffix of dso to be added by rld on dlopen() calls.
pub const DT_MIPS_PERF_SUFFIX: u32 = 0x7000_002e;
/// (O32)Size of compact rel section.
pub const DT_MIPS_COMPACT_SIZE: u32 = 0x7000_002f;
/// GP value for aux GOTs.
pub const DT_MIPS_GP_VALUE: u32 = 0x7000_0030;
/// Address of aux .dynamic.
pub const DT_MIPS_AUX_DYNAMIC: u32 = 0x7000_0031;
/// The address of .got.plt in an executable using the new non-PIC ABI.
pub const DT_MIPS_PLTGOT: u32 = 0x7000_0032;
/// The base of the PLT in an executable using the new non-PIC ABI if that PLT is writable.  For a non-writable PLT, this is omitted or has a zero value.
pub const DT_MIPS_RWPLT: u32 = 0x7000_0034;
/// An alternative description of the classic MIPS RLD_MAP that is usable in a PIE as it stores a relative offset from the address of the tag rather than an absolute address.
pub const DT_MIPS_RLD_MAP_REL: u32 = 0x7000_0035;

// Values for `DT_MIPS_FLAGS` `Dyn32` entry.

/// No flags
pub const RHF_NONE: u32 = 0;
/// Use quickstart
pub const RHF_QUICKSTART: u32 = 1 << 0;
/// Hash size not power of 2
pub const RHF_NOTPOT: u32 = 1 << 1;
/// Ignore LD_LIBRARY_PATH
pub const RHF_NO_LIBRARY_REPLACEMENT: u32 = 1 << 2;
pub const RHF_NO_MOVE: u32 = 1 << 3;
pub const RHF_SGI_ONLY: u32 = 1 << 4;
pub const RHF_GUARANTEE_INIT: u32 = 1 << 5;
pub const RHF_DELTA_C_PLUS_PLUS: u32 = 1 << 6;
pub const RHF_GUARANTEE_START_INIT: u32 = 1 << 7;
pub const RHF_PIXIE: u32 = 1 << 8;
pub const RHF_DEFAULT_DELAY_LOAD: u32 = 1 << 9;
pub const RHF_REQUICKSTART: u32 = 1 << 10;
pub const RHF_REQUICKSTARTED: u32 = 1 << 11;
pub const RHF_CORD: u32 = 1 << 12;
pub const RHF_NO_UNRES_UNDEF: u32 = 1 << 13;
pub const RHF_RLD_ORDER_SAFE: u32 = 1 << 14;

// Entries found in sections of type `SHT_MIPS_LIBLIST`.

// TODO: Elf32_Lib, Elf64_Lib

// Values for `Lib*::l_flags`.

pub const LL_NONE: u32 = 0;
/// Require exact match
pub const LL_EXACT_MATCH: u32 = 1 << 0;
/// Ignore interface version
pub const LL_IGNORE_INT_VER: u32 = 1 << 1;
pub const LL_REQUIRE_MINOR: u32 = 1 << 2;
pub const LL_EXPORTS: u32 = 1 << 3;
pub const LL_DELAY_LOAD: u32 = 1 << 4;
pub const LL_DELTA: u32 = 1 << 5;

// TODO: MIPS ABI flags

// PA-RISC specific definitions.

// PA-RISC values for `FileHeader32::e_flags`.

/// Trap nil pointer dereference.
pub const EF_PARISC_TRAPNIL: u32 = 0x0001_0000;
/// Program uses arch. extensions.
pub const EF_PARISC_EXT: u32 = 0x0002_0000;
/// Program expects little endian.
pub const EF_PARISC_LSB: u32 = 0x0004_0000;
/// Program expects wide mode.
pub const EF_PARISC_WIDE: u32 = 0x0008_0000;
/// No kernel assisted branch prediction.
pub const EF_PARISC_NO_KABP: u32 = 0x0010_0000;
/// Allow lazy swapping.
pub const EF_PARISC_LAZYSWAP: u32 = 0x0040_0000;
/// Architecture version.
pub const EF_PARISC_ARCH: u32 = 0x0000_ffff;

// Values for `EF_PARISC_ARCH'.

/// PA-RISC 1.0 big-endian.
pub const EFA_PARISC_1_0: u32 = 0x020b;
/// PA-RISC 1.1 big-endian.
pub const EFA_PARISC_1_1: u32 = 0x0210;
/// PA-RISC 2.0 big-endian.
pub const EFA_PARISC_2_0: u32 = 0x0214;

// PA-RISC values for `Sym*::st_shndx`.

/// Section for tentatively declared symbols in ANSI C.
pub const SHN_PARISC_ANSI_COMMON: u16 = 0xff00;
/// Common blocks in huge model.
pub const SHN_PARISC_HUGE_COMMON: u16 = 0xff01;

// PA-RISC values for `SectionHeader32::sh_type`.

/// Contains product specific ext.
pub const SHT_PARISC_EXT: u32 = 0x7000_0000;
/// Unwind information.
pub const SHT_PARISC_UNWIND: u32 = 0x7000_0001;
/// Debug info for optimized code.
pub const SHT_PARISC_DOC: u32 = 0x7000_0002;

// PA-RISC values for `SectionHeader32::sh_flags`.

/// Section with short addressing.
pub const SHF_PARISC_SHORT: u32 = 0x2000_0000;
/// Section far from gp.
pub const SHF_PARISC_HUGE: u32 = 0x4000_0000;
/// Static branch prediction code.
pub const SHF_PARISC_SBP: u32 = 0x8000_0000;

// PA-RISC values for `st_type` component of `Sym32::st_info`.

/// Millicode function entry point.
pub const STT_PARISC_MILLICODE: u8 = 13;

pub const STT_HP_OPAQUE: u8 = STT_LOOS + 0x1;
pub const STT_HP_STUB: u8 = STT_LOOS + 0x2;

// PA-RISC values for `Rel*::r_type`.

/// No reloc.
pub const R_PARISC_NONE: u32 = 0;
/// Direct 32-bit reference.
pub const R_PARISC_DIR32: u32 = 1;
/// Left 21 bits of eff. address.
pub const R_PARISC_DIR21L: u32 = 2;
/// Right 17 bits of eff. address.
pub const R_PARISC_DIR17R: u32 = 3;
/// 17 bits of eff. address.
pub const R_PARISC_DIR17F: u32 = 4;
/// Right 14 bits of eff. address.
pub const R_PARISC_DIR14R: u32 = 6;
/// 32-bit rel. address.
pub const R_PARISC_PCREL32: u32 = 9;
/// Left 21 bits of rel. address.
pub const R_PARISC_PCREL21L: u32 = 10;
/// Right 17 bits of rel. address.
pub const R_PARISC_PCREL17R: u32 = 11;
/// 17 bits of rel. address.
pub const R_PARISC_PCREL17F: u32 = 12;
/// Right 14 bits of rel. address.
pub const R_PARISC_PCREL14R: u32 = 14;
/// Left 21 bits of rel. address.
pub const R_PARISC_DPREL21L: u32 = 18;
/// Right 14 bits of rel. address.
pub const R_PARISC_DPREL14R: u32 = 22;
/// GP-relative, left 21 bits.
pub const R_PARISC_GPREL21L: u32 = 26;
/// GP-relative, right 14 bits.
pub const R_PARISC_GPREL14R: u32 = 30;
/// LT-relative, left 21 bits.
pub const R_PARISC_LTOFF21L: u32 = 34;
/// LT-relative, right 14 bits.
pub const R_PARISC_LTOFF14R: u32 = 38;
/// 32 bits section rel. address.
pub const R_PARISC_SECREL32: u32 = 41;
/// No relocation, set segment base.
pub const R_PARISC_SEGBASE: u32 = 48;
/// 32 bits segment rel. address.
pub const R_PARISC_SEGREL32: u32 = 49;
/// PLT rel. address, left 21 bits.
pub const R_PARISC_PLTOFF21L: u32 = 50;
/// PLT rel. address, right 14 bits.
pub const R_PARISC_PLTOFF14R: u32 = 54;
/// 32 bits LT-rel. function pointer.
pub const R_PARISC_LTOFF_FPTR32: u32 = 57;
/// LT-rel. fct ptr, left 21 bits.
pub const R_PARISC_LTOFF_FPTR21L: u32 = 58;
/// LT-rel. fct ptr, right 14 bits.
pub const R_PARISC_LTOFF_FPTR14R: u32 = 62;
/// 64 bits function address.
pub const R_PARISC_FPTR64: u32 = 64;
/// 32 bits function address.
pub const R_PARISC_PLABEL32: u32 = 65;
/// Left 21 bits of fdesc address.
pub const R_PARISC_PLABEL21L: u32 = 66;
/// Right 14 bits of fdesc address.
pub const R_PARISC_PLABEL14R: u32 = 70;
/// 64 bits PC-rel. address.
pub const R_PARISC_PCREL64: u32 = 72;
/// 22 bits PC-rel. address.
pub const R_PARISC_PCREL22F: u32 = 74;
/// PC-rel. address, right 14 bits.
pub const R_PARISC_PCREL14WR: u32 = 75;
/// PC rel. address, right 14 bits.
pub const R_PARISC_PCREL14DR: u32 = 76;
/// 16 bits PC-rel. address.
pub const R_PARISC_PCREL16F: u32 = 77;
/// 16 bits PC-rel. address.
pub const R_PARISC_PCREL16WF: u32 = 78;
/// 16 bits PC-rel. address.
pub const R_PARISC_PCREL16DF: u32 = 79;
/// 64 bits of eff. address.
pub const R_PARISC_DIR64: u32 = 80;
/// 14 bits of eff. address.
pub const R_PARISC_DIR14WR: u32 = 83;
/// 14 bits of eff. address.
pub const R_PARISC_DIR14DR: u32 = 84;
/// 16 bits of eff. address.
pub const R_PARISC_DIR16F: u32 = 85;
/// 16 bits of eff. address.
pub const R_PARISC_DIR16WF: u32 = 86;
/// 16 bits of eff. address.
pub const R_PARISC_DIR16DF: u32 = 87;
/// 64 bits of GP-rel. address.
pub const R_PARISC_GPREL64: u32 = 88;
/// GP-rel. address, right 14 bits.
pub const R_PARISC_GPREL14WR: u32 = 91;
/// GP-rel. address, right 14 bits.
pub const R_PARISC_GPREL14DR: u32 = 92;
/// 16 bits GP-rel. address.
pub const R_PARISC_GPREL16F: u32 = 93;
/// 16 bits GP-rel. address.
pub const R_PARISC_GPREL16WF: u32 = 94;
/// 16 bits GP-rel. address.
pub const R_PARISC_GPREL16DF: u32 = 95;
/// 64 bits LT-rel. address.
pub const R_PARISC_LTOFF64: u32 = 96;
/// LT-rel. address, right 14 bits.
pub const R_PARISC_LTOFF14WR: u32 = 99;
/// LT-rel. address, right 14 bits.
pub const R_PARISC_LTOFF14DR: u32 = 100;
/// 16 bits LT-rel. address.
pub const R_PARISC_LTOFF16F: u32 = 101;
/// 16 bits LT-rel. address.
pub const R_PARISC_LTOFF16WF: u32 = 102;
/// 16 bits LT-rel. address.
pub const R_PARISC_LTOFF16DF: u32 = 103;
/// 64 bits section rel. address.
pub const R_PARISC_SECREL64: u32 = 104;
/// 64 bits segment rel. address.
pub const R_PARISC_SEGREL64: u32 = 112;
/// PLT-rel. address, right 14 bits.
pub const R_PARISC_PLTOFF14WR: u32 = 115;
/// PLT-rel. address, right 14 bits.
pub const R_PARISC_PLTOFF14DR: u32 = 116;
/// 16 bits LT-rel. address.
pub const R_PARISC_PLTOFF16F: u32 = 117;
/// 16 bits PLT-rel. address.
pub const R_PARISC_PLTOFF16WF: u32 = 118;
/// 16 bits PLT-rel. address.
pub const R_PARISC_PLTOFF16DF: u32 = 119;
/// 64 bits LT-rel. function ptr.
pub const R_PARISC_LTOFF_FPTR64: u32 = 120;
/// LT-rel. fct. ptr., right 14 bits.
pub const R_PARISC_LTOFF_FPTR14WR: u32 = 123;
/// LT-rel. fct. ptr., right 14 bits.
pub const R_PARISC_LTOFF_FPTR14DR: u32 = 124;
/// 16 bits LT-rel. function ptr.
pub const R_PARISC_LTOFF_FPTR16F: u32 = 125;
/// 16 bits LT-rel. function ptr.
pub const R_PARISC_LTOFF_FPTR16WF: u32 = 126;
/// 16 bits LT-rel. function ptr.
pub const R_PARISC_LTOFF_FPTR16DF: u32 = 127;
pub const R_PARISC_LORESERVE: u32 = 128;
/// Copy relocation.
pub const R_PARISC_COPY: u32 = 128;
/// Dynamic reloc, imported PLT
pub const R_PARISC_IPLT: u32 = 129;
/// Dynamic reloc, exported PLT
pub const R_PARISC_EPLT: u32 = 130;
/// 32 bits TP-rel. address.
pub const R_PARISC_TPREL32: u32 = 153;
/// TP-rel. address, left 21 bits.
pub const R_PARISC_TPREL21L: u32 = 154;
/// TP-rel. address, right 14 bits.
pub const R_PARISC_TPREL14R: u32 = 158;
/// LT-TP-rel. address, left 21 bits.
pub const R_PARISC_LTOFF_TP21L: u32 = 162;
/// LT-TP-rel. address, right 14 bits.
pub const R_PARISC_LTOFF_TP14R: u32 = 166;
/// 14 bits LT-TP-rel. address.
pub const R_PARISC_LTOFF_TP14F: u32 = 167;
/// 64 bits TP-rel. address.
pub const R_PARISC_TPREL64: u32 = 216;
/// TP-rel. address, right 14 bits.
pub const R_PARISC_TPREL14WR: u32 = 219;
/// TP-rel. address, right 14 bits.
pub const R_PARISC_TPREL14DR: u32 = 220;
/// 16 bits TP-rel. address.
pub const R_PARISC_TPREL16F: u32 = 221;
/// 16 bits TP-rel. address.
pub const R_PARISC_TPREL16WF: u32 = 222;
/// 16 bits TP-rel. address.
pub const R_PARISC_TPREL16DF: u32 = 223;
/// 64 bits LT-TP-rel. address.
pub const R_PARISC_LTOFF_TP64: u32 = 224;
/// LT-TP-rel. address, right 14 bits.
pub const R_PARISC_LTOFF_TP14WR: u32 = 227;
/// LT-TP-rel. address, right 14 bits.
pub const R_PARISC_LTOFF_TP14DR: u32 = 228;
/// 16 bits LT-TP-rel. address.
pub const R_PARISC_LTOFF_TP16F: u32 = 229;
/// 16 bits LT-TP-rel. address.
pub const R_PARISC_LTOFF_TP16WF: u32 = 230;
/// 16 bits LT-TP-rel. address.
pub const R_PARISC_LTOFF_TP16DF: u32 = 231;
pub const R_PARISC_GNU_VTENTRY: u32 = 232;
pub const R_PARISC_GNU_VTINHERIT: u32 = 233;
/// GD 21-bit left.
pub const R_PARISC_TLS_GD21L: u32 = 234;
/// GD 14-bit right.
pub const R_PARISC_TLS_GD14R: u32 = 235;
/// GD call to __t_g_a.
pub const R_PARISC_TLS_GDCALL: u32 = 236;
/// LD module 21-bit left.
pub const R_PARISC_TLS_LDM21L: u32 = 237;
/// LD module 14-bit right.
pub const R_PARISC_TLS_LDM14R: u32 = 238;
/// LD module call to __t_g_a.
pub const R_PARISC_TLS_LDMCALL: u32 = 239;
/// LD offset 21-bit left.
pub const R_PARISC_TLS_LDO21L: u32 = 240;
/// LD offset 14-bit right.
pub const R_PARISC_TLS_LDO14R: u32 = 241;
/// DTP module 32-bit.
pub const R_PARISC_TLS_DTPMOD32: u32 = 242;
/// DTP module 64-bit.
pub const R_PARISC_TLS_DTPMOD64: u32 = 243;
/// DTP offset 32-bit.
pub const R_PARISC_TLS_DTPOFF32: u32 = 244;
/// DTP offset 32-bit.
pub const R_PARISC_TLS_DTPOFF64: u32 = 245;
pub const R_PARISC_TLS_LE21L: u32 = R_PARISC_TPREL21L;
pub const R_PARISC_TLS_LE14R: u32 = R_PARISC_TPREL14R;
pub const R_PARISC_TLS_IE21L: u32 = R_PARISC_LTOFF_TP21L;
pub const R_PARISC_TLS_IE14R: u32 = R_PARISC_LTOFF_TP14R;
pub const R_PARISC_TLS_TPREL32: u32 = R_PARISC_TPREL32;
pub const R_PARISC_TLS_TPREL64: u32 = R_PARISC_TPREL64;
pub const R_PARISC_HIRESERVE: u32 = 255;

// PA-RISC values for `ProgramHeader*::p_type`.

pub const PT_HP_TLS: u32 = PT_LOOS + 0x0;
pub const PT_HP_CORE_NONE: u32 = PT_LOOS + 0x1;
pub const PT_HP_CORE_VERSION: u32 = PT_LOOS + 0x2;
pub const PT_HP_CORE_KERNEL: u32 = PT_LOOS + 0x3;
pub const PT_HP_CORE_COMM: u32 = PT_LOOS + 0x4;
pub const PT_HP_CORE_PROC: u32 = PT_LOOS + 0x5;
pub const PT_HP_CORE_LOADABLE: u32 = PT_LOOS + 0x6;
pub const PT_HP_CORE_STACK: u32 = PT_LOOS + 0x7;
pub const PT_HP_CORE_SHM: u32 = PT_LOOS + 0x8;
pub const PT_HP_CORE_MMF: u32 = PT_LOOS + 0x9;
pub const PT_HP_PARALLEL: u32 = PT_LOOS + 0x10;
pub const PT_HP_FASTBIND: u32 = PT_LOOS + 0x11;
pub const PT_HP_OPT_ANNOT: u32 = PT_LOOS + 0x12;
pub const PT_HP_HSL_ANNOT: u32 = PT_LOOS + 0x13;
pub const PT_HP_STACK: u32 = PT_LOOS + 0x14;

pub const PT_PARISC_ARCHEXT: u32 = 0x7000_0000;
pub const PT_PARISC_UNWIND: u32 = 0x7000_0001;

// PA-RISC values for `ProgramHeader*::p_flags`.

pub const PF_PARISC_SBP: u32 = 0x0800_0000;

pub const PF_HP_PAGE_SIZE: u32 = 0x0010_0000;
pub const PF_HP_FAR_SHARED: u32 = 0x0020_0000;
pub const PF_HP_NEAR_SHARED: u32 = 0x0040_0000;
pub const PF_HP_CODE: u32 = 0x0100_0000;
pub const PF_HP_MODIFY: u32 = 0x0200_0000;
pub const PF_HP_LAZYSWAP: u32 = 0x0400_0000;
pub const PF_HP_SBP: u32 = 0x0800_0000;

// Alpha specific definitions.

// Alpha values for `FileHeader64::e_flags`.

/// All addresses must be < 2GB.
pub const EF_ALPHA_32BIT: u32 = 1;
/// Relocations for relaxing exist.
pub const EF_ALPHA_CANRELAX: u32 = 2;

// Alpha values for `SectionHeader64::sh_type`.

// These two are primerily concerned with ECOFF debugging info.
pub const SHT_ALPHA_DEBUG: u32 = 0x7000_0001;
pub const SHT_ALPHA_REGINFO: u32 = 0x7000_0002;

// Alpha values for `SectionHeader64::sh_flags`.

pub const SHF_ALPHA_GPREL: u32 = 0x1000_0000;

// Alpha values for `Sym64::st_other`.
/// No PV required.
pub const STO_ALPHA_NOPV: u8 = 0x80;
/// PV only used for initial ldgp.
pub const STO_ALPHA_STD_GPLOAD: u8 = 0x88;

// Alpha values for `Rel64::r_type`.

/// No reloc
pub const R_ALPHA_NONE: u32 = 0;
/// Direct 32 bit
pub const R_ALPHA_REFLONG: u32 = 1;
/// Direct 64 bit
pub const R_ALPHA_REFQUAD: u32 = 2;
/// GP relative 32 bit
pub const R_ALPHA_GPREL32: u32 = 3;
/// GP relative 16 bit w/optimization
pub const R_ALPHA_LITERAL: u32 = 4;
/// Optimization hint for LITERAL
pub const R_ALPHA_LITUSE: u32 = 5;
/// Add displacement to GP
pub const R_ALPHA_GPDISP: u32 = 6;
/// PC+4 relative 23 bit shifted
pub const R_ALPHA_BRADDR: u32 = 7;
/// PC+4 relative 16 bit shifted
pub const R_ALPHA_HINT: u32 = 8;
/// PC relative 16 bit
pub const R_ALPHA_SREL16: u32 = 9;
/// PC relative 32 bit
pub const R_ALPHA_SREL32: u32 = 10;
/// PC relative 64 bit
pub const R_ALPHA_SREL64: u32 = 11;
/// GP relative 32 bit, high 16 bits
pub const R_ALPHA_GPRELHIGH: u32 = 17;
/// GP relative 32 bit, low 16 bits
pub const R_ALPHA_GPRELLOW: u32 = 18;
/// GP relative 16 bit
pub const R_ALPHA_GPREL16: u32 = 19;
/// Copy symbol at runtime
pub const R_ALPHA_COPY: u32 = 24;
/// Create GOT entry
pub const R_ALPHA_GLOB_DAT: u32 = 25;
/// Create PLT entry
pub const R_ALPHA_JMP_SLOT: u32 = 26;
/// Adjust by program base
pub const R_ALPHA_RELATIVE: u32 = 27;
pub const R_ALPHA_TLS_GD_HI: u32 = 28;
pub const R_ALPHA_TLSGD: u32 = 29;
pub const R_ALPHA_TLS_LDM: u32 = 30;
pub const R_ALPHA_DTPMOD64: u32 = 31;
pub const R_ALPHA_GOTDTPREL: u32 = 32;
pub const R_ALPHA_DTPREL64: u32 = 33;
pub const R_ALPHA_DTPRELHI: u32 = 34;
pub const R_ALPHA_DTPRELLO: u32 = 35;
pub const R_ALPHA_DTPREL16: u32 = 36;
pub const R_ALPHA_GOTTPREL: u32 = 37;
pub const R_ALPHA_TPREL64: u32 = 38;
pub const R_ALPHA_TPRELHI: u32 = 39;
pub const R_ALPHA_TPRELLO: u32 = 40;
pub const R_ALPHA_TPREL16: u32 = 41;

// Magic values of the `R_ALPHA_LITUSE` relocation addend.
pub const LITUSE_ALPHA_ADDR: u32 = 0;
pub const LITUSE_ALPHA_BASE: u32 = 1;
pub const LITUSE_ALPHA_BYTOFF: u32 = 2;
pub const LITUSE_ALPHA_JSR: u32 = 3;
pub const LITUSE_ALPHA_TLS_GD: u32 = 4;
pub const LITUSE_ALPHA_TLS_LDM: u32 = 5;

// Alpha values for `Dyn64::d_tag`.
pub const DT_ALPHA_PLTRO: u32 = DT_LOPROC + 0;

// PowerPC specific declarations.

// PowerPC values for `FileHeader*::e_flags`.
/// PowerPC embedded flag
pub const EF_PPC_EMB: u32 = 0x8000_0000;

// Cygnus local bits below .
/// PowerPC -mrelocatable flag
pub const EF_PPC_RELOCATABLE: u32 = 0x0001_0000;
/// PowerPC -mrelocatable-lib flag
pub const EF_PPC_RELOCATABLE_LIB: u32 = 0x0000_8000;

// PowerPC values for `Rel*::r_type` defined by the ABIs.
pub const R_PPC_NONE: u32 = 0;
/// 32bit absolute address
pub const R_PPC_ADDR32: u32 = 1;
/// 26bit address, 2 bits ignored.
pub const R_PPC_ADDR24: u32 = 2;
/// 16bit absolute address
pub const R_PPC_ADDR16: u32 = 3;
/// lower 16bit of absolute address
pub const R_PPC_ADDR16_LO: u32 = 4;
/// high 16bit of absolute address
pub const R_PPC_ADDR16_HI: u32 = 5;
/// adjusted high 16bit
pub const R_PPC_ADDR16_HA: u32 = 6;
/// 16bit address, 2 bits ignored
pub const R_PPC_ADDR14: u32 = 7;
pub const R_PPC_ADDR14_BRTAKEN: u32 = 8;
pub const R_PPC_ADDR14_BRNTAKEN: u32 = 9;
/// PC relative 26 bit
pub const R_PPC_REL24: u32 = 10;
/// PC relative 16 bit
pub const R_PPC_REL14: u32 = 11;
pub const R_PPC_REL14_BRTAKEN: u32 = 12;
pub const R_PPC_REL14_BRNTAKEN: u32 = 13;
pub const R_PPC_GOT16: u32 = 14;
pub const R_PPC_GOT16_LO: u32 = 15;
pub const R_PPC_GOT16_HI: u32 = 16;
pub const R_PPC_GOT16_HA: u32 = 17;
pub const R_PPC_PLTREL24: u32 = 18;
pub const R_PPC_COPY: u32 = 19;
pub const R_PPC_GLOB_DAT: u32 = 20;
pub const R_PPC_JMP_SLOT: u32 = 21;
pub const R_PPC_RELATIVE: u32 = 22;
pub const R_PPC_LOCAL24PC: u32 = 23;
pub const R_PPC_UADDR32: u32 = 24;
pub const R_PPC_UADDR16: u32 = 25;
pub const R_PPC_REL32: u32 = 26;
pub const R_PPC_PLT32: u32 = 27;
pub const R_PPC_PLTREL32: u32 = 28;
pub const R_PPC_PLT16_LO: u32 = 29;
pub const R_PPC_PLT16_HI: u32 = 30;
pub const R_PPC_PLT16_HA: u32 = 31;
pub const R_PPC_SDAREL16: u32 = 32;
pub const R_PPC_SECTOFF: u32 = 33;
pub const R_PPC_SECTOFF_LO: u32 = 34;
pub const R_PPC_SECTOFF_HI: u32 = 35;
pub const R_PPC_SECTOFF_HA: u32 = 36;

// PowerPC values for `Rel*::r_type` defined for the TLS access ABI.
/// none    (sym+add)@tls
pub const R_PPC_TLS: u32 = 67;
/// word32  (sym+add)@dtpmod
pub const R_PPC_DTPMOD32: u32 = 68;
/// half16* (sym+add)@tprel
pub const R_PPC_TPREL16: u32 = 69;
/// half16  (sym+add)@tprel@l
pub const R_PPC_TPREL16_LO: u32 = 70;
/// half16  (sym+add)@tprel@h
pub const R_PPC_TPREL16_HI: u32 = 71;
/// half16  (sym+add)@tprel@ha
pub const R_PPC_TPREL16_HA: u32 = 72;
/// word32  (sym+add)@tprel
pub const R_PPC_TPREL32: u32 = 73;
/// half16*(sym+add)@dtprel
pub const R_PPC_DTPREL16: u32 = 74;
/// half16  (sym+add)@dtprel@l
pub const R_PPC_DTPREL16_LO: u32 = 75;
/// half16  (sym+add)@dtprel@h
pub const R_PPC_DTPREL16_HI: u32 = 76;
/// half16  (sym+add)@dtprel@ha
pub const R_PPC_DTPREL16_HA: u32 = 77;
/// word32  (sym+add)@dtprel
pub const R_PPC_DTPREL32: u32 = 78;
/// half16* (sym+add)@got@tlsgd
pub const R_PPC_GOT_TLSGD16: u32 = 79;
/// half16  (sym+add)@got@tlsgd@l
pub const R_PPC_GOT_TLSGD16_LO: u32 = 80;
/// half16  (sym+add)@got@tlsgd@h
pub const R_PPC_GOT_TLSGD16_HI: u32 = 81;
/// half16  (sym+add)@got@tlsgd@ha
pub const R_PPC_GOT_TLSGD16_HA: u32 = 82;
/// half16* (sym+add)@got@tlsld
pub const R_PPC_GOT_TLSLD16: u32 = 83;
/// half16  (sym+add)@got@tlsld@l
pub const R_PPC_GOT_TLSLD16_LO: u32 = 84;
/// half16  (sym+add)@got@tlsld@h
pub const R_PPC_GOT_TLSLD16_HI: u32 = 85;
/// half16  (sym+add)@got@tlsld@ha
pub const R_PPC_GOT_TLSLD16_HA: u32 = 86;
/// half16* (sym+add)@got@tprel
pub const R_PPC_GOT_TPREL16: u32 = 87;
/// half16  (sym+add)@got@tprel@l
pub const R_PPC_GOT_TPREL16_LO: u32 = 88;
/// half16  (sym+add)@got@tprel@h
pub const R_PPC_GOT_TPREL16_HI: u32 = 89;
/// half16  (sym+add)@got@tprel@ha
pub const R_PPC_GOT_TPREL16_HA: u32 = 90;
/// half16* (sym+add)@got@dtprel
pub const R_PPC_GOT_DTPREL16: u32 = 91;
/// half16* (sym+add)@got@dtprel@l
pub const R_PPC_GOT_DTPREL16_LO: u32 = 92;
/// half16* (sym+add)@got@dtprel@h
pub const R_PPC_GOT_DTPREL16_HI: u32 = 93;
/// half16* (sym+add)@got@dtprel@ha
pub const R_PPC_GOT_DTPREL16_HA: u32 = 94;
/// none    (sym+add)@tlsgd
pub const R_PPC_TLSGD: u32 = 95;
/// none    (sym+add)@tlsld
pub const R_PPC_TLSLD: u32 = 96;

// PowerPC values for `Rel*::r_type` from the Embedded ELF ABI.
pub const R_PPC_EMB_NADDR32: u32 = 101;
pub const R_PPC_EMB_NADDR16: u32 = 102;
pub const R_PPC_EMB_NADDR16_LO: u32 = 103;
pub const R_PPC_EMB_NADDR16_HI: u32 = 104;
pub const R_PPC_EMB_NADDR16_HA: u32 = 105;
pub const R_PPC_EMB_SDAI16: u32 = 106;
pub const R_PPC_EMB_SDA2I16: u32 = 107;
pub const R_PPC_EMB_SDA2REL: u32 = 108;
/// 16 bit offset in SDA
pub const R_PPC_EMB_SDA21: u32 = 109;
pub const R_PPC_EMB_MRKREF: u32 = 110;
pub const R_PPC_EMB_RELSEC16: u32 = 111;
pub const R_PPC_EMB_RELST_LO: u32 = 112;
pub const R_PPC_EMB_RELST_HI: u32 = 113;
pub const R_PPC_EMB_RELST_HA: u32 = 114;
pub const R_PPC_EMB_BIT_FLD: u32 = 115;
/// 16 bit relative offset in SDA
pub const R_PPC_EMB_RELSDA: u32 = 116;

// Diab tool values for `Rel*::r_type`.
/// like EMB_SDA21, but lower 16 bit
pub const R_PPC_DIAB_SDA21_LO: u32 = 180;
/// like EMB_SDA21, but high 16 bit
pub const R_PPC_DIAB_SDA21_HI: u32 = 181;
/// like EMB_SDA21, adjusted high 16
pub const R_PPC_DIAB_SDA21_HA: u32 = 182;
/// like EMB_RELSDA, but lower 16 bit
pub const R_PPC_DIAB_RELSDA_LO: u32 = 183;
/// like EMB_RELSDA, but high 16 bit
pub const R_PPC_DIAB_RELSDA_HI: u32 = 184;
/// like EMB_RELSDA, adjusted high 16
pub const R_PPC_DIAB_RELSDA_HA: u32 = 185;

/// GNU extension to support local ifunc.
pub const R_PPC_IRELATIVE: u32 = 248;

// GNU relocs used in PIC code sequences.
/// half16   (sym+add-.)
pub const R_PPC_REL16: u32 = 249;
/// half16   (sym+add-.)@l
pub const R_PPC_REL16_LO: u32 = 250;
/// half16   (sym+add-.)@h
pub const R_PPC_REL16_HI: u32 = 251;
/// half16   (sym+add-.)@ha
pub const R_PPC_REL16_HA: u32 = 252;

/// This is a phony reloc to handle any old fashioned TOC16 references that may
/// still be in object files.
pub const R_PPC_TOC16: u32 = 255;

// PowerPC specific values for `Dyn*::d_tag`.
pub const DT_PPC_GOT: u32 = DT_LOPROC + 0;
pub const DT_PPC_OPT: u32 = DT_LOPROC + 1;

// PowerPC specific values for the `DT_PPC_OPT` entry.
pub const PPC_OPT_TLS: u32 = 1;

// PowerPC64 values for `Rel*::r_type` defined by the ABIs.
pub const R_PPC64_NONE: u32 = R_PPC_NONE;
/// 32bit absolute address
pub const R_PPC64_ADDR32: u32 = R_PPC_ADDR32;
/// 26bit address, word aligned
pub const R_PPC64_ADDR24: u32 = R_PPC_ADDR24;
/// 16bit absolute address
pub const R_PPC64_ADDR16: u32 = R_PPC_ADDR16;
/// lower 16bits of address
pub const R_PPC64_ADDR16_LO: u32 = R_PPC_ADDR16_LO;
/// high 16bits of address.
pub const R_PPC64_ADDR16_HI: u32 = R_PPC_ADDR16_HI;
/// adjusted high 16bits.
pub const R_PPC64_ADDR16_HA: u32 = R_PPC_ADDR16_HA;
/// 16bit address, word aligned
pub const R_PPC64_ADDR14: u32 = R_PPC_ADDR14;
pub const R_PPC64_ADDR14_BRTAKEN: u32 = R_PPC_ADDR14_BRTAKEN;
pub const R_PPC64_ADDR14_BRNTAKEN: u32 = R_PPC_ADDR14_BRNTAKEN;
/// PC-rel. 26 bit, word aligned
pub const R_PPC64_REL24: u32 = R_PPC_REL24;
/// PC relative 16 bit
pub const R_PPC64_REL14: u32 = R_PPC_REL14;
pub const R_PPC64_REL14_BRTAKEN: u32 = R_PPC_REL14_BRTAKEN;
pub const R_PPC64_REL14_BRNTAKEN: u32 = R_PPC_REL14_BRNTAKEN;
pub const R_PPC64_GOT16: u32 = R_PPC_GOT16;
pub const R_PPC64_GOT16_LO: u32 = R_PPC_GOT16_LO;
pub const R_PPC64_GOT16_HI: u32 = R_PPC_GOT16_HI;
pub const R_PPC64_GOT16_HA: u32 = R_PPC_GOT16_HA;

pub const R_PPC64_COPY: u32 = R_PPC_COPY;
pub const R_PPC64_GLOB_DAT: u32 = R_PPC_GLOB_DAT;
pub const R_PPC64_JMP_SLOT: u32 = R_PPC_JMP_SLOT;
pub const R_PPC64_RELATIVE: u32 = R_PPC_RELATIVE;

pub const R_PPC64_UADDR32: u32 = R_PPC_UADDR32;
pub const R_PPC64_UADDR16: u32 = R_PPC_UADDR16;
pub const R_PPC64_REL32: u32 = R_PPC_REL32;
pub const R_PPC64_PLT32: u32 = R_PPC_PLT32;
pub const R_PPC64_PLTREL32: u32 = R_PPC_PLTREL32;
pub const R_PPC64_PLT16_LO: u32 = R_PPC_PLT16_LO;
pub const R_PPC64_PLT16_HI: u32 = R_PPC_PLT16_HI;
pub const R_PPC64_PLT16_HA: u32 = R_PPC_PLT16_HA;

pub const R_PPC64_SECTOFF: u32 = R_PPC_SECTOFF;
pub const R_PPC64_SECTOFF_LO: u32 = R_PPC_SECTOFF_LO;
pub const R_PPC64_SECTOFF_HI: u32 = R_PPC_SECTOFF_HI;
pub const R_PPC64_SECTOFF_HA: u32 = R_PPC_SECTOFF_HA;
/// word30 (S + A - P) >> 2
pub const R_PPC64_ADDR30: u32 = 37;
/// doubleword64 S + A
pub const R_PPC64_ADDR64: u32 = 38;
/// half16 #higher(S + A)
pub const R_PPC64_ADDR16_HIGHER: u32 = 39;
/// half16 #highera(S + A)
pub const R_PPC64_ADDR16_HIGHERA: u32 = 40;
/// half16 #highest(S + A)
pub const R_PPC64_ADDR16_HIGHEST: u32 = 41;
/// half16 #highesta(S + A)
pub const R_PPC64_ADDR16_HIGHESTA: u32 = 42;
/// doubleword64 S + A
pub const R_PPC64_UADDR64: u32 = 43;
/// doubleword64 S + A - P
pub const R_PPC64_REL64: u32 = 44;
/// doubleword64 L + A
pub const R_PPC64_PLT64: u32 = 45;
/// doubleword64 L + A - P
pub const R_PPC64_PLTREL64: u32 = 46;
/// half16* S + A - .TOC
pub const R_PPC64_TOC16: u32 = 47;
/// half16 #lo(S + A - .TOC.)
pub const R_PPC64_TOC16_LO: u32 = 48;
/// half16 #hi(S + A - .TOC.)
pub const R_PPC64_TOC16_HI: u32 = 49;
/// half16 #ha(S + A - .TOC.)
pub const R_PPC64_TOC16_HA: u32 = 50;
/// doubleword64 .TOC
pub const R_PPC64_TOC: u32 = 51;
/// half16* M + A
pub const R_PPC64_PLTGOT16: u32 = 52;
/// half16 #lo(M + A)
pub const R_PPC64_PLTGOT16_LO: u32 = 53;
/// half16 #hi(M + A)
pub const R_PPC64_PLTGOT16_HI: u32 = 54;
/// half16 #ha(M + A)
pub const R_PPC64_PLTGOT16_HA: u32 = 55;

/// half16ds* (S + A) >> 2
pub const R_PPC64_ADDR16_DS: u32 = 56;
/// half16ds  #lo(S + A) >> 2
pub const R_PPC64_ADDR16_LO_DS: u32 = 57;
/// half16ds* (G + A) >> 2
pub const R_PPC64_GOT16_DS: u32 = 58;
/// half16ds  #lo(G + A) >> 2
pub const R_PPC64_GOT16_LO_DS: u32 = 59;
/// half16ds  #lo(L + A) >> 2
pub const R_PPC64_PLT16_LO_DS: u32 = 60;
/// half16ds* (R + A) >> 2
pub const R_PPC64_SECTOFF_DS: u32 = 61;
/// half16ds  #lo(R + A) >> 2
pub const R_PPC64_SECTOFF_LO_DS: u32 = 62;
/// half16ds* (S + A - .TOC.) >> 2
pub const R_PPC64_TOC16_DS: u32 = 63;
/// half16ds  #lo(S + A - .TOC.) >> 2
pub const R_PPC64_TOC16_LO_DS: u32 = 64;
/// half16ds* (M + A) >> 2
pub const R_PPC64_PLTGOT16_DS: u32 = 65;
/// half16ds  #lo(M + A) >> 2
pub const R_PPC64_PLTGOT16_LO_DS: u32 = 66;

// PowerPC64 values for `Rel*::r_type` defined for the TLS access ABI.
/// none    (sym+add)@tls
pub const R_PPC64_TLS: u32 = 67;
/// doubleword64 (sym+add)@dtpmod
pub const R_PPC64_DTPMOD64: u32 = 68;
/// half16* (sym+add)@tprel
pub const R_PPC64_TPREL16: u32 = 69;
/// half16  (sym+add)@tprel@l
pub const R_PPC64_TPREL16_LO: u32 = 70;
/// half16  (sym+add)@tprel@h
pub const R_PPC64_TPREL16_HI: u32 = 71;
/// half16  (sym+add)@tprel@ha
pub const R_PPC64_TPREL16_HA: u32 = 72;
/// doubleword64 (sym+add)@tprel
pub const R_PPC64_TPREL64: u32 = 73;
/// half16* (sym+add)@dtprel
pub const R_PPC64_DTPREL16: u32 = 74;
/// half16  (sym+add)@dtprel@l
pub const R_PPC64_DTPREL16_LO: u32 = 75;
/// half16  (sym+add)@dtprel@h
pub const R_PPC64_DTPREL16_HI: u32 = 76;
/// half16  (sym+add)@dtprel@ha
pub const R_PPC64_DTPREL16_HA: u32 = 77;
/// doubleword64 (sym+add)@dtprel
pub const R_PPC64_DTPREL64: u32 = 78;
/// half16* (sym+add)@got@tlsgd
pub const R_PPC64_GOT_TLSGD16: u32 = 79;
/// half16  (sym+add)@got@tlsgd@l
pub const R_PPC64_GOT_TLSGD16_LO: u32 = 80;
/// half16  (sym+add)@got@tlsgd@h
pub const R_PPC64_GOT_TLSGD16_HI: u32 = 81;
/// half16  (sym+add)@got@tlsgd@ha
pub const R_PPC64_GOT_TLSGD16_HA: u32 = 82;
/// half16* (sym+add)@got@tlsld
pub const R_PPC64_GOT_TLSLD16: u32 = 83;
/// half16  (sym+add)@got@tlsld@l
pub const R_PPC64_GOT_TLSLD16_LO: u32 = 84;
/// half16  (sym+add)@got@tlsld@h
pub const R_PPC64_GOT_TLSLD16_HI: u32 = 85;
/// half16  (sym+add)@got@tlsld@ha
pub const R_PPC64_GOT_TLSLD16_HA: u32 = 86;
/// half16ds* (sym+add)@got@tprel
pub const R_PPC64_GOT_TPREL16_DS: u32 = 87;
/// half16ds (sym+add)@got@tprel@l
pub const R_PPC64_GOT_TPREL16_LO_DS: u32 = 88;
/// half16  (sym+add)@got@tprel@h
pub const R_PPC64_GOT_TPREL16_HI: u32 = 89;
/// half16  (sym+add)@got@tprel@ha
pub const R_PPC64_GOT_TPREL16_HA: u32 = 90;
/// half16ds* (sym+add)@got@dtprel
pub const R_PPC64_GOT_DTPREL16_DS: u32 = 91;
/// half16ds (sym+add)@got@dtprel@l
pub const R_PPC64_GOT_DTPREL16_LO_DS: u32 = 92;
/// half16  (sym+add)@got@dtprel@h
pub const R_PPC64_GOT_DTPREL16_HI: u32 = 93;
/// half16  (sym+add)@got@dtprel@ha
pub const R_PPC64_GOT_DTPREL16_HA: u32 = 94;
/// half16ds* (sym+add)@tprel
pub const R_PPC64_TPREL16_DS: u32 = 95;
/// half16ds (sym+add)@tprel@l
pub const R_PPC64_TPREL16_LO_DS: u32 = 96;
/// half16  (sym+add)@tprel@higher
pub const R_PPC64_TPREL16_HIGHER: u32 = 97;
/// half16  (sym+add)@tprel@highera
pub const R_PPC64_TPREL16_HIGHERA: u32 = 98;
/// half16  (sym+add)@tprel@highest
pub const R_PPC64_TPREL16_HIGHEST: u32 = 99;
/// half16  (sym+add)@tprel@highesta
pub const R_PPC64_TPREL16_HIGHESTA: u32 = 100;
/// half16ds* (sym+add)@dtprel
pub const R_PPC64_DTPREL16_DS: u32 = 101;
/// half16ds (sym+add)@dtprel@l
pub const R_PPC64_DTPREL16_LO_DS: u32 = 102;
/// half16  (sym+add)@dtprel@higher
pub const R_PPC64_DTPREL16_HIGHER: u32 = 103;
/// half16  (sym+add)@dtprel@highera
pub const R_PPC64_DTPREL16_HIGHERA: u32 = 104;
/// half16  (sym+add)@dtprel@highest
pub const R_PPC64_DTPREL16_HIGHEST: u32 = 105;
/// half16  (sym+add)@dtprel@highesta
pub const R_PPC64_DTPREL16_HIGHESTA: u32 = 106;
/// none    (sym+add)@tlsgd
pub const R_PPC64_TLSGD: u32 = 107;
/// none    (sym+add)@tlsld
pub const R_PPC64_TLSLD: u32 = 108;
/// none
pub const R_PPC64_TOCSAVE: u32 = 109;

// Added when HA and HI relocs were changed to report overflows.
pub const R_PPC64_ADDR16_HIGH: u32 = 110;
pub const R_PPC64_ADDR16_HIGHA: u32 = 111;
pub const R_PPC64_TPREL16_HIGH: u32 = 112;
pub const R_PPC64_TPREL16_HIGHA: u32 = 113;
pub const R_PPC64_DTPREL16_HIGH: u32 = 114;
pub const R_PPC64_DTPREL16_HIGHA: u32 = 115;

/// GNU extension to support local ifunc.
pub const R_PPC64_JMP_IREL: u32 = 247;
/// GNU extension to support local ifunc.
pub const R_PPC64_IRELATIVE: u32 = 248;
/// half16   (sym+add-.)
pub const R_PPC64_REL16: u32 = 249;
/// half16   (sym+add-.)@l
pub const R_PPC64_REL16_LO: u32 = 250;
/// half16   (sym+add-.)@h
pub const R_PPC64_REL16_HI: u32 = 251;
/// half16   (sym+add-.)@ha
pub const R_PPC64_REL16_HA: u32 = 252;

// PowerPC64 values for `FileHeader64::e_flags.
/// PowerPC64 bits specifying ABI.
///
/// 1 for original function descriptor using ABI,
/// 2 for revised ABI without function descriptors,
/// 0 for unspecified or not using any features affected by the differences.
pub const EF_PPC64_ABI: u32 = 3;

// PowerPC64 values for `Dyn64::d_tag.
pub const DT_PPC64_GLINK: u32 = DT_LOPROC + 0;
pub const DT_PPC64_OPD: u32 = DT_LOPROC + 1;
pub const DT_PPC64_OPDSZ: u32 = DT_LOPROC + 2;
pub const DT_PPC64_OPT: u32 = DT_LOPROC + 3;

// PowerPC64 bits for `DT_PPC64_OPT` entry.
pub const PPC64_OPT_TLS: u32 = 1;
pub const PPC64_OPT_MULTI_TOC: u32 = 2;
pub const PPC64_OPT_LOCALENTRY: u32 = 4;

// PowerPC64 values for `Sym64::st_other.
pub const STO_PPC64_LOCAL_BIT: u8 = 5;
pub const STO_PPC64_LOCAL_MASK: u8 = 7 << STO_PPC64_LOCAL_BIT;

// ARM specific declarations.

// ARM values for `FileHeader*::e_flags`.
pub const EF_ARM_RELEXEC: u32 = 0x01;
pub const EF_ARM_HASENTRY: u32 = 0x02;
pub const EF_ARM_INTERWORK: u32 = 0x04;
pub const EF_ARM_APCS_26: u32 = 0x08;
pub const EF_ARM_APCS_FLOAT: u32 = 0x10;
pub const EF_ARM_PIC: u32 = 0x20;
/// 8-bit structure alignment is in use
pub const EF_ARM_ALIGN8: u32 = 0x40;
pub const EF_ARM_NEW_ABI: u32 = 0x80;
pub const EF_ARM_OLD_ABI: u32 = 0x100;
pub const EF_ARM_SOFT_FLOAT: u32 = 0x200;
pub const EF_ARM_VFP_FLOAT: u32 = 0x400;
pub const EF_ARM_MAVERICK_FLOAT: u32 = 0x800;

/// NB conflicts with EF_ARM_SOFT_FLOAT
pub const EF_ARM_ABI_FLOAT_SOFT: u32 = 0x200;
/// NB conflicts with EF_ARM_VFP_FLOAT
pub const EF_ARM_ABI_FLOAT_HARD: u32 = 0x400;

// Other constants defined in the ARM ELF spec. version B-01.
// NB. These conflict with values defined above.
pub const EF_ARM_SYMSARESORTED: u32 = 0x04;
pub const EF_ARM_DYNSYMSUSESEGIDX: u32 = 0x08;
pub const EF_ARM_MAPSYMSFIRST: u32 = 0x10;

// Constants defined in AAELF.
pub const EF_ARM_BE8: u32 = 0x0080_0000;
pub const EF_ARM_LE8: u32 = 0x0040_0000;

pub const EF_ARM_EABIMASK: u32 = 0xff00_0000;
pub const EF_ARM_EABI_UNKNOWN: u32 = 0x0000_0000;
pub const EF_ARM_EABI_VER1: u32 = 0x0100_0000;
pub const EF_ARM_EABI_VER2: u32 = 0x0200_0000;
pub const EF_ARM_EABI_VER3: u32 = 0x0300_0000;
pub const EF_ARM_EABI_VER4: u32 = 0x0400_0000;
pub const EF_ARM_EABI_VER5: u32 = 0x0500_0000;

// ARM Thumb values for `st_type` component of `Sym*::st_info`.
/// A Thumb function.
pub const STT_ARM_TFUNC: u8 = STT_LOPROC;
/// A Thumb label.
pub const STT_ARM_16BIT: u8 = STT_HIPROC;

// ARM values for `SectionHeader*::sh_flags`.
/// Section contains an entry point
pub const SHF_ARM_ENTRYSECT: u32 = 0x1000_0000;
/// Section may be multiply defined in the input to a link step.
pub const SHF_ARM_COMDEF: u32 = 0x8000_0000;

// ARM values for `ProgramHeader*::p_flags`.
/// Segment contains the location addressed by the static base.
pub const PF_ARM_SB: u32 = 0x1000_0000;
/// Position-independent segment.
pub const PF_ARM_PI: u32 = 0x2000_0000;
/// Absolute segment.
pub const PF_ARM_ABS: u32 = 0x4000_0000;

// ARM values for `ProgramHeader*::p_type`.
/// ARM unwind segment.
pub const PT_ARM_EXIDX: u32 = PT_LOPROC + 1;

// ARM values for `SectionHeader*::sh_type`.
/// ARM unwind section.
pub const SHT_ARM_EXIDX: u32 = SHT_LOPROC + 1;
/// Preemption details.
pub const SHT_ARM_PREEMPTMAP: u32 = SHT_LOPROC + 2;
/// ARM attributes section.
pub const SHT_ARM_ATTRIBUTES: u32 = SHT_LOPROC + 3;

// AArch64 values for `SectionHeader*::sh_type`.
/// AArch64 attributes section.
pub const SHT_AARCH64_ATTRIBUTES: u32 = SHT_LOPROC + 3;

// AArch64 values for `Rel*::r_type`.

/// No relocation.
pub const R_AARCH64_NONE: u32 = 0;

// ILP32 AArch64 relocs.
/// Direct 32 bit.
pub const R_AARCH64_P32_ABS32: u32 = 1;
/// Copy symbol at runtime.
pub const R_AARCH64_P32_COPY: u32 = 180;
/// Create GOT entry.
pub const R_AARCH64_P32_GLOB_DAT: u32 = 181;
/// Create PLT entry.
pub const R_AARCH64_P32_JUMP_SLOT: u32 = 182;
/// Adjust by program base.
pub const R_AARCH64_P32_RELATIVE: u32 = 183;
/// Module number, 32 bit.
pub const R_AARCH64_P32_TLS_DTPMOD: u32 = 184;
/// Module-relative offset, 32 bit.
pub const R_AARCH64_P32_TLS_DTPREL: u32 = 185;
/// TP-relative offset, 32 bit.
pub const R_AARCH64_P32_TLS_TPREL: u32 = 186;
/// TLS Descriptor.
pub const R_AARCH64_P32_TLSDESC: u32 = 187;
/// STT_GNU_IFUNC relocation.
pub const R_AARCH64_P32_IRELATIVE: u32 = 188;

// LP64 AArch64 relocs.
/// Direct 64 bit.
pub const R_AARCH64_ABS64: u32 = 257;
/// Direct 32 bit.
pub const R_AARCH64_ABS32: u32 = 258;
/// Direct 16-bit.
pub const R_AARCH64_ABS16: u32 = 259;
/// PC-relative 64-bit.
pub const R_AARCH64_PREL64: u32 = 260;
/// PC-relative 32-bit.
pub const R_AARCH64_PREL32: u32 = 261;
/// PC-relative 16-bit.
pub const R_AARCH64_PREL16: u32 = 262;
/// Dir. MOVZ imm. from bits 15:0.
pub const R_AARCH64_MOVW_UABS_G0: u32 = 263;
/// Likewise for MOVK; no check.
pub const R_AARCH64_MOVW_UABS_G0_NC: u32 = 264;
/// Dir. MOVZ imm. from bits 31:16.
pub const R_AARCH64_MOVW_UABS_G1: u32 = 265;
/// Likewise for MOVK; no check.
pub const R_AARCH64_MOVW_UABS_G1_NC: u32 = 266;
/// Dir. MOVZ imm. from bits 47:32.
pub const R_AARCH64_MOVW_UABS_G2: u32 = 267;
/// Likewise for MOVK; no check.
pub const R_AARCH64_MOVW_UABS_G2_NC: u32 = 268;
/// Dir. MOV{K,Z} imm. from 63:48.
pub const R_AARCH64_MOVW_UABS_G3: u32 = 269;
/// Dir. MOV{N,Z} imm. from 15:0.
pub const R_AARCH64_MOVW_SABS_G0: u32 = 270;
/// Dir. MOV{N,Z} imm. from 31:16.
pub const R_AARCH64_MOVW_SABS_G1: u32 = 271;
/// Dir. MOV{N,Z} imm. from 47:32.
pub const R_AARCH64_MOVW_SABS_G2: u32 = 272;
/// PC-rel. LD imm. from bits 20:2.
pub const R_AARCH64_LD_PREL_LO19: u32 = 273;
/// PC-rel. ADR imm. from bits 20:0.
pub const R_AARCH64_ADR_PREL_LO21: u32 = 274;
/// Page-rel. ADRP imm. from 32:12.
pub const R_AARCH64_ADR_PREL_PG_HI21: u32 = 275;
/// Likewise; no overflow check.
pub const R_AARCH64_ADR_PREL_PG_HI21_NC: u32 = 276;
/// Dir. ADD imm. from bits 11:0.
pub const R_AARCH64_ADD_ABS_LO12_NC: u32 = 277;
/// Likewise for LD/ST; no check.
pub const R_AARCH64_LDST8_ABS_LO12_NC: u32 = 278;
/// PC-rel. TBZ/TBNZ imm. from 15:2.
pub const R_AARCH64_TSTBR14: u32 = 279;
/// PC-rel. cond. br. imm. from 20:2.
pub const R_AARCH64_CONDBR19: u32 = 280;
/// PC-rel. B imm. from bits 27:2.
pub const R_AARCH64_JUMP26: u32 = 282;
/// Likewise for CALL.
pub const R_AARCH64_CALL26: u32 = 283;
/// Dir. ADD imm. from bits 11:1.
pub const R_AARCH64_LDST16_ABS_LO12_NC: u32 = 284;
/// Likewise for bits 11:2.
pub const R_AARCH64_LDST32_ABS_LO12_NC: u32 = 285;
/// Likewise for bits 11:3.
pub const R_AARCH64_LDST64_ABS_LO12_NC: u32 = 286;
/// PC-rel. MOV{N,Z} imm. from 15:0.
pub const R_AARCH64_MOVW_PREL_G0: u32 = 287;
/// Likewise for MOVK; no check.
pub const R_AARCH64_MOVW_PREL_G0_NC: u32 = 288;
/// PC-rel. MOV{N,Z} imm. from 31:16.
pub const R_AARCH64_MOVW_PREL_G1: u32 = 289;
/// Likewise for MOVK; no check.
pub const R_AARCH64_MOVW_PREL_G1_NC: u32 = 290;
/// PC-rel. MOV{N,Z} imm. from 47:32.
pub const R_AARCH64_MOVW_PREL_G2: u32 = 291;
/// Likewise for MOVK; no check.
pub const R_AARCH64_MOVW_PREL_G2_NC: u32 = 292;
/// PC-rel. MOV{N,Z} imm. from 63:48.
pub const R_AARCH64_MOVW_PREL_G3: u32 = 293;
/// Dir. ADD imm. from bits 11:4.
pub const R_AARCH64_LDST128_ABS_LO12_NC: u32 = 299;
/// GOT-rel. off. MOV{N,Z} imm. 15:0.
pub const R_AARCH64_MOVW_GOTOFF_G0: u32 = 300;
/// Likewise for MOVK; no check.
pub const R_AARCH64_MOVW_GOTOFF_G0_NC: u32 = 301;
/// GOT-rel. o. MOV{N,Z} imm. 31:16.
pub const R_AARCH64_MOVW_GOTOFF_G1: u32 = 302;
/// Likewise for MOVK; no check.
pub const R_AARCH64_MOVW_GOTOFF_G1_NC: u32 = 303;
/// GOT-rel. o. MOV{N,Z} imm. 47:32.
pub const R_AARCH64_MOVW_GOTOFF_G2: u32 = 304;
/// Likewise for MOVK; no check.
pub const R_AARCH64_MOVW_GOTOFF_G2_NC: u32 = 305;
/// GOT-rel. o. MOV{N,Z} imm. 63:48.
pub const R_AARCH64_MOVW_GOTOFF_G3: u32 = 306;
/// GOT-relative 64-bit.
pub const R_AARCH64_GOTREL64: u32 = 307;
/// GOT-relative 32-bit.
pub const R_AARCH64_GOTREL32: u32 = 308;
/// PC-rel. GOT off. load imm. 20:2.
pub const R_AARCH64_GOT_LD_PREL19: u32 = 309;
/// GOT-rel. off. LD/ST imm. 14:3.
pub const R_AARCH64_LD64_GOTOFF_LO15: u32 = 310;
/// P-page-rel. GOT off. ADRP 32:12.
pub const R_AARCH64_ADR_GOT_PAGE: u32 = 311;
/// Dir. GOT off. LD/ST imm. 11:3.
pub const R_AARCH64_LD64_GOT_LO12_NC: u32 = 312;
/// GOT-page-rel. GOT off. LD/ST 14:3
pub const R_AARCH64_LD64_GOTPAGE_LO15: u32 = 313;
/// PC-relative ADR imm. 20:0.
pub const R_AARCH64_TLSGD_ADR_PREL21: u32 = 512;
/// page-rel. ADRP imm. 32:12.
pub const R_AARCH64_TLSGD_ADR_PAGE21: u32 = 513;
/// direct ADD imm. from 11:0.
pub const R_AARCH64_TLSGD_ADD_LO12_NC: u32 = 514;
/// GOT-rel. MOV{N,Z} 31:16.
pub const R_AARCH64_TLSGD_MOVW_G1: u32 = 515;
/// GOT-rel. MOVK imm. 15:0.
pub const R_AARCH64_TLSGD_MOVW_G0_NC: u32 = 516;
/// Like 512; local dynamic model.
pub const R_AARCH64_TLSLD_ADR_PREL21: u32 = 517;
/// Like 513; local dynamic model.
pub const R_AARCH64_TLSLD_ADR_PAGE21: u32 = 518;
/// Like 514; local dynamic model.
pub const R_AARCH64_TLSLD_ADD_LO12_NC: u32 = 519;
/// Like 515; local dynamic model.
pub const R_AARCH64_TLSLD_MOVW_G1: u32 = 520;
/// Like 516; local dynamic model.
pub const R_AARCH64_TLSLD_MOVW_G0_NC: u32 = 521;
/// TLS PC-rel. load imm. 20:2.
pub const R_AARCH64_TLSLD_LD_PREL19: u32 = 522;
/// TLS DTP-rel. MOV{N,Z} 47:32.
pub const R_AARCH64_TLSLD_MOVW_DTPREL_G2: u32 = 523;
/// TLS DTP-rel. MOV{N,Z} 31:16.
pub const R_AARCH64_TLSLD_MOVW_DTPREL_G1: u32 = 524;
/// Likewise; MOVK; no check.
pub const R_AARCH64_TLSLD_MOVW_DTPREL_G1_NC: u32 = 525;
/// TLS DTP-rel. MOV{N,Z} 15:0.
pub const R_AARCH64_TLSLD_MOVW_DTPREL_G0: u32 = 526;
/// Likewise; MOVK; no check.
pub const R_AARCH64_TLSLD_MOVW_DTPREL_G0_NC: u32 = 527;
/// DTP-rel. ADD imm. from 23:12.
pub const R_AARCH64_TLSLD_ADD_DTPREL_HI12: u32 = 528;
/// DTP-rel. ADD imm. from 11:0.
pub const R_AARCH64_TLSLD_ADD_DTPREL_LO12: u32 = 529;
/// Likewise; no ovfl. check.
pub const R_AARCH64_TLSLD_ADD_DTPREL_LO12_NC: u32 = 530;
/// DTP-rel. LD/ST imm. 11:0.
pub const R_AARCH64_TLSLD_LDST8_DTPREL_LO12: u32 = 531;
/// Likewise; no check.
pub const R_AARCH64_TLSLD_LDST8_DTPREL_LO12_NC: u32 = 532;
/// DTP-rel. LD/ST imm. 11:1.
pub const R_AARCH64_TLSLD_LDST16_DTPREL_LO12: u32 = 533;
/// Likewise; no check.
pub const R_AARCH64_TLSLD_LDST16_DTPREL_LO12_NC: u32 = 534;
/// DTP-rel. LD/ST imm. 11:2.
pub const R_AARCH64_TLSLD_LDST32_DTPREL_LO12: u32 = 535;
/// Likewise; no check.
pub const R_AARCH64_TLSLD_LDST32_DTPREL_LO12_NC: u32 = 536;
/// DTP-rel. LD/ST imm. 11:3.
pub const R_AARCH64_TLSLD_LDST64_DTPREL_LO12: u32 = 537;
/// Likewise; no check.
pub const R_AARCH64_TLSLD_LDST64_DTPREL_LO12_NC: u32 = 538;
/// GOT-rel. MOV{N,Z} 31:16.
pub const R_AARCH64_TLSIE_MOVW_GOTTPREL_G1: u32 = 539;
/// GOT-rel. MOVK 15:0.
pub const R_AARCH64_TLSIE_MOVW_GOTTPREL_G0_NC: u32 = 540;
/// Page-rel. ADRP 32:12.
pub const R_AARCH64_TLSIE_ADR_GOTTPREL_PAGE21: u32 = 541;
/// Direct LD off. 11:3.
pub const R_AARCH64_TLSIE_LD64_GOTTPREL_LO12_NC: u32 = 542;
/// PC-rel. load imm. 20:2.
pub const R_AARCH64_TLSIE_LD_GOTTPREL_PREL19: u32 = 543;
/// TLS TP-rel. MOV{N,Z} 47:32.
pub const R_AARCH64_TLSLE_MOVW_TPREL_G2: u32 = 544;
/// TLS TP-rel. MOV{N,Z} 31:16.
pub const R_AARCH64_TLSLE_MOVW_TPREL_G1: u32 = 545;
/// Likewise; MOVK; no check.
pub const R_AARCH64_TLSLE_MOVW_TPREL_G1_NC: u32 = 546;
/// TLS TP-rel. MOV{N,Z} 15:0.
pub const R_AARCH64_TLSLE_MOVW_TPREL_G0: u32 = 547;
/// Likewise; MOVK; no check.
pub const R_AARCH64_TLSLE_MOVW_TPREL_G0_NC: u32 = 548;
/// TP-rel. ADD imm. 23:12.
pub const R_AARCH64_TLSLE_ADD_TPREL_HI12: u32 = 549;
/// TP-rel. ADD imm. 11:0.
pub const R_AARCH64_TLSLE_ADD_TPREL_LO12: u32 = 550;
/// Likewise; no ovfl. check.
pub const R_AARCH64_TLSLE_ADD_TPREL_LO12_NC: u32 = 551;
/// TP-rel. LD/ST off. 11:0.
pub const R_AARCH64_TLSLE_LDST8_TPREL_LO12: u32 = 552;
/// Likewise; no ovfl. check.
pub const R_AARCH64_TLSLE_LDST8_TPREL_LO12_NC: u32 = 553;
/// TP-rel. LD/ST off. 11:1.
pub const R_AARCH64_TLSLE_LDST16_TPREL_LO12: u32 = 554;
/// Likewise; no check.
pub const R_AARCH64_TLSLE_LDST16_TPREL_LO12_NC: u32 = 555;
/// TP-rel. LD/ST off. 11:2.
pub const R_AARCH64_TLSLE_LDST32_TPREL_LO12: u32 = 556;
/// Likewise; no check.
pub const R_AARCH64_TLSLE_LDST32_TPREL_LO12_NC: u32 = 557;
/// TP-rel. LD/ST off. 11:3.
pub const R_AARCH64_TLSLE_LDST64_TPREL_LO12: u32 = 558;
/// Likewise; no check.
pub const R_AARCH64_TLSLE_LDST64_TPREL_LO12_NC: u32 = 559;
/// PC-rel. load immediate 20:2.
pub const R_AARCH64_TLSDESC_LD_PREL19: u32 = 560;
/// PC-rel. ADR immediate 20:0.
pub const R_AARCH64_TLSDESC_ADR_PREL21: u32 = 561;
/// Page-rel. ADRP imm. 32:12.
pub const R_AARCH64_TLSDESC_ADR_PAGE21: u32 = 562;
/// Direct LD off. from 11:3.
pub const R_AARCH64_TLSDESC_LD64_LO12: u32 = 563;
/// Direct ADD imm. from 11:0.
pub const R_AARCH64_TLSDESC_ADD_LO12: u32 = 564;
/// GOT-rel. MOV{N,Z} imm. 31:16.
pub const R_AARCH64_TLSDESC_OFF_G1: u32 = 565;
/// GOT-rel. MOVK imm. 15:0; no ck.
pub const R_AARCH64_TLSDESC_OFF_G0_NC: u32 = 566;
/// Relax LDR.
pub const R_AARCH64_TLSDESC_LDR: u32 = 567;
/// Relax ADD.
pub const R_AARCH64_TLSDESC_ADD: u32 = 568;
/// Relax BLR.
pub const R_AARCH64_TLSDESC_CALL: u32 = 569;
/// TP-rel. LD/ST off. 11:4.
pub const R_AARCH64_TLSLE_LDST128_TPREL_LO12: u32 = 570;
/// Likewise; no check.
pub const R_AARCH64_TLSLE_LDST128_TPREL_LO12_NC: u32 = 571;
/// DTP-rel. LD/ST imm. 11:4.
pub const R_AARCH64_TLSLD_LDST128_DTPREL_LO12: u32 = 572;
/// Likewise; no check.
pub const R_AARCH64_TLSLD_LDST128_DTPREL_LO12_NC: u32 = 573;
/// Copy symbol at runtime.
pub const R_AARCH64_COPY: u32 = 1024;
/// Create GOT entry.
pub const R_AARCH64_GLOB_DAT: u32 = 1025;
/// Create PLT entry.
pub const R_AARCH64_JUMP_SLOT: u32 = 1026;
/// Adjust by program base.
pub const R_AARCH64_RELATIVE: u32 = 1027;
/// Module number, 64 bit.
pub const R_AARCH64_TLS_DTPMOD: u32 = 1028;
/// Module-relative offset, 64 bit.
pub const R_AARCH64_TLS_DTPREL: u32 = 1029;
/// TP-relative offset, 64 bit.
pub const R_AARCH64_TLS_TPREL: u32 = 1030;
/// TLS Descriptor.
pub const R_AARCH64_TLSDESC: u32 = 1031;
/// STT_GNU_IFUNC relocation.
pub const R_AARCH64_IRELATIVE: u32 = 1032;

// AVR values for `FileHeader*::e_flags`.

/// Bitmask for `EF_AVR_ARCH_*`.
pub const EF_AVR_ARCH: u32 = 0x7F;

/// If set, it is assumed that the elf file uses local symbols as reference
/// for the relocations so that linker relaxation is possible.
pub const EF_AVR_LINKRELAX_PREPARED: u32 = 0x80;

pub const EF_AVR_ARCH_AVR1: u32 = 1;
pub const EF_AVR_ARCH_AVR2: u32 = 2;
pub const EF_AVR_ARCH_AVR25: u32 = 25;
pub const EF_AVR_ARCH_AVR3: u32 = 3;
pub const EF_AVR_ARCH_AVR31: u32 = 31;
pub const EF_AVR_ARCH_AVR35: u32 = 35;
pub const EF_AVR_ARCH_AVR4: u32 = 4;
pub const EF_AVR_ARCH_AVR5: u32 = 5;
pub const EF_AVR_ARCH_AVR51: u32 = 51;
pub const EF_AVR_ARCH_AVR6: u32 = 6;
pub const EF_AVR_ARCH_AVRTINY: u32 = 100;
pub const EF_AVR_ARCH_XMEGA1: u32 = 101;
pub const EF_AVR_ARCH_XMEGA2: u32 = 102;
pub const EF_AVR_ARCH_XMEGA3: u32 = 103;
pub const EF_AVR_ARCH_XMEGA4: u32 = 104;
pub const EF_AVR_ARCH_XMEGA5: u32 = 105;
pub const EF_AVR_ARCH_XMEGA6: u32 = 106;
pub const EF_AVR_ARCH_XMEGA7: u32 = 107;

// AVR values for `Rel*::r_type`.

pub const R_AVR_NONE: u32 = 0;
/// Direct 32 bit
pub const R_AVR_32: u32 = 1;
pub const R_AVR_7_PCREL: u32 = 2;
pub const R_AVR_13_PCREL: u32 = 3;
/// Direct 16 bit
pub const R_AVR_16: u32 = 4;
pub const R_AVR_16_PM: u32 = 5;
pub const R_AVR_LO8_LDI: u32 = 6;
pub const R_AVR_HI8_LDI: u32 = 7;
pub const R_AVR_HH8_LDI: u32 = 8;
pub const R_AVR_LO8_LDI_NEG: u32 = 9;
pub const R_AVR_HI8_LDI_NEG: u32 = 10;
pub const R_AVR_HH8_LDI_NEG: u32 = 11;
pub const R_AVR_LO8_LDI_PM: u32 = 12;
pub const R_AVR_HI8_LDI_PM: u32 = 13;
pub const R_AVR_HH8_LDI_PM: u32 = 14;
pub const R_AVR_LO8_LDI_PM_NEG: u32 = 15;
pub const R_AVR_HI8_LDI_PM_NEG: u32 = 16;
pub const R_AVR_HH8_LDI_PM_NEG: u32 = 17;
pub const R_AVR_CALL: u32 = 18;
pub const R_AVR_LDI: u32 = 19;
pub const R_AVR_6: u32 = 20;
pub const R_AVR_6_ADIW: u32 = 21;
pub const R_AVR_MS8_LDI: u32 = 22;
pub const R_AVR_MS8_LDI_NEG: u32 = 23;
pub const R_AVR_LO8_LDI_GS: u32 = 24;
pub const R_AVR_HI8_LDI_GS: u32 = 25;
pub const R_AVR_8: u32 = 26;
pub const R_AVR_8_LO8: u32 = 27;
pub const R_AVR_8_HI8: u32 = 28;
pub const R_AVR_8_HLO8: u32 = 29;
pub const R_AVR_DIFF8: u32 = 30;
pub const R_AVR_DIFF16: u32 = 31;
pub const R_AVR_DIFF32: u32 = 32;
pub const R_AVR_LDS_STS_16: u32 = 33;
pub const R_AVR_PORT6: u32 = 34;
pub const R_AVR_PORT5: u32 = 35;
pub const R_AVR_32_PCREL: u32 = 36;

// MSP430 values for `Rel*::r_type`.

/// Direct 32 bit
pub const R_MSP430_32: u32 = 1;
/// Direct 16 bit
pub const R_MSP430_16_BYTE: u32 = 5;

// Hexagon values for `Rel*::r_type`.

/// Direct 32 bit
pub const R_HEX_32: u32 = 6;

// ARM values for `Rel*::r_type`.

/// No reloc
pub const R_ARM_NONE: u32 = 0;
/// Deprecated PC relative 26 bit branch.
pub const R_ARM_PC24: u32 = 1;
/// Direct 32 bit
pub const R_ARM_ABS32: u32 = 2;
/// PC relative 32 bit
pub const R_ARM_REL32: u32 = 3;
pub const R_ARM_PC13: u32 = 4;
/// Direct 16 bit
pub const R_ARM_ABS16: u32 = 5;
/// Direct 12 bit
pub const R_ARM_ABS12: u32 = 6;
/// Direct & 0x7C (`LDR`, `STR`).
pub const R_ARM_THM_ABS5: u32 = 7;
/// Direct 8 bit
pub const R_ARM_ABS8: u32 = 8;
pub const R_ARM_SBREL32: u32 = 9;
/// PC relative 24 bit (Thumb32 `BL`).
pub const R_ARM_THM_PC22: u32 = 10;
/// PC relative & 0x3FC (Thumb16 `LDR`, `ADD`, `ADR`).
pub const R_ARM_THM_PC8: u32 = 11;
pub const R_ARM_AMP_VCALL9: u32 = 12;
/// Obsolete static relocation.
pub const R_ARM_SWI24: u32 = 13;
/// Dynamic relocation.
pub const R_ARM_TLS_DESC: u32 = 13;
/// Reserved.
pub const R_ARM_THM_SWI8: u32 = 14;
/// Reserved.
pub const R_ARM_XPC25: u32 = 15;
/// Reserved.
pub const R_ARM_THM_XPC22: u32 = 16;
/// ID of module containing symbol
pub const R_ARM_TLS_DTPMOD32: u32 = 17;
/// Offset in TLS block
pub const R_ARM_TLS_DTPOFF32: u32 = 18;
/// Offset in static TLS block
pub const R_ARM_TLS_TPOFF32: u32 = 19;
/// Copy symbol at runtime
pub const R_ARM_COPY: u32 = 20;
/// Create GOT entry
pub const R_ARM_GLOB_DAT: u32 = 21;
/// Create PLT entry
pub const R_ARM_JUMP_SLOT: u32 = 22;
/// Adjust by program base
pub const R_ARM_RELATIVE: u32 = 23;
/// 32 bit offset to GOT
pub const R_ARM_GOTOFF: u32 = 24;
/// 32 bit PC relative offset to GOT
pub const R_ARM_GOTPC: u32 = 25;
/// 32 bit GOT entry
pub const R_ARM_GOT32: u32 = 26;
/// Deprecated, 32 bit PLT address.
pub const R_ARM_PLT32: u32 = 27;
/// PC relative 24 bit (`BL`, `BLX`).
pub const R_ARM_CALL: u32 = 28;
/// PC relative 24 bit (`B`, `BL<cond>`).
pub const R_ARM_JUMP24: u32 = 29;
/// PC relative 24 bit (Thumb32 `B.W`).
pub const R_ARM_THM_JUMP24: u32 = 30;
/// Adjust by program base.
pub const R_ARM_BASE_ABS: u32 = 31;
/// Obsolete.
pub const R_ARM_ALU_PCREL_7_0: u32 = 32;
/// Obsolete.
pub const R_ARM_ALU_PCREL_15_8: u32 = 33;
/// Obsolete.
pub const R_ARM_ALU_PCREL_23_15: u32 = 34;
/// Deprecated, prog. base relative.
pub const R_ARM_LDR_SBREL_11_0: u32 = 35;
/// Deprecated, prog. base relative.
pub const R_ARM_ALU_SBREL_19_12: u32 = 36;
/// Deprecated, prog. base relative.
pub const R_ARM_ALU_SBREL_27_20: u32 = 37;
pub const R_ARM_TARGET1: u32 = 38;
/// Program base relative.
pub const R_ARM_SBREL31: u32 = 39;
pub const R_ARM_V4BX: u32 = 40;
pub const R_ARM_TARGET2: u32 = 41;
/// 32 bit PC relative.
pub const R_ARM_PREL31: u32 = 42;
/// Direct 16-bit (`MOVW`).
pub const R_ARM_MOVW_ABS_NC: u32 = 43;
/// Direct high 16-bit (`MOVT`).
pub const R_ARM_MOVT_ABS: u32 = 44;
/// PC relative 16-bit (`MOVW`).
pub const R_ARM_MOVW_PREL_NC: u32 = 45;
/// PC relative (MOVT).
pub const R_ARM_MOVT_PREL: u32 = 46;
/// Direct 16 bit (Thumb32 `MOVW`).
pub const R_ARM_THM_MOVW_ABS_NC: u32 = 47;
/// Direct high 16 bit (Thumb32 `MOVT`).
pub const R_ARM_THM_MOVT_ABS: u32 = 48;
/// PC relative 16 bit (Thumb32 `MOVW`).
pub const R_ARM_THM_MOVW_PREL_NC: u32 = 49;
/// PC relative high 16 bit (Thumb32 `MOVT`).
pub const R_ARM_THM_MOVT_PREL: u32 = 50;
/// PC relative 20 bit (Thumb32 `B<cond>.W`).
pub const R_ARM_THM_JUMP19: u32 = 51;
/// PC relative X & 0x7E (Thumb16 `CBZ`, `CBNZ`).
pub const R_ARM_THM_JUMP6: u32 = 52;
/// PC relative 12 bit (Thumb32 `ADR.W`).
pub const R_ARM_THM_ALU_PREL_11_0: u32 = 53;
/// PC relative 12 bit (Thumb32 `LDR{D,SB,H,SH}`).
pub const R_ARM_THM_PC12: u32 = 54;
/// Direct 32-bit.
pub const R_ARM_ABS32_NOI: u32 = 55;
/// PC relative 32-bit.
pub const R_ARM_REL32_NOI: u32 = 56;
/// PC relative (`ADD`, `SUB`).
pub const R_ARM_ALU_PC_G0_NC: u32 = 57;
/// PC relative (`ADD`, `SUB`).
pub const R_ARM_ALU_PC_G0: u32 = 58;
/// PC relative (`ADD`, `SUB`).
pub const R_ARM_ALU_PC_G1_NC: u32 = 59;
/// PC relative (`ADD`, `SUB`).
pub const R_ARM_ALU_PC_G1: u32 = 60;
/// PC relative (`ADD`, `SUB`).
pub const R_ARM_ALU_PC_G2: u32 = 61;
/// PC relative (`LDR`,`STR`,`LDRB`,`STRB`).
pub const R_ARM_LDR_PC_G1: u32 = 62;
/// PC relative (`LDR`,`STR`,`LDRB`,`STRB`).
pub const R_ARM_LDR_PC_G2: u32 = 63;
/// PC relative (`STR{D,H}`, `LDR{D,SB,H,SH}`).
pub const R_ARM_LDRS_PC_G0: u32 = 64;
/// PC relative (`STR{D,H}`, `LDR{D,SB,H,SH}`).
pub const R_ARM_LDRS_PC_G1: u32 = 65;
/// PC relative (`STR{D,H}`, `LDR{D,SB,H,SH}`).
pub const R_ARM_LDRS_PC_G2: u32 = 66;
/// PC relative (`LDC`, `STC`).
pub const R_ARM_LDC_PC_G0: u32 = 67;
/// PC relative (`LDC`, `STC`).
pub const R_ARM_LDC_PC_G1: u32 = 68;
/// PC relative (`LDC`, `STC`).
pub const R_ARM_LDC_PC_G2: u32 = 69;
/// Program base relative (`ADD`,`SUB`).
pub const R_ARM_ALU_SB_G0_NC: u32 = 70;
/// Program base relative (`ADD`,`SUB`).
pub const R_ARM_ALU_SB_G0: u32 = 71;
/// Program base relative (`ADD`,`SUB`).
pub const R_ARM_ALU_SB_G1_NC: u32 = 72;
/// Program base relative (`ADD`,`SUB`).
pub const R_ARM_ALU_SB_G1: u32 = 73;
/// Program base relative (`ADD`,`SUB`).
pub const R_ARM_ALU_SB_G2: u32 = 74;
/// Program base relative (`LDR`, `STR`, `LDRB`, `STRB`).
pub const R_ARM_LDR_SB_G0: u32 = 75;
/// Program base relative (`LDR`, `STR`, `LDRB`, `STRB`).
pub const R_ARM_LDR_SB_G1: u32 = 76;
/// Program base relative (`LDR`, `STR`, `LDRB`, `STRB`).
pub const R_ARM_LDR_SB_G2: u32 = 77;
/// Program base relative (`LDR`, `STR`, `LDRB`, `STRB`).
pub const R_ARM_LDRS_SB_G0: u32 = 78;
/// Program base relative (`LDR`, `STR`, `LDRB`, `STRB`).
pub const R_ARM_LDRS_SB_G1: u32 = 79;
/// Program base relative (`LDR`, `STR`, `LDRB`, `STRB`).
pub const R_ARM_LDRS_SB_G2: u32 = 80;
/// Program base relative (`LDC`,`STC`).
pub const R_ARM_LDC_SB_G0: u32 = 81;
/// Program base relative (`LDC`,`STC`).
pub const R_ARM_LDC_SB_G1: u32 = 82;
/// Program base relative (`LDC`,`STC`).
pub const R_ARM_LDC_SB_G2: u32 = 83;
/// Program base relative 16 bit (`MOVW`).
pub const R_ARM_MOVW_BREL_NC: u32 = 84;
/// Program base relative high 16 bit (`MOVT`).
pub const R_ARM_MOVT_BREL: u32 = 85;
/// Program base relative 16 bit (`MOVW`).
pub const R_ARM_MOVW_BREL: u32 = 86;
/// Program base relative 16 bit (Thumb32 `MOVW`).
pub const R_ARM_THM_MOVW_BREL_NC: u32 = 87;
/// Program base relative high 16 bit (Thumb32 `MOVT`).
pub const R_ARM_THM_MOVT_BREL: u32 = 88;
/// Program base relative 16 bit (Thumb32 `MOVW`).
pub const R_ARM_THM_MOVW_BREL: u32 = 89;
pub const R_ARM_TLS_GOTDESC: u32 = 90;
pub const R_ARM_TLS_CALL: u32 = 91;
/// TLS relaxation.
pub const R_ARM_TLS_DESCSEQ: u32 = 92;
pub const R_ARM_THM_TLS_CALL: u32 = 93;
pub const R_ARM_PLT32_ABS: u32 = 94;
/// GOT entry.
pub const R_ARM_GOT_ABS: u32 = 95;
/// PC relative GOT entry.
pub const R_ARM_GOT_PREL: u32 = 96;
/// GOT entry relative to GOT origin (`LDR`).
pub const R_ARM_GOT_BREL12: u32 = 97;
/// 12 bit, GOT entry relative to GOT origin (`LDR`, `STR`).
pub const R_ARM_GOTOFF12: u32 = 98;
pub const R_ARM_GOTRELAX: u32 = 99;
pub const R_ARM_GNU_VTENTRY: u32 = 100;
pub const R_ARM_GNU_VTINHERIT: u32 = 101;
/// PC relative & 0xFFE (Thumb16 `B`).
pub const R_ARM_THM_PC11: u32 = 102;
/// PC relative & 0x1FE (Thumb16 `B`/`B<cond>`).
pub const R_ARM_THM_PC9: u32 = 103;
/// PC-rel 32 bit for global dynamic thread local data
pub const R_ARM_TLS_GD32: u32 = 104;
/// PC-rel 32 bit for local dynamic thread local data
pub const R_ARM_TLS_LDM32: u32 = 105;
/// 32 bit offset relative to TLS block
pub const R_ARM_TLS_LDO32: u32 = 106;
/// PC-rel 32 bit for GOT entry of static TLS block offset
pub const R_ARM_TLS_IE32: u32 = 107;
/// 32 bit offset relative to static TLS block
pub const R_ARM_TLS_LE32: u32 = 108;
/// 12 bit relative to TLS block (`LDR`, `STR`).
pub const R_ARM_TLS_LDO12: u32 = 109;
/// 12 bit relative to static TLS block (`LDR`, `STR`).
pub const R_ARM_TLS_LE12: u32 = 110;
/// 12 bit GOT entry relative to GOT origin (`LDR`).
pub const R_ARM_TLS_IE12GP: u32 = 111;
/// Obsolete.
pub const R_ARM_ME_TOO: u32 = 128;
pub const R_ARM_THM_TLS_DESCSEQ: u32 = 129;
pub const R_ARM_THM_TLS_DESCSEQ16: u32 = 129;
pub const R_ARM_THM_TLS_DESCSEQ32: u32 = 130;
/// GOT entry relative to GOT origin, 12 bit (Thumb32 `LDR`).
pub const R_ARM_THM_GOT_BREL12: u32 = 131;
pub const R_ARM_IRELATIVE: u32 = 160;
pub const R_ARM_RXPC25: u32 = 249;
pub const R_ARM_RSBREL32: u32 = 250;
pub const R_ARM_THM_RPC22: u32 = 251;
pub const R_ARM_RREL32: u32 = 252;
pub const R_ARM_RABS22: u32 = 253;
pub const R_ARM_RPC24: u32 = 254;
pub const R_ARM_RBASE: u32 = 255;

// C-SKY values for `Rel*::r_type`.
/// no reloc
pub const R_CKCORE_NONE: u32 = 0;
/// direct 32 bit (S + A)
pub const R_CKCORE_ADDR32: u32 = 1;
/// disp ((S + A - P) >> 2) & 0xff
pub const R_CKCORE_PCRELIMM8BY4: u32 = 2;
/// disp ((S + A - P) >> 1) & 0x7ff
pub const R_CKCORE_PCRELIMM11BY2: u32 = 3;
/// 32-bit rel (S + A - P)
pub const R_CKCORE_PCREL32: u32 = 5;
/// disp ((S + A - P) >>1) & 0x7ff
pub const R_CKCORE_PCRELJSR_IMM11BY2: u32 = 6;
/// 32 bit adjust program base(B + A)
pub const R_CKCORE_RELATIVE: u32 = 9;
/// 32 bit adjust by program base
pub const R_CKCORE_COPY: u32 = 10;
/// off between got and sym (S)
pub const R_CKCORE_GLOB_DAT: u32 = 11;
/// PLT entry (S)
pub const R_CKCORE_JUMP_SLOT: u32 = 12;
/// offset to GOT (S + A - GOT)
pub const R_CKCORE_GOTOFF: u32 = 13;
/// PC offset to GOT (GOT + A - P)
pub const R_CKCORE_GOTPC: u32 = 14;
/// 32 bit GOT entry (G)
pub const R_CKCORE_GOT32: u32 = 15;
/// 32 bit PLT entry (G)
pub const R_CKCORE_PLT32: u32 = 16;
/// GOT entry in GLOB_DAT (GOT + G)
pub const R_CKCORE_ADDRGOT: u32 = 17;
/// PLT entry in GLOB_DAT (GOT + G)
pub const R_CKCORE_ADDRPLT: u32 = 18;
/// ((S + A - P) >> 1) & 0x3ff_ffff
pub const R_CKCORE_PCREL_IMM26BY2: u32 = 19;
/// disp ((S + A - P) >> 1) & 0xffff
pub const R_CKCORE_PCREL_IMM16BY2: u32 = 20;
/// disp ((S + A - P) >> 2) & 0xffff
pub const R_CKCORE_PCREL_IMM16BY4: u32 = 21;
/// disp ((S + A - P) >> 1) & 0x3ff
pub const R_CKCORE_PCREL_IMM10BY2: u32 = 22;
/// disp ((S + A - P) >> 2) & 0x3ff
pub const R_CKCORE_PCREL_IMM10BY4: u32 = 23;
/// high & low 16 bit ADDR, ((S + A) >> 16) & 0xffff
pub const R_CKCORE_ADDR_HI16: u32 = 24;
/// (S + A) & 0xffff
pub const R_CKCORE_ADDR_LO16: u32 = 25;
/// high & low 16 bit GOTPC, ((GOT + A - P) >> 16) & 0xffff
pub const R_CKCORE_GOTPC_HI16: u32 = 26;
/// (GOT + A - P) & 0xffff
pub const R_CKCORE_GOTPC_LO16: u32 = 27;
/// high & low 16 bit GOTOFF, ((S + A - GOT) >> 16) & 0xffff
pub const R_CKCORE_GOTOFF_HI16: u32 = 28;
/// (S + A - GOT) & 0xffff
pub const R_CKCORE_GOTOFF_LO16: u32 = 29;
/// 12 bit disp GOT entry (G)
pub const R_CKCORE_GOT12: u32 = 30;
/// high & low 16 bit GOT, (G >> 16) & 0xffff
pub const R_CKCORE_GOT_HI16: u32 = 31;
/// (G & 0xffff)
pub const R_CKCORE_GOT_LO16: u32 = 32;
/// 12 bit disp PLT entry (G)
pub const R_CKCORE_PLT12: u32 = 33;
/// high & low 16 bit PLT, (G >> 16) & 0xffff
pub const R_CKCORE_PLT_HI16: u32 = 34;
/// G & 0xffff
pub const R_CKCORE_PLT_LO16: u32 = 35;
/// high & low 16 bit ADDRGOT, (GOT + G * 4) & 0xffff
pub const R_CKCORE_ADDRGOT_HI16: u32 = 36;
/// (GOT + G * 4) & 0xffff
pub const R_CKCORE_ADDRGOT_LO16: u32 = 37;
/// high & low 16 bit ADDRPLT, ((GOT + G * 4) >> 16) & 0xFFFF
pub const R_CKCORE_ADDRPLT_HI16: u32 = 38;
/// (GOT+G*4) & 0xffff
pub const R_CKCORE_ADDRPLT_LO16: u32 = 39;
/// disp ((S+A-P) >>1) & x3ff_ffff
pub const R_CKCORE_PCREL_JSR_IMM26BY2: u32 = 40;
/// (S+A-BTEXT) & 0xffff
pub const R_CKCORE_TOFFSET_LO16: u32 = 41;
/// (S+A-BTEXT) & 0xffff
pub const R_CKCORE_DOFFSET_LO16: u32 = 42;
/// disp ((S+A-P) >>1) & 0x3ffff
pub const R_CKCORE_PCREL_IMM18BY2: u32 = 43;
/// disp (S+A-BDATA) & 0x3ffff
pub const R_CKCORE_DOFFSET_IMM18: u32 = 44;
/// disp ((S+A-BDATA)>>1) & 0x3ffff
pub const R_CKCORE_DOFFSET_IMM18BY2: u32 = 45;
/// disp ((S+A-BDATA)>>2) & 0x3ffff
pub const R_CKCORE_DOFFSET_IMM18BY4: u32 = 46;
/// disp (G >> 2)
pub const R_CKCORE_GOT_IMM18BY4: u32 = 48;
/// disp (G >> 2)
pub const R_CKCORE_PLT_IMM18BY4: u32 = 49;
/// disp ((S+A-P) >>2) & 0x7f
pub const R_CKCORE_PCREL_IMM7BY4: u32 = 50;
/// 32 bit offset to TLS block
pub const R_CKCORE_TLS_LE32: u32 = 51;
pub const R_CKCORE_TLS_IE32: u32 = 52;
pub const R_CKCORE_TLS_GD32: u32 = 53;
pub const R_CKCORE_TLS_LDM32: u32 = 54;
pub const R_CKCORE_TLS_LDO32: u32 = 55;
pub const R_CKCORE_TLS_DTPMOD32: u32 = 56;
pub const R_CKCORE_TLS_DTPOFF32: u32 = 57;
pub const R_CKCORE_TLS_TPOFF32: u32 = 58;

// C-SKY values for `FileHeader*::e_flags`.
pub const EF_CSKY_ABIMASK: u32 = 0xF000_0000;
pub const EF_CSKY_OTHER: u32 = 0x0FFF_0000;
pub const EF_CSKY_PROCESSOR: u32 = 0x0000_FFFF;

pub const EF_CSKY_ABIV1: u32 = 0x1000_0000;
pub const EF_CSKY_ABIV2: u32 = 0x2000_0000;

// C-SKY values for `SectionHeader*::sh_type`.
/// C-SKY attributes section.
pub const SHT_CSKY_ATTRIBUTES: u32 = SHT_LOPROC + 1;

// IA-64 specific declarations.

// IA-64 values for `FileHeader64::e_flags`.
/// os-specific flags
pub const EF_IA_64_MASKOS: u32 = 0x0000_000f;
/// 64-bit ABI
pub const EF_IA_64_ABI64: u32 = 0x0000_0010;
/// arch. version mask
pub const EF_IA_64_ARCH: u32 = 0xff00_0000;

// IA-64 values for `ProgramHeader64::p_type`.
/// arch extension bits
pub const PT_IA_64_ARCHEXT: u32 = PT_LOPROC + 0;
/// ia64 unwind bits
pub const PT_IA_64_UNWIND: u32 = PT_LOPROC + 1;
pub const PT_IA_64_HP_OPT_ANOT: u32 = PT_LOOS + 0x12;
pub const PT_IA_64_HP_HSL_ANOT: u32 = PT_LOOS + 0x13;
pub const PT_IA_64_HP_STACK: u32 = PT_LOOS + 0x14;

// IA-64 values for `ProgramHeader64::p_flags`.
/// spec insns w/o recovery
pub const PF_IA_64_NORECOV: u32 = 0x8000_0000;

// IA-64 values for `SectionHeader64::sh_type`.
/// extension bits
pub const SHT_IA_64_EXT: u32 = SHT_LOPROC + 0;
/// unwind bits
pub const SHT_IA_64_UNWIND: u32 = SHT_LOPROC + 1;

// IA-64 values for `SectionHeader64::sh_flags`.
/// section near gp
pub const SHF_IA_64_SHORT: u32 = 0x1000_0000;
/// spec insns w/o recovery
pub const SHF_IA_64_NORECOV: u32 = 0x2000_0000;

// IA-64 values for `Dyn64::d_tag`.
pub const DT_IA_64_PLT_RESERVE: u32 = DT_LOPROC + 0;

// IA-64 values for `Rel*::r_type`.
/// none
pub const R_IA64_NONE: u32 = 0x00;
/// symbol + addend, add imm14
pub const R_IA64_IMM14: u32 = 0x21;
/// symbol + addend, add imm22
pub const R_IA64_IMM22: u32 = 0x22;
/// symbol + addend, mov imm64
pub const R_IA64_IMM64: u32 = 0x23;
/// symbol + addend, data4 MSB
pub const R_IA64_DIR32MSB: u32 = 0x24;
/// symbol + addend, data4 LSB
pub const R_IA64_DIR32LSB: u32 = 0x25;
/// symbol + addend, data8 MSB
pub const R_IA64_DIR64MSB: u32 = 0x26;
/// symbol + addend, data8 LSB
pub const R_IA64_DIR64LSB: u32 = 0x27;
/// @gprel(sym + add), add imm22
pub const R_IA64_GPREL22: u32 = 0x2a;
/// @gprel(sym + add), mov imm64
pub const R_IA64_GPREL64I: u32 = 0x2b;
/// @gprel(sym + add), data4 MSB
pub const R_IA64_GPREL32MSB: u32 = 0x2c;
/// @gprel(sym + add), data4 LSB
pub const R_IA64_GPREL32LSB: u32 = 0x2d;
/// @gprel(sym + add), data8 MSB
pub const R_IA64_GPREL64MSB: u32 = 0x2e;
/// @gprel(sym + add), data8 LSB
pub const R_IA64_GPREL64LSB: u32 = 0x2f;
/// @ltoff(sym + add), add imm22
pub const R_IA64_LTOFF22: u32 = 0x32;
/// @ltoff(sym + add), mov imm64
pub const R_IA64_LTOFF64I: u32 = 0x33;
/// @pltoff(sym + add), add imm22
pub const R_IA64_PLTOFF22: u32 = 0x3a;
/// @pltoff(sym + add), mov imm64
pub const R_IA64_PLTOFF64I: u32 = 0x3b;
/// @pltoff(sym + add), data8 MSB
pub const R_IA64_PLTOFF64MSB: u32 = 0x3e;
/// @pltoff(sym + add), data8 LSB
pub const R_IA64_PLTOFF64LSB: u32 = 0x3f;
/// @fptr(sym + add), mov imm64
pub const R_IA64_FPTR64I: u32 = 0x43;
/// @fptr(sym + add), data4 MSB
pub const R_IA64_FPTR32MSB: u32 = 0x44;
/// @fptr(sym + add), data4 LSB
pub const R_IA64_FPTR32LSB: u32 = 0x45;
/// @fptr(sym + add), data8 MSB
pub const R_IA64_FPTR64MSB: u32 = 0x46;
/// @fptr(sym + add), data8 LSB
pub const R_IA64_FPTR64LSB: u32 = 0x47;
/// @pcrel(sym + add), brl
pub const R_IA64_PCREL60B: u32 = 0x48;
/// @pcrel(sym + add), ptb, call
pub const R_IA64_PCREL21B: u32 = 0x49;
/// @pcrel(sym + add), chk.s
pub const R_IA64_PCREL21M: u32 = 0x4a;
/// @pcrel(sym + add), fchkf
pub const R_IA64_PCREL21F: u32 = 0x4b;
/// @pcrel(sym + add), data4 MSB
pub const R_IA64_PCREL32MSB: u32 = 0x4c;
/// @pcrel(sym + add), data4 LSB
pub const R_IA64_PCREL32LSB: u32 = 0x4d;
/// @pcrel(sym + add), data8 MSB
pub const R_IA64_PCREL64MSB: u32 = 0x4e;
/// @pcrel(sym + add), data8 LSB
pub const R_IA64_PCREL64LSB: u32 = 0x4f;
/// @ltoff(@fptr(s+a)), imm22
pub const R_IA64_LTOFF_FPTR22: u32 = 0x52;
/// @ltoff(@fptr(s+a)), imm64
pub const R_IA64_LTOFF_FPTR64I: u32 = 0x53;
/// @ltoff(@fptr(s+a)), data4 MSB
pub const R_IA64_LTOFF_FPTR32MSB: u32 = 0x54;
/// @ltoff(@fptr(s+a)), data4 LSB
pub const R_IA64_LTOFF_FPTR32LSB: u32 = 0x55;
/// @ltoff(@fptr(s+a)), data8 MSB
pub const R_IA64_LTOFF_FPTR64MSB: u32 = 0x56;
/// @ltoff(@fptr(s+a)), data8 LSB
pub const R_IA64_LTOFF_FPTR64LSB: u32 = 0x57;
/// @segrel(sym + add), data4 MSB
pub const R_IA64_SEGREL32MSB: u32 = 0x5c;
/// @segrel(sym + add), data4 LSB
pub const R_IA64_SEGREL32LSB: u32 = 0x5d;
/// @segrel(sym + add), data8 MSB
pub const R_IA64_SEGREL64MSB: u32 = 0x5e;
/// @segrel(sym + add), data8 LSB
pub const R_IA64_SEGREL64LSB: u32 = 0x5f;
/// @secrel(sym + add), data4 MSB
pub const R_IA64_SECREL32MSB: u32 = 0x64;
/// @secrel(sym + add), data4 LSB
pub const R_IA64_SECREL32LSB: u32 = 0x65;
/// @secrel(sym + add), data8 MSB
pub const R_IA64_SECREL64MSB: u32 = 0x66;
/// @secrel(sym + add), data8 LSB
pub const R_IA64_SECREL64LSB: u32 = 0x67;
/// data 4 + REL
pub const R_IA64_REL32MSB: u32 = 0x6c;
/// data 4 + REL
pub const R_IA64_REL32LSB: u32 = 0x6d;
/// data 8 + REL
pub const R_IA64_REL64MSB: u32 = 0x6e;
/// data 8 + REL
pub const R_IA64_REL64LSB: u32 = 0x6f;
/// symbol + addend, data4 MSB
pub const R_IA64_LTV32MSB: u32 = 0x74;
/// symbol + addend, data4 LSB
pub const R_IA64_LTV32LSB: u32 = 0x75;
/// symbol + addend, data8 MSB
pub const R_IA64_LTV64MSB: u32 = 0x76;
/// symbol + addend, data8 LSB
pub const R_IA64_LTV64LSB: u32 = 0x77;
/// @pcrel(sym + add), 21bit inst
pub const R_IA64_PCREL21BI: u32 = 0x79;
/// @pcrel(sym + add), 22bit inst
pub const R_IA64_PCREL22: u32 = 0x7a;
/// @pcrel(sym + add), 64bit inst
pub const R_IA64_PCREL64I: u32 = 0x7b;
/// dynamic reloc, imported PLT, MSB
pub const R_IA64_IPLTMSB: u32 = 0x80;
/// dynamic reloc, imported PLT, LSB
pub const R_IA64_IPLTLSB: u32 = 0x81;
/// copy relocation
pub const R_IA64_COPY: u32 = 0x84;
/// Addend and symbol difference
pub const R_IA64_SUB: u32 = 0x85;
/// LTOFF22, relaxable.
pub const R_IA64_LTOFF22X: u32 = 0x86;
/// Use of LTOFF22X.
pub const R_IA64_LDXMOV: u32 = 0x87;
/// @tprel(sym + add), imm14
pub const R_IA64_TPREL14: u32 = 0x91;
/// @tprel(sym + add), imm22
pub const R_IA64_TPREL22: u32 = 0x92;
/// @tprel(sym + add), imm64
pub const R_IA64_TPREL64I: u32 = 0x93;
/// @tprel(sym + add), data8 MSB
pub const R_IA64_TPREL64MSB: u32 = 0x96;
/// @tprel(sym + add), data8 LSB
pub const R_IA64_TPREL64LSB: u32 = 0x97;
/// @ltoff(@tprel(s+a)), imm2
pub const R_IA64_LTOFF_TPREL22: u32 = 0x9a;
/// @dtpmod(sym + add), data8 MSB
pub const R_IA64_DTPMOD64MSB: u32 = 0xa6;
/// @dtpmod(sym + add), data8 LSB
pub const R_IA64_DTPMOD64LSB: u32 = 0xa7;
/// @ltoff(@dtpmod(sym + add)), imm22
pub const R_IA64_LTOFF_DTPMOD22: u32 = 0xaa;
/// @dtprel(sym + add), imm14
pub const R_IA64_DTPREL14: u32 = 0xb1;
/// @dtprel(sym + add), imm22
pub const R_IA64_DTPREL22: u32 = 0xb2;
/// @dtprel(sym + add), imm64
pub const R_IA64_DTPREL64I: u32 = 0xb3;
/// @dtprel(sym + add), data4 MSB
pub const R_IA64_DTPREL32MSB: u32 = 0xb4;
/// @dtprel(sym + add), data4 LSB
pub const R_IA64_DTPREL32LSB: u32 = 0xb5;
/// @dtprel(sym + add), data8 MSB
pub const R_IA64_DTPREL64MSB: u32 = 0xb6;
/// @dtprel(sym + add), data8 LSB
pub const R_IA64_DTPREL64LSB: u32 = 0xb7;
/// @ltoff(@dtprel(s+a)), imm22
pub const R_IA64_LTOFF_DTPREL22: u32 = 0xba;

// SH specific declarations.

// SH values `FileHeader*::e_flags`.
pub const EF_SH_MACH_MASK: u32 = 0x1f;
pub const EF_SH_UNKNOWN: u32 = 0x0;
pub const EF_SH1: u32 = 0x1;
pub const EF_SH2: u32 = 0x2;
pub const EF_SH3: u32 = 0x3;
pub const EF_SH_DSP: u32 = 0x4;
pub const EF_SH3_DSP: u32 = 0x5;
pub const EF_SH4AL_DSP: u32 = 0x6;
pub const EF_SH3E: u32 = 0x8;
pub const EF_SH4: u32 = 0x9;
pub const EF_SH2E: u32 = 0xb;
pub const EF_SH4A: u32 = 0xc;
pub const EF_SH2A: u32 = 0xd;
pub const EF_SH4_NOFPU: u32 = 0x10;
pub const EF_SH4A_NOFPU: u32 = 0x11;
pub const EF_SH4_NOMMU_NOFPU: u32 = 0x12;
pub const EF_SH2A_NOFPU: u32 = 0x13;
pub const EF_SH3_NOMMU: u32 = 0x14;
pub const EF_SH2A_SH4_NOFPU: u32 = 0x15;
pub const EF_SH2A_SH3_NOFPU: u32 = 0x16;
pub const EF_SH2A_SH4: u32 = 0x17;
pub const EF_SH2A_SH3E: u32 = 0x18;

// SH values `Rel*::r_type`.
pub const R_SH_NONE: u32 = 0;
pub const R_SH_DIR32: u32 = 1;
pub const R_SH_REL32: u32 = 2;
pub const R_SH_DIR8WPN: u32 = 3;
pub const R_SH_IND12W: u32 = 4;
pub const R_SH_DIR8WPL: u32 = 5;
pub const R_SH_DIR8WPZ: u32 = 6;
pub const R_SH_DIR8BP: u32 = 7;
pub const R_SH_DIR8W: u32 = 8;
pub const R_SH_DIR8L: u32 = 9;
pub const R_SH_SWITCH16: u32 = 25;
pub const R_SH_SWITCH32: u32 = 26;
pub const R_SH_USES: u32 = 27;
pub const R_SH_COUNT: u32 = 28;
pub const R_SH_ALIGN: u32 = 29;
pub const R_SH_CODE: u32 = 30;
pub const R_SH_DATA: u32 = 31;
pub const R_SH_LABEL: u32 = 32;
pub const R_SH_SWITCH8: u32 = 33;
pub const R_SH_GNU_VTINHERIT: u32 = 34;
pub const R_SH_GNU_VTENTRY: u32 = 35;
pub const R_SH_TLS_GD_32: u32 = 144;
pub const R_SH_TLS_LD_32: u32 = 145;
pub const R_SH_TLS_LDO_32: u32 = 146;
pub const R_SH_TLS_IE_32: u32 = 147;
pub const R_SH_TLS_LE_32: u32 = 148;
pub const R_SH_TLS_DTPMOD32: u32 = 149;
pub const R_SH_TLS_DTPOFF32: u32 = 150;
pub const R_SH_TLS_TPOFF32: u32 = 151;
pub const R_SH_GOT32: u32 = 160;
pub const R_SH_PLT32: u32 = 161;
pub const R_SH_COPY: u32 = 162;
pub const R_SH_GLOB_DAT: u32 = 163;
pub const R_SH_JMP_SLOT: u32 = 164;
pub const R_SH_RELATIVE: u32 = 165;
pub const R_SH_GOTOFF: u32 = 166;
pub const R_SH_GOTPC: u32 = 167;

// S/390 specific definitions.

// S/390 values `FileHeader*::e_flags`.

/// High GPRs kernel facility needed.
pub const EF_S390_HIGH_GPRS: u32 = 0x0000_0001;

// S/390 values `Rel*::r_type`.

/// No reloc.
pub const R_390_NONE: u32 = 0;
/// Direct 8 bit.
pub const R_390_8: u32 = 1;
/// Direct 12 bit.
pub const R_390_12: u32 = 2;
/// Direct 16 bit.
pub const R_390_16: u32 = 3;
/// Direct 32 bit.
pub const R_390_32: u32 = 4;
/// PC relative 32 bit.
pub const R_390_PC32: u32 = 5;
/// 12 bit GOT offset.
pub const R_390_GOT12: u32 = 6;
/// 32 bit GOT offset.
pub const R_390_GOT32: u32 = 7;
/// 32 bit PC relative PLT address.
pub const R_390_PLT32: u32 = 8;
/// Copy symbol at runtime.
pub const R_390_COPY: u32 = 9;
/// Create GOT entry.
pub const R_390_GLOB_DAT: u32 = 10;
/// Create PLT entry.
pub const R_390_JMP_SLOT: u32 = 11;
/// Adjust by program base.
pub const R_390_RELATIVE: u32 = 12;
/// 32 bit offset to GOT.
pub const R_390_GOTOFF32: u32 = 13;
/// 32 bit PC relative offset to GOT.
pub const R_390_GOTPC: u32 = 14;
/// 16 bit GOT offset.
pub const R_390_GOT16: u32 = 15;
/// PC relative 16 bit.
pub const R_390_PC16: u32 = 16;
/// PC relative 16 bit shifted by 1.
pub const R_390_PC16DBL: u32 = 17;
/// 16 bit PC rel. PLT shifted by 1.
pub const R_390_PLT16DBL: u32 = 18;
/// PC relative 32 bit shifted by 1.
pub const R_390_PC32DBL: u32 = 19;
/// 32 bit PC rel. PLT shifted by 1.
pub const R_390_PLT32DBL: u32 = 20;
/// 32 bit PC rel. GOT shifted by 1.
pub const R_390_GOTPCDBL: u32 = 21;
/// Direct 64 bit.
pub const R_390_64: u32 = 22;
/// PC relative 64 bit.
pub const R_390_PC64: u32 = 23;
/// 64 bit GOT offset.
pub const R_390_GOT64: u32 = 24;
/// 64 bit PC relative PLT address.
pub const R_390_PLT64: u32 = 25;
/// 32 bit PC rel. to GOT entry >> 1.
pub const R_390_GOTENT: u32 = 26;
/// 16 bit offset to GOT.
pub const R_390_GOTOFF16: u32 = 27;
/// 64 bit offset to GOT.
pub const R_390_GOTOFF64: u32 = 28;
/// 12 bit offset to jump slot.
pub const R_390_GOTPLT12: u32 = 29;
/// 16 bit offset to jump slot.
pub const R_390_GOTPLT16: u32 = 30;
/// 32 bit offset to jump slot.
pub const R_390_GOTPLT32: u32 = 31;
/// 64 bit offset to jump slot.
pub const R_390_GOTPLT64: u32 = 32;
/// 32 bit rel. offset to jump slot.
pub const R_390_GOTPLTENT: u32 = 33;
/// 16 bit offset from GOT to PLT.
pub const R_390_PLTOFF16: u32 = 34;
/// 32 bit offset from GOT to PLT.
pub const R_390_PLTOFF32: u32 = 35;
/// 16 bit offset from GOT to PLT.
pub const R_390_PLTOFF64: u32 = 36;
/// Tag for load insn in TLS code.
pub const R_390_TLS_LOAD: u32 = 37;
/// Tag for function call in general dynamic TLS code.
pub const R_390_TLS_GDCALL: u32 = 38;
/// Tag for function call in local dynamic TLS code.
pub const R_390_TLS_LDCALL: u32 = 39;
/// Direct 32 bit for general dynamic thread local data.
pub const R_390_TLS_GD32: u32 = 40;
/// Direct 64 bit for general dynamic thread local data.
pub const R_390_TLS_GD64: u32 = 41;
/// 12 bit GOT offset for static TLS block offset.
pub const R_390_TLS_GOTIE12: u32 = 42;
/// 32 bit GOT offset for static TLS block offset.
pub const R_390_TLS_GOTIE32: u32 = 43;
/// 64 bit GOT offset for static TLS block offset.
pub const R_390_TLS_GOTIE64: u32 = 44;
/// Direct 32 bit for local dynamic thread local data in LE code.
pub const R_390_TLS_LDM32: u32 = 45;
/// Direct 64 bit for local dynamic thread local data in LE code.
pub const R_390_TLS_LDM64: u32 = 46;
/// 32 bit address of GOT entry for negated static TLS block offset.
pub const R_390_TLS_IE32: u32 = 47;
/// 64 bit address of GOT entry for negated static TLS block offset.
pub const R_390_TLS_IE64: u32 = 48;
/// 32 bit rel. offset to GOT entry for negated static TLS block offset.
pub const R_390_TLS_IEENT: u32 = 49;
/// 32 bit negated offset relative to static TLS block.
pub const R_390_TLS_LE32: u32 = 50;
/// 64 bit negated offset relative to static TLS block.
pub const R_390_TLS_LE64: u32 = 51;
/// 32 bit offset relative to TLS block.
pub const R_390_TLS_LDO32: u32 = 52;
/// 64 bit offset relative to TLS block.
pub const R_390_TLS_LDO64: u32 = 53;
/// ID of module containing symbol.
pub const R_390_TLS_DTPMOD: u32 = 54;
/// Offset in TLS block.
pub const R_390_TLS_DTPOFF: u32 = 55;
/// Negated offset in static TLS block.
pub const R_390_TLS_TPOFF: u32 = 56;
/// Direct 20 bit.
pub const R_390_20: u32 = 57;
/// 20 bit GOT offset.
pub const R_390_GOT20: u32 = 58;
/// 20 bit offset to jump slot.
pub const R_390_GOTPLT20: u32 = 59;
/// 20 bit GOT offset for static TLS block offset.
pub const R_390_TLS_GOTIE20: u32 = 60;
/// STT_GNU_IFUNC relocation.
pub const R_390_IRELATIVE: u32 = 61;

// CRIS values `Rel*::r_type`.
pub const R_CRIS_NONE: u32 = 0;
pub const R_CRIS_8: u32 = 1;
pub const R_CRIS_16: u32 = 2;
pub const R_CRIS_32: u32 = 3;
pub const R_CRIS_8_PCREL: u32 = 4;
pub const R_CRIS_16_PCREL: u32 = 5;
pub const R_CRIS_32_PCREL: u32 = 6;
pub const R_CRIS_GNU_VTINHERIT: u32 = 7;
pub const R_CRIS_GNU_VTENTRY: u32 = 8;
pub const R_CRIS_COPY: u32 = 9;
pub const R_CRIS_GLOB_DAT: u32 = 10;
pub const R_CRIS_JUMP_SLOT: u32 = 11;
pub const R_CRIS_RELATIVE: u32 = 12;
pub const R_CRIS_16_GOT: u32 = 13;
pub const R_CRIS_32_GOT: u32 = 14;
pub const R_CRIS_16_GOTPLT: u32 = 15;
pub const R_CRIS_32_GOTPLT: u32 = 16;
pub const R_CRIS_32_GOTREL: u32 = 17;
pub const R_CRIS_32_PLT_GOTREL: u32 = 18;
pub const R_CRIS_32_PLT_PCREL: u32 = 19;

// AMD x86-64 values `Rel*::r_type`.
/// No reloc
pub const R_X86_64_NONE: u32 = 0;
/// Direct 64 bit
pub const R_X86_64_64: u32 = 1;
/// PC relative 32 bit signed
pub const R_X86_64_PC32: u32 = 2;
/// 32 bit GOT entry
pub const R_X86_64_GOT32: u32 = 3;
/// 32 bit PLT address
pub const R_X86_64_PLT32: u32 = 4;
/// Copy symbol at runtime
pub const R_X86_64_COPY: u32 = 5;
/// Create GOT entry
pub const R_X86_64_GLOB_DAT: u32 = 6;
/// Create PLT entry
pub const R_X86_64_JUMP_SLOT: u32 = 7;
/// Adjust by program base
pub const R_X86_64_RELATIVE: u32 = 8;
/// 32 bit signed PC relative offset to GOT
pub const R_X86_64_GOTPCREL: u32 = 9;
/// Direct 32 bit zero extended
pub const R_X86_64_32: u32 = 10;
/// Direct 32 bit sign extended
pub const R_X86_64_32S: u32 = 11;
/// Direct 16 bit zero extended
pub const R_X86_64_16: u32 = 12;
/// 16 bit sign extended pc relative
pub const R_X86_64_PC16: u32 = 13;
/// Direct 8 bit sign extended
pub const R_X86_64_8: u32 = 14;
/// 8 bit sign extended pc relative
pub const R_X86_64_PC8: u32 = 15;
/// ID of module containing symbol
pub const R_X86_64_DTPMOD64: u32 = 16;
/// Offset in module's TLS block
pub const R_X86_64_DTPOFF64: u32 = 17;
/// Offset in initial TLS block
pub const R_X86_64_TPOFF64: u32 = 18;
/// 32 bit signed PC relative offset to two GOT entries for GD symbol
pub const R_X86_64_TLSGD: u32 = 19;
/// 32 bit signed PC relative offset to two GOT entries for LD symbol
pub const R_X86_64_TLSLD: u32 = 20;
/// Offset in TLS block
pub const R_X86_64_DTPOFF32: u32 = 21;
/// 32 bit signed PC relative offset to GOT entry for IE symbol
pub const R_X86_64_GOTTPOFF: u32 = 22;
/// Offset in initial TLS block
pub const R_X86_64_TPOFF32: u32 = 23;
/// PC relative 64 bit
pub const R_X86_64_PC64: u32 = 24;
/// 64 bit offset to GOT
pub const R_X86_64_GOTOFF64: u32 = 25;
/// 32 bit signed pc relative offset to GOT
pub const R_X86_64_GOTPC32: u32 = 26;
/// 64-bit GOT entry offset
pub const R_X86_64_GOT64: u32 = 27;
/// 64-bit PC relative offset to GOT entry
pub const R_X86_64_GOTPCREL64: u32 = 28;
/// 64-bit PC relative offset to GOT
pub const R_X86_64_GOTPC64: u32 = 29;
/// like GOT64, says PLT entry needed
pub const R_X86_64_GOTPLT64: u32 = 30;
/// 64-bit GOT relative offset to PLT entry
pub const R_X86_64_PLTOFF64: u32 = 31;
/// Size of symbol plus 32-bit addend
pub const R_X86_64_SIZE32: u32 = 32;
/// Size of symbol plus 64-bit addend
pub const R_X86_64_SIZE64: u32 = 33;
/// GOT offset for TLS descriptor.
pub const R_X86_64_GOTPC32_TLSDESC: u32 = 34;
/// Marker for call through TLS descriptor.
pub const R_X86_64_TLSDESC_CALL: u32 = 35;
/// TLS descriptor.
pub const R_X86_64_TLSDESC: u32 = 36;
/// Adjust indirectly by program base
pub const R_X86_64_IRELATIVE: u32 = 37;
/// 64-bit adjust by program base
pub const R_X86_64_RELATIVE64: u32 = 38;
// 39 Reserved was R_X86_64_PC32_BND
// 40 Reserved was R_X86_64_PLT32_BND
/// Load from 32 bit signed pc relative offset to GOT entry without REX prefix, relaxable.
pub const R_X86_64_GOTPCRELX: u32 = 41;
/// Load from 32 bit signed pc relative offset to GOT entry with REX prefix, relaxable.
pub const R_X86_64_REX_GOTPCRELX: u32 = 42;

// AMD x86-64 values `SectionHeader*::sh_type`.
/// Unwind information.
pub const SHT_X86_64_UNWIND: u32 = 0x7000_0001;

// AM33 values `Rel*::r_type`.
/// No reloc.
pub const R_MN10300_NONE: u32 = 0;
/// Direct 32 bit.
pub const R_MN10300_32: u32 = 1;
/// Direct 16 bit.
pub const R_MN10300_16: u32 = 2;
/// Direct 8 bit.
pub const R_MN10300_8: u32 = 3;
/// PC-relative 32-bit.
pub const R_MN10300_PCREL32: u32 = 4;
/// PC-relative 16-bit signed.
pub const R_MN10300_PCREL16: u32 = 5;
/// PC-relative 8-bit signed.
pub const R_MN10300_PCREL8: u32 = 6;
/// Ancient C++ vtable garbage...
pub const R_MN10300_GNU_VTINHERIT: u32 = 7;
/// ... collection annotation.
pub const R_MN10300_GNU_VTENTRY: u32 = 8;
/// Direct 24 bit.
pub const R_MN10300_24: u32 = 9;
/// 32-bit PCrel offset to GOT.
pub const R_MN10300_GOTPC32: u32 = 10;
/// 16-bit PCrel offset to GOT.
pub const R_MN10300_GOTPC16: u32 = 11;
/// 32-bit offset from GOT.
pub const R_MN10300_GOTOFF32: u32 = 12;
/// 24-bit offset from GOT.
pub const R_MN10300_GOTOFF24: u32 = 13;
/// 16-bit offset from GOT.
pub const R_MN10300_GOTOFF16: u32 = 14;
/// 32-bit PCrel to PLT entry.
pub const R_MN10300_PLT32: u32 = 15;
/// 16-bit PCrel to PLT entry.
pub const R_MN10300_PLT16: u32 = 16;
/// 32-bit offset to GOT entry.
pub const R_MN10300_GOT32: u32 = 17;
/// 24-bit offset to GOT entry.
pub const R_MN10300_GOT24: u32 = 18;
/// 16-bit offset to GOT entry.
pub const R_MN10300_GOT16: u32 = 19;
/// Copy symbol at runtime.
pub const R_MN10300_COPY: u32 = 20;
/// Create GOT entry.
pub const R_MN10300_GLOB_DAT: u32 = 21;
/// Create PLT entry.
pub const R_MN10300_JMP_SLOT: u32 = 22;
/// Adjust by program base.
pub const R_MN10300_RELATIVE: u32 = 23;
/// 32-bit offset for global dynamic.
pub const R_MN10300_TLS_GD: u32 = 24;
/// 32-bit offset for local dynamic.
pub const R_MN10300_TLS_LD: u32 = 25;
/// Module-relative offset.
pub const R_MN10300_TLS_LDO: u32 = 26;
/// GOT offset for static TLS block offset.
pub const R_MN10300_TLS_GOTIE: u32 = 27;
/// GOT address for static TLS block offset.
pub const R_MN10300_TLS_IE: u32 = 28;
/// Offset relative to static TLS block.
pub const R_MN10300_TLS_LE: u32 = 29;
/// ID of module containing symbol.
pub const R_MN10300_TLS_DTPMOD: u32 = 30;
/// Offset in module TLS block.
pub const R_MN10300_TLS_DTPOFF: u32 = 31;
/// Offset in static TLS block.
pub const R_MN10300_TLS_TPOFF: u32 = 32;
/// Adjustment for next reloc as needed by linker relaxation.
pub const R_MN10300_SYM_DIFF: u32 = 33;
/// Alignment requirement for linker relaxation.
pub const R_MN10300_ALIGN: u32 = 34;

// M32R values `Rel32::r_type`.
/// No reloc.
pub const R_M32R_NONE: u32 = 0;
/// Direct 16 bit.
pub const R_M32R_16: u32 = 1;
/// Direct 32 bit.
pub const R_M32R_32: u32 = 2;
/// Direct 24 bit.
pub const R_M32R_24: u32 = 3;
/// PC relative 10 bit shifted.
pub const R_M32R_10_PCREL: u32 = 4;
/// PC relative 18 bit shifted.
pub const R_M32R_18_PCREL: u32 = 5;
/// PC relative 26 bit shifted.
pub const R_M32R_26_PCREL: u32 = 6;
/// High 16 bit with unsigned low.
pub const R_M32R_HI16_ULO: u32 = 7;
/// High 16 bit with signed low.
pub const R_M32R_HI16_SLO: u32 = 8;
/// Low 16 bit.
pub const R_M32R_LO16: u32 = 9;
/// 16 bit offset in SDA.
pub const R_M32R_SDA16: u32 = 10;
pub const R_M32R_GNU_VTINHERIT: u32 = 11;
pub const R_M32R_GNU_VTENTRY: u32 = 12;
// M32R values `Rela32::r_type`.
/// Direct 16 bit.
pub const R_M32R_16_RELA: u32 = 33;
/// Direct 32 bit.
pub const R_M32R_32_RELA: u32 = 34;
/// Direct 24 bit.
pub const R_M32R_24_RELA: u32 = 35;
/// PC relative 10 bit shifted.
pub const R_M32R_10_PCREL_RELA: u32 = 36;
/// PC relative 18 bit shifted.
pub const R_M32R_18_PCREL_RELA: u32 = 37;
/// PC relative 26 bit shifted.
pub const R_M32R_26_PCREL_RELA: u32 = 38;
/// High 16 bit with unsigned low
pub const R_M32R_HI16_ULO_RELA: u32 = 39;
/// High 16 bit with signed low
pub const R_M32R_HI16_SLO_RELA: u32 = 40;
/// Low 16 bit
pub const R_M32R_LO16_RELA: u32 = 41;
/// 16 bit offset in SDA
pub const R_M32R_SDA16_RELA: u32 = 42;
pub const R_M32R_RELA_GNU_VTINHERIT: u32 = 43;
pub const R_M32R_RELA_GNU_VTENTRY: u32 = 44;
/// PC relative 32 bit.
pub const R_M32R_REL32: u32 = 45;

/// 24 bit GOT entry
pub const R_M32R_GOT24: u32 = 48;
/// 26 bit PC relative to PLT shifted
pub const R_M32R_26_PLTREL: u32 = 49;
/// Copy symbol at runtime
pub const R_M32R_COPY: u32 = 50;
/// Create GOT entry
pub const R_M32R_GLOB_DAT: u32 = 51;
/// Create PLT entry
pub const R_M32R_JMP_SLOT: u32 = 52;
/// Adjust by program base
pub const R_M32R_RELATIVE: u32 = 53;
/// 24 bit offset to GOT
pub const R_M32R_GOTOFF: u32 = 54;
/// 24 bit PC relative offset to GOT
pub const R_M32R_GOTPC24: u32 = 55;
/// High 16 bit GOT entry with unsigned low
pub const R_M32R_GOT16_HI_ULO: u32 = 56;
/// High 16 bit GOT entry with signed low
pub const R_M32R_GOT16_HI_SLO: u32 = 57;
/// Low 16 bit GOT entry
pub const R_M32R_GOT16_LO: u32 = 58;
/// High 16 bit PC relative offset to GOT with unsigned low
pub const R_M32R_GOTPC_HI_ULO: u32 = 59;
/// High 16 bit PC relative offset to GOT with signed low
pub const R_M32R_GOTPC_HI_SLO: u32 = 60;
/// Low 16 bit PC relative offset to GOT
pub const R_M32R_GOTPC_LO: u32 = 61;
/// High 16 bit offset to GOT with unsigned low
pub const R_M32R_GOTOFF_HI_ULO: u32 = 62;
/// High 16 bit offset to GOT with signed low
pub const R_M32R_GOTOFF_HI_SLO: u32 = 63;
/// Low 16 bit offset to GOT
pub const R_M32R_GOTOFF_LO: u32 = 64;
/// Keep this the last entry.
pub const R_M32R_NUM: u32 = 256;

// MicroBlaze values `Rel*::r_type`.
/// No reloc.
pub const R_MICROBLAZE_NONE: u32 = 0;
/// Direct 32 bit.
pub const R_MICROBLAZE_32: u32 = 1;
/// PC relative 32 bit.
pub const R_MICROBLAZE_32_PCREL: u32 = 2;
/// PC relative 64 bit.
pub const R_MICROBLAZE_64_PCREL: u32 = 3;
/// Low 16 bits of PCREL32.
pub const R_MICROBLAZE_32_PCREL_LO: u32 = 4;
/// Direct 64 bit.
pub const R_MICROBLAZE_64: u32 = 5;
/// Low 16 bit.
pub const R_MICROBLAZE_32_LO: u32 = 6;
/// Read-only small data area.
pub const R_MICROBLAZE_SRO32: u32 = 7;
/// Read-write small data area.
pub const R_MICROBLAZE_SRW32: u32 = 8;
/// No reloc.
pub const R_MICROBLAZE_64_NONE: u32 = 9;
/// Symbol Op Symbol relocation.
pub const R_MICROBLAZE_32_SYM_OP_SYM: u32 = 10;
/// GNU C++ vtable hierarchy.
pub const R_MICROBLAZE_GNU_VTINHERIT: u32 = 11;
/// GNU C++ vtable member usage.
pub const R_MICROBLAZE_GNU_VTENTRY: u32 = 12;
/// PC-relative GOT offset.
pub const R_MICROBLAZE_GOTPC_64: u32 = 13;
/// GOT entry offset.
pub const R_MICROBLAZE_GOT_64: u32 = 14;
/// PLT offset (PC-relative).
pub const R_MICROBLAZE_PLT_64: u32 = 15;
/// Adjust by program base.
pub const R_MICROBLAZE_REL: u32 = 16;
/// Create PLT entry.
pub const R_MICROBLAZE_JUMP_SLOT: u32 = 17;
/// Create GOT entry.
pub const R_MICROBLAZE_GLOB_DAT: u32 = 18;
/// 64 bit offset to GOT.
pub const R_MICROBLAZE_GOTOFF_64: u32 = 19;
/// 32 bit offset to GOT.
pub const R_MICROBLAZE_GOTOFF_32: u32 = 20;
/// Runtime copy.
pub const R_MICROBLAZE_COPY: u32 = 21;
/// TLS Reloc.
pub const R_MICROBLAZE_TLS: u32 = 22;
/// TLS General Dynamic.
pub const R_MICROBLAZE_TLSGD: u32 = 23;
/// TLS Local Dynamic.
pub const R_MICROBLAZE_TLSLD: u32 = 24;
/// TLS Module ID.
pub const R_MICROBLAZE_TLSDTPMOD32: u32 = 25;
/// TLS Offset Within TLS Block.
pub const R_MICROBLAZE_TLSDTPREL32: u32 = 26;
/// TLS Offset Within TLS Block.
pub const R_MICROBLAZE_TLSDTPREL64: u32 = 27;
/// TLS Offset From Thread Pointer.
pub const R_MICROBLAZE_TLSGOTTPREL32: u32 = 28;
/// TLS Offset From Thread Pointer.
pub const R_MICROBLAZE_TLSTPREL32: u32 = 29;

// Nios II values `Dyn::d_tag`.
/// Address of _gp.
pub const DT_NIOS2_GP: u32 = 0x7000_0002;

// Nios II values `Rel*::r_type`.
/// No reloc.
pub const R_NIOS2_NONE: u32 = 0;
/// Direct signed 16 bit.
pub const R_NIOS2_S16: u32 = 1;
/// Direct unsigned 16 bit.
pub const R_NIOS2_U16: u32 = 2;
/// PC relative 16 bit.
pub const R_NIOS2_PCREL16: u32 = 3;
/// Direct call.
pub const R_NIOS2_CALL26: u32 = 4;
/// 5 bit constant expression.
pub const R_NIOS2_IMM5: u32 = 5;
/// 5 bit expression, shift 22.
pub const R_NIOS2_CACHE_OPX: u32 = 6;
/// 6 bit constant expression.
pub const R_NIOS2_IMM6: u32 = 7;
/// 8 bit constant expression.
pub const R_NIOS2_IMM8: u32 = 8;
/// High 16 bit.
pub const R_NIOS2_HI16: u32 = 9;
/// Low 16 bit.
pub const R_NIOS2_LO16: u32 = 10;
/// High 16 bit, adjusted.
pub const R_NIOS2_HIADJ16: u32 = 11;
/// 32 bit symbol value + addend.
pub const R_NIOS2_BFD_RELOC_32: u32 = 12;
/// 16 bit symbol value + addend.
pub const R_NIOS2_BFD_RELOC_16: u32 = 13;
/// 8 bit symbol value + addend.
pub const R_NIOS2_BFD_RELOC_8: u32 = 14;
/// 16 bit GP pointer offset.
pub const R_NIOS2_GPREL: u32 = 15;
/// GNU C++ vtable hierarchy.
pub const R_NIOS2_GNU_VTINHERIT: u32 = 16;
/// GNU C++ vtable member usage.
pub const R_NIOS2_GNU_VTENTRY: u32 = 17;
/// Unconditional branch.
pub const R_NIOS2_UJMP: u32 = 18;
/// Conditional branch.
pub const R_NIOS2_CJMP: u32 = 19;
/// Indirect call through register.
pub const R_NIOS2_CALLR: u32 = 20;
/// Alignment requirement for linker relaxation.
pub const R_NIOS2_ALIGN: u32 = 21;
/// 16 bit GOT entry.
pub const R_NIOS2_GOT16: u32 = 22;
/// 16 bit GOT entry for function.
pub const R_NIOS2_CALL16: u32 = 23;
/// %lo of offset to GOT pointer.
pub const R_NIOS2_GOTOFF_LO: u32 = 24;
/// %hiadj of offset to GOT pointer.
pub const R_NIOS2_GOTOFF_HA: u32 = 25;
/// %lo of PC relative offset.
pub const R_NIOS2_PCREL_LO: u32 = 26;
/// %hiadj of PC relative offset.
pub const R_NIOS2_PCREL_HA: u32 = 27;
/// 16 bit GOT offset for TLS GD.
pub const R_NIOS2_TLS_GD16: u32 = 28;
/// 16 bit GOT offset for TLS LDM.
pub const R_NIOS2_TLS_LDM16: u32 = 29;
/// 16 bit module relative offset.
pub const R_NIOS2_TLS_LDO16: u32 = 30;
/// 16 bit GOT offset for TLS IE.
pub const R_NIOS2_TLS_IE16: u32 = 31;
/// 16 bit LE TP-relative offset.
pub const R_NIOS2_TLS_LE16: u32 = 32;
/// Module number.
pub const R_NIOS2_TLS_DTPMOD: u32 = 33;
/// Module-relative offset.
pub const R_NIOS2_TLS_DTPREL: u32 = 34;
/// TP-relative offset.
pub const R_NIOS2_TLS_TPREL: u32 = 35;
/// Copy symbol at runtime.
pub const R_NIOS2_COPY: u32 = 36;
/// Create GOT entry.
pub const R_NIOS2_GLOB_DAT: u32 = 37;
/// Create PLT entry.
pub const R_NIOS2_JUMP_SLOT: u32 = 38;
/// Adjust by program base.
pub const R_NIOS2_RELATIVE: u32 = 39;
/// 16 bit offset to GOT pointer.
pub const R_NIOS2_GOTOFF: u32 = 40;
/// Direct call in .noat section.
pub const R_NIOS2_CALL26_NOAT: u32 = 41;
/// %lo() of GOT entry.
pub const R_NIOS2_GOT_LO: u32 = 42;
/// %hiadj() of GOT entry.
pub const R_NIOS2_GOT_HA: u32 = 43;
/// %lo() of function GOT entry.
pub const R_NIOS2_CALL_LO: u32 = 44;
/// %hiadj() of function GOT entry.
pub const R_NIOS2_CALL_HA: u32 = 45;

// TILEPro values `Rel*::r_type`.
/// No reloc
pub const R_TILEPRO_NONE: u32 = 0;
/// Direct 32 bit
pub const R_TILEPRO_32: u32 = 1;
/// Direct 16 bit
pub const R_TILEPRO_16: u32 = 2;
/// Direct 8 bit
pub const R_TILEPRO_8: u32 = 3;
/// PC relative 32 bit
pub const R_TILEPRO_32_PCREL: u32 = 4;
/// PC relative 16 bit
pub const R_TILEPRO_16_PCREL: u32 = 5;
/// PC relative 8 bit
pub const R_TILEPRO_8_PCREL: u32 = 6;
/// Low 16 bit
pub const R_TILEPRO_LO16: u32 = 7;
/// High 16 bit
pub const R_TILEPRO_HI16: u32 = 8;
/// High 16 bit, adjusted
pub const R_TILEPRO_HA16: u32 = 9;
/// Copy relocation
pub const R_TILEPRO_COPY: u32 = 10;
/// Create GOT entry
pub const R_TILEPRO_GLOB_DAT: u32 = 11;
/// Create PLT entry
pub const R_TILEPRO_JMP_SLOT: u32 = 12;
/// Adjust by program base
pub const R_TILEPRO_RELATIVE: u32 = 13;
/// X1 pipe branch offset
pub const R_TILEPRO_BROFF_X1: u32 = 14;
/// X1 pipe jump offset
pub const R_TILEPRO_JOFFLONG_X1: u32 = 15;
/// X1 pipe jump offset to PLT
pub const R_TILEPRO_JOFFLONG_X1_PLT: u32 = 16;
/// X0 pipe 8-bit
pub const R_TILEPRO_IMM8_X0: u32 = 17;
/// Y0 pipe 8-bit
pub const R_TILEPRO_IMM8_Y0: u32 = 18;
/// X1 pipe 8-bit
pub const R_TILEPRO_IMM8_X1: u32 = 19;
/// Y1 pipe 8-bit
pub const R_TILEPRO_IMM8_Y1: u32 = 20;
/// X1 pipe mtspr
pub const R_TILEPRO_MT_IMM15_X1: u32 = 21;
/// X1 pipe mfspr
pub const R_TILEPRO_MF_IMM15_X1: u32 = 22;
/// X0 pipe 16-bit
pub const R_TILEPRO_IMM16_X0: u32 = 23;
/// X1 pipe 16-bit
pub const R_TILEPRO_IMM16_X1: u32 = 24;
/// X0 pipe low 16-bit
pub const R_TILEPRO_IMM16_X0_LO: u32 = 25;
/// X1 pipe low 16-bit
pub const R_TILEPRO_IMM16_X1_LO: u32 = 26;
/// X0 pipe high 16-bit
pub const R_TILEPRO_IMM16_X0_HI: u32 = 27;
/// X1 pipe high 16-bit
pub const R_TILEPRO_IMM16_X1_HI: u32 = 28;
/// X0 pipe high 16-bit, adjusted
pub const R_TILEPRO_IMM16_X0_HA: u32 = 29;
/// X1 pipe high 16-bit, adjusted
pub const R_TILEPRO_IMM16_X1_HA: u32 = 30;
/// X0 pipe PC relative 16 bit
pub const R_TILEPRO_IMM16_X0_PCREL: u32 = 31;
/// X1 pipe PC relative 16 bit
pub const R_TILEPRO_IMM16_X1_PCREL: u32 = 32;
/// X0 pipe PC relative low 16 bit
pub const R_TILEPRO_IMM16_X0_LO_PCREL: u32 = 33;
/// X1 pipe PC relative low 16 bit
pub const R_TILEPRO_IMM16_X1_LO_PCREL: u32 = 34;
/// X0 pipe PC relative high 16 bit
pub const R_TILEPRO_IMM16_X0_HI_PCREL: u32 = 35;
/// X1 pipe PC relative high 16 bit
pub const R_TILEPRO_IMM16_X1_HI_PCREL: u32 = 36;
/// X0 pipe PC relative ha() 16 bit
pub const R_TILEPRO_IMM16_X0_HA_PCREL: u32 = 37;
/// X1 pipe PC relative ha() 16 bit
pub const R_TILEPRO_IMM16_X1_HA_PCREL: u32 = 38;
/// X0 pipe 16-bit GOT offset
pub const R_TILEPRO_IMM16_X0_GOT: u32 = 39;
/// X1 pipe 16-bit GOT offset
pub const R_TILEPRO_IMM16_X1_GOT: u32 = 40;
/// X0 pipe low 16-bit GOT offset
pub const R_TILEPRO_IMM16_X0_GOT_LO: u32 = 41;
/// X1 pipe low 16-bit GOT offset
pub const R_TILEPRO_IMM16_X1_GOT_LO: u32 = 42;
/// X0 pipe high 16-bit GOT offset
pub const R_TILEPRO_IMM16_X0_GOT_HI: u32 = 43;
/// X1 pipe high 16-bit GOT offset
pub const R_TILEPRO_IMM16_X1_GOT_HI: u32 = 44;
/// X0 pipe ha() 16-bit GOT offset
pub const R_TILEPRO_IMM16_X0_GOT_HA: u32 = 45;
/// X1 pipe ha() 16-bit GOT offset
pub const R_TILEPRO_IMM16_X1_GOT_HA: u32 = 46;
/// X0 pipe mm "start"
pub const R_TILEPRO_MMSTART_X0: u32 = 47;
/// X0 pipe mm "end"
pub const R_TILEPRO_MMEND_X0: u32 = 48;
/// X1 pipe mm "start"
pub const R_TILEPRO_MMSTART_X1: u32 = 49;
/// X1 pipe mm "end"
pub const R_TILEPRO_MMEND_X1: u32 = 50;
/// X0 pipe shift amount
pub const R_TILEPRO_SHAMT_X0: u32 = 51;
/// X1 pipe shift amount
pub const R_TILEPRO_SHAMT_X1: u32 = 52;
/// Y0 pipe shift amount
pub const R_TILEPRO_SHAMT_Y0: u32 = 53;
/// Y1 pipe shift amount
pub const R_TILEPRO_SHAMT_Y1: u32 = 54;
/// X1 pipe destination 8-bit
pub const R_TILEPRO_DEST_IMM8_X1: u32 = 55;
// Relocs 56-59 are currently not defined.
/// "jal" for TLS GD
pub const R_TILEPRO_TLS_GD_CALL: u32 = 60;
/// X0 pipe "addi" for TLS GD
pub const R_TILEPRO_IMM8_X0_TLS_GD_ADD: u32 = 61;
/// X1 pipe "addi" for TLS GD
pub const R_TILEPRO_IMM8_X1_TLS_GD_ADD: u32 = 62;
/// Y0 pipe "addi" for TLS GD
pub const R_TILEPRO_IMM8_Y0_TLS_GD_ADD: u32 = 63;
/// Y1 pipe "addi" for TLS GD
pub const R_TILEPRO_IMM8_Y1_TLS_GD_ADD: u32 = 64;
/// "lw_tls" for TLS IE
pub const R_TILEPRO_TLS_IE_LOAD: u32 = 65;
/// X0 pipe 16-bit TLS GD offset
pub const R_TILEPRO_IMM16_X0_TLS_GD: u32 = 66;
/// X1 pipe 16-bit TLS GD offset
pub const R_TILEPRO_IMM16_X1_TLS_GD: u32 = 67;
/// X0 pipe low 16-bit TLS GD offset
pub const R_TILEPRO_IMM16_X0_TLS_GD_LO: u32 = 68;
/// X1 pipe low 16-bit TLS GD offset
pub const R_TILEPRO_IMM16_X1_TLS_GD_LO: u32 = 69;
/// X0 pipe high 16-bit TLS GD offset
pub const R_TILEPRO_IMM16_X0_TLS_GD_HI: u32 = 70;
/// X1 pipe high 16-bit TLS GD offset
pub const R_TILEPRO_IMM16_X1_TLS_GD_HI: u32 = 71;
/// X0 pipe ha() 16-bit TLS GD offset
pub const R_TILEPRO_IMM16_X0_TLS_GD_HA: u32 = 72;
/// X1 pipe ha() 16-bit TLS GD offset
pub const R_TILEPRO_IMM16_X1_TLS_GD_HA: u32 = 73;
/// X0 pipe 16-bit TLS IE offset
pub const R_TILEPRO_IMM16_X0_TLS_IE: u32 = 74;
/// X1 pipe 16-bit TLS IE offset
pub const R_TILEPRO_IMM16_X1_TLS_IE: u32 = 75;
/// X0 pipe low 16-bit TLS IE offset
pub const R_TILEPRO_IMM16_X0_TLS_IE_LO: u32 = 76;
/// X1 pipe low 16-bit TLS IE offset
pub const R_TILEPRO_IMM16_X1_TLS_IE_LO: u32 = 77;
/// X0 pipe high 16-bit TLS IE offset
pub const R_TILEPRO_IMM16_X0_TLS_IE_HI: u32 = 78;
/// X1 pipe high 16-bit TLS IE offset
pub const R_TILEPRO_IMM16_X1_TLS_IE_HI: u32 = 79;
/// X0 pipe ha() 16-bit TLS IE offset
pub const R_TILEPRO_IMM16_X0_TLS_IE_HA: u32 = 80;
/// X1 pipe ha() 16-bit TLS IE offset
pub const R_TILEPRO_IMM16_X1_TLS_IE_HA: u32 = 81;
/// ID of module containing symbol
pub const R_TILEPRO_TLS_DTPMOD32: u32 = 82;
/// Offset in TLS block
pub const R_TILEPRO_TLS_DTPOFF32: u32 = 83;
/// Offset in static TLS block
pub const R_TILEPRO_TLS_TPOFF32: u32 = 84;
/// X0 pipe 16-bit TLS LE offset
pub const R_TILEPRO_IMM16_X0_TLS_LE: u32 = 85;
/// X1 pipe 16-bit TLS LE offset
pub const R_TILEPRO_IMM16_X1_TLS_LE: u32 = 86;
/// X0 pipe low 16-bit TLS LE offset
pub const R_TILEPRO_IMM16_X0_TLS_LE_LO: u32 = 87;
/// X1 pipe low 16-bit TLS LE offset
pub const R_TILEPRO_IMM16_X1_TLS_LE_LO: u32 = 88;
/// X0 pipe high 16-bit TLS LE offset
pub const R_TILEPRO_IMM16_X0_TLS_LE_HI: u32 = 89;
/// X1 pipe high 16-bit TLS LE offset
pub const R_TILEPRO_IMM16_X1_TLS_LE_HI: u32 = 90;
/// X0 pipe ha() 16-bit TLS LE offset
pub const R_TILEPRO_IMM16_X0_TLS_LE_HA: u32 = 91;
/// X1 pipe ha() 16-bit TLS LE offset
pub const R_TILEPRO_IMM16_X1_TLS_LE_HA: u32 = 92;

/// GNU C++ vtable hierarchy
pub const R_TILEPRO_GNU_VTINHERIT: u32 = 128;
/// GNU C++ vtable member usage
pub const R_TILEPRO_GNU_VTENTRY: u32 = 129;

// TILE-Gx values `Rel*::r_type`.
/// No reloc
pub const R_TILEGX_NONE: u32 = 0;
/// Direct 64 bit
pub const R_TILEGX_64: u32 = 1;
/// Direct 32 bit
pub const R_TILEGX_32: u32 = 2;
/// Direct 16 bit
pub const R_TILEGX_16: u32 = 3;
/// Direct 8 bit
pub const R_TILEGX_8: u32 = 4;
/// PC relative 64 bit
pub const R_TILEGX_64_PCREL: u32 = 5;
/// PC relative 32 bit
pub const R_TILEGX_32_PCREL: u32 = 6;
/// PC relative 16 bit
pub const R_TILEGX_16_PCREL: u32 = 7;
/// PC relative 8 bit
pub const R_TILEGX_8_PCREL: u32 = 8;
/// hword 0 16-bit
pub const R_TILEGX_HW0: u32 = 9;
/// hword 1 16-bit
pub const R_TILEGX_HW1: u32 = 10;
/// hword 2 16-bit
pub const R_TILEGX_HW2: u32 = 11;
/// hword 3 16-bit
pub const R_TILEGX_HW3: u32 = 12;
/// last hword 0 16-bit
pub const R_TILEGX_HW0_LAST: u32 = 13;
/// last hword 1 16-bit
pub const R_TILEGX_HW1_LAST: u32 = 14;
/// last hword 2 16-bit
pub const R_TILEGX_HW2_LAST: u32 = 15;
/// Copy relocation
pub const R_TILEGX_COPY: u32 = 16;
/// Create GOT entry
pub const R_TILEGX_GLOB_DAT: u32 = 17;
/// Create PLT entry
pub const R_TILEGX_JMP_SLOT: u32 = 18;
/// Adjust by program base
pub const R_TILEGX_RELATIVE: u32 = 19;
/// X1 pipe branch offset
pub const R_TILEGX_BROFF_X1: u32 = 20;
/// X1 pipe jump offset
pub const R_TILEGX_JUMPOFF_X1: u32 = 21;
/// X1 pipe jump offset to PLT
pub const R_TILEGX_JUMPOFF_X1_PLT: u32 = 22;
/// X0 pipe 8-bit
pub const R_TILEGX_IMM8_X0: u32 = 23;
/// Y0 pipe 8-bit
pub const R_TILEGX_IMM8_Y0: u32 = 24;
/// X1 pipe 8-bit
pub const R_TILEGX_IMM8_X1: u32 = 25;
/// Y1 pipe 8-bit
pub const R_TILEGX_IMM8_Y1: u32 = 26;
/// X1 pipe destination 8-bit
pub const R_TILEGX_DEST_IMM8_X1: u32 = 27;
/// X1 pipe mtspr
pub const R_TILEGX_MT_IMM14_X1: u32 = 28;
/// X1 pipe mfspr
pub const R_TILEGX_MF_IMM14_X1: u32 = 29;
/// X0 pipe mm "start"
pub const R_TILEGX_MMSTART_X0: u32 = 30;
/// X0 pipe mm "end"
pub const R_TILEGX_MMEND_X0: u32 = 31;
/// X0 pipe shift amount
pub const R_TILEGX_SHAMT_X0: u32 = 32;
/// X1 pipe shift amount
pub const R_TILEGX_SHAMT_X1: u32 = 33;
/// Y0 pipe shift amount
pub const R_TILEGX_SHAMT_Y0: u32 = 34;
/// Y1 pipe shift amount
pub const R_TILEGX_SHAMT_Y1: u32 = 35;
/// X0 pipe hword 0
pub const R_TILEGX_IMM16_X0_HW0: u32 = 36;
/// X1 pipe hword 0
pub const R_TILEGX_IMM16_X1_HW0: u32 = 37;
/// X0 pipe hword 1
pub const R_TILEGX_IMM16_X0_HW1: u32 = 38;
/// X1 pipe hword 1
pub const R_TILEGX_IMM16_X1_HW1: u32 = 39;
/// X0 pipe hword 2
pub const R_TILEGX_IMM16_X0_HW2: u32 = 40;
/// X1 pipe hword 2
pub const R_TILEGX_IMM16_X1_HW2: u32 = 41;
/// X0 pipe hword 3
pub const R_TILEGX_IMM16_X0_HW3: u32 = 42;
/// X1 pipe hword 3
pub const R_TILEGX_IMM16_X1_HW3: u32 = 43;
/// X0 pipe last hword 0
pub const R_TILEGX_IMM16_X0_HW0_LAST: u32 = 44;
/// X1 pipe last hword 0
pub const R_TILEGX_IMM16_X1_HW0_LAST: u32 = 45;
/// X0 pipe last hword 1
pub const R_TILEGX_IMM16_X0_HW1_LAST: u32 = 46;
/// X1 pipe last hword 1
pub const R_TILEGX_IMM16_X1_HW1_LAST: u32 = 47;
/// X0 pipe last hword 2
pub const R_TILEGX_IMM16_X0_HW2_LAST: u32 = 48;
/// X1 pipe last hword 2
pub const R_TILEGX_IMM16_X1_HW2_LAST: u32 = 49;
/// X0 pipe PC relative hword 0
pub const R_TILEGX_IMM16_X0_HW0_PCREL: u32 = 50;
/// X1 pipe PC relative hword 0
pub const R_TILEGX_IMM16_X1_HW0_PCREL: u32 = 51;
/// X0 pipe PC relative hword 1
pub const R_TILEGX_IMM16_X0_HW1_PCREL: u32 = 52;
/// X1 pipe PC relative hword 1
pub const R_TILEGX_IMM16_X1_HW1_PCREL: u32 = 53;
/// X0 pipe PC relative hword 2
pub const R_TILEGX_IMM16_X0_HW2_PCREL: u32 = 54;
/// X1 pipe PC relative hword 2
pub const R_TILEGX_IMM16_X1_HW2_PCREL: u32 = 55;
/// X0 pipe PC relative hword 3
pub const R_TILEGX_IMM16_X0_HW3_PCREL: u32 = 56;
/// X1 pipe PC relative hword 3
pub const R_TILEGX_IMM16_X1_HW3_PCREL: u32 = 57;
/// X0 pipe PC-rel last hword 0
pub const R_TILEGX_IMM16_X0_HW0_LAST_PCREL: u32 = 58;
/// X1 pipe PC-rel last hword 0
pub const R_TILEGX_IMM16_X1_HW0_LAST_PCREL: u32 = 59;
/// X0 pipe PC-rel last hword 1
pub const R_TILEGX_IMM16_X0_HW1_LAST_PCREL: u32 = 60;
/// X1 pipe PC-rel last hword 1
pub const R_TILEGX_IMM16_X1_HW1_LAST_PCREL: u32 = 61;
/// X0 pipe PC-rel last hword 2
pub const R_TILEGX_IMM16_X0_HW2_LAST_PCREL: u32 = 62;
/// X1 pipe PC-rel last hword 2
pub const R_TILEGX_IMM16_X1_HW2_LAST_PCREL: u32 = 63;
/// X0 pipe hword 0 GOT offset
pub const R_TILEGX_IMM16_X0_HW0_GOT: u32 = 64;
/// X1 pipe hword 0 GOT offset
pub const R_TILEGX_IMM16_X1_HW0_GOT: u32 = 65;
/// X0 pipe PC-rel PLT hword 0
pub const R_TILEGX_IMM16_X0_HW0_PLT_PCREL: u32 = 66;
/// X1 pipe PC-rel PLT hword 0
pub const R_TILEGX_IMM16_X1_HW0_PLT_PCREL: u32 = 67;
/// X0 pipe PC-rel PLT hword 1
pub const R_TILEGX_IMM16_X0_HW1_PLT_PCREL: u32 = 68;
/// X1 pipe PC-rel PLT hword 1
pub const R_TILEGX_IMM16_X1_HW1_PLT_PCREL: u32 = 69;
/// X0 pipe PC-rel PLT hword 2
pub const R_TILEGX_IMM16_X0_HW2_PLT_PCREL: u32 = 70;
/// X1 pipe PC-rel PLT hword 2
pub const R_TILEGX_IMM16_X1_HW2_PLT_PCREL: u32 = 71;
/// X0 pipe last hword 0 GOT offset
pub const R_TILEGX_IMM16_X0_HW0_LAST_GOT: u32 = 72;
/// X1 pipe last hword 0 GOT offset
pub const R_TILEGX_IMM16_X1_HW0_LAST_GOT: u32 = 73;
/// X0 pipe last hword 1 GOT offset
pub const R_TILEGX_IMM16_X0_HW1_LAST_GOT: u32 = 74;
/// X1 pipe last hword 1 GOT offset
pub const R_TILEGX_IMM16_X1_HW1_LAST_GOT: u32 = 75;
/// X0 pipe PC-rel PLT hword 3
pub const R_TILEGX_IMM16_X0_HW3_PLT_PCREL: u32 = 76;
/// X1 pipe PC-rel PLT hword 3
pub const R_TILEGX_IMM16_X1_HW3_PLT_PCREL: u32 = 77;
/// X0 pipe hword 0 TLS GD offset
pub const R_TILEGX_IMM16_X0_HW0_TLS_GD: u32 = 78;
/// X1 pipe hword 0 TLS GD offset
pub const R_TILEGX_IMM16_X1_HW0_TLS_GD: u32 = 79;
/// X0 pipe hword 0 TLS LE offset
pub const R_TILEGX_IMM16_X0_HW0_TLS_LE: u32 = 80;
/// X1 pipe hword 0 TLS LE offset
pub const R_TILEGX_IMM16_X1_HW0_TLS_LE: u32 = 81;
/// X0 pipe last hword 0 LE off
pub const R_TILEGX_IMM16_X0_HW0_LAST_TLS_LE: u32 = 82;
/// X1 pipe last hword 0 LE off
pub const R_TILEGX_IMM16_X1_HW0_LAST_TLS_LE: u32 = 83;
/// X0 pipe last hword 1 LE off
pub const R_TILEGX_IMM16_X0_HW1_LAST_TLS_LE: u32 = 84;
/// X1 pipe last hword 1 LE off
pub const R_TILEGX_IMM16_X1_HW1_LAST_TLS_LE: u32 = 85;
/// X0 pipe last hword 0 GD off
pub const R_TILEGX_IMM16_X0_HW0_LAST_TLS_GD: u32 = 86;
/// X1 pipe last hword 0 GD off
pub const R_TILEGX_IMM16_X1_HW0_LAST_TLS_GD: u32 = 87;
/// X0 pipe last hword 1 GD off
pub const R_TILEGX_IMM16_X0_HW1_LAST_TLS_GD: u32 = 88;
/// X1 pipe last hword 1 GD off
pub const R_TILEGX_IMM16_X1_HW1_LAST_TLS_GD: u32 = 89;
// Relocs 90-91 are currently not defined.
/// X0 pipe hword 0 TLS IE offset
pub const R_TILEGX_IMM16_X0_HW0_TLS_IE: u32 = 92;
/// X1 pipe hword 0 TLS IE offset
pub const R_TILEGX_IMM16_X1_HW0_TLS_IE: u32 = 93;
/// X0 pipe PC-rel PLT last hword 0
pub const R_TILEGX_IMM16_X0_HW0_LAST_PLT_PCREL: u32 = 94;
/// X1 pipe PC-rel PLT last hword 0
pub const R_TILEGX_IMM16_X1_HW0_LAST_PLT_PCREL: u32 = 95;
/// X0 pipe PC-rel PLT last hword 1
pub const R_TILEGX_IMM16_X0_HW1_LAST_PLT_PCREL: u32 = 96;
/// X1 pipe PC-rel PLT last hword 1
pub const R_TILEGX_IMM16_X1_HW1_LAST_PLT_PCREL: u32 = 97;
/// X0 pipe PC-rel PLT last hword 2
pub const R_TILEGX_IMM16_X0_HW2_LAST_PLT_PCREL: u32 = 98;
/// X1 pipe PC-rel PLT last hword 2
pub const R_TILEGX_IMM16_X1_HW2_LAST_PLT_PCREL: u32 = 99;
/// X0 pipe last hword 0 IE off
pub const R_TILEGX_IMM16_X0_HW0_LAST_TLS_IE: u32 = 100;
/// X1 pipe last hword 0 IE off
pub const R_TILEGX_IMM16_X1_HW0_LAST_TLS_IE: u32 = 101;
/// X0 pipe last hword 1 IE off
pub const R_TILEGX_IMM16_X0_HW1_LAST_TLS_IE: u32 = 102;
/// X1 pipe last hword 1 IE off
pub const R_TILEGX_IMM16_X1_HW1_LAST_TLS_IE: u32 = 103;
// Relocs 104-105 are currently not defined.
/// 64-bit ID of symbol's module
pub const R_TILEGX_TLS_DTPMOD64: u32 = 106;
/// 64-bit offset in TLS block
pub const R_TILEGX_TLS_DTPOFF64: u32 = 107;
/// 64-bit offset in static TLS block
pub const R_TILEGX_TLS_TPOFF64: u32 = 108;
/// 32-bit ID of symbol's module
pub const R_TILEGX_TLS_DTPMOD32: u32 = 109;
/// 32-bit offset in TLS block
pub const R_TILEGX_TLS_DTPOFF32: u32 = 110;
/// 32-bit offset in static TLS block
pub const R_TILEGX_TLS_TPOFF32: u32 = 111;
/// "jal" for TLS GD
pub const R_TILEGX_TLS_GD_CALL: u32 = 112;
/// X0 pipe "addi" for TLS GD
pub const R_TILEGX_IMM8_X0_TLS_GD_ADD: u32 = 113;
/// X1 pipe "addi" for TLS GD
pub const R_TILEGX_IMM8_X1_TLS_GD_ADD: u32 = 114;
/// Y0 pipe "addi" for TLS GD
pub const R_TILEGX_IMM8_Y0_TLS_GD_ADD: u32 = 115;
/// Y1 pipe "addi" for TLS GD
pub const R_TILEGX_IMM8_Y1_TLS_GD_ADD: u32 = 116;
/// "ld_tls" for TLS IE
pub const R_TILEGX_TLS_IE_LOAD: u32 = 117;
/// X0 pipe "addi" for TLS GD/IE
pub const R_TILEGX_IMM8_X0_TLS_ADD: u32 = 118;
/// X1 pipe "addi" for TLS GD/IE
pub const R_TILEGX_IMM8_X1_TLS_ADD: u32 = 119;
/// Y0 pipe "addi" for TLS GD/IE
pub const R_TILEGX_IMM8_Y0_TLS_ADD: u32 = 120;
/// Y1 pipe "addi" for TLS GD/IE
pub const R_TILEGX_IMM8_Y1_TLS_ADD: u32 = 121;

/// GNU C++ vtable hierarchy
pub const R_TILEGX_GNU_VTINHERIT: u32 = 128;
/// GNU C++ vtable member usage
pub const R_TILEGX_GNU_VTENTRY: u32 = 129;

// RISC-V values `FileHeader*::e_flags`.
pub const EF_RISCV_RVC: u32 = 0x0001;
pub const EF_RISCV_FLOAT_ABI: u32 = 0x0006;
pub const EF_RISCV_FLOAT_ABI_SOFT: u32 = 0x0000;
pub const EF_RISCV_FLOAT_ABI_SINGLE: u32 = 0x0002;
pub const EF_RISCV_FLOAT_ABI_DOUBLE: u32 = 0x0004;
pub const EF_RISCV_FLOAT_ABI_QUAD: u32 = 0x0006;
pub const EF_RISCV_RVE: u32 = 0x0008;
pub const EF_RISCV_TSO: u32 = 0x0010;

// RISC-V values for `SectionHeader*::sh_type`.
/// RISC-V attributes section.
pub const SHT_RISCV_ATTRIBUTES: u32 = SHT_LOPROC + 3;

// RISC-V values `Rel*::r_type`.
pub const R_RISCV_NONE: u32 = 0;
pub const R_RISCV_32: u32 = 1;
pub const R_RISCV_64: u32 = 2;
pub const R_RISCV_RELATIVE: u32 = 3;
pub const R_RISCV_COPY: u32 = 4;
pub const R_RISCV_JUMP_SLOT: u32 = 5;
pub const R_RISCV_TLS_DTPMOD32: u32 = 6;
pub const R_RISCV_TLS_DTPMOD64: u32 = 7;
pub const R_RISCV_TLS_DTPREL32: u32 = 8;
pub const R_RISCV_TLS_DTPREL64: u32 = 9;
pub const R_RISCV_TLS_TPREL32: u32 = 10;
pub const R_RISCV_TLS_TPREL64: u32 = 11;
pub const R_RISCV_BRANCH: u32 = 16;
pub const R_RISCV_JAL: u32 = 17;
pub const R_RISCV_CALL: u32 = 18;
pub const R_RISCV_CALL_PLT: u32 = 19;
pub const R_RISCV_GOT_HI20: u32 = 20;
pub const R_RISCV_TLS_GOT_HI20: u32 = 21;
pub const R_RISCV_TLS_GD_HI20: u32 = 22;
pub const R_RISCV_PCREL_HI20: u32 = 23;
pub const R_RISCV_PCREL_LO12_I: u32 = 24;
pub const R_RISCV_PCREL_LO12_S: u32 = 25;
pub const R_RISCV_HI20: u32 = 26;
pub const R_RISCV_LO12_I: u32 = 27;
pub const R_RISCV_LO12_S: u32 = 28;
pub const R_RISCV_TPREL_HI20: u32 = 29;
pub const R_RISCV_TPREL_LO12_I: u32 = 30;
pub const R_RISCV_TPREL_LO12_S: u32 = 31;
pub const R_RISCV_TPREL_ADD: u32 = 32;
pub const R_RISCV_ADD8: u32 = 33;
pub const R_RISCV_ADD16: u32 = 34;
pub const R_RISCV_ADD32: u32 = 35;
pub const R_RISCV_ADD64: u32 = 36;
pub const R_RISCV_SUB8: u32 = 37;
pub const R_RISCV_SUB16: u32 = 38;
pub const R_RISCV_SUB32: u32 = 39;
pub const R_RISCV_SUB64: u32 = 40;
pub const R_RISCV_GNU_VTINHERIT: u32 = 41;
pub const R_RISCV_GNU_VTENTRY: u32 = 42;
pub const R_RISCV_ALIGN: u32 = 43;
pub const R_RISCV_RVC_BRANCH: u32 = 44;
pub const R_RISCV_RVC_JUMP: u32 = 45;
pub const R_RISCV_RVC_LUI: u32 = 46;
pub const R_RISCV_GPREL_I: u32 = 47;
pub const R_RISCV_GPREL_S: u32 = 48;
pub const R_RISCV_TPREL_I: u32 = 49;
pub const R_RISCV_TPREL_S: u32 = 50;
pub const R_RISCV_RELAX: u32 = 51;
pub const R_RISCV_SUB6: u32 = 52;
pub const R_RISCV_SET6: u32 = 53;
pub const R_RISCV_SET8: u32 = 54;
pub const R_RISCV_SET16: u32 = 55;
pub const R_RISCV_SET32: u32 = 56;
pub const R_RISCV_32_PCREL: u32 = 57;
pub const R_RISCV_IRELATIVE: u32 = 58;
pub const R_RISCV_PLT32: u32 = 59;
pub const R_RISCV_SET_ULEB128: u32 = 60;
pub const R_RISCV_SUB_ULEB128: u32 = 61;
pub const R_RISCV_TLSDESC_HI20: u32 = 62;
pub const R_RISCV_TLSDESC_LOAD_LO12: u32 = 63;
pub const R_RISCV_TLSDESC_ADD_LO12: u32 = 64;
pub const R_RISCV_TLSDESC_CALL: u32 = 65;

// BPF values `Rel*::r_type`.
/// No reloc
pub const R_BPF_NONE: u32 = 0;
pub const R_BPF_64_64: u32 = 1;
pub const R_BPF_64_32: u32 = 10;

// SBF values `Rel*::r_type`.
/// No reloc
pub const R_SBF_NONE: u32 = 0;
pub const R_SBF_64_64: u32 = 1;
pub const R_SBF_64_32: u32 = 10;

// Imagination Meta values `Rel*::r_type`.

pub const R_METAG_HIADDR16: u32 = 0;
pub const R_METAG_LOADDR16: u32 = 1;
/// 32bit absolute address
pub const R_METAG_ADDR32: u32 = 2;
/// No reloc
pub const R_METAG_NONE: u32 = 3;
pub const R_METAG_RELBRANCH: u32 = 4;
pub const R_METAG_GETSETOFF: u32 = 5;

// Backward compatibility
pub const R_METAG_REG32OP1: u32 = 6;
pub const R_METAG_REG32OP2: u32 = 7;
pub const R_METAG_REG32OP3: u32 = 8;
pub const R_METAG_REG16OP1: u32 = 9;
pub const R_METAG_REG16OP2: u32 = 10;
pub const R_METAG_REG16OP3: u32 = 11;
pub const R_METAG_REG32OP4: u32 = 12;

pub const R_METAG_HIOG: u32 = 13;
pub const R_METAG_LOOG: u32 = 14;

pub const R_METAG_REL8: u32 = 15;
pub const R_METAG_REL16: u32 = 16;

pub const R_METAG_GNU_VTINHERIT: u32 = 30;
pub const R_METAG_GNU_VTENTRY: u32 = 31;

// PIC relocations
pub const R_METAG_HI16_GOTOFF: u32 = 32;
pub const R_METAG_LO16_GOTOFF: u32 = 33;
pub const R_METAG_GETSET_GOTOFF: u32 = 34;
pub const R_METAG_GETSET_GOT: u32 = 35;
pub const R_METAG_HI16_GOTPC: u32 = 36;
pub const R_METAG_LO16_GOTPC: u32 = 37;
pub const R_METAG_HI16_PLT: u32 = 38;
pub const R_METAG_LO16_PLT: u32 = 39;
pub const R_METAG_RELBRANCH_PLT: u32 = 40;
pub const R_METAG_GOTOFF: u32 = 41;
pub const R_METAG_PLT: u32 = 42;
pub const R_METAG_COPY: u32 = 43;
pub const R_METAG_JMP_SLOT: u32 = 44;
pub const R_METAG_RELATIVE: u32 = 45;
pub const R_METAG_GLOB_DAT: u32 = 46;

// TLS relocations
pub const R_METAG_TLS_GD: u32 = 47;
pub const R_METAG_TLS_LDM: u32 = 48;
pub const R_METAG_TLS_LDO_HI16: u32 = 49;
pub const R_METAG_TLS_LDO_LO16: u32 = 50;
pub const R_METAG_TLS_LDO: u32 = 51;
pub const R_METAG_TLS_IE: u32 = 52;
pub const R_METAG_TLS_IENONPIC: u32 = 53;
pub const R_METAG_TLS_IENONPIC_HI16: u32 = 54;
pub const R_METAG_TLS_IENONPIC_LO16: u32 = 55;
pub const R_METAG_TLS_TPOFF: u32 = 56;
pub const R_METAG_TLS_DTPMOD: u32 = 57;
pub const R_METAG_TLS_DTPOFF: u32 = 58;
pub const R_METAG_TLS_LE: u32 = 59;
pub const R_METAG_TLS_LE_HI16: u32 = 60;
pub const R_METAG_TLS_LE_LO16: u32 = 61;

// NDS32 values `Rel*::r_type`.
pub const R_NDS32_NONE: u32 = 0;
pub const R_NDS32_32_RELA: u32 = 20;
pub const R_NDS32_COPY: u32 = 39;
pub const R_NDS32_GLOB_DAT: u32 = 40;
pub const R_NDS32_JMP_SLOT: u32 = 41;
pub const R_NDS32_RELATIVE: u32 = 42;
pub const R_NDS32_TLS_TPOFF: u32 = 102;
pub const R_NDS32_TLS_DESC: u32 = 119;

// LoongArch values `FileHeader*::e_flags`.
/// Additional properties of the base ABI type, including the FP calling
/// convention.
pub const EF_LARCH_ABI_MODIFIER_MASK: u32 = 0x7;
/// Uses GPRs and the stack for parameter passing
pub const EF_LARCH_ABI_SOFT_FLOAT: u32 = 0x1;
/// Uses GPRs, 32-bit FPRs and the stack for parameter passing
pub const EF_LARCH_ABI_SINGLE_FLOAT: u32 = 0x2;
/// Uses GPRs, 64-bit FPRs and the stack for parameter passing
pub const EF_LARCH_ABI_DOUBLE_FLOAT: u32 = 0x3;
/// Uses relocation types directly writing to immediate slots
pub const EF_LARCH_OBJABI_V1: u32 = 0x40;

// LoongArch values `Rel*::r_type`.
/// No reloc
pub const R_LARCH_NONE: u32 = 0;
/// Runtime address resolving
pub const R_LARCH_32: u32 = 1;
/// Runtime address resolving
pub const R_LARCH_64: u32 = 2;
/// Runtime fixup for load-address
pub const R_LARCH_RELATIVE: u32 = 3;
/// Runtime memory copy in executable
pub const R_LARCH_COPY: u32 = 4;
/// Runtime PLT supporting
pub const R_LARCH_JUMP_SLOT: u32 = 5;
/// Runtime relocation for TLS-GD
pub const R_LARCH_TLS_DTPMOD32: u32 = 6;
/// Runtime relocation for TLS-GD
pub const R_LARCH_TLS_DTPMOD64: u32 = 7;
/// Runtime relocation for TLS-GD
pub const R_LARCH_TLS_DTPREL32: u32 = 8;
/// Runtime relocation for TLS-GD
pub const R_LARCH_TLS_DTPREL64: u32 = 9;
/// Runtime relocation for TLE-IE
pub const R_LARCH_TLS_TPREL32: u32 = 10;
/// Runtime relocation for TLE-IE
pub const R_LARCH_TLS_TPREL64: u32 = 11;
/// Runtime local indirect function resolving
pub const R_LARCH_IRELATIVE: u32 = 12;
/// Mark la.abs: load absolute address for static link.
pub const R_LARCH_MARK_LA: u32 = 20;
/// Mark external label branch: access PC relative address for static link.
pub const R_LARCH_MARK_PCREL: u32 = 21;
/// Push PC-relative offset
pub const R_LARCH_SOP_PUSH_PCREL: u32 = 22;
/// Push constant or absolute address
pub const R_LARCH_SOP_PUSH_ABSOLUTE: u32 = 23;
/// Duplicate stack top
pub const R_LARCH_SOP_PUSH_DUP: u32 = 24;
/// Push for access GOT entry
pub const R_LARCH_SOP_PUSH_GPREL: u32 = 25;
/// Push for TLS-LE
pub const R_LARCH_SOP_PUSH_TLS_TPREL: u32 = 26;
/// Push for TLS-IE
pub const R_LARCH_SOP_PUSH_TLS_GOT: u32 = 27;
/// Push for TLS-GD
pub const R_LARCH_SOP_PUSH_TLS_GD: u32 = 28;
/// Push for external function calling
pub const R_LARCH_SOP_PUSH_PLT_PCREL: u32 = 29;
/// Assert stack top
pub const R_LARCH_SOP_ASSERT: u32 = 30;
/// Stack top logical not (unary)
pub const R_LARCH_SOP_NOT: u32 = 31;
/// Stack top subtraction (binary)
pub const R_LARCH_SOP_SUB: u32 = 32;
/// Stack top left shift (binary)
pub const R_LARCH_SOP_SL: u32 = 33;
/// Stack top right shift (binary)
pub const R_LARCH_SOP_SR: u32 = 34;
/// Stack top addition (binary)
pub const R_LARCH_SOP_ADD: u32 = 35;
/// Stack top bitwise and (binary)
pub const R_LARCH_SOP_AND: u32 = 36;
/// Stack top selection (tertiary)
pub const R_LARCH_SOP_IF_ELSE: u32 = 37;
/// Pop stack top to fill 5-bit signed immediate operand
pub const R_LARCH_SOP_POP_32_S_10_5: u32 = 38;
/// Pop stack top to fill 12-bit unsigned immediate operand
pub const R_LARCH_SOP_POP_32_U_10_12: u32 = 39;
/// Pop stack top to fill 12-bit signed immediate operand
pub const R_LARCH_SOP_POP_32_S_10_12: u32 = 40;
/// Pop stack top to fill 16-bit signed immediate operand
pub const R_LARCH_SOP_POP_32_S_10_16: u32 = 41;
/// Pop stack top to fill 18-bit signed immediate operand with two trailing
/// zeros implied
pub const R_LARCH_SOP_POP_32_S_10_16_S2: u32 = 42;
/// Pop stack top to fill 20-bit signed immediate operand
pub const R_LARCH_SOP_POP_32_S_5_20: u32 = 43;
/// Pop stack top to fill 23-bit signed immediate operand with two trailing
/// zeros implied
pub const R_LARCH_SOP_POP_32_S_0_5_10_16_S2: u32 = 44;
/// Pop stack top to fill 28-bit signed immediate operand with two trailing
/// zeros implied
pub const R_LARCH_SOP_POP_32_S_0_10_10_16_S2: u32 = 45;
/// Pop stack top to fill an instruction
pub const R_LARCH_SOP_POP_32_U: u32 = 46;
/// 8-bit in-place addition
pub const R_LARCH_ADD8: u32 = 47;
/// 16-bit in-place addition
pub const R_LARCH_ADD16: u32 = 48;
/// 24-bit in-place addition
pub const R_LARCH_ADD24: u32 = 49;
/// 32-bit in-place addition
pub const R_LARCH_ADD32: u32 = 50;
/// 64-bit in-place addition
pub const R_LARCH_ADD64: u32 = 51;
/// 8-bit in-place subtraction
pub const R_LARCH_SUB8: u32 = 52;
/// 16-bit in-place subtraction
pub const R_LARCH_SUB16: u32 = 53;
/// 24-bit in-place subtraction
pub const R_LARCH_SUB24: u32 = 54;
/// 32-bit in-place subtraction
pub const R_LARCH_SUB32: u32 = 55;
/// 64-bit in-place subtraction
pub const R_LARCH_SUB64: u32 = 56;
/// GNU C++ vtable hierarchy
pub const R_LARCH_GNU_VTINHERIT: u32 = 57;
/// GNU C++ vtable member usage
pub const R_LARCH_GNU_VTENTRY: u32 = 58;
/// 18-bit PC-relative jump offset with two trailing zeros
pub const R_LARCH_B16: u32 = 64;
/// 23-bit PC-relative jump offset with two trailing zeros
pub const R_LARCH_B21: u32 = 65;
/// 28-bit PC-relative jump offset with two trailing zeros
pub const R_LARCH_B26: u32 = 66;
/// 12..=31 bits of 32/64-bit absolute address
pub const R_LARCH_ABS_HI20: u32 = 67;
/// 0..=11 bits of 32/64-bit absolute address
pub const R_LARCH_ABS_LO12: u32 = 68;
/// 32..=51 bits of 64-bit absolute address
pub const R_LARCH_ABS64_LO20: u32 = 69;
/// 52..=63 bits of 64-bit absolute address
pub const R_LARCH_ABS64_HI12: u32 = 70;
/// The signed 32-bit offset `offs` from `PC & 0xfffff000` to
/// `(S + A + 0x800) & 0xfffff000`, with 12 trailing zeros removed.
///
/// We define the *PC relative anchor* for `S + A` as `PC + offs` (`offs`
/// is sign-extended to VA bits).
pub const R_LARCH_PCALA_HI20: u32 = 71;
/// Same as R_LARCH_ABS_LO12.  0..=11 bits of the 32/64-bit offset from the
/// [PC relative anchor][R_LARCH_PCALA_HI20].
pub const R_LARCH_PCALA_LO12: u32 = 72;
/// 32..=51 bits of the 64-bit offset from the
/// [PC relative anchor][R_LARCH_PCALA_HI20].
pub const R_LARCH_PCALA64_LO20: u32 = 73;
/// 52..=63 bits of the 64-bit offset from the
/// [PC relative anchor][R_LARCH_PCALA_HI20].
pub const R_LARCH_PCALA64_HI12: u32 = 74;
/// The signed 32-bit offset `offs` from `PC & 0xfffff000` to
/// `(GP + G + 0x800) & 0xfffff000`, with 12 trailing zeros removed.
///
/// We define the *PC relative anchor* for the GOT entry at `GP + G` as
/// `PC + offs` (`offs` is sign-extended to VA bits).
pub const R_LARCH_GOT_PC_HI20: u32 = 75;
/// 0..=11 bits of the 32/64-bit offset from the
/// [PC relative anchor][R_LARCH_GOT_PC_HI20] to the GOT entry.
pub const R_LARCH_GOT_PC_LO12: u32 = 76;
/// 32..=51 bits of the 64-bit offset from the
/// [PC relative anchor][R_LARCH_GOT_PC_HI20] to the GOT entry.
pub const R_LARCH_GOT64_PC_LO20: u32 = 77;
/// 52..=63 bits of the 64-bit offset from the
/// [PC relative anchor][R_LARCH_GOT_PC_HI20] to the GOT entry.
pub const R_LARCH_GOT64_PC_HI12: u32 = 78;
/// 12..=31 bits of 32/64-bit GOT entry absolute address
pub const R_LARCH_GOT_HI20: u32 = 79;
/// 0..=11 bits of 32/64-bit GOT entry absolute address
pub const R_LARCH_GOT_LO12: u32 = 80;
/// 32..=51 bits of 64-bit GOT entry absolute address
pub const R_LARCH_GOT64_LO20: u32 = 81;
/// 52..=63 bits of 64-bit GOT entry absolute address
pub const R_LARCH_GOT64_HI12: u32 = 82;
/// 12..=31 bits of TLS LE 32/64-bit offset from thread pointer
pub const R_LARCH_TLS_LE_HI20: u32 = 83;
/// 0..=11 bits of TLS LE 32/64-bit offset from thread pointer
pub const R_LARCH_TLS_LE_LO12: u32 = 84;
/// 32..=51 bits of TLS LE 64-bit offset from thread pointer
pub const R_LARCH_TLS_LE64_LO20: u32 = 85;
/// 52..=63 bits of TLS LE 64-bit offset from thread pointer
pub const R_LARCH_TLS_LE64_HI12: u32 = 86;
/// The signed 32-bit offset `offs` from `PC & 0xfffff000` to
/// `(GP + IE + 0x800) & 0xfffff000`, with 12 trailing zeros removed.
///
/// We define the *PC relative anchor* for the TLS IE GOT entry at
/// `GP + IE` as `PC + offs` (`offs` is sign-extended to VA bits).
pub const R_LARCH_TLS_IE_PC_HI20: u32 = 87;
/// 0..=12 bits of the 32/64-bit offset from the
/// [PC-relative anchor][R_LARCH_TLS_IE_PC_HI20] to the TLS IE GOT entry.
pub const R_LARCH_TLS_IE_PC_LO12: u32 = 88;
/// 32..=51 bits of the 64-bit offset from the
/// [PC-relative anchor][R_LARCH_TLS_IE_PC_HI20] to the TLS IE GOT entry.
pub const R_LARCH_TLS_IE64_PC_LO20: u32 = 89;
/// 52..=63 bits of the 64-bit offset from the
/// [PC-relative anchor][R_LARCH_TLS_IE_PC_HI20] to the TLS IE GOT entry.
pub const R_LARCH_TLS_IE64_PC_HI12: u32 = 90;
/// 12..=31 bits of TLS IE GOT entry 32/64-bit absolute address
pub const R_LARCH_TLS_IE_HI20: u32 = 91;
/// 0..=11 bits of TLS IE GOT entry 32/64-bit absolute address
pub const R_LARCH_TLS_IE_LO12: u32 = 92;
/// 32..=51 bits of TLS IE GOT entry 64-bit absolute address
pub const R_LARCH_TLS_IE64_LO20: u32 = 93;
/// 51..=63 bits of TLS IE GOT entry 64-bit absolute address
pub const R_LARCH_TLS_IE64_HI12: u32 = 94;
/// 12..=31 bits of the offset from `PC` to `GP + GD + 0x800`, where
/// `GP + GD` is a TLS LD GOT entry
pub const R_LARCH_TLS_LD_PC_HI20: u32 = 95;
/// 12..=31 bits of TLS LD GOT entry 32/64-bit absolute address
pub const R_LARCH_TLS_LD_HI20: u32 = 96;
/// 12..=31 bits of the 32/64-bit PC-relative offset to the PC-relative
/// anchor for the TLE GD GOT entry.
pub const R_LARCH_TLS_GD_PC_HI20: u32 = 97;
/// 12..=31 bits of TLS GD GOT entry 32/64-bit absolute address
pub const R_LARCH_TLS_GD_HI20: u32 = 98;
/// 32-bit PC relative
pub const R_LARCH_32_PCREL: u32 = 99;
/// Paired with a normal relocation at the same address to indicate the
/// instruction can be relaxed
pub const R_LARCH_RELAX: u32 = 100;
/// Reserved
pub const R_LARCH_DELETE: u32 = 101;
/// Delete some bytes to ensure the instruction at PC + A aligned to
/// `A.next_power_of_two()`-byte boundary
pub const R_LARCH_ALIGN: u32 = 102;
/// 22-bit PC-relative offset with two trailing zeros
pub const R_LARCH_PCREL20_S2: u32 = 103;
/// Reserved
pub const R_LARCH_CFA: u32 = 104;
/// 6-bit in-place addition
pub const R_LARCH_ADD6: u32 = 105;
/// 6-bit in-place subtraction
pub const R_LARCH_SUB6: u32 = 106;
/// LEB128 in-place addition
pub const R_LARCH_ADD_ULEB128: u32 = 107;
/// LEB128 in-place subtraction
pub const R_LARCH_SUB_ULEB128: u32 = 108;
/// 64-bit PC relative
pub const R_LARCH_64_PCREL: u32 = 109;
/// 18..=37 bits of `S + A - PC` into the `pcaddu18i` instruction at `PC`,
/// and 2..=17 bits of `S + A - PC` into the `jirl` instruction at `PC + 4`
pub const R_LARCH_CALL36: u32 = 110;

// Xtensa values Rel*::r_type`.
pub const R_XTENSA_NONE: u32 = 0;
pub const R_XTENSA_32: u32 = 1;
pub const R_XTENSA_RTLD: u32 = 2;
pub const R_XTENSA_GLOB_DAT: u32 = 3;
pub const R_XTENSA_JMP_SLOT: u32 = 4;
pub const R_XTENSA_RELATIVE: u32 = 5;
pub const R_XTENSA_PLT: u32 = 6;
pub const R_XTENSA_OP0: u32 = 8;
pub const R_XTENSA_OP1: u32 = 9;
pub const R_XTENSA_OP2: u32 = 10;
pub const R_XTENSA_ASM_EXPAND: u32 = 11;
pub const R_XTENSA_ASM_SIMPLIFY: u32 = 12;
pub const R_XTENSA_32_PCREL: u32 = 14;
pub const R_XTENSA_GNU_VTINHERIT: u32 = 15;
pub const R_XTENSA_GNU_VTENTRY: u32 = 16;
pub const R_XTENSA_DIFF8: u32 = 17;
pub const R_XTENSA_DIFF16: u32 = 18;
pub const R_XTENSA_DIFF32: u32 = 19;
pub const R_XTENSA_SLOT0_OP: u32 = 20;
pub const R_XTENSA_SLOT1_OP: u32 = 21;
pub const R_XTENSA_SLOT2_OP: u32 = 22;
pub const R_XTENSA_SLOT3_OP: u32 = 23;
pub const R_XTENSA_SLOT4_OP: u32 = 24;
pub const R_XTENSA_SLOT5_OP: u32 = 25;
pub const R_XTENSA_SLOT6_OP: u32 = 26;
pub const R_XTENSA_SLOT7_OP: u32 = 27;
pub const R_XTENSA_SLOT8_OP: u32 = 28;
pub const R_XTENSA_SLOT9_OP: u32 = 29;
pub const R_XTENSA_SLOT10_OP: u32 = 30;
pub const R_XTENSA_SLOT11_OP: u32 = 31;
pub const R_XTENSA_SLOT12_OP: u32 = 32;
pub const R_XTENSA_SLOT13_OP: u32 = 33;
pub const R_XTENSA_SLOT14_OP: u32 = 34;
pub const R_XTENSA_SLOT0_ALT: u32 = 35;
pub const R_XTENSA_SLOT1_ALT: u32 = 36;
pub const R_XTENSA_SLOT2_ALT: u32 = 37;
pub const R_XTENSA_SLOT3_ALT: u32 = 38;
pub const R_XTENSA_SLOT4_ALT: u32 = 39;
pub const R_XTENSA_SLOT5_ALT: u32 = 40;
pub const R_XTENSA_SLOT6_ALT: u32 = 41;
pub const R_XTENSA_SLOT7_ALT: u32 = 42;
pub const R_XTENSA_SLOT8_ALT: u32 = 43;
pub const R_XTENSA_SLOT9_ALT: u32 = 44;
pub const R_XTENSA_SLOT10_ALT: u32 = 45;
pub const R_XTENSA_SLOT11_ALT: u32 = 46;
pub const R_XTENSA_SLOT12_ALT: u32 = 47;
pub const R_XTENSA_SLOT13_ALT: u32 = 48;
pub const R_XTENSA_SLOT14_ALT: u32 = 49;
pub const R_XTENSA_TLSDESC_FN: u32 = 50;
pub const R_XTENSA_TLSDESC_ARG: u32 = 51;
pub const R_XTENSA_TLS_DTPOFF: u32 = 52;
pub const R_XTENSA_TLS_TPOFF: u32 = 53;
pub const R_XTENSA_TLS_FUNC: u32 = 54;
pub const R_XTENSA_TLS_ARG: u32 = 55;
pub const R_XTENSA_TLS_CALL: u32 = 56;
pub const R_XTENSA_PDIFF8: u32 = 57;
pub const R_XTENSA_PDIFF16: u32 = 58;
pub const R_XTENSA_PDIFF32: u32 = 59;
pub const R_XTENSA_NDIFF8: u32 = 60;
pub const R_XTENSA_NDIFF16: u32 = 61;
pub const R_XTENSA_NDIFF32: u32 = 62;

// E2K values for `FileHeader*::e_flags`.
pub const EF_E2K_IPD: u32 = 3;
pub const EF_E2K_X86APP: u32 = 4;
pub const EF_E2K_4MB_PAGES: u32 = 8;
pub const EF_E2K_INCOMPAT: u32 = 16;
pub const EF_E2K_PM: u32 = 32;
pub const EF_E2K_PACK_SEGMENTS: u32 = 64;

/// Encode `E_E2K_MACH_*` into `FileHeader*::e_flags`.
pub const fn ef_e2k_mach_to_flag(e_flags: u32, x: u32) -> u32 {
    (e_flags & 0xffffff) | (x << 24)
}

/// Decode `E_E2K_MACH_*` from `FileHeader*::e_flags`.
pub const fn ef_e2k_flag_to_mach(e_flags: u32) -> u32 {
    e_flags >> 24
}

// Codes of supported E2K machines.

/// -march=generic code.
///
/// Legacy. Shouldn't be created nowadays.
pub const E_E2K_MACH_BASE: u32 = 0;
/// -march=elbrus-v1 code.
///
/// Legacy. Shouldn't be created nowadays.
pub const E_E2K_MACH_EV1: u32 = 1;
/// -march=elbrus-v2 code.
pub const E_E2K_MACH_EV2: u32 = 2;
/// -march=elbrus-v3 code.
pub const E_E2K_MACH_EV3: u32 = 3;
/// -march=elbrus-v4 code.
pub const E_E2K_MACH_EV4: u32 = 4;
/// -march=elbrus-v5 code.
pub const E_E2K_MACH_EV5: u32 = 5;
/// -march=elbrus-v6 code.
pub const E_E2K_MACH_EV6: u32 = 6;
/// -march=elbrus-v7 code.
pub const E_E2K_MACH_EV7: u32 = 7;
/// -mtune=elbrus-8c code.
pub const E_E2K_MACH_8C: u32 = 19;
/// -mtune=elbrus-1c+ code.
pub const E_E2K_MACH_1CPLUS: u32 = 20;
/// -mtune=elbrus-12c code.
pub const E_E2K_MACH_12C: u32 = 21;
/// -mtune=elbrus-16c code.
pub const E_E2K_MACH_16C: u32 = 22;
/// -mtune=elbrus-2c3 code.
pub const E_E2K_MACH_2C3: u32 = 23;
/// -mtune=elbrus-48c code.
pub const E_E2K_MACH_48C: u32 = 24;
/// -mtune=elbrus-8v7 code.
pub const E_E2K_MACH_8V7: u32 = 25;

// E2K values `Rel*::r_type`.

/// Direct 32 bit.
pub const R_E2K_32_ABS: u32 = 0;
/// PC relative 32 bit.
pub const R_E2K_32_PC: u32 = 2;
/// 32-bit offset of AP GOT entry.
pub const R_E2K_AP_GOT: u32 = 3;
/// 32-bit offset of PL GOT entry.
pub const R_E2K_PL_GOT: u32 = 4;
/// Create PLT entry.
pub const R_E2K_32_JMP_SLOT: u32 = 8;
/// Copy relocation, 32-bit case.
pub const R_E2K_32_COPY: u32 = 9;
/// Adjust by program base, 32-bit case.
pub const R_E2K_32_RELATIVE: u32 = 10;
/// Adjust indirectly by program base, 32-bit case.
pub const R_E2K_32_IRELATIVE: u32 = 11;
/// Size of symbol plus 32-bit addend.
pub const R_E2K_32_SIZE: u32 = 12;
/// Symbol value if resolved by the definition in the same
/// compilation unit or NULL otherwise, 32-bit case.
pub const R_E2K_32_DYNOPT: u32 = 13;
/// Direct 64 bit.
pub const R_E2K_64_ABS: u32 = 50;
/// Direct 64 bit for literal.
pub const R_E2K_64_ABS_LIT: u32 = 51;
/// PC relative 64 bit for literal.
pub const R_E2K_64_PC_LIT: u32 = 54;
/// Create PLT entry, 64-bit case.
pub const R_E2K_64_JMP_SLOT: u32 = 63;
/// Copy relocation, 64-bit case.
pub const R_E2K_64_COPY: u32 = 64;
/// Adjust by program base, 64-bit case.
pub const R_E2K_64_RELATIVE: u32 = 65;
/// Adjust by program base for literal, 64-bit case.
pub const R_E2K_64_RELATIVE_LIT: u32 = 66;
/// Adjust indirectly by program base, 64-bit case.
pub const R_E2K_64_IRELATIVE: u32 = 67;
/// Size of symbol plus 64-bit addend.
pub const R_E2K_64_SIZE: u32 = 68;
/// 64-bit offset of the symbol from GOT.
pub const R_E2K_64_GOTOFF: u32 = 69;

/// GOT entry for ID of module containing symbol.
pub const R_E2K_TLS_GDMOD: u32 = 70;
/// GOT entry for offset in module TLS block.
pub const R_E2K_TLS_GDREL: u32 = 71;
/// Static TLS block offset GOT entry.
pub const R_E2K_TLS_IE: u32 = 74;
/// Offset relative to static TLS block, 32-bit case.
pub const R_E2K_32_TLS_LE: u32 = 75;
/// Offset relative to static TLS block, 64-bit case.
pub const R_E2K_64_TLS_LE: u32 = 76;
/// ID of module containing symbol, 32-bit case.
pub const R_E2K_TLS_32_DTPMOD: u32 = 80;
/// Offset in module TLS block, 32-bit case.
pub const R_E2K_TLS_32_DTPREL: u32 = 81;
/// ID of module containing symbol, 64-bit case.
pub const R_E2K_TLS_64_DTPMOD: u32 = 82;
/// Offset in module TLS block, 64-bit case.
pub const R_E2K_TLS_64_DTPREL: u32 = 83;
/// Offset in static TLS block, 32-bit case.
pub const R_E2K_TLS_32_TPREL: u32 = 84;
/// Offset in static TLS block, 64-bit case.
pub const R_E2K_TLS_64_TPREL: u32 = 85;

/// Direct AP.
pub const R_E2K_AP: u32 = 100;
/// Direct PL.
pub const R_E2K_PL: u32 = 101;

/// 32-bit offset of the symbol's entry in GOT.
pub const R_E2K_GOT: u32 = 108;
/// 32-bit offset of the symbol from GOT.
pub const R_E2K_GOTOFF: u32 = 109;
/// PC relative 28 bit for DISP.
pub const R_E2K_DISP: u32 = 110;
/// Prefetch insn line containing the label (symbol).
pub const R_E2K_PREF: u32 = 111;
/// No reloc.
pub const R_E2K_NONE: u32 = 112;
/// 32-bit offset of the symbol's entry in .got.plt.
pub const R_E2K_GOTPLT: u32 = 114;
/// Is symbol resolved locally during the link.
/// The result is encoded in 5-bit ALS.src1.
pub const R_E2K_ISLOCAL: u32 = 115;
/// Is symbol resloved locally during the link.
/// The result is encoded in a long 32-bit LTS.
pub const R_E2K_ISLOCAL32: u32 = 118;
/// The symbol's offset from GOT encoded within a 64-bit literal.
pub const R_E2K_64_GOTOFF_LIT: u32 = 256;
/// Symbol value if resolved by the definition in the same
/// compilation unit or NULL otherwise, 64-bit case.
pub const R_E2K_64_DYNOPT: u32 = 257;
/// PC relative 64 bit in data.
pub const R_E2K_64_PC: u32 = 258;

// E2K values for `Dyn32::d_tag`.

pub const DT_E2K_LAZY: u32 = DT_LOPROC + 1;
pub const DT_E2K_LAZY_GOT: u32 = DT_LOPROC + 3;

pub const DT_E2K_INIT_GOT: u32 = DT_LOPROC + 0x101c;
pub const DT_E2K_EXPORT_PL: u32 = DT_LOPROC + 0x101d;
pub const DT_E2K_EXPORT_PLSZ: u32 = DT_LOPROC + 0x101e;
pub const DT_E2K_REAL_PLTGOT: u32 = DT_LOPROC + 0x101f;
pub const DT_E2K_NO_SELFINIT: u32 = DT_LOPROC + 0x1020;

pub const DT_E2K_NUM: u32 = 0x1021;

#[allow(non_upper_case_globals)]
pub const Tag_File: u8 = 1;
#[allow(non_upper_case_globals)]
pub const Tag_Section: u8 = 2;
#[allow(non_upper_case_globals)]
pub const Tag_Symbol: u8 = 3;

unsafe_impl_endian_pod!(
    FileHeader32,
    FileHeader64,
    SectionHeader32,
    SectionHeader64,
    CompressionHeader32,
    CompressionHeader64,
    Sym32,
    Sym64,
    Syminfo32,
    Syminfo64,
    Rel32,
    Rel64,
    Rela32,
    Rela64,
    Relr32,
    Relr64,
    ProgramHeader32,
    ProgramHeader64,
    Dyn32,
    Dyn64,
    Versym,
    Verdef,
    Verdaux,
    Verneed,
    Vernaux,
    NoteHeader32,
    NoteHeader64,
    HashHeader,
    GnuHashHeader,
);
