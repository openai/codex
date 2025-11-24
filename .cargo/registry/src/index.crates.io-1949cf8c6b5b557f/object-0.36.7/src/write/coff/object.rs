use alloc::vec::Vec;

use crate::pe as coff;
use crate::write::coff::writer;
use crate::write::util::*;
use crate::write::*;

#[derive(Default, Clone, Copy)]
struct SectionOffsets {
    name: writer::Name,
    offset: u32,
    reloc_offset: u32,
    selection: u8,
    associative_section: u32,
}

#[derive(Default, Clone, Copy)]
struct SymbolOffsets {
    name: writer::Name,
    index: u32,
    aux_count: u8,
}

/// Internal format to use for the `.drectve` section containing linker
/// directives for symbol exports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoffExportStyle {
    /// MSVC format supported by link.exe and LLD.
    Msvc,
    /// Gnu format supported by GNU LD and LLD.
    Gnu,
}

impl<'a> Object<'a> {
    pub(crate) fn coff_section_info(
        &self,
        section: StandardSection,
    ) -> (&'static [u8], &'static [u8], SectionKind, SectionFlags) {
        match section {
            StandardSection::Text => (&[], &b".text"[..], SectionKind::Text, SectionFlags::None),
            StandardSection::Data => (&[], &b".data"[..], SectionKind::Data, SectionFlags::None),
            StandardSection::ReadOnlyData
            | StandardSection::ReadOnlyDataWithRel
            | StandardSection::ReadOnlyString => (
                &[],
                &b".rdata"[..],
                SectionKind::ReadOnlyData,
                SectionFlags::None,
            ),
            StandardSection::UninitializedData => (
                &[],
                &b".bss"[..],
                SectionKind::UninitializedData,
                SectionFlags::None,
            ),
            // TLS sections are data sections with a special name.
            StandardSection::Tls => (&[], &b".tls$"[..], SectionKind::Data, SectionFlags::None),
            StandardSection::UninitializedTls => {
                // Unsupported section.
                (&[], &[], SectionKind::UninitializedTls, SectionFlags::None)
            }
            StandardSection::TlsVariables => {
                // Unsupported section.
                (&[], &[], SectionKind::TlsVariables, SectionFlags::None)
            }
            StandardSection::Common => {
                // Unsupported section.
                (&[], &[], SectionKind::Common, SectionFlags::None)
            }
            StandardSection::GnuProperty => {
                // Unsupported section.
                (&[], &[], SectionKind::Note, SectionFlags::None)
            }
        }
    }

    pub(crate) fn coff_subsection_name(&self, section: &[u8], value: &[u8]) -> Vec<u8> {
        let mut name = section.to_vec();
        if !value.is_empty() {
            name.push(b'$');
            name.extend_from_slice(value);
        }
        name
    }

    pub(crate) fn coff_translate_relocation(&mut self, reloc: &mut Relocation) -> Result<()> {
        use RelocationEncoding as E;
        use RelocationKind as K;

        let (mut kind, encoding, size) = if let RelocationFlags::Generic {
            kind,
            encoding,
            size,
        } = reloc.flags
        {
            (kind, encoding, size)
        } else {
            return Ok(());
        };
        if kind == K::GotRelative {
            // Use a stub symbol for the relocation instead.
            // This isn't really a GOT, but it's a similar purpose.
            // TODO: need to handle DLL imports differently?
            kind = K::Relative;
            reloc.symbol = self.coff_add_stub_symbol(reloc.symbol)?;
        } else if kind == K::PltRelative {
            // Windows doesn't need a separate relocation type for
            // references to functions in import libraries.
            // For convenience, treat this the same as Relative.
            kind = K::Relative;
        }

        let unsupported_reloc = || Err(Error(format!("unimplemented relocation {:?}", reloc)));
        let typ = match self.architecture {
            Architecture::I386 => match (kind, size) {
                (K::Absolute, 16) => coff::IMAGE_REL_I386_DIR16,
                (K::Relative, 16) => coff::IMAGE_REL_I386_REL16,
                (K::Absolute, 32) => coff::IMAGE_REL_I386_DIR32,
                (K::ImageOffset, 32) => coff::IMAGE_REL_I386_DIR32NB,
                (K::SectionIndex, 16) => coff::IMAGE_REL_I386_SECTION,
                (K::SectionOffset, 32) => coff::IMAGE_REL_I386_SECREL,
                (K::SectionOffset, 7) => coff::IMAGE_REL_I386_SECREL7,
                (K::Relative, 32) => coff::IMAGE_REL_I386_REL32,
                _ => return unsupported_reloc(),
            },
            Architecture::X86_64 => match (kind, size) {
                (K::Absolute, 64) => coff::IMAGE_REL_AMD64_ADDR64,
                (K::Absolute, 32) => coff::IMAGE_REL_AMD64_ADDR32,
                (K::ImageOffset, 32) => coff::IMAGE_REL_AMD64_ADDR32NB,
                (K::Relative, 32) => match reloc.addend {
                    -5 => coff::IMAGE_REL_AMD64_REL32_1,
                    -6 => coff::IMAGE_REL_AMD64_REL32_2,
                    -7 => coff::IMAGE_REL_AMD64_REL32_3,
                    -8 => coff::IMAGE_REL_AMD64_REL32_4,
                    -9 => coff::IMAGE_REL_AMD64_REL32_5,
                    _ => coff::IMAGE_REL_AMD64_REL32,
                },
                (K::SectionIndex, 16) => coff::IMAGE_REL_AMD64_SECTION,
                (K::SectionOffset, 32) => coff::IMAGE_REL_AMD64_SECREL,
                (K::SectionOffset, 7) => coff::IMAGE_REL_AMD64_SECREL7,
                _ => return unsupported_reloc(),
            },
            Architecture::Arm => match (kind, size) {
                (K::Absolute, 32) => coff::IMAGE_REL_ARM_ADDR32,
                (K::ImageOffset, 32) => coff::IMAGE_REL_ARM_ADDR32NB,
                (K::Relative, 32) => coff::IMAGE_REL_ARM_REL32,
                (K::SectionIndex, 16) => coff::IMAGE_REL_ARM_SECTION,
                (K::SectionOffset, 32) => coff::IMAGE_REL_ARM_SECREL,
                _ => return unsupported_reloc(),
            },
            Architecture::Aarch64 => match (kind, encoding, size) {
                (K::Absolute, _, 32) => coff::IMAGE_REL_ARM64_ADDR32,
                (K::ImageOffset, _, 32) => coff::IMAGE_REL_ARM64_ADDR32NB,
                (K::SectionIndex, _, 16) => coff::IMAGE_REL_ARM64_SECTION,
                (K::SectionOffset, _, 32) => coff::IMAGE_REL_ARM64_SECREL,
                (K::Absolute, _, 64) => coff::IMAGE_REL_ARM64_ADDR64,
                (K::Relative, _, 32) => coff::IMAGE_REL_ARM64_REL32,
                (K::Relative, E::AArch64Call, 26) => coff::IMAGE_REL_ARM64_BRANCH26,
                _ => return unsupported_reloc(),
            },
            _ => {
                return Err(Error(format!(
                    "unimplemented architecture {:?}",
                    self.architecture
                )));
            }
        };
        reloc.flags = RelocationFlags::Coff { typ };
        Ok(())
    }

    pub(crate) fn coff_adjust_addend(&self, relocation: &mut Relocation) -> Result<bool> {
        let typ = if let RelocationFlags::Coff { typ } = relocation.flags {
            typ
        } else {
            return Err(Error(format!("invalid relocation flags {:?}", relocation)));
        };
        let offset = match self.architecture {
            Architecture::Arm => {
                if typ == coff::IMAGE_REL_ARM_REL32 {
                    4
                } else {
                    0
                }
            }
            Architecture::Aarch64 => {
                if typ == coff::IMAGE_REL_ARM64_REL32 {
                    4
                } else {
                    0
                }
            }
            Architecture::I386 => {
                if typ == coff::IMAGE_REL_I386_REL32 {
                    4
                } else {
                    0
                }
            }
            Architecture::X86_64 => match typ {
                coff::IMAGE_REL_AMD64_REL32 => 4,
                coff::IMAGE_REL_AMD64_REL32_1 => 5,
                coff::IMAGE_REL_AMD64_REL32_2 => 6,
                coff::IMAGE_REL_AMD64_REL32_3 => 7,
                coff::IMAGE_REL_AMD64_REL32_4 => 8,
                coff::IMAGE_REL_AMD64_REL32_5 => 9,
                _ => 0,
            },
            _ => return Err(Error(format!("unimplemented relocation {:?}", relocation))),
        };
        relocation.addend += offset;
        Ok(true)
    }

    pub(crate) fn coff_relocation_size(&self, reloc: &Relocation) -> Result<u8> {
        let typ = if let RelocationFlags::Coff { typ } = reloc.flags {
            typ
        } else {
            return Err(Error(format!("unexpected relocation for size {:?}", reloc)));
        };
        let size = match self.architecture {
            Architecture::I386 => match typ {
                coff::IMAGE_REL_I386_DIR16
                | coff::IMAGE_REL_I386_REL16
                | coff::IMAGE_REL_I386_SECTION => Some(16),
                coff::IMAGE_REL_I386_DIR32
                | coff::IMAGE_REL_I386_DIR32NB
                | coff::IMAGE_REL_I386_SECREL
                | coff::IMAGE_REL_I386_TOKEN
                | coff::IMAGE_REL_I386_REL32 => Some(32),
                _ => None,
            },
            Architecture::X86_64 => match typ {
                coff::IMAGE_REL_AMD64_SECTION => Some(16),
                coff::IMAGE_REL_AMD64_ADDR32
                | coff::IMAGE_REL_AMD64_ADDR32NB
                | coff::IMAGE_REL_AMD64_REL32
                | coff::IMAGE_REL_AMD64_REL32_1
                | coff::IMAGE_REL_AMD64_REL32_2
                | coff::IMAGE_REL_AMD64_REL32_3
                | coff::IMAGE_REL_AMD64_REL32_4
                | coff::IMAGE_REL_AMD64_REL32_5
                | coff::IMAGE_REL_AMD64_SECREL
                | coff::IMAGE_REL_AMD64_TOKEN => Some(32),
                coff::IMAGE_REL_AMD64_ADDR64 => Some(64),
                _ => None,
            },
            Architecture::Arm => match typ {
                coff::IMAGE_REL_ARM_SECTION => Some(16),
                coff::IMAGE_REL_ARM_ADDR32
                | coff::IMAGE_REL_ARM_ADDR32NB
                | coff::IMAGE_REL_ARM_TOKEN
                | coff::IMAGE_REL_ARM_REL32
                | coff::IMAGE_REL_ARM_SECREL => Some(32),
                _ => None,
            },
            Architecture::Aarch64 => match typ {
                coff::IMAGE_REL_ARM64_SECTION => Some(16),
                coff::IMAGE_REL_ARM64_ADDR32
                | coff::IMAGE_REL_ARM64_ADDR32NB
                | coff::IMAGE_REL_ARM64_SECREL
                | coff::IMAGE_REL_ARM64_TOKEN
                | coff::IMAGE_REL_ARM64_REL32 => Some(32),
                coff::IMAGE_REL_ARM64_ADDR64 => Some(64),
                _ => None,
            },
            _ => None,
        };
        size.ok_or_else(|| Error(format!("unsupported relocation for size {:?}", reloc)))
    }

    fn coff_add_stub_symbol(&mut self, symbol_id: SymbolId) -> Result<SymbolId> {
        if let Some(stub_id) = self.stub_symbols.get(&symbol_id) {
            return Ok(*stub_id);
        }
        let stub_size = self.architecture.address_size().unwrap().bytes();

        let name = b".rdata$.refptr".to_vec();
        let section_id = self.add_section(Vec::new(), name, SectionKind::ReadOnlyData);
        let section = self.section_mut(section_id);
        section.set_data(vec![0; stub_size as usize], u64::from(stub_size));
        self.add_relocation(
            section_id,
            Relocation {
                offset: 0,
                symbol: symbol_id,
                addend: 0,
                flags: RelocationFlags::Generic {
                    kind: RelocationKind::Absolute,
                    encoding: RelocationEncoding::Generic,
                    size: stub_size * 8,
                },
            },
        )?;

        let mut name = b".refptr.".to_vec();
        name.extend_from_slice(&self.symbol(symbol_id).name);
        let stub_id = self.add_raw_symbol(Symbol {
            name,
            value: 0,
            size: u64::from(stub_size),
            kind: SymbolKind::Data,
            scope: SymbolScope::Compilation,
            weak: false,
            section: SymbolSection::Section(section_id),
            flags: SymbolFlags::None,
        });
        self.stub_symbols.insert(symbol_id, stub_id);

        Ok(stub_id)
    }

    /// Appends linker directives to the `.drectve` section to tell the linker
    /// to export all symbols with `SymbolScope::Dynamic`.
    ///
    /// This must be called after all symbols have been defined.
    pub fn add_coff_exports(&mut self, style: CoffExportStyle) {
        assert_eq!(self.format, BinaryFormat::Coff);

        let mut directives = vec![];
        for symbol in &self.symbols {
            if symbol.scope == SymbolScope::Dynamic {
                match style {
                    CoffExportStyle::Msvc => directives.extend(b" /EXPORT:\""),
                    CoffExportStyle::Gnu => directives.extend(b" -export:\""),
                }
                directives.extend(&symbol.name);
                directives.extend(b"\"");
                if symbol.kind != SymbolKind::Text {
                    match style {
                        CoffExportStyle::Msvc => directives.extend(b",DATA"),
                        CoffExportStyle::Gnu => directives.extend(b",data"),
                    }
                }
            }
        }
        let drectve = self.add_section(vec![], b".drectve".to_vec(), SectionKind::Linker);
        self.append_section_data(drectve, &directives, 1);
    }

    pub(crate) fn coff_write(&self, buffer: &mut dyn WritableBuffer) -> Result<()> {
        let mut writer = writer::Writer::new(buffer);

        // Add section strings to strtab.
        let mut section_offsets = vec![SectionOffsets::default(); self.sections.len()];
        for (index, section) in self.sections.iter().enumerate() {
            section_offsets[index].name = writer.add_name(&section.name);
        }

        // Set COMDAT flags.
        for comdat in &self.comdats {
            let symbol = &self.symbols[comdat.symbol.0];
            let comdat_section = match symbol.section {
                SymbolSection::Section(id) => id.0,
                _ => {
                    return Err(Error(format!(
                        "unsupported COMDAT symbol `{}` section {:?}",
                        symbol.name().unwrap_or(""),
                        symbol.section
                    )));
                }
            };
            section_offsets[comdat_section].selection = match comdat.kind {
                ComdatKind::NoDuplicates => coff::IMAGE_COMDAT_SELECT_NODUPLICATES,
                ComdatKind::Any => coff::IMAGE_COMDAT_SELECT_ANY,
                ComdatKind::SameSize => coff::IMAGE_COMDAT_SELECT_SAME_SIZE,
                ComdatKind::ExactMatch => coff::IMAGE_COMDAT_SELECT_EXACT_MATCH,
                ComdatKind::Largest => coff::IMAGE_COMDAT_SELECT_LARGEST,
                ComdatKind::Newest => coff::IMAGE_COMDAT_SELECT_NEWEST,
                ComdatKind::Unknown => {
                    return Err(Error(format!(
                        "unsupported COMDAT symbol `{}` kind {:?}",
                        symbol.name().unwrap_or(""),
                        comdat.kind
                    )));
                }
            };
            for id in &comdat.sections {
                let section = &self.sections[id.0];
                if section.symbol.is_none() {
                    return Err(Error(format!(
                        "missing symbol for COMDAT section `{}`",
                        section.name().unwrap_or(""),
                    )));
                }
                if id.0 != comdat_section {
                    section_offsets[id.0].selection = coff::IMAGE_COMDAT_SELECT_ASSOCIATIVE;
                    section_offsets[id.0].associative_section = comdat_section as u32 + 1;
                }
            }
        }

        // Reserve symbol indices and add symbol strings to strtab.
        let mut symbol_offsets = vec![SymbolOffsets::default(); self.symbols.len()];
        for (index, symbol) in self.symbols.iter().enumerate() {
            symbol_offsets[index].index = writer.reserve_symbol_index();
            let mut name = &*symbol.name;
            match symbol.kind {
                SymbolKind::File => {
                    // Name goes in auxiliary symbol records.
                    symbol_offsets[index].aux_count = writer.reserve_aux_file_name(&symbol.name);
                    name = b".file";
                }
                SymbolKind::Section if symbol.section.id().is_some() => {
                    symbol_offsets[index].aux_count = writer.reserve_aux_section();
                }
                _ => {}
            };
            symbol_offsets[index].name = writer.add_name(name);
        }

        // Reserve file ranges.
        writer.reserve_file_header();
        writer.reserve_section_headers(self.sections.len() as u16);
        for (index, section) in self.sections.iter().enumerate() {
            section_offsets[index].offset = writer.reserve_section(section.data.len());
            section_offsets[index].reloc_offset =
                writer.reserve_relocations(section.relocations.len());
        }
        writer.reserve_symtab_strtab();

        // Start writing.
        writer.write_file_header(writer::FileHeader {
            machine: match (self.architecture, self.sub_architecture) {
                (Architecture::Arm, None) => coff::IMAGE_FILE_MACHINE_ARMNT,
                (Architecture::Aarch64, None) => coff::IMAGE_FILE_MACHINE_ARM64,
                (Architecture::Aarch64, Some(SubArchitecture::Arm64EC)) => {
                    coff::IMAGE_FILE_MACHINE_ARM64EC
                }
                (Architecture::I386, None) => coff::IMAGE_FILE_MACHINE_I386,
                (Architecture::X86_64, None) => coff::IMAGE_FILE_MACHINE_AMD64,
                _ => {
                    return Err(Error(format!(
                        "unimplemented architecture {:?} with sub-architecture {:?}",
                        self.architecture, self.sub_architecture
                    )));
                }
            },
            time_date_stamp: 0,
            characteristics: match self.flags {
                FileFlags::Coff { characteristics } => characteristics,
                _ => 0,
            },
        })?;

        // Write section headers.
        for (index, section) in self.sections.iter().enumerate() {
            let mut characteristics = if let SectionFlags::Coff {
                characteristics, ..
            } = section.flags
            {
                characteristics
            } else {
                match section.kind {
                    SectionKind::Text => {
                        coff::IMAGE_SCN_CNT_CODE
                            | coff::IMAGE_SCN_MEM_EXECUTE
                            | coff::IMAGE_SCN_MEM_READ
                    }
                    SectionKind::Data => {
                        coff::IMAGE_SCN_CNT_INITIALIZED_DATA
                            | coff::IMAGE_SCN_MEM_READ
                            | coff::IMAGE_SCN_MEM_WRITE
                    }
                    SectionKind::UninitializedData => {
                        coff::IMAGE_SCN_CNT_UNINITIALIZED_DATA
                            | coff::IMAGE_SCN_MEM_READ
                            | coff::IMAGE_SCN_MEM_WRITE
                    }
                    SectionKind::ReadOnlyData
                    | SectionKind::ReadOnlyDataWithRel
                    | SectionKind::ReadOnlyString => {
                        coff::IMAGE_SCN_CNT_INITIALIZED_DATA | coff::IMAGE_SCN_MEM_READ
                    }
                    SectionKind::Debug
                    | SectionKind::DebugString
                    | SectionKind::Other
                    | SectionKind::OtherString => {
                        coff::IMAGE_SCN_CNT_INITIALIZED_DATA
                            | coff::IMAGE_SCN_MEM_READ
                            | coff::IMAGE_SCN_MEM_DISCARDABLE
                    }
                    SectionKind::Linker => coff::IMAGE_SCN_LNK_INFO | coff::IMAGE_SCN_LNK_REMOVE,
                    SectionKind::Common
                    | SectionKind::Tls
                    | SectionKind::UninitializedTls
                    | SectionKind::TlsVariables
                    | SectionKind::Note
                    | SectionKind::Unknown
                    | SectionKind::Metadata
                    | SectionKind::Elf(_) => {
                        return Err(Error(format!(
                            "unimplemented section `{}` kind {:?}",
                            section.name().unwrap_or(""),
                            section.kind
                        )));
                    }
                }
            };
            if section_offsets[index].selection != 0 {
                characteristics |= coff::IMAGE_SCN_LNK_COMDAT;
            };
            if section.relocations.len() > 0xffff {
                characteristics |= coff::IMAGE_SCN_LNK_NRELOC_OVFL;
            }
            characteristics |= match section.align {
                1 => coff::IMAGE_SCN_ALIGN_1BYTES,
                2 => coff::IMAGE_SCN_ALIGN_2BYTES,
                4 => coff::IMAGE_SCN_ALIGN_4BYTES,
                8 => coff::IMAGE_SCN_ALIGN_8BYTES,
                16 => coff::IMAGE_SCN_ALIGN_16BYTES,
                32 => coff::IMAGE_SCN_ALIGN_32BYTES,
                64 => coff::IMAGE_SCN_ALIGN_64BYTES,
                128 => coff::IMAGE_SCN_ALIGN_128BYTES,
                256 => coff::IMAGE_SCN_ALIGN_256BYTES,
                512 => coff::IMAGE_SCN_ALIGN_512BYTES,
                1024 => coff::IMAGE_SCN_ALIGN_1024BYTES,
                2048 => coff::IMAGE_SCN_ALIGN_2048BYTES,
                4096 => coff::IMAGE_SCN_ALIGN_4096BYTES,
                8192 => coff::IMAGE_SCN_ALIGN_8192BYTES,
                _ => {
                    return Err(Error(format!(
                        "unimplemented section `{}` align {}",
                        section.name().unwrap_or(""),
                        section.align
                    )));
                }
            };
            writer.write_section_header(writer::SectionHeader {
                name: section_offsets[index].name,
                size_of_raw_data: section.size as u32,
                pointer_to_raw_data: section_offsets[index].offset,
                pointer_to_relocations: section_offsets[index].reloc_offset,
                pointer_to_linenumbers: 0,
                number_of_relocations: section.relocations.len() as u32,
                number_of_linenumbers: 0,
                characteristics,
            });
        }

        // Write section data and relocations.
        for section in &self.sections {
            writer.write_section(&section.data);

            if !section.relocations.is_empty() {
                //debug_assert_eq!(section_offsets[index].reloc_offset, buffer.len());
                writer.write_relocations_count(section.relocations.len());
                for reloc in &section.relocations {
                    let typ = if let RelocationFlags::Coff { typ } = reloc.flags {
                        typ
                    } else {
                        return Err(Error("invalid relocation flags".into()));
                    };
                    writer.write_relocation(writer::Relocation {
                        virtual_address: reloc.offset as u32,
                        symbol: symbol_offsets[reloc.symbol.0].index,
                        typ,
                    });
                }
            }
        }

        // Write symbols.
        for (index, symbol) in self.symbols.iter().enumerate() {
            let section_number = match symbol.section {
                SymbolSection::None => {
                    debug_assert_eq!(symbol.kind, SymbolKind::File);
                    coff::IMAGE_SYM_DEBUG as u16
                }
                SymbolSection::Undefined => coff::IMAGE_SYM_UNDEFINED as u16,
                SymbolSection::Absolute => coff::IMAGE_SYM_ABSOLUTE as u16,
                SymbolSection::Common => coff::IMAGE_SYM_UNDEFINED as u16,
                SymbolSection::Section(id) => id.0 as u16 + 1,
            };
            let typ = if symbol.kind == SymbolKind::Text {
                coff::IMAGE_SYM_DTYPE_FUNCTION << coff::IMAGE_SYM_DTYPE_SHIFT
            } else {
                coff::IMAGE_SYM_TYPE_NULL
            };
            let storage_class = match symbol.kind {
                SymbolKind::File => coff::IMAGE_SYM_CLASS_FILE,
                SymbolKind::Section => {
                    if symbol.section.id().is_some() {
                        coff::IMAGE_SYM_CLASS_STATIC
                    } else {
                        coff::IMAGE_SYM_CLASS_SECTION
                    }
                }
                SymbolKind::Label => coff::IMAGE_SYM_CLASS_LABEL,
                SymbolKind::Text | SymbolKind::Data | SymbolKind::Tls => {
                    match symbol.section {
                        SymbolSection::None => {
                            return Err(Error(format!(
                                "missing section for symbol `{}`",
                                symbol.name().unwrap_or("")
                            )));
                        }
                        SymbolSection::Undefined | SymbolSection::Common => {
                            coff::IMAGE_SYM_CLASS_EXTERNAL
                        }
                        SymbolSection::Absolute | SymbolSection::Section(_) => {
                            match symbol.scope {
                                // TODO: does this need aux symbol records too?
                                _ if symbol.weak => coff::IMAGE_SYM_CLASS_WEAK_EXTERNAL,
                                SymbolScope::Unknown => {
                                    return Err(Error(format!(
                                        "unimplemented symbol `{}` scope {:?}",
                                        symbol.name().unwrap_or(""),
                                        symbol.scope
                                    )));
                                }
                                SymbolScope::Compilation => coff::IMAGE_SYM_CLASS_STATIC,
                                SymbolScope::Linkage | SymbolScope::Dynamic => {
                                    coff::IMAGE_SYM_CLASS_EXTERNAL
                                }
                            }
                        }
                    }
                }
                SymbolKind::Unknown => {
                    return Err(Error(format!(
                        "unimplemented symbol `{}` kind {:?}",
                        symbol.name().unwrap_or(""),
                        symbol.kind
                    )));
                }
            };
            let number_of_aux_symbols = symbol_offsets[index].aux_count;
            let value = if symbol.section == SymbolSection::Common {
                symbol.size as u32
            } else {
                symbol.value as u32
            };
            writer.write_symbol(writer::Symbol {
                name: symbol_offsets[index].name,
                value,
                section_number,
                typ,
                storage_class,
                number_of_aux_symbols,
            });

            // Write auxiliary symbols.
            match symbol.kind {
                SymbolKind::File => {
                    writer.write_aux_file_name(&symbol.name, number_of_aux_symbols);
                }
                SymbolKind::Section if symbol.section.id().is_some() => {
                    debug_assert_eq!(number_of_aux_symbols, 1);
                    let section_index = symbol.section.id().unwrap().0;
                    let section = &self.sections[section_index];
                    writer.write_aux_section(writer::AuxSymbolSection {
                        length: section.size as u32,
                        number_of_relocations: section.relocations.len() as u32,
                        number_of_linenumbers: 0,
                        check_sum: if section.is_bss() {
                            0
                        } else {
                            checksum(section.data())
                        },
                        number: section_offsets[section_index].associative_section,
                        selection: section_offsets[section_index].selection,
                    });
                }
                _ => {
                    debug_assert_eq!(number_of_aux_symbols, 0);
                }
            }
        }

        writer.write_strtab();

        debug_assert_eq!(writer.reserved_len(), writer.len());

        Ok(())
    }
}

// JamCRC
fn checksum(data: &[u8]) -> u32 {
    let mut hasher = crc32fast::Hasher::new_with_initial(0xffff_ffff);
    hasher.update(data);
    !hasher.finalize()
}
