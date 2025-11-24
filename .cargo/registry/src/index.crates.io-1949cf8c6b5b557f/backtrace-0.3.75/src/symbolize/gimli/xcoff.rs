use super::mystd::ffi::OsStr;
use super::mystd::os::unix::ffi::OsStrExt;
use super::mystd::path::Path;
use super::{gimli, Context, Endian, EndianSlice, Mapping, Stash};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::Deref;
use core::str;
use object::read::archive::ArchiveFile;
use object::read::xcoff::{FileHeader, SectionHeader, XcoffFile, XcoffSymbol};
use object::Object as _;
use object::ObjectSection as _;
use object::ObjectSymbol as _;
use object::SymbolFlags;

#[cfg(target_pointer_width = "32")]
type Xcoff = object::xcoff::FileHeader32;
#[cfg(target_pointer_width = "64")]
type Xcoff = object::xcoff::FileHeader64;

impl Mapping {
    pub fn new(path: &Path, member_name: &OsStr) -> Option<Mapping> {
        let map = super::mmap(path)?;
        Mapping::mk(map, |data, stash| {
            if member_name.is_empty() {
                Context::new(stash, Object::parse(data)?, None, None)
            } else {
                let archive = ArchiveFile::parse(data).ok()?;
                for member in archive
                    .members()
                    .filter_map(|m| m.ok())
                    .filter(|m| OsStr::from_bytes(m.name()) == member_name)
                {
                    let member_data = member.data(data).ok()?;
                    if let Some(obj) = Object::parse(member_data) {
                        return Context::new(stash, obj, None, None);
                    }
                }
                None
            }
        })
    }
}

struct ParsedSym<'a> {
    address: u64,
    size: u64,
    name: &'a str,
}

pub struct Object<'a> {
    syms: Vec<ParsedSym<'a>>,
    file: XcoffFile<'a, Xcoff>,
}

pub struct Image {
    pub offset: usize,
    pub base: u64,
    pub size: usize,
}

pub fn parse_xcoff(data: &[u8]) -> Option<Image> {
    let mut offset = 0;
    let header = Xcoff::parse(data, &mut offset).ok()?;
    let _ = header.aux_header(data, &mut offset).ok()?;
    let sections = header.sections(data, &mut offset).ok()?;
    if let Some(section) = sections.iter().find(|s| {
        if let Ok(name) = str::from_utf8(&s.s_name()[0..5]) {
            name == ".text"
        } else {
            false
        }
    }) {
        Some(Image {
            offset: section.s_scnptr() as usize,
            base: section.s_paddr() as u64,
            size: section.s_size() as usize,
        })
    } else {
        None
    }
}

pub fn parse_image(path: &Path, member_name: &OsStr) -> Option<Image> {
    let map = super::mmap(path)?;
    let data = map.deref();
    if member_name.is_empty() {
        return parse_xcoff(data);
    } else {
        let archive = ArchiveFile::parse(data).ok()?;
        for member in archive
            .members()
            .filter_map(|m| m.ok())
            .filter(|m| OsStr::from_bytes(m.name()) == member_name)
        {
            let member_data = member.data(data).ok()?;
            if let Some(image) = parse_xcoff(member_data) {
                return Some(image);
            }
        }
        None
    }
}

impl<'a> Object<'a> {
    fn get_concrete_size(file: &XcoffFile<'a, Xcoff>, sym: &XcoffSymbol<'a, '_, Xcoff>) -> u64 {
        match sym.flags() {
            SymbolFlags::Xcoff {
                n_sclass: _,
                x_smtyp: _,
                x_smclas: _,
                containing_csect: Some(index),
            } => {
                if let Ok(tgt_sym) = file.symbol_by_index(index) {
                    Self::get_concrete_size(file, &tgt_sym)
                } else {
                    0
                }
            }
            _ => sym.size(),
        }
    }

    fn parse(data: &'a [u8]) -> Option<Object<'a>> {
        let file = XcoffFile::parse(data).ok()?;
        let mut syms = file
            .symbols()
            .filter_map(|sym| {
                let name = sym.name().map_or("", |v| v);
                let address = sym.address();
                let size = Self::get_concrete_size(&file, &sym);
                if name == ".text" || name == ".data" {
                    // We don't want to include ".text" and ".data" symbols.
                    // If they are included, since their ranges cover other
                    // symbols, when searching a symbol for a given address,
                    // ".text" or ".data" is returned. That's not what we expect.
                    None
                } else {
                    Some(ParsedSym {
                        address,
                        size,
                        name,
                    })
                }
            })
            .collect::<Vec<_>>();
        syms.sort_by_key(|s| s.address);
        Some(Object { syms, file })
    }

    pub fn section(&self, _: &Stash, name: &str) -> Option<&'a [u8]> {
        Some(self.file.section_by_name(name)?.data().ok()?)
    }

    pub fn search_symtab<'b>(&'b self, addr: u64) -> Option<&'b [u8]> {
        // Symbols, except ".text" and ".data", are sorted and are not overlapped each other,
        // so we can just perform a binary search here.
        let i = match self.syms.binary_search_by_key(&addr, |sym| sym.address) {
            Ok(i) => i,
            Err(i) => i.checked_sub(1)?,
        };
        let sym = self.syms.get(i)?;
        if (sym.address..sym.address + sym.size).contains(&addr) {
            // On AIX, for a function call, for example, `foo()`, we have
            // two symbols `foo` and `.foo`. `foo` references the function
            // descriptor and `.foo` references the function entry.
            // See https://www.ibm.com/docs/en/xl-fortran-aix/16.1.0?topic=calls-linkage-convention-function
            // for more information.
            // We trim the prefix `.` here, so that the rust demangler can work
            // properly.
            Some(sym.name.trim_start_matches(".").as_bytes())
        } else {
            None
        }
    }

    pub(super) fn search_object_map(&self, _addr: u64) -> Option<(&Context<'_>, u64)> {
        None
    }
}

pub(super) fn handle_split_dwarf<'data>(
    _package: Option<&gimli::DwarfPackage<EndianSlice<'data, Endian>>>,
    _stash: &'data Stash,
    _load: addr2line::SplitDwarfLoad<EndianSlice<'data, Endian>>,
) -> Option<Arc<gimli::Dwarf<EndianSlice<'data, Endian>>>> {
    None
}
