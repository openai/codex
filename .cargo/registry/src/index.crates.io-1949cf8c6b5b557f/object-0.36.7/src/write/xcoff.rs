use core::mem;

use crate::endian::{BigEndian as BE, I16, U16, U32};
use crate::write::string::*;
use crate::write::util::*;
use crate::write::*;

use crate::xcoff;

#[derive(Default, Clone, Copy)]
struct SectionOffsets {
    address: u64,
    data_offset: usize,
    reloc_offset: usize,
}

#[derive(Default, Clone, Copy)]
struct SymbolOffsets {
    index: usize,
    str_id: Option<StringId>,
    aux_count: u8,
    storage_class: u8,
}

impl<'a> Object<'a> {
    pub(crate) fn xcoff_section_info(
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
            StandardSection::GnuProperty => {
                // Unsupported section.
                (&[], &[], SectionKind::Note, SectionFlags::None)
            }
        }
    }

    pub(crate) fn xcoff_translate_relocation(&mut self, reloc: &mut Relocation) -> Result<()> {
        let (kind, _encoding, size) = if let RelocationFlags::Generic {
            kind,
            encoding,
            size,
        } = reloc.flags
        {
            (kind, encoding, size)
        } else {
            return Ok(());
        };

        let r_rtype = match kind {
            RelocationKind::Absolute => xcoff::R_POS,
            RelocationKind::Relative => xcoff::R_REL,
            RelocationKind::Got => xcoff::R_TOC,
            _ => {
                return Err(Error(format!("unimplemented relocation {:?}", reloc)));
            }
        };
        let r_rsize = size - 1;
        reloc.flags = RelocationFlags::Xcoff { r_rtype, r_rsize };
        Ok(())
    }

    pub(crate) fn xcoff_adjust_addend(&mut self, relocation: &mut Relocation) -> Result<bool> {
        let r_rtype = if let RelocationFlags::Xcoff { r_rtype, .. } = relocation.flags {
            r_rtype
        } else {
            return Err(Error(format!("invalid relocation flags {:?}", relocation)));
        };
        if r_rtype == xcoff::R_REL {
            relocation.addend += 4;
        }
        Ok(true)
    }

    pub(crate) fn xcoff_relocation_size(&self, reloc: &Relocation) -> Result<u8> {
        let r_rsize = if let RelocationFlags::Xcoff { r_rsize, .. } = reloc.flags {
            r_rsize
        } else {
            return Err(Error(format!("unexpected relocation {:?}", reloc)));
        };
        Ok(r_rsize + 1)
    }

    pub(crate) fn xcoff_write(&self, buffer: &mut dyn WritableBuffer) -> Result<()> {
        let is_64 = match self.architecture.address_size().unwrap() {
            AddressSize::U8 | AddressSize::U16 | AddressSize::U32 => false,
            AddressSize::U64 => true,
        };

        let (hdr_size, sechdr_size, rel_size, sym_size) = if is_64 {
            (
                mem::size_of::<xcoff::FileHeader64>(),
                mem::size_of::<xcoff::SectionHeader64>(),
                mem::size_of::<xcoff::Rel64>(),
                mem::size_of::<xcoff::Symbol64>(),
            )
        } else {
            (
                mem::size_of::<xcoff::FileHeader32>(),
                mem::size_of::<xcoff::SectionHeader32>(),
                mem::size_of::<xcoff::Rel32>(),
                mem::size_of::<xcoff::Symbol32>(),
            )
        };

        // Calculate offsets and build strtab.
        let mut offset = 0;
        let mut strtab = StringTable::default();
        // We place the shared address 0 immediately after the section header table.
        let mut address = 0;

        // XCOFF file header.
        offset += hdr_size;
        // Section headers.
        offset += self.sections.len() * sechdr_size;

        // Calculate size of section data.
        let mut section_offsets = vec![SectionOffsets::default(); self.sections.len()];
        for (index, section) in self.sections.iter().enumerate() {
            let len = section.data.len();
            let sectype = section.kind;
            // Section address should be 0 for all sections except the .text, .data, and .bss sections.
            if sectype == SectionKind::Data
                || sectype == SectionKind::Text
                || sectype == SectionKind::UninitializedData
            {
                section_offsets[index].address = address as u64;
                address += len;
                address = align(address, 4);
            } else {
                section_offsets[index].address = 0;
            }
            if len != 0 {
                // Set the default section alignment as 4.
                offset = align(offset, 4);
                section_offsets[index].data_offset = offset;
                offset += len;
            } else {
                section_offsets[index].data_offset = 0;
            }
        }

        // Calculate size of relocations.
        for (index, section) in self.sections.iter().enumerate() {
            let count = section.relocations.len();
            if count != 0 {
                section_offsets[index].reloc_offset = offset;
                offset += count * rel_size;
            } else {
                section_offsets[index].reloc_offset = 0;
            }
        }

        // Calculate size of symbols.
        let mut file_str_id = None;
        let mut symbol_offsets = vec![SymbolOffsets::default(); self.symbols.len()];
        let mut symtab_count = 0;
        for (index, symbol) in self.symbols.iter().enumerate() {
            symbol_offsets[index].index = symtab_count;
            symtab_count += 1;

            let storage_class = if let SymbolFlags::Xcoff { n_sclass, .. } = symbol.flags {
                n_sclass
            } else {
                match symbol.kind {
                    SymbolKind::File => xcoff::C_FILE,
                    SymbolKind::Text | SymbolKind::Data | SymbolKind::Tls => {
                        if symbol.is_local() {
                            xcoff::C_STAT
                        } else if symbol.weak {
                            xcoff::C_WEAKEXT
                        } else {
                            xcoff::C_EXT
                        }
                    }
                    SymbolKind::Section | SymbolKind::Label | SymbolKind::Unknown => {
                        return Err(Error(format!(
                            "unimplemented symbol `{}` kind {:?}",
                            symbol.name().unwrap_or(""),
                            symbol.kind
                        )));
                    }
                }
            };
            symbol_offsets[index].storage_class = storage_class;

            if storage_class == xcoff::C_FILE {
                if is_64 && file_str_id.is_none() {
                    file_str_id = Some(strtab.add(b".file"));
                }
                if symbol.name.len() > 8 {
                    symbol_offsets[index].str_id = Some(strtab.add(&symbol.name));
                }
            } else if is_64 || symbol.name.len() > 8 {
                symbol_offsets[index].str_id = Some(strtab.add(&symbol.name));
            }

            symbol_offsets[index].aux_count = 0;
            match storage_class {
                xcoff::C_FILE => {
                    symbol_offsets[index].aux_count = 1;
                    symtab_count += 1;
                }
                xcoff::C_EXT | xcoff::C_WEAKEXT | xcoff::C_HIDEXT => {
                    symbol_offsets[index].aux_count = 1;
                    symtab_count += 1;
                }
                // TODO: support auxiliary entry for other types of symbol.
                _ => {}
            }
        }
        let symtab_offset = offset;
        let symtab_len = symtab_count * sym_size;
        offset += symtab_len;

        // Calculate size of strtab.
        let strtab_offset = offset;
        let mut strtab_data = Vec::new();
        // First 4 bytes of strtab are the length.
        strtab.write(4, &mut strtab_data);
        let strtab_len = strtab_data.len() + 4;
        offset += strtab_len;

        // Start writing.
        buffer
            .reserve(offset)
            .map_err(|_| Error(String::from("Cannot allocate buffer")))?;

        // Write file header.
        if is_64 {
            let header = xcoff::FileHeader64 {
                f_magic: U16::new(BE, xcoff::MAGIC_64),
                f_nscns: U16::new(BE, self.sections.len() as u16),
                f_timdat: U32::new(BE, 0),
                f_symptr: U64::new(BE, symtab_offset as u64),
                f_nsyms: U32::new(BE, symtab_count as u32),
                f_opthdr: U16::new(BE, 0),
                f_flags: match self.flags {
                    FileFlags::Xcoff { f_flags } => U16::new(BE, f_flags),
                    _ => U16::default(),
                },
            };
            buffer.write(&header);
        } else {
            let header = xcoff::FileHeader32 {
                f_magic: U16::new(BE, xcoff::MAGIC_32),
                f_nscns: U16::new(BE, self.sections.len() as u16),
                f_timdat: U32::new(BE, 0),
                f_symptr: U32::new(BE, symtab_offset as u32),
                f_nsyms: U32::new(BE, symtab_count as u32),
                f_opthdr: U16::new(BE, 0),
                f_flags: match self.flags {
                    FileFlags::Xcoff { f_flags } => U16::new(BE, f_flags),
                    _ => U16::default(),
                },
            };
            buffer.write(&header);
        }

        // Write section headers.
        for (index, section) in self.sections.iter().enumerate() {
            let mut sectname = [0; 8];
            sectname
                .get_mut(..section.name.len())
                .ok_or_else(|| {
                    Error(format!(
                        "section name `{}` is too long",
                        section.name().unwrap_or(""),
                    ))
                })?
                .copy_from_slice(&section.name);
            let flags = if let SectionFlags::Xcoff { s_flags } = section.flags {
                s_flags
            } else {
                match section.kind {
                    SectionKind::Text
                    | SectionKind::ReadOnlyData
                    | SectionKind::ReadOnlyString
                    | SectionKind::ReadOnlyDataWithRel => xcoff::STYP_TEXT,
                    SectionKind::Data => xcoff::STYP_DATA,
                    SectionKind::UninitializedData => xcoff::STYP_BSS,
                    SectionKind::Tls => xcoff::STYP_TDATA,
                    SectionKind::UninitializedTls => xcoff::STYP_TBSS,
                    SectionKind::OtherString => xcoff::STYP_INFO,
                    SectionKind::Debug | SectionKind::DebugString => xcoff::STYP_DEBUG,
                    SectionKind::Other | SectionKind::Metadata => 0,
                    SectionKind::Note
                    | SectionKind::Linker
                    | SectionKind::Common
                    | SectionKind::Unknown
                    | SectionKind::TlsVariables
                    | SectionKind::Elf(_) => {
                        return Err(Error(format!(
                            "unimplemented section `{}` kind {:?}",
                            section.name().unwrap_or(""),
                            section.kind
                        )));
                    }
                }
                .into()
            };
            if is_64 {
                let section_header = xcoff::SectionHeader64 {
                    s_name: sectname,
                    s_paddr: U64::new(BE, section_offsets[index].address),
                    // This field has the same value as the s_paddr field.
                    s_vaddr: U64::new(BE, section_offsets[index].address),
                    s_size: U64::new(BE, section.data.len() as u64),
                    s_scnptr: U64::new(BE, section_offsets[index].data_offset as u64),
                    s_relptr: U64::new(BE, section_offsets[index].reloc_offset as u64),
                    s_lnnoptr: U64::new(BE, 0),
                    s_nreloc: U32::new(BE, section.relocations.len() as u32),
                    s_nlnno: U32::new(BE, 0),
                    s_flags: U32::new(BE, flags),
                    s_reserve: U32::new(BE, 0),
                };
                buffer.write(&section_header);
            } else {
                let section_header = xcoff::SectionHeader32 {
                    s_name: sectname,
                    s_paddr: U32::new(BE, section_offsets[index].address as u32),
                    // This field has the same value as the s_paddr field.
                    s_vaddr: U32::new(BE, section_offsets[index].address as u32),
                    s_size: U32::new(BE, section.data.len() as u32),
                    s_scnptr: U32::new(BE, section_offsets[index].data_offset as u32),
                    s_relptr: U32::new(BE, section_offsets[index].reloc_offset as u32),
                    s_lnnoptr: U32::new(BE, 0),
                    // TODO: If more than 65,534 relocation entries are required, the field
                    // value will be 65535, and an STYP_OVRFLO section header will contain
                    // the actual count of relocation entries in the s_paddr field.
                    s_nreloc: U16::new(BE, section.relocations.len() as u16),
                    s_nlnno: U16::new(BE, 0),
                    s_flags: U32::new(BE, flags),
                };
                buffer.write(&section_header);
            }
        }

        // Write section data.
        for (index, section) in self.sections.iter().enumerate() {
            let len = section.data.len();
            if len != 0 {
                write_align(buffer, 4);
                debug_assert_eq!(section_offsets[index].data_offset, buffer.len());
                buffer.write_bytes(&section.data);
            }
        }

        // Write relocations.
        for (index, section) in self.sections.iter().enumerate() {
            if !section.relocations.is_empty() {
                debug_assert_eq!(section_offsets[index].reloc_offset, buffer.len());
                for reloc in &section.relocations {
                    let (r_rtype, r_rsize) =
                        if let RelocationFlags::Xcoff { r_rtype, r_rsize } = reloc.flags {
                            (r_rtype, r_rsize)
                        } else {
                            return Err(Error("invalid relocation flags".into()));
                        };
                    if is_64 {
                        let xcoff_rel = xcoff::Rel64 {
                            r_vaddr: U64::new(BE, reloc.offset),
                            r_symndx: U32::new(BE, symbol_offsets[reloc.symbol.0].index as u32),
                            r_rsize,
                            r_rtype,
                        };
                        buffer.write(&xcoff_rel);
                    } else {
                        let xcoff_rel = xcoff::Rel32 {
                            r_vaddr: U32::new(BE, reloc.offset as u32),
                            r_symndx: U32::new(BE, symbol_offsets[reloc.symbol.0].index as u32),
                            r_rsize,
                            r_rtype,
                        };
                        buffer.write(&xcoff_rel);
                    }
                }
            }
        }

        // Write symbols.
        debug_assert_eq!(symtab_offset, buffer.len());
        for (index, symbol) in self.symbols.iter().enumerate() {
            let (n_value, section_kind) = if let SymbolSection::Section(id) = symbol.section {
                (
                    section_offsets[id.0].address + symbol.value,
                    self.sections[id.0].kind,
                )
            } else {
                (symbol.value, SectionKind::Unknown)
            };
            let n_scnum = match symbol.section {
                SymbolSection::None => {
                    debug_assert_eq!(symbol.kind, SymbolKind::File);
                    xcoff::N_DEBUG
                }
                SymbolSection::Undefined | SymbolSection::Common => xcoff::N_UNDEF,
                SymbolSection::Absolute => xcoff::N_ABS,
                SymbolSection::Section(id) => id.0 as i16 + 1,
            };
            let n_sclass = symbol_offsets[index].storage_class;
            let n_type = if (symbol.scope == SymbolScope::Linkage)
                && (n_sclass == xcoff::C_EXT
                    || n_sclass == xcoff::C_WEAKEXT
                    || n_sclass == xcoff::C_HIDEXT)
            {
                xcoff::SYM_V_HIDDEN
            } else {
                0
            };
            let n_numaux = symbol_offsets[index].aux_count;
            if is_64 {
                let str_id = if n_sclass == xcoff::C_FILE {
                    file_str_id.unwrap()
                } else {
                    symbol_offsets[index].str_id.unwrap()
                };
                let xcoff_sym = xcoff::Symbol64 {
                    n_value: U64::new(BE, n_value),
                    n_offset: U32::new(BE, strtab.get_offset(str_id) as u32),
                    n_scnum: I16::new(BE, n_scnum),
                    n_type: U16::new(BE, n_type),
                    n_sclass,
                    n_numaux,
                };
                buffer.write(&xcoff_sym);
            } else {
                let mut sym_name = [0; 8];
                if n_sclass == xcoff::C_FILE {
                    sym_name[..5].copy_from_slice(b".file");
                } else if symbol.name.len() <= 8 {
                    sym_name[..symbol.name.len()].copy_from_slice(&symbol.name[..]);
                } else {
                    let str_offset = strtab.get_offset(symbol_offsets[index].str_id.unwrap());
                    sym_name[4..8].copy_from_slice(&u32::to_be_bytes(str_offset as u32));
                }
                let xcoff_sym = xcoff::Symbol32 {
                    n_name: sym_name,
                    n_value: U32::new(BE, n_value as u32),
                    n_scnum: I16::new(BE, n_scnum),
                    n_type: U16::new(BE, n_type),
                    n_sclass,
                    n_numaux,
                };
                buffer.write(&xcoff_sym);
            }
            // Generate auxiliary entries.
            if n_sclass == xcoff::C_FILE {
                debug_assert_eq!(n_numaux, 1);
                let mut x_fname = [0; 8];
                if symbol.name.len() <= 8 {
                    x_fname[..symbol.name.len()].copy_from_slice(&symbol.name[..]);
                } else {
                    let str_offset = strtab.get_offset(symbol_offsets[index].str_id.unwrap());
                    x_fname[4..8].copy_from_slice(&u32::to_be_bytes(str_offset as u32));
                }
                if is_64 {
                    let file_aux = xcoff::FileAux64 {
                        x_fname,
                        x_fpad: Default::default(),
                        x_ftype: xcoff::XFT_FN,
                        x_freserve: Default::default(),
                        x_auxtype: xcoff::AUX_FILE,
                    };
                    buffer.write(&file_aux);
                } else {
                    let file_aux = xcoff::FileAux32 {
                        x_fname,
                        x_fpad: Default::default(),
                        x_ftype: xcoff::XFT_FN,
                        x_freserve: Default::default(),
                    };
                    buffer.write(&file_aux);
                }
            } else if n_sclass == xcoff::C_EXT
                || n_sclass == xcoff::C_WEAKEXT
                || n_sclass == xcoff::C_HIDEXT
            {
                debug_assert_eq!(n_numaux, 1);
                let (x_smtyp, x_smclas) = if let SymbolFlags::Xcoff {
                    x_smtyp, x_smclas, ..
                } = symbol.flags
                {
                    (x_smtyp, x_smclas)
                } else {
                    match symbol.kind {
                        SymbolKind::Text => (xcoff::XTY_SD, xcoff::XMC_PR),
                        SymbolKind::Data => {
                            if section_kind == SectionKind::UninitializedData {
                                (xcoff::XTY_CM, xcoff::XMC_BS)
                            } else if section_kind == SectionKind::ReadOnlyData {
                                (xcoff::XTY_SD, xcoff::XMC_RO)
                            } else {
                                (xcoff::XTY_SD, xcoff::XMC_RW)
                            }
                        }
                        SymbolKind::Tls => {
                            if section_kind == SectionKind::UninitializedTls {
                                (xcoff::XTY_CM, xcoff::XMC_UL)
                            } else {
                                (xcoff::XTY_SD, xcoff::XMC_TL)
                            }
                        }
                        _ => {
                            return Err(Error(format!(
                                "unimplemented symbol `{}` kind {:?}",
                                symbol.name().unwrap_or(""),
                                symbol.kind
                            )));
                        }
                    }
                };
                let scnlen = if let SymbolFlags::Xcoff {
                    containing_csect: Some(containing_csect),
                    ..
                } = symbol.flags
                {
                    symbol_offsets[containing_csect.0].index as u64
                } else {
                    symbol.size
                };
                if is_64 {
                    let csect_aux = xcoff::CsectAux64 {
                        x_scnlen_lo: U32::new(BE, (scnlen & 0xFFFFFFFF) as u32),
                        x_scnlen_hi: U32::new(BE, ((scnlen >> 32) & 0xFFFFFFFF) as u32),
                        x_parmhash: U32::new(BE, 0),
                        x_snhash: U16::new(BE, 0),
                        x_smtyp,
                        x_smclas,
                        pad: 0,
                        x_auxtype: xcoff::AUX_CSECT,
                    };
                    buffer.write(&csect_aux);
                } else {
                    let csect_aux = xcoff::CsectAux32 {
                        x_scnlen: U32::new(BE, scnlen as u32),
                        x_parmhash: U32::new(BE, 0),
                        x_snhash: U16::new(BE, 0),
                        x_smtyp,
                        x_smclas,
                        x_stab: U32::new(BE, 0),
                        x_snstab: U16::new(BE, 0),
                    };
                    buffer.write(&csect_aux);
                }
            }
        }

        // Write string table.
        debug_assert_eq!(strtab_offset, buffer.len());
        buffer.write_bytes(&u32::to_be_bytes(strtab_len as u32));
        buffer.write_bytes(&strtab_data);

        debug_assert_eq!(offset, buffer.len());
        Ok(())
    }
}
