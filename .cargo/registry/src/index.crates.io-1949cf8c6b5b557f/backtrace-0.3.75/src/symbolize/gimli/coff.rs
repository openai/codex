use super::mystd::path::Path;
use super::{gimli, Context, Endian, EndianSlice, Mapping, Stash};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::convert::TryFrom;
use object::pe::{ImageDosHeader, ImageSymbol};
use object::read::coff::ImageSymbol as _;
use object::read::pe::{ImageNtHeaders, ImageOptionalHeader, SectionTable};
use object::read::StringTable;
use object::LittleEndian as LE;

#[cfg(target_pointer_width = "32")]
type Pe = object::pe::ImageNtHeaders32;
#[cfg(target_pointer_width = "64")]
type Pe = object::pe::ImageNtHeaders64;

impl Mapping {
    pub fn new(path: &Path) -> Option<Mapping> {
        let map = super::mmap(path)?;
        Mapping::mk(map, |data, stash| {
            Context::new(stash, Object::parse(data)?, None, None)
        })
    }
}

pub struct Object<'a> {
    data: &'a [u8],
    sections: SectionTable<'a>,
    symbols: Vec<(usize, &'a ImageSymbol)>,
    strings: StringTable<'a>,
}

pub fn get_image_base(data: &[u8]) -> Option<usize> {
    let dos_header = ImageDosHeader::parse(data).ok()?;
    let mut offset = dos_header.nt_headers_offset().into();
    let (nt_headers, _) = Pe::parse(data, &mut offset).ok()?;
    usize::try_from(nt_headers.optional_header().image_base()).ok()
}

impl<'a> Object<'a> {
    fn parse(data: &'a [u8]) -> Option<Object<'a>> {
        let dos_header = ImageDosHeader::parse(data).ok()?;
        let mut offset = dos_header.nt_headers_offset().into();
        let (nt_headers, _) = Pe::parse(data, &mut offset).ok()?;
        let sections = nt_headers.sections(data, offset).ok()?;
        let symtab = nt_headers.symbols(data).ok()?;
        let strings = symtab.strings();
        let image_base = usize::try_from(nt_headers.optional_header().image_base()).ok()?;

        // Collect all the symbols into a local vector which is sorted
        // by address and contains enough data to learn about the symbol
        // name. Note that we only look at function symbols and also
        // note that the sections are 1-indexed because the zero section
        // is special (apparently).
        let mut symbols = Vec::new();
        for (_, sym) in symtab.iter() {
            if sym.derived_type() != object::pe::IMAGE_SYM_DTYPE_FUNCTION {
                continue;
            }
            let Some(section_index) = sym.section() else {
                continue;
            };
            let addr = usize::try_from(sym.value.get(LE)).ok()?;
            let section = sections.section(section_index).ok()?;
            let va = usize::try_from(section.virtual_address.get(LE)).ok()?;
            symbols.push((addr + va + image_base, sym));
        }
        symbols.sort_unstable_by_key(|x| x.0);
        Some(Object {
            data,
            sections,
            strings,
            symbols,
        })
    }

    pub fn section(&self, _: &Stash, name: &str) -> Option<&'a [u8]> {
        Some(
            self.sections
                .section_by_name(self.strings, name.as_bytes())?
                .1
                .pe_data(self.data)
                .ok()?,
        )
    }

    pub fn search_symtab<'b>(&'b self, addr: u64) -> Option<&'b [u8]> {
        // Note that unlike other formats COFF doesn't embed the size of
        // each symbol. As a last ditch effort search for the *closest*
        // symbol to a particular address and return that one. This gets
        // really wonky once symbols start getting removed because the
        // symbols returned here can be totally incorrect, but we have
        // no idea of knowing how to detect that.
        let addr = usize::try_from(addr).ok()?;
        let i = match self.symbols.binary_search_by_key(&addr, |p| p.0) {
            Ok(i) => i,
            // typically `addr` isn't in the array, but `i` is where
            // we'd insert it, so the previous position must be the
            // greatest less than `addr`
            Err(i) => i.checked_sub(1)?,
        };
        self.symbols[i].1.name(self.strings).ok()
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
