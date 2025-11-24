use alloc::fmt;
use alloc::vec::Vec;
use core::fmt::Debug;
use core::slice;

use crate::elf;
use crate::endian::{self, Endianness};
use crate::pod::Pod;
use crate::read::{
    self, Error, ReadRef, Relocation, RelocationEncoding, RelocationFlags, RelocationKind,
    RelocationTarget, SectionIndex, SymbolIndex,
};

use super::{ElfFile, FileHeader, SectionHeader, SectionTable};

/// A mapping from section index to associated relocation sections.
#[derive(Debug, Default)]
pub struct RelocationSections {
    relocations: Vec<usize>,
}

impl RelocationSections {
    /// Create a new mapping using the section table.
    ///
    /// Skips relocation sections that do not use the given symbol table section.
    pub fn parse<'data, Elf: FileHeader, R: ReadRef<'data>>(
        endian: Elf::Endian,
        sections: &SectionTable<'data, Elf, R>,
        symbol_section: SectionIndex,
    ) -> read::Result<Self> {
        let mut relocations = vec![0; sections.len()];
        for (index, section) in sections.iter().enumerate().rev() {
            let sh_type = section.sh_type(endian);
            if sh_type == elf::SHT_REL || sh_type == elf::SHT_RELA {
                // The symbol indices used in relocations must be for the symbol table
                // we are expecting to use.
                let sh_link = section.link(endian);
                if sh_link != symbol_section {
                    continue;
                }

                let sh_info = section.info_link(endian);
                if sh_info == SectionIndex(0) {
                    // Skip dynamic relocations.
                    continue;
                }
                if sh_info.0 >= relocations.len() {
                    return Err(Error("Invalid ELF sh_info for relocation section"));
                }

                // We don't support relocations that apply to other relocation sections
                // because it interferes with the chaining of relocation sections below.
                let sh_info_type = sections.section(sh_info)?.sh_type(endian);
                if sh_info_type == elf::SHT_REL || sh_info_type == elf::SHT_RELA {
                    return Err(Error("Unsupported ELF sh_info for relocation section"));
                }

                // Handle multiple relocation sections by chaining them.
                let next = relocations[sh_info.0];
                relocations[sh_info.0] = index;
                relocations[index] = next;
            }
        }
        Ok(Self { relocations })
    }

    /// Given a section index, return the section index of the associated relocation section.
    ///
    /// This may also be called with a relocation section index, and it will return the
    /// next associated relocation section.
    pub fn get(&self, index: SectionIndex) -> Option<SectionIndex> {
        self.relocations
            .get(index.0)
            .cloned()
            .filter(|x| *x != 0)
            .map(SectionIndex)
    }
}

pub(super) enum ElfRelaIterator<'data, Elf: FileHeader> {
    Rel(slice::Iter<'data, Elf::Rel>),
    Rela(slice::Iter<'data, Elf::Rela>),
}

impl<'data, Elf: FileHeader> ElfRelaIterator<'data, Elf> {
    fn is_rel(&self) -> bool {
        match self {
            ElfRelaIterator::Rel(_) => true,
            ElfRelaIterator::Rela(_) => false,
        }
    }
}

impl<'data, Elf: FileHeader> Iterator for ElfRelaIterator<'data, Elf> {
    type Item = Elf::Rela;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            ElfRelaIterator::Rel(ref mut i) => i.next().cloned().map(Self::Item::from),
            ElfRelaIterator::Rela(ref mut i) => i.next().cloned(),
        }
    }
}

/// An iterator for the dynamic relocations in an [`ElfFile32`](super::ElfFile32).
pub type ElfDynamicRelocationIterator32<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfDynamicRelocationIterator<'data, 'file, elf::FileHeader32<Endian>, R>;
/// An iterator for the dynamic relocations in an [`ElfFile64`](super::ElfFile64).
pub type ElfDynamicRelocationIterator64<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfDynamicRelocationIterator<'data, 'file, elf::FileHeader64<Endian>, R>;

/// An iterator for the dynamic relocations in an [`ElfFile`].
pub struct ElfDynamicRelocationIterator<'data, 'file, Elf, R = &'data [u8]>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    /// The current relocation section index.
    pub(super) section_index: SectionIndex,
    pub(super) file: &'file ElfFile<'data, Elf, R>,
    pub(super) relocations: Option<ElfRelaIterator<'data, Elf>>,
}

impl<'data, 'file, Elf, R> Iterator for ElfDynamicRelocationIterator<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    type Item = (u64, Relocation);

    fn next(&mut self) -> Option<Self::Item> {
        let endian = self.file.endian;
        loop {
            if let Some(ref mut relocations) = self.relocations {
                if let Some(reloc) = relocations.next() {
                    let relocation =
                        parse_relocation(self.file.header, endian, reloc, relocations.is_rel());
                    return Some((reloc.r_offset(endian).into(), relocation));
                }
                self.relocations = None;
            }

            let section = self.file.sections.section(self.section_index).ok()?;
            self.section_index.0 += 1;

            if section.link(endian) != self.file.dynamic_symbols.section() {
                continue;
            }

            match section.sh_type(endian) {
                elf::SHT_REL => {
                    if let Ok(relocations) = section.data_as_array(endian, self.file.data) {
                        self.relocations = Some(ElfRelaIterator::Rel(relocations.iter()));
                    }
                }
                elf::SHT_RELA => {
                    if let Ok(relocations) = section.data_as_array(endian, self.file.data) {
                        self.relocations = Some(ElfRelaIterator::Rela(relocations.iter()));
                    }
                }
                _ => {}
            }
        }
    }
}

impl<'data, 'file, Elf, R> fmt::Debug for ElfDynamicRelocationIterator<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ElfDynamicRelocationIterator").finish()
    }
}

/// An iterator for the relocations for an [`ElfSection32`](super::ElfSection32).
pub type ElfSectionRelocationIterator32<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfSectionRelocationIterator<'data, 'file, elf::FileHeader32<Endian>, R>;
/// An iterator for the relocations for an [`ElfSection64`](super::ElfSection64).
pub type ElfSectionRelocationIterator64<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    ElfSectionRelocationIterator<'data, 'file, elf::FileHeader64<Endian>, R>;

/// An iterator for the relocations for an [`ElfSection`](super::ElfSection).
pub struct ElfSectionRelocationIterator<'data, 'file, Elf, R = &'data [u8]>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    /// The current pointer in the chain of relocation sections.
    pub(super) section_index: SectionIndex,
    pub(super) file: &'file ElfFile<'data, Elf, R>,
    pub(super) relocations: Option<ElfRelaIterator<'data, Elf>>,
}

impl<'data, 'file, Elf, R> Iterator for ElfSectionRelocationIterator<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    type Item = (u64, Relocation);

    fn next(&mut self) -> Option<Self::Item> {
        let endian = self.file.endian;
        loop {
            if let Some(ref mut relocations) = self.relocations {
                if let Some(reloc) = relocations.next() {
                    let relocation =
                        parse_relocation(self.file.header, endian, reloc, relocations.is_rel());
                    return Some((reloc.r_offset(endian).into(), relocation));
                }
                self.relocations = None;
            }
            self.section_index = self.file.relocations.get(self.section_index)?;
            // The construction of RelocationSections ensures section_index is valid.
            let section = self.file.sections.section(self.section_index).unwrap();
            match section.sh_type(endian) {
                elf::SHT_REL => {
                    if let Ok(relocations) = section.data_as_array(endian, self.file.data) {
                        self.relocations = Some(ElfRelaIterator::Rel(relocations.iter()));
                    }
                }
                elf::SHT_RELA => {
                    if let Ok(relocations) = section.data_as_array(endian, self.file.data) {
                        self.relocations = Some(ElfRelaIterator::Rela(relocations.iter()));
                    }
                }
                _ => {}
            }
        }
    }
}

impl<'data, 'file, Elf, R> fmt::Debug for ElfSectionRelocationIterator<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ElfSectionRelocationIterator").finish()
    }
}

fn parse_relocation<Elf: FileHeader>(
    header: &Elf,
    endian: Elf::Endian,
    reloc: Elf::Rela,
    implicit_addend: bool,
) -> Relocation {
    use RelocationEncoding as E;
    use RelocationKind as K;

    let is_mips64el = header.is_mips64el(endian);
    let r_type = reloc.r_type(endian, is_mips64el);
    let flags = RelocationFlags::Elf { r_type };
    let g = E::Generic;
    let unknown = (K::Unknown, E::Generic, 0);
    let (kind, encoding, size) = match header.e_machine(endian) {
        elf::EM_AARCH64 => {
            if header.is_type_64() {
                match r_type {
                    elf::R_AARCH64_ABS64 => (K::Absolute, g, 64),
                    elf::R_AARCH64_ABS32 => (K::Absolute, g, 32),
                    elf::R_AARCH64_ABS16 => (K::Absolute, g, 16),
                    elf::R_AARCH64_PREL64 => (K::Relative, g, 64),
                    elf::R_AARCH64_PREL32 => (K::Relative, g, 32),
                    elf::R_AARCH64_PREL16 => (K::Relative, g, 16),
                    elf::R_AARCH64_CALL26 => (K::PltRelative, E::AArch64Call, 26),
                    _ => unknown,
                }
            } else {
                match r_type {
                    elf::R_AARCH64_P32_ABS32 => (K::Absolute, g, 32),
                    _ => unknown,
                }
            }
        }
        elf::EM_ARM => match r_type {
            elf::R_ARM_ABS32 => (K::Absolute, g, 32),
            _ => unknown,
        },
        elf::EM_AVR => match r_type {
            elf::R_AVR_32 => (K::Absolute, g, 32),
            elf::R_AVR_16 => (K::Absolute, g, 16),
            _ => unknown,
        },
        elf::EM_BPF => match r_type {
            elf::R_BPF_64_64 => (K::Absolute, g, 64),
            elf::R_BPF_64_32 => (K::Absolute, g, 32),
            _ => unknown,
        },
        elf::EM_CSKY => match r_type {
            elf::R_CKCORE_ADDR32 => (K::Absolute, g, 32),
            elf::R_CKCORE_PCREL32 => (K::Relative, g, 32),
            _ => unknown,
        },
        elf::EM_MCST_ELBRUS => match r_type {
            elf::R_E2K_32_ABS => (K::Absolute, g, 32),
            elf::R_E2K_64_ABS => (K::Absolute, g, 64),
            elf::R_E2K_64_ABS_LIT => (K::Absolute, E::E2KLit, 64),
            elf::R_E2K_DISP => (K::Relative, E::E2KDisp, 28),
            elf::R_E2K_GOT => (K::Got, g, 32),
            _ => unknown,
        },
        elf::EM_386 => match r_type {
            elf::R_386_32 => (K::Absolute, g, 32),
            elf::R_386_PC32 => (K::Relative, g, 32),
            elf::R_386_GOT32 => (K::Got, g, 32),
            elf::R_386_PLT32 => (K::PltRelative, g, 32),
            elf::R_386_GOTOFF => (K::GotBaseOffset, g, 32),
            elf::R_386_GOTPC => (K::GotBaseRelative, g, 32),
            elf::R_386_16 => (K::Absolute, g, 16),
            elf::R_386_PC16 => (K::Relative, g, 16),
            elf::R_386_8 => (K::Absolute, g, 8),
            elf::R_386_PC8 => (K::Relative, g, 8),
            _ => unknown,
        },
        elf::EM_X86_64 => match r_type {
            elf::R_X86_64_64 => (K::Absolute, g, 64),
            elf::R_X86_64_PC32 => (K::Relative, g, 32),
            elf::R_X86_64_GOT32 => (K::Got, g, 32),
            elf::R_X86_64_PLT32 => (K::PltRelative, g, 32),
            elf::R_X86_64_GOTPCREL => (K::GotRelative, g, 32),
            elf::R_X86_64_32 => (K::Absolute, g, 32),
            elf::R_X86_64_32S => (K::Absolute, E::X86Signed, 32),
            elf::R_X86_64_16 => (K::Absolute, g, 16),
            elf::R_X86_64_PC16 => (K::Relative, g, 16),
            elf::R_X86_64_8 => (K::Absolute, g, 8),
            elf::R_X86_64_PC8 => (K::Relative, g, 8),
            _ => unknown,
        },
        elf::EM_HEXAGON => match r_type {
            elf::R_HEX_32 => (K::Absolute, g, 32),
            _ => unknown,
        },
        elf::EM_LOONGARCH => match r_type {
            elf::R_LARCH_32 => (K::Absolute, g, 32),
            elf::R_LARCH_64 => (K::Absolute, g, 64),
            elf::R_LARCH_32_PCREL => (K::Relative, g, 32),
            elf::R_LARCH_64_PCREL => (K::Relative, g, 64),
            elf::R_LARCH_B16 => (K::Relative, E::LoongArchBranch, 16),
            elf::R_LARCH_B21 => (K::Relative, E::LoongArchBranch, 21),
            elf::R_LARCH_B26 => (K::Relative, E::LoongArchBranch, 26),
            _ => unknown,
        },
        elf::EM_68K => match r_type {
            elf::R_68K_32 => (K::Absolute, g, 32),
            elf::R_68K_16 => (K::Absolute, g, 16),
            elf::R_68K_8 => (K::Absolute, g, 8),
            elf::R_68K_PC32 => (K::Relative, g, 32),
            elf::R_68K_PC16 => (K::Relative, g, 16),
            elf::R_68K_PC8 => (K::Relative, g, 8),
            elf::R_68K_GOT32O => (K::Got, g, 32),
            elf::R_68K_GOT16O => (K::Got, g, 16),
            elf::R_68K_GOT8O => (K::Got, g, 8),
            elf::R_68K_GOT32 => (K::GotRelative, g, 32),
            elf::R_68K_GOT16 => (K::GotRelative, g, 16),
            elf::R_68K_GOT8 => (K::GotRelative, g, 8),
            elf::R_68K_PLT32 => (K::PltRelative, g, 32),
            elf::R_68K_PLT16 => (K::PltRelative, g, 16),
            elf::R_68K_PLT8 => (K::PltRelative, g, 8),
            _ => unknown,
        },
        elf::EM_MIPS => match r_type {
            elf::R_MIPS_16 => (K::Absolute, g, 16),
            elf::R_MIPS_32 => (K::Absolute, g, 32),
            elf::R_MIPS_64 => (K::Absolute, g, 64),
            _ => unknown,
        },
        elf::EM_MSP430 => match r_type {
            elf::R_MSP430_32 => (K::Absolute, g, 32),
            elf::R_MSP430_16_BYTE => (K::Absolute, g, 16),
            _ => unknown,
        },
        elf::EM_PPC => match r_type {
            elf::R_PPC_ADDR32 => (K::Absolute, g, 32),
            _ => unknown,
        },
        elf::EM_PPC64 => match r_type {
            elf::R_PPC64_ADDR32 => (K::Absolute, g, 32),
            elf::R_PPC64_ADDR64 => (K::Absolute, g, 64),
            _ => unknown,
        },
        elf::EM_RISCV => match r_type {
            elf::R_RISCV_32 => (K::Absolute, g, 32),
            elf::R_RISCV_64 => (K::Absolute, g, 64),
            _ => unknown,
        },
        elf::EM_S390 => match r_type {
            elf::R_390_8 => (K::Absolute, g, 8),
            elf::R_390_16 => (K::Absolute, g, 16),
            elf::R_390_32 => (K::Absolute, g, 32),
            elf::R_390_64 => (K::Absolute, g, 64),
            elf::R_390_PC16 => (K::Relative, g, 16),
            elf::R_390_PC32 => (K::Relative, g, 32),
            elf::R_390_PC64 => (K::Relative, g, 64),
            elf::R_390_PC16DBL => (K::Relative, E::S390xDbl, 16),
            elf::R_390_PC32DBL => (K::Relative, E::S390xDbl, 32),
            elf::R_390_PLT16DBL => (K::PltRelative, E::S390xDbl, 16),
            elf::R_390_PLT32DBL => (K::PltRelative, E::S390xDbl, 32),
            elf::R_390_GOT16 => (K::Got, g, 16),
            elf::R_390_GOT32 => (K::Got, g, 32),
            elf::R_390_GOT64 => (K::Got, g, 64),
            elf::R_390_GOTENT => (K::GotRelative, E::S390xDbl, 32),
            elf::R_390_GOTOFF16 => (K::GotBaseOffset, g, 16),
            elf::R_390_GOTOFF32 => (K::GotBaseOffset, g, 32),
            elf::R_390_GOTOFF64 => (K::GotBaseOffset, g, 64),
            elf::R_390_GOTPC => (K::GotBaseRelative, g, 64),
            elf::R_390_GOTPCDBL => (K::GotBaseRelative, E::S390xDbl, 32),
            _ => unknown,
        },
        elf::EM_SBF => match r_type {
            elf::R_SBF_64_64 => (K::Absolute, g, 64),
            elf::R_SBF_64_32 => (K::Absolute, g, 32),
            _ => unknown,
        },
        elf::EM_SHARC => match r_type {
            elf::R_SHARC_ADDR24_V3 => (K::Absolute, E::SharcTypeA, 24),
            elf::R_SHARC_ADDR32_V3 => (K::Absolute, E::SharcTypeA, 32),
            elf::R_SHARC_ADDR_VAR_V3 => (K::Absolute, E::Generic, 32),
            elf::R_SHARC_PCRSHORT_V3 => (K::Relative, E::SharcTypeA, 6),
            elf::R_SHARC_PCRLONG_V3 => (K::Relative, E::SharcTypeA, 24),
            elf::R_SHARC_DATA6_V3 => (K::Absolute, E::SharcTypeA, 6),
            elf::R_SHARC_DATA16_V3 => (K::Absolute, E::SharcTypeA, 16),
            elf::R_SHARC_DATA6_VISA_V3 => (K::Absolute, E::SharcTypeB, 6),
            elf::R_SHARC_DATA7_VISA_V3 => (K::Absolute, E::SharcTypeB, 7),
            elf::R_SHARC_DATA16_VISA_V3 => (K::Absolute, E::SharcTypeB, 16),
            elf::R_SHARC_PCR6_VISA_V3 => (K::Relative, E::SharcTypeB, 16),
            elf::R_SHARC_ADDR_VAR16_V3 => (K::Absolute, E::Generic, 16),
            _ => unknown,
        },
        elf::EM_SPARC | elf::EM_SPARC32PLUS | elf::EM_SPARCV9 => match r_type {
            elf::R_SPARC_32 | elf::R_SPARC_UA32 => (K::Absolute, g, 32),
            elf::R_SPARC_64 | elf::R_SPARC_UA64 => (K::Absolute, g, 64),
            _ => unknown,
        },
        elf::EM_XTENSA => match r_type {
            elf::R_XTENSA_32 => (K::Absolute, g, 32),
            elf::R_XTENSA_32_PCREL => (K::Relative, g, 32),
            _ => unknown,
        },
        _ => unknown,
    };
    let target = match reloc.symbol(endian, is_mips64el) {
        None => RelocationTarget::Absolute,
        Some(symbol) => RelocationTarget::Symbol(symbol),
    };
    Relocation {
        kind,
        encoding,
        size,
        target,
        addend: reloc.r_addend(endian).into(),
        implicit_addend,
        flags,
    }
}

/// A trait for generic access to [`elf::Rel32`] and [`elf::Rel64`].
#[allow(missing_docs)]
pub trait Rel: Debug + Pod + Clone {
    type Word: Into<u64>;
    type Sword: Into<i64>;
    type Endian: endian::Endian;

    fn r_offset(&self, endian: Self::Endian) -> Self::Word;
    fn r_info(&self, endian: Self::Endian) -> Self::Word;
    fn r_sym(&self, endian: Self::Endian) -> u32;
    fn r_type(&self, endian: Self::Endian) -> u32;

    /// Get the symbol index referenced by the relocation.
    ///
    /// Returns `None` for the null symbol index.
    fn symbol(&self, endian: Self::Endian) -> Option<SymbolIndex> {
        let sym = self.r_sym(endian);
        if sym == 0 {
            None
        } else {
            Some(SymbolIndex(sym as usize))
        }
    }
}

impl<Endian: endian::Endian> Rel for elf::Rel32<Endian> {
    type Word = u32;
    type Sword = i32;
    type Endian = Endian;

    #[inline]
    fn r_offset(&self, endian: Self::Endian) -> Self::Word {
        self.r_offset.get(endian)
    }

    #[inline]
    fn r_info(&self, endian: Self::Endian) -> Self::Word {
        self.r_info.get(endian)
    }

    #[inline]
    fn r_sym(&self, endian: Self::Endian) -> u32 {
        self.r_sym(endian)
    }

    #[inline]
    fn r_type(&self, endian: Self::Endian) -> u32 {
        self.r_type(endian)
    }
}

impl<Endian: endian::Endian> Rel for elf::Rel64<Endian> {
    type Word = u64;
    type Sword = i64;
    type Endian = Endian;

    #[inline]
    fn r_offset(&self, endian: Self::Endian) -> Self::Word {
        self.r_offset.get(endian)
    }

    #[inline]
    fn r_info(&self, endian: Self::Endian) -> Self::Word {
        self.r_info.get(endian)
    }

    #[inline]
    fn r_sym(&self, endian: Self::Endian) -> u32 {
        self.r_sym(endian)
    }

    #[inline]
    fn r_type(&self, endian: Self::Endian) -> u32 {
        self.r_type(endian)
    }
}

/// A trait for generic access to [`elf::Rela32`] and [`elf::Rela64`].
#[allow(missing_docs)]
pub trait Rela: Debug + Pod + Clone {
    type Word: Into<u64>;
    type Sword: Into<i64>;
    type Endian: endian::Endian;

    fn r_offset(&self, endian: Self::Endian) -> Self::Word;
    fn r_info(&self, endian: Self::Endian, is_mips64el: bool) -> Self::Word;
    fn r_addend(&self, endian: Self::Endian) -> Self::Sword;
    fn r_sym(&self, endian: Self::Endian, is_mips64el: bool) -> u32;
    fn r_type(&self, endian: Self::Endian, is_mips64el: bool) -> u32;

    /// Get the symbol index referenced by the relocation.
    ///
    /// Returns `None` for the null symbol index.
    fn symbol(&self, endian: Self::Endian, is_mips64el: bool) -> Option<SymbolIndex> {
        let sym = self.r_sym(endian, is_mips64el);
        if sym == 0 {
            None
        } else {
            Some(SymbolIndex(sym as usize))
        }
    }
}

impl<Endian: endian::Endian> Rela for elf::Rela32<Endian> {
    type Word = u32;
    type Sword = i32;
    type Endian = Endian;

    #[inline]
    fn r_offset(&self, endian: Self::Endian) -> Self::Word {
        self.r_offset.get(endian)
    }

    #[inline]
    fn r_info(&self, endian: Self::Endian, _is_mips64el: bool) -> Self::Word {
        self.r_info.get(endian)
    }

    #[inline]
    fn r_addend(&self, endian: Self::Endian) -> Self::Sword {
        self.r_addend.get(endian)
    }

    #[inline]
    fn r_sym(&self, endian: Self::Endian, _is_mips64el: bool) -> u32 {
        self.r_sym(endian)
    }

    #[inline]
    fn r_type(&self, endian: Self::Endian, _is_mips64el: bool) -> u32 {
        self.r_type(endian)
    }
}

impl<Endian: endian::Endian> Rela for elf::Rela64<Endian> {
    type Word = u64;
    type Sword = i64;
    type Endian = Endian;

    #[inline]
    fn r_offset(&self, endian: Self::Endian) -> Self::Word {
        self.r_offset.get(endian)
    }

    #[inline]
    fn r_info(&self, endian: Self::Endian, is_mips64el: bool) -> Self::Word {
        self.get_r_info(endian, is_mips64el)
    }

    #[inline]
    fn r_addend(&self, endian: Self::Endian) -> Self::Sword {
        self.r_addend.get(endian)
    }

    #[inline]
    fn r_sym(&self, endian: Self::Endian, is_mips64el: bool) -> u32 {
        self.r_sym(endian, is_mips64el)
    }

    #[inline]
    fn r_type(&self, endian: Self::Endian, is_mips64el: bool) -> u32 {
        self.r_type(endian, is_mips64el)
    }
}

/// An iterator over the relative relocations in an ELF `SHT_RELR` section.
///
/// Returned by [`SectionHeader::relr`](super::SectionHeader::relr).
#[derive(Debug)]
pub struct RelrIterator<'data, Elf: FileHeader> {
    offset: Elf::Word,
    bits: Elf::Word,
    count: u8,
    iter: slice::Iter<'data, Elf::Relr>,
    endian: Elf::Endian,
}

impl<'data, Elf: FileHeader> RelrIterator<'data, Elf> {
    /// Create a new iterator given the `SHT_RELR` section data.
    pub fn new(endian: Elf::Endian, data: &'data [Elf::Relr]) -> Self {
        RelrIterator {
            offset: Elf::Word::default(),
            bits: Elf::Word::default(),
            count: 0,
            iter: data.iter(),
            endian,
        }
    }
}

impl<'data, Elf: FileHeader> Iterator for RelrIterator<'data, Elf> {
    type Item = Elf::Word;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            while self.count > 0 {
                self.count -= 1;
                let offset = Elf::Relr::next(&mut self.offset, &mut self.bits);
                if offset.is_some() {
                    return offset;
                }
            }
            let next = self.iter.next()?.get(self.endian);
            if next.into() & 1 == 0 {
                self.offset = next;
                return Some(next);
            }
            self.bits = next;
            self.count = Elf::Relr::COUNT;
        }
    }
}

/// A trait for generic access to [`elf::Relr32`] and [`elf::Relr64`].
#[allow(missing_docs)]
pub trait Relr: Debug + Pod + Clone {
    type Word: Into<u64>;
    type Endian: endian::Endian;

    /// The number of bits in the bit mask, excluding the lowest bit.
    const COUNT: u8;

    /// Get the relocation entry.
    ///
    /// This value is an offset if the lowest bit is clear, or a bit mask if the lowest bit is set.
    fn get(&self, endian: Self::Endian) -> Self::Word;

    /// Return the offset corresponding to the next bit in the bit mask.
    ///
    /// Updates the offset and bit mask. This method should be called 31 times
    /// for Relr32 and 63 times for Relr64 to iterate over all the bits.
    ///
    /// Returns `None` if the bit is not set.
    fn next(offset: &mut Self::Word, bits: &mut Self::Word) -> Option<Self::Word>;
}

impl<Endian: endian::Endian> Relr for elf::Relr32<Endian> {
    type Word = u32;
    type Endian = Endian;
    const COUNT: u8 = 31;

    fn get(&self, endian: Self::Endian) -> Self::Word {
        self.0.get(endian)
    }

    fn next(offset: &mut Self::Word, bits: &mut Self::Word) -> Option<Self::Word> {
        *offset += 4;
        *bits >>= 1;
        if *bits & 1 != 0 {
            Some(*offset)
        } else {
            None
        }
    }
}

impl<Endian: endian::Endian> Relr for elf::Relr64<Endian> {
    type Word = u64;
    type Endian = Endian;
    const COUNT: u8 = 63;

    fn get(&self, endian: Self::Endian) -> Self::Word {
        self.0.get(endian)
    }

    fn next(offset: &mut Self::Word, bits: &mut Self::Word) -> Option<Self::Word> {
        *offset += 8;
        *bits >>= 1;
        if *bits & 1 != 0 {
            Some(*offset)
        } else {
            None
        }
    }
}
