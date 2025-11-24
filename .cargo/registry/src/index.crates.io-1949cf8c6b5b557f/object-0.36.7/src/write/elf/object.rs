use alloc::vec::Vec;

use crate::write::elf::writer::*;
use crate::write::string::StringId;
use crate::write::*;
use crate::{elf, pod};

#[derive(Clone, Copy)]
struct ComdatOffsets {
    offset: usize,
    str_id: StringId,
}

#[derive(Clone, Copy)]
struct SectionOffsets {
    index: SectionIndex,
    offset: usize,
    str_id: StringId,
    reloc_offset: usize,
    reloc_str_id: Option<StringId>,
}

#[derive(Default, Clone, Copy)]
struct SymbolOffsets {
    index: SymbolIndex,
    str_id: Option<StringId>,
}

// Public methods.
impl<'a> Object<'a> {
    /// Add a property with a u32 value to the ELF ".note.gnu.property" section.
    ///
    /// Requires `feature = "elf"`.
    pub fn add_elf_gnu_property_u32(&mut self, property: u32, value: u32) {
        if self.format != BinaryFormat::Elf {
            return;
        }

        let align = if self.elf_is_64() { 8 } else { 4 };
        let mut data = Vec::with_capacity(32);
        let n_name = b"GNU\0";
        data.extend_from_slice(pod::bytes_of(&elf::NoteHeader32 {
            n_namesz: U32::new(self.endian, n_name.len() as u32),
            n_descsz: U32::new(self.endian, util::align(3 * 4, align) as u32),
            n_type: U32::new(self.endian, elf::NT_GNU_PROPERTY_TYPE_0),
        }));
        data.extend_from_slice(n_name);
        // This happens to already be aligned correctly.
        debug_assert_eq!(util::align(data.len(), align), data.len());
        data.extend_from_slice(pod::bytes_of(&U32::new(self.endian, property)));
        // Value size
        data.extend_from_slice(pod::bytes_of(&U32::new(self.endian, 4)));
        data.extend_from_slice(pod::bytes_of(&U32::new(self.endian, value)));
        util::write_align(&mut data, align);

        let section = self.section_id(StandardSection::GnuProperty);
        self.append_section_data(section, &data, align as u64);
    }
}

// Private methods.
impl<'a> Object<'a> {
    pub(crate) fn elf_section_info(
        &self,
        section: StandardSection,
    ) -> (&'static [u8], &'static [u8], SectionKind, SectionFlags) {
        match section {
            StandardSection::Text => (&[], &b".text"[..], SectionKind::Text, SectionFlags::None),
            StandardSection::Data => (&[], &b".data"[..], SectionKind::Data, SectionFlags::None),
            StandardSection::ReadOnlyData | StandardSection::ReadOnlyString => (
                &[],
                &b".rodata"[..],
                SectionKind::ReadOnlyData,
                SectionFlags::None,
            ),
            StandardSection::ReadOnlyDataWithRel => (
                &[],
                b".data.rel.ro",
                SectionKind::ReadOnlyDataWithRel,
                SectionFlags::None,
            ),
            StandardSection::UninitializedData => (
                &[],
                &b".bss"[..],
                SectionKind::UninitializedData,
                SectionFlags::None,
            ),
            StandardSection::Tls => (&[], &b".tdata"[..], SectionKind::Tls, SectionFlags::None),
            StandardSection::UninitializedTls => (
                &[],
                &b".tbss"[..],
                SectionKind::UninitializedTls,
                SectionFlags::None,
            ),
            StandardSection::TlsVariables => {
                // Unsupported section.
                (&[], &[], SectionKind::TlsVariables, SectionFlags::None)
            }
            StandardSection::Common => {
                // Unsupported section.
                (&[], &[], SectionKind::Common, SectionFlags::None)
            }
            StandardSection::GnuProperty => (
                &[],
                &b".note.gnu.property"[..],
                SectionKind::Note,
                SectionFlags::Elf {
                    sh_flags: u64::from(elf::SHF_ALLOC),
                },
            ),
        }
    }

    pub(crate) fn elf_subsection_name(&self, section: &[u8], value: &[u8]) -> Vec<u8> {
        let mut name = section.to_vec();
        if !value.is_empty() {
            name.push(b'.');
            name.extend_from_slice(value);
        }
        name
    }

    fn elf_has_relocation_addend(&self) -> Result<bool> {
        Ok(match self.architecture {
            Architecture::Aarch64 => true,
            Architecture::Aarch64_Ilp32 => true,
            Architecture::Arm => false,
            Architecture::Avr => true,
            Architecture::Bpf => false,
            Architecture::Csky => true,
            Architecture::E2K32 => true,
            Architecture::E2K64 => true,
            Architecture::I386 => false,
            Architecture::X86_64 => true,
            Architecture::X86_64_X32 => true,
            Architecture::Hexagon => true,
            Architecture::LoongArch64 => true,
            Architecture::M68k => true,
            Architecture::Mips => false,
            Architecture::Mips64 => true,
            Architecture::Mips64_N32 => true,
            Architecture::Msp430 => true,
            Architecture::PowerPc => true,
            Architecture::PowerPc64 => true,
            Architecture::Riscv64 => true,
            Architecture::Riscv32 => true,
            Architecture::S390x => true,
            Architecture::Sbf => false,
            Architecture::Sharc => true,
            Architecture::Sparc => true,
            Architecture::Sparc32Plus => true,
            Architecture::Sparc64 => true,
            Architecture::Xtensa => true,
            _ => {
                return Err(Error(format!(
                    "unimplemented architecture {:?}",
                    self.architecture
                )));
            }
        })
    }

    pub(crate) fn elf_translate_relocation(&mut self, reloc: &mut Relocation) -> Result<()> {
        use RelocationEncoding as E;
        use RelocationKind as K;

        let (kind, encoding, size) = if let RelocationFlags::Generic {
            kind,
            encoding,
            size,
        } = reloc.flags
        {
            (kind, encoding, size)
        } else {
            return Ok(());
        };

        let unsupported_reloc = || Err(Error(format!("unimplemented ELF relocation {:?}", reloc)));
        let r_type = match self.architecture {
            Architecture::Aarch64 => match (kind, encoding, size) {
                (K::Absolute, E::Generic, 64) => elf::R_AARCH64_ABS64,
                (K::Absolute, E::Generic, 32) => elf::R_AARCH64_ABS32,
                (K::Absolute, E::Generic, 16) => elf::R_AARCH64_ABS16,
                (K::Relative, E::Generic, 64) => elf::R_AARCH64_PREL64,
                (K::Relative, E::Generic, 32) => elf::R_AARCH64_PREL32,
                (K::Relative, E::Generic, 16) => elf::R_AARCH64_PREL16,
                (K::Relative, E::AArch64Call, 26) => elf::R_AARCH64_CALL26,
                (K::PltRelative, E::AArch64Call, 26) => elf::R_AARCH64_CALL26,
                _ => return unsupported_reloc(),
            },
            Architecture::Aarch64_Ilp32 => match (kind, encoding, size) {
                (K::Absolute, E::Generic, 32) => elf::R_AARCH64_P32_ABS32,
                _ => return unsupported_reloc(),
            },
            Architecture::Arm => match (kind, encoding, size) {
                (K::Absolute, _, 32) => elf::R_ARM_ABS32,
                _ => return unsupported_reloc(),
            },
            Architecture::Avr => match (kind, encoding, size) {
                (K::Absolute, _, 32) => elf::R_AVR_32,
                (K::Absolute, _, 16) => elf::R_AVR_16,
                _ => return unsupported_reloc(),
            },
            Architecture::Bpf => match (kind, encoding, size) {
                (K::Absolute, _, 64) => elf::R_BPF_64_64,
                (K::Absolute, _, 32) => elf::R_BPF_64_32,
                _ => return unsupported_reloc(),
            },
            Architecture::Csky => match (kind, encoding, size) {
                (K::Absolute, _, 32) => elf::R_CKCORE_ADDR32,
                (K::Relative, E::Generic, 32) => elf::R_CKCORE_PCREL32,
                _ => return unsupported_reloc(),
            },
            Architecture::I386 => match (kind, size) {
                (K::Absolute, 32) => elf::R_386_32,
                (K::Relative, 32) => elf::R_386_PC32,
                (K::Got, 32) => elf::R_386_GOT32,
                (K::PltRelative, 32) => elf::R_386_PLT32,
                (K::GotBaseOffset, 32) => elf::R_386_GOTOFF,
                (K::GotBaseRelative, 32) => elf::R_386_GOTPC,
                (K::Absolute, 16) => elf::R_386_16,
                (K::Relative, 16) => elf::R_386_PC16,
                (K::Absolute, 8) => elf::R_386_8,
                (K::Relative, 8) => elf::R_386_PC8,
                _ => return unsupported_reloc(),
            },
            Architecture::E2K32 | Architecture::E2K64 => match (kind, encoding, size) {
                (K::Absolute, E::Generic, 32) => elf::R_E2K_32_ABS,
                (K::Absolute, E::E2KLit, 64) => elf::R_E2K_64_ABS_LIT,
                (K::Absolute, E::Generic, 64) => elf::R_E2K_64_ABS,
                (K::Relative, E::E2KDisp, 28) => elf::R_E2K_DISP,
                (K::Got, _, 32) => elf::R_E2K_GOT,
                _ => return unsupported_reloc(),
            },
            Architecture::X86_64 | Architecture::X86_64_X32 => match (kind, encoding, size) {
                (K::Absolute, E::Generic, 64) => elf::R_X86_64_64,
                (K::Relative, E::X86Branch, 32) => elf::R_X86_64_PLT32,
                (K::Relative, _, 32) => elf::R_X86_64_PC32,
                (K::Got, _, 32) => elf::R_X86_64_GOT32,
                (K::PltRelative, _, 32) => elf::R_X86_64_PLT32,
                (K::GotRelative, _, 32) => elf::R_X86_64_GOTPCREL,
                (K::Absolute, E::Generic, 32) => elf::R_X86_64_32,
                (K::Absolute, E::X86Signed, 32) => elf::R_X86_64_32S,
                (K::Absolute, _, 16) => elf::R_X86_64_16,
                (K::Relative, _, 16) => elf::R_X86_64_PC16,
                (K::Absolute, _, 8) => elf::R_X86_64_8,
                (K::Relative, _, 8) => elf::R_X86_64_PC8,
                _ => return unsupported_reloc(),
            },
            Architecture::Hexagon => match (kind, encoding, size) {
                (K::Absolute, _, 32) => elf::R_HEX_32,
                _ => return unsupported_reloc(),
            },
            Architecture::LoongArch64 => match (kind, encoding, size) {
                (K::Absolute, _, 32) => elf::R_LARCH_32,
                (K::Absolute, _, 64) => elf::R_LARCH_64,
                (K::Relative, _, 32) => elf::R_LARCH_32_PCREL,
                (K::Relative, _, 64) => elf::R_LARCH_64_PCREL,
                (K::Relative, E::LoongArchBranch, 16) => elf::R_LARCH_B16,
                (K::PltRelative, E::LoongArchBranch, 16) => elf::R_LARCH_B16,
                (K::Relative, E::LoongArchBranch, 21) => elf::R_LARCH_B21,
                (K::PltRelative, E::LoongArchBranch, 21) => elf::R_LARCH_B21,
                (K::Relative, E::LoongArchBranch, 26) => elf::R_LARCH_B26,
                (K::PltRelative, E::LoongArchBranch, 26) => elf::R_LARCH_B26,
                _ => return unsupported_reloc(),
            },
            Architecture::M68k => match (kind, encoding, size) {
                (K::Absolute, _, 8) => elf::R_68K_8,
                (K::Absolute, _, 16) => elf::R_68K_16,
                (K::Absolute, _, 32) => elf::R_68K_32,
                (K::Relative, _, 8) => elf::R_68K_PC8,
                (K::Relative, _, 16) => elf::R_68K_PC16,
                (K::Relative, _, 32) => elf::R_68K_PC32,
                (K::GotRelative, _, 8) => elf::R_68K_GOT8,
                (K::GotRelative, _, 16) => elf::R_68K_GOT16,
                (K::GotRelative, _, 32) => elf::R_68K_GOT32,
                (K::Got, _, 8) => elf::R_68K_GOT8O,
                (K::Got, _, 16) => elf::R_68K_GOT16O,
                (K::Got, _, 32) => elf::R_68K_GOT32O,
                (K::PltRelative, _, 8) => elf::R_68K_PLT8,
                (K::PltRelative, _, 16) => elf::R_68K_PLT16,
                (K::PltRelative, _, 32) => elf::R_68K_PLT32,
                _ => return unsupported_reloc(),
            },
            Architecture::Mips | Architecture::Mips64 | Architecture::Mips64_N32 => {
                match (kind, encoding, size) {
                    (K::Absolute, _, 16) => elf::R_MIPS_16,
                    (K::Absolute, _, 32) => elf::R_MIPS_32,
                    (K::Absolute, _, 64) => elf::R_MIPS_64,
                    _ => return unsupported_reloc(),
                }
            }
            Architecture::Msp430 => match (kind, encoding, size) {
                (K::Absolute, _, 32) => elf::R_MSP430_32,
                (K::Absolute, _, 16) => elf::R_MSP430_16_BYTE,
                _ => return unsupported_reloc(),
            },
            Architecture::PowerPc => match (kind, encoding, size) {
                (K::Absolute, _, 32) => elf::R_PPC_ADDR32,
                _ => return unsupported_reloc(),
            },
            Architecture::PowerPc64 => match (kind, encoding, size) {
                (K::Absolute, _, 32) => elf::R_PPC64_ADDR32,
                (K::Absolute, _, 64) => elf::R_PPC64_ADDR64,
                _ => return unsupported_reloc(),
            },
            Architecture::Riscv32 | Architecture::Riscv64 => match (kind, encoding, size) {
                (K::Absolute, _, 32) => elf::R_RISCV_32,
                (K::Absolute, _, 64) => elf::R_RISCV_64,
                (K::Relative, E::Generic, 32) => elf::R_RISCV_32_PCREL,
                _ => return unsupported_reloc(),
            },
            Architecture::S390x => match (kind, encoding, size) {
                (K::Absolute, E::Generic, 8) => elf::R_390_8,
                (K::Absolute, E::Generic, 16) => elf::R_390_16,
                (K::Absolute, E::Generic, 32) => elf::R_390_32,
                (K::Absolute, E::Generic, 64) => elf::R_390_64,
                (K::Relative, E::Generic, 16) => elf::R_390_PC16,
                (K::Relative, E::Generic, 32) => elf::R_390_PC32,
                (K::Relative, E::Generic, 64) => elf::R_390_PC64,
                (K::Relative, E::S390xDbl, 16) => elf::R_390_PC16DBL,
                (K::Relative, E::S390xDbl, 32) => elf::R_390_PC32DBL,
                (K::PltRelative, E::S390xDbl, 16) => elf::R_390_PLT16DBL,
                (K::PltRelative, E::S390xDbl, 32) => elf::R_390_PLT32DBL,
                (K::Got, E::Generic, 16) => elf::R_390_GOT16,
                (K::Got, E::Generic, 32) => elf::R_390_GOT32,
                (K::Got, E::Generic, 64) => elf::R_390_GOT64,
                (K::GotRelative, E::S390xDbl, 32) => elf::R_390_GOTENT,
                (K::GotBaseOffset, E::Generic, 16) => elf::R_390_GOTOFF16,
                (K::GotBaseOffset, E::Generic, 32) => elf::R_390_GOTOFF32,
                (K::GotBaseOffset, E::Generic, 64) => elf::R_390_GOTOFF64,
                (K::GotBaseRelative, E::Generic, 64) => elf::R_390_GOTPC,
                (K::GotBaseRelative, E::S390xDbl, 32) => elf::R_390_GOTPCDBL,
                _ => return unsupported_reloc(),
            },
            Architecture::Sbf => match (kind, encoding, size) {
                (K::Absolute, _, 64) => elf::R_SBF_64_64,
                (K::Absolute, _, 32) => elf::R_SBF_64_32,
                _ => return unsupported_reloc(),
            },
            Architecture::Sharc => match (kind, encoding, size) {
                (K::Absolute, E::SharcTypeA, 32) => elf::R_SHARC_ADDR32_V3,
                (K::Absolute, E::Generic, 32) => elf::R_SHARC_ADDR_VAR_V3,
                (K::Relative, E::SharcTypeA, 24) => elf::R_SHARC_PCRLONG_V3,
                (K::Relative, E::SharcTypeA, 6) => elf::R_SHARC_PCRSHORT_V3,
                (K::Relative, E::SharcTypeB, 6) => elf::R_SHARC_PCRSHORT_V3,
                (K::Absolute, E::Generic, 16) => elf::R_SHARC_ADDR_VAR16_V3,
                (K::Absolute, E::SharcTypeA, 16) => elf::R_SHARC_DATA16_V3,
                (K::Absolute, E::SharcTypeB, 16) => elf::R_SHARC_DATA16_VISA_V3,
                (K::Absolute, E::SharcTypeA, 24) => elf::R_SHARC_ADDR24_V3,
                (K::Absolute, E::SharcTypeA, 6) => elf::R_SHARC_DATA6_V3,
                (K::Absolute, E::SharcTypeB, 6) => elf::R_SHARC_DATA6_VISA_V3,
                (K::Absolute, E::SharcTypeB, 7) => elf::R_SHARC_DATA7_VISA_V3,
                _ => return unsupported_reloc(),
            },
            Architecture::Sparc | Architecture::Sparc32Plus => match (kind, encoding, size) {
                // TODO: use R_SPARC_32 if aligned.
                (K::Absolute, _, 32) => elf::R_SPARC_UA32,
                _ => return unsupported_reloc(),
            },
            Architecture::Sparc64 => match (kind, encoding, size) {
                // TODO: use R_SPARC_32/R_SPARC_64 if aligned.
                (K::Absolute, _, 32) => elf::R_SPARC_UA32,
                (K::Absolute, _, 64) => elf::R_SPARC_UA64,
                _ => return unsupported_reloc(),
            },
            Architecture::Xtensa => match (kind, encoding, size) {
                (K::Absolute, _, 32) => elf::R_XTENSA_32,
                (K::Relative, E::Generic, 32) => elf::R_XTENSA_32_PCREL,
                _ => return unsupported_reloc(),
            },
            _ => {
                return Err(Error(format!(
                    "unimplemented architecture {:?}",
                    self.architecture
                )));
            }
        };
        reloc.flags = RelocationFlags::Elf { r_type };
        Ok(())
    }

    pub(crate) fn elf_adjust_addend(&mut self, _relocation: &mut Relocation) -> Result<bool> {
        // Determine whether the addend is stored in the relocation or the data.
        let implicit = !self.elf_has_relocation_addend()?;
        Ok(implicit)
    }

    pub(crate) fn elf_relocation_size(&self, reloc: &Relocation) -> Result<u8> {
        let r_type = if let RelocationFlags::Elf { r_type } = reloc.flags {
            r_type
        } else {
            return Err(Error("invalid relocation flags".into()));
        };
        // This only needs to support architectures that use implicit addends.
        let size = match self.architecture {
            Architecture::Arm => match r_type {
                elf::R_ARM_ABS16 => Some(16),
                elf::R_ARM_ABS32 | elf::R_ARM_REL32 => Some(32),
                _ => None,
            },
            Architecture::Bpf => match r_type {
                elf::R_BPF_64_32 => Some(32),
                elf::R_BPF_64_64 => Some(64),
                _ => None,
            },
            Architecture::I386 => match r_type {
                elf::R_386_8 | elf::R_386_PC8 => Some(8),
                elf::R_386_16 | elf::R_386_PC16 => Some(16),
                elf::R_386_32
                | elf::R_386_PC32
                | elf::R_386_GOT32
                | elf::R_386_PLT32
                | elf::R_386_GOTOFF
                | elf::R_386_GOTPC => Some(32),
                _ => None,
            },
            Architecture::Mips => match r_type {
                elf::R_MIPS_16 => Some(16),
                elf::R_MIPS_32 => Some(32),
                elf::R_MIPS_64 => Some(64),
                _ => None,
            },
            Architecture::Sbf => match r_type {
                elf::R_SBF_64_32 => Some(32),
                elf::R_SBF_64_64 => Some(64),
                _ => None,
            },
            _ => {
                return Err(Error(format!(
                    "unimplemented architecture {:?}",
                    self.architecture
                )));
            }
        };
        size.ok_or_else(|| Error(format!("unsupported relocation for size {:?}", reloc)))
    }

    pub(crate) fn elf_is_64(&self) -> bool {
        match self.architecture.address_size().unwrap() {
            AddressSize::U8 | AddressSize::U16 | AddressSize::U32 => false,
            AddressSize::U64 => true,
        }
    }

    pub(crate) fn elf_write(&self, buffer: &mut dyn WritableBuffer) -> Result<()> {
        // Create reloc section header names so we can reference them.
        let is_rela = self.elf_has_relocation_addend()?;
        let reloc_names: Vec<_> = self
            .sections
            .iter()
            .map(|section| {
                let mut reloc_name = Vec::with_capacity(
                    if is_rela { ".rela".len() } else { ".rel".len() } + section.name.len(),
                );
                if !section.relocations.is_empty() {
                    reloc_name.extend_from_slice(if is_rela {
                        &b".rela"[..]
                    } else {
                        &b".rel"[..]
                    });
                    reloc_name.extend_from_slice(&section.name);
                }
                reloc_name
            })
            .collect();

        // Start calculating offsets of everything.
        let mut writer = Writer::new(self.endian, self.elf_is_64(), buffer);
        writer.reserve_file_header();

        // Calculate size of section data.
        let mut comdat_offsets = Vec::with_capacity(self.comdats.len());
        for comdat in &self.comdats {
            if comdat.kind != ComdatKind::Any {
                return Err(Error(format!(
                    "unsupported COMDAT symbol `{}` kind {:?}",
                    self.symbols[comdat.symbol.0].name().unwrap_or(""),
                    comdat.kind
                )));
            }

            writer.reserve_section_index();
            let offset = writer.reserve_comdat(comdat.sections.len());
            let str_id = writer.add_section_name(b".group");
            comdat_offsets.push(ComdatOffsets { offset, str_id });
        }
        let mut section_offsets = Vec::with_capacity(self.sections.len());
        for (section, reloc_name) in self.sections.iter().zip(reloc_names.iter()) {
            let index = writer.reserve_section_index();
            let offset = writer.reserve(section.data.len(), section.align as usize);
            let str_id = writer.add_section_name(&section.name);
            let mut reloc_str_id = None;
            if !section.relocations.is_empty() {
                writer.reserve_section_index();
                reloc_str_id = Some(writer.add_section_name(reloc_name));
            }
            section_offsets.push(SectionOffsets {
                index,
                offset,
                str_id,
                // Relocation data is reserved later.
                reloc_offset: 0,
                reloc_str_id,
            });
        }

        // Calculate index of symbols and add symbol strings to strtab.
        let mut symbol_offsets = vec![SymbolOffsets::default(); self.symbols.len()];
        writer.reserve_null_symbol_index();
        // Local symbols must come before global.
        for (index, symbol) in self.symbols.iter().enumerate() {
            if symbol.is_local() {
                let section_index = symbol.section.id().map(|s| section_offsets[s.0].index);
                symbol_offsets[index].index = writer.reserve_symbol_index(section_index);
            }
        }
        let symtab_num_local = writer.symbol_count();
        for (index, symbol) in self.symbols.iter().enumerate() {
            if !symbol.is_local() {
                let section_index = symbol.section.id().map(|s| section_offsets[s.0].index);
                symbol_offsets[index].index = writer.reserve_symbol_index(section_index);
            }
        }
        for (index, symbol) in self.symbols.iter().enumerate() {
            if symbol.kind != SymbolKind::Section && !symbol.name.is_empty() {
                symbol_offsets[index].str_id = Some(writer.add_string(&symbol.name));
            }
        }

        // Calculate size of symbols.
        writer.reserve_symtab_section_index();
        writer.reserve_symtab();
        if writer.symtab_shndx_needed() {
            writer.reserve_symtab_shndx_section_index();
        }
        writer.reserve_symtab_shndx();
        writer.reserve_strtab_section_index();
        writer.reserve_strtab();

        // Calculate size of relocations.
        for (index, section) in self.sections.iter().enumerate() {
            let count = section.relocations.len();
            if count != 0 {
                section_offsets[index].reloc_offset = writer.reserve_relocations(count, is_rela);
            }
        }

        // Calculate size of section headers.
        writer.reserve_shstrtab_section_index();
        writer.reserve_shstrtab();
        writer.reserve_section_headers();

        // Start writing.
        let e_type = elf::ET_REL;
        let e_machine = match (self.architecture, self.sub_architecture) {
            (Architecture::Aarch64, None) => elf::EM_AARCH64,
            (Architecture::Aarch64_Ilp32, None) => elf::EM_AARCH64,
            (Architecture::Arm, None) => elf::EM_ARM,
            (Architecture::Avr, None) => elf::EM_AVR,
            (Architecture::Bpf, None) => elf::EM_BPF,
            (Architecture::Csky, None) => elf::EM_CSKY,
            (Architecture::E2K32, None) => elf::EM_MCST_ELBRUS,
            (Architecture::E2K64, None) => elf::EM_MCST_ELBRUS,
            (Architecture::I386, None) => elf::EM_386,
            (Architecture::X86_64, None) => elf::EM_X86_64,
            (Architecture::X86_64_X32, None) => elf::EM_X86_64,
            (Architecture::Hexagon, None) => elf::EM_HEXAGON,
            (Architecture::LoongArch64, None) => elf::EM_LOONGARCH,
            (Architecture::M68k, None) => elf::EM_68K,
            (Architecture::Mips, None) => elf::EM_MIPS,
            (Architecture::Mips64, None) => elf::EM_MIPS,
            (Architecture::Mips64_N32, None) => elf::EM_MIPS,
            (Architecture::Msp430, None) => elf::EM_MSP430,
            (Architecture::PowerPc, None) => elf::EM_PPC,
            (Architecture::PowerPc64, None) => elf::EM_PPC64,
            (Architecture::Riscv32, None) => elf::EM_RISCV,
            (Architecture::Riscv64, None) => elf::EM_RISCV,
            (Architecture::S390x, None) => elf::EM_S390,
            (Architecture::Sbf, None) => elf::EM_SBF,
            (Architecture::Sharc, None) => elf::EM_SHARC,
            (Architecture::Sparc, None) => elf::EM_SPARC,
            (Architecture::Sparc32Plus, None) => elf::EM_SPARC32PLUS,
            (Architecture::Sparc64, None) => elf::EM_SPARCV9,
            (Architecture::Xtensa, None) => elf::EM_XTENSA,
            _ => {
                return Err(Error(format!(
                    "unimplemented architecture {:?} with sub-architecture {:?}",
                    self.architecture, self.sub_architecture
                )));
            }
        };
        let (os_abi, abi_version, mut e_flags) = if let FileFlags::Elf {
            os_abi,
            abi_version,
            e_flags,
        } = self.flags
        {
            (os_abi, abi_version, e_flags)
        } else {
            (elf::ELFOSABI_NONE, 0, 0)
        };

        if self.architecture == Architecture::Mips64_N32 {
            e_flags |= elf::EF_MIPS_ABI2;
        }

        writer.write_file_header(&FileHeader {
            os_abi,
            abi_version,
            e_type,
            e_machine,
            e_entry: 0,
            e_flags,
        })?;

        // Write section data.
        for comdat in &self.comdats {
            writer.write_comdat_header();
            for section in &comdat.sections {
                writer.write_comdat_entry(section_offsets[section.0].index);
            }
        }
        for (index, section) in self.sections.iter().enumerate() {
            writer.write_align(section.align as usize);
            debug_assert_eq!(section_offsets[index].offset, writer.len());
            writer.write(&section.data);
        }

        // Write symbols.
        writer.write_null_symbol();
        let mut write_symbol = |index: usize, symbol: &Symbol| -> Result<()> {
            let st_info = if let SymbolFlags::Elf { st_info, .. } = symbol.flags {
                st_info
            } else {
                let st_type = match symbol.kind {
                    SymbolKind::Text => {
                        if symbol.is_undefined() {
                            elf::STT_NOTYPE
                        } else {
                            elf::STT_FUNC
                        }
                    }
                    SymbolKind::Data => {
                        if symbol.is_undefined() {
                            elf::STT_NOTYPE
                        } else if symbol.is_common() {
                            elf::STT_COMMON
                        } else {
                            elf::STT_OBJECT
                        }
                    }
                    SymbolKind::Section => elf::STT_SECTION,
                    SymbolKind::File => elf::STT_FILE,
                    SymbolKind::Tls => elf::STT_TLS,
                    SymbolKind::Label => elf::STT_NOTYPE,
                    SymbolKind::Unknown => {
                        if symbol.is_undefined() {
                            elf::STT_NOTYPE
                        } else {
                            return Err(Error(format!(
                                "unimplemented symbol `{}` kind {:?}",
                                symbol.name().unwrap_or(""),
                                symbol.kind
                            )));
                        }
                    }
                };
                let st_bind = if symbol.weak {
                    elf::STB_WEAK
                } else if symbol.is_undefined() {
                    elf::STB_GLOBAL
                } else if symbol.is_local() {
                    elf::STB_LOCAL
                } else {
                    elf::STB_GLOBAL
                };
                (st_bind << 4) + st_type
            };
            let st_other = if let SymbolFlags::Elf { st_other, .. } = symbol.flags {
                st_other
            } else if symbol.scope == SymbolScope::Linkage {
                elf::STV_HIDDEN
            } else {
                elf::STV_DEFAULT
            };
            let (st_shndx, section) = match symbol.section {
                SymbolSection::None => {
                    debug_assert_eq!(symbol.kind, SymbolKind::File);
                    (elf::SHN_ABS, None)
                }
                SymbolSection::Undefined => (elf::SHN_UNDEF, None),
                SymbolSection::Absolute => (elf::SHN_ABS, None),
                SymbolSection::Common => (elf::SHN_COMMON, None),
                SymbolSection::Section(id) => (0, Some(section_offsets[id.0].index)),
            };
            writer.write_symbol(&Sym {
                name: symbol_offsets[index].str_id,
                section,
                st_info,
                st_other,
                st_shndx,
                st_value: symbol.value,
                st_size: symbol.size,
            });
            Ok(())
        };
        for (index, symbol) in self.symbols.iter().enumerate() {
            if symbol.is_local() {
                write_symbol(index, symbol)?;
            }
        }
        for (index, symbol) in self.symbols.iter().enumerate() {
            if !symbol.is_local() {
                write_symbol(index, symbol)?;
            }
        }
        writer.write_symtab_shndx();
        writer.write_strtab();

        // Write relocations.
        for (index, section) in self.sections.iter().enumerate() {
            if !section.relocations.is_empty() {
                writer.write_align_relocation();
                debug_assert_eq!(section_offsets[index].reloc_offset, writer.len());
                for reloc in &section.relocations {
                    let r_type = if let RelocationFlags::Elf { r_type } = reloc.flags {
                        r_type
                    } else {
                        return Err(Error("invalid relocation flags".into()));
                    };
                    let r_sym = symbol_offsets[reloc.symbol.0].index.0;
                    writer.write_relocation(
                        is_rela,
                        &Rel {
                            r_offset: reloc.offset,
                            r_sym,
                            r_type,
                            r_addend: reloc.addend,
                        },
                    );
                }
            }
        }

        writer.write_shstrtab();

        // Write section headers.
        writer.write_null_section_header();

        let symtab_index = writer.symtab_index();
        for (comdat, comdat_offset) in self.comdats.iter().zip(comdat_offsets.iter()) {
            writer.write_comdat_section_header(
                comdat_offset.str_id,
                symtab_index,
                symbol_offsets[comdat.symbol.0].index,
                comdat_offset.offset,
                comdat.sections.len(),
            );
        }
        for (index, section) in self.sections.iter().enumerate() {
            let sh_type = match section.kind {
                SectionKind::UninitializedData | SectionKind::UninitializedTls => elf::SHT_NOBITS,
                SectionKind::Note => elf::SHT_NOTE,
                SectionKind::Elf(sh_type) => sh_type,
                _ => elf::SHT_PROGBITS,
            };
            let sh_flags = if let SectionFlags::Elf { sh_flags } = section.flags {
                sh_flags
            } else {
                match section.kind {
                    SectionKind::Text => elf::SHF_ALLOC | elf::SHF_EXECINSTR,
                    SectionKind::Data | SectionKind::ReadOnlyDataWithRel => {
                        elf::SHF_ALLOC | elf::SHF_WRITE
                    }
                    SectionKind::Tls => elf::SHF_ALLOC | elf::SHF_WRITE | elf::SHF_TLS,
                    SectionKind::UninitializedData => elf::SHF_ALLOC | elf::SHF_WRITE,
                    SectionKind::UninitializedTls => elf::SHF_ALLOC | elf::SHF_WRITE | elf::SHF_TLS,
                    SectionKind::ReadOnlyData => elf::SHF_ALLOC,
                    SectionKind::ReadOnlyString => {
                        elf::SHF_ALLOC | elf::SHF_STRINGS | elf::SHF_MERGE
                    }
                    SectionKind::OtherString | SectionKind::DebugString => {
                        elf::SHF_STRINGS | elf::SHF_MERGE
                    }
                    SectionKind::Other
                    | SectionKind::Debug
                    | SectionKind::Metadata
                    | SectionKind::Linker
                    | SectionKind::Note
                    | SectionKind::Elf(_) => 0,
                    SectionKind::Unknown | SectionKind::Common | SectionKind::TlsVariables => {
                        return Err(Error(format!(
                            "unimplemented section `{}` kind {:?}",
                            section.name().unwrap_or(""),
                            section.kind
                        )));
                    }
                }
                .into()
            };
            // TODO: not sure if this is correct, maybe user should determine this
            let sh_entsize = match section.kind {
                SectionKind::ReadOnlyString | SectionKind::OtherString => 1,
                _ => 0,
            };
            writer.write_section_header(&SectionHeader {
                name: Some(section_offsets[index].str_id),
                sh_type,
                sh_flags,
                sh_addr: 0,
                sh_offset: section_offsets[index].offset as u64,
                sh_size: section.size,
                sh_link: 0,
                sh_info: 0,
                sh_addralign: section.align,
                sh_entsize,
            });

            if !section.relocations.is_empty() {
                writer.write_relocation_section_header(
                    section_offsets[index].reloc_str_id.unwrap(),
                    section_offsets[index].index,
                    symtab_index,
                    section_offsets[index].reloc_offset,
                    section.relocations.len(),
                    is_rela,
                );
            }
        }

        writer.write_symtab_section_header(symtab_num_local);
        writer.write_symtab_shndx_section_header();
        writer.write_strtab_section_header();
        writer.write_shstrtab_section_header();

        debug_assert_eq!(writer.reserved_len(), writer.len());

        Ok(())
    }
}
