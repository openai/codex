use alloc::borrow::Cow;
use alloc::vec::Vec;

use crate::endian::Endianness;
use crate::read::{
    self, Architecture, CodeView, ComdatKind, CompressedData, CompressedFileRange, Export,
    FileFlags, Import, ObjectKind, ObjectMap, Relocation, RelocationMap, Result, SectionFlags,
    SectionIndex, SectionKind, SegmentFlags, SubArchitecture, SymbolFlags, SymbolIndex, SymbolKind,
    SymbolMap, SymbolMapName, SymbolScope, SymbolSection,
};

/// An object file.
///
/// This is the primary trait for the unified read API.
pub trait Object<'data>: read::private::Sealed {
    /// A loadable segment in the object file.
    type Segment<'file>: ObjectSegment<'data>
    where
        Self: 'file,
        'data: 'file;

    /// An iterator for the loadable segments in the object file.
    type SegmentIterator<'file>: Iterator<Item = Self::Segment<'file>>
    where
        Self: 'file,
        'data: 'file;

    /// A section in the object file.
    type Section<'file>: ObjectSection<'data>
    where
        Self: 'file,
        'data: 'file;

    /// An iterator for the sections in the object file.
    type SectionIterator<'file>: Iterator<Item = Self::Section<'file>>
    where
        Self: 'file,
        'data: 'file;

    /// A COMDAT section group in the object file.
    type Comdat<'file>: ObjectComdat<'data>
    where
        Self: 'file,
        'data: 'file;

    /// An iterator for the COMDAT section groups in the object file.
    type ComdatIterator<'file>: Iterator<Item = Self::Comdat<'file>>
    where
        Self: 'file,
        'data: 'file;

    /// A symbol in the object file.
    type Symbol<'file>: ObjectSymbol<'data>
    where
        Self: 'file,
        'data: 'file;

    /// An iterator for symbols in the object file.
    type SymbolIterator<'file>: Iterator<Item = Self::Symbol<'file>>
    where
        Self: 'file,
        'data: 'file;

    /// A symbol table in the object file.
    type SymbolTable<'file>: ObjectSymbolTable<
        'data,
        Symbol = Self::Symbol<'file>,
        SymbolIterator = Self::SymbolIterator<'file>,
    >
    where
        Self: 'file,
        'data: 'file;

    /// An iterator for the dynamic relocations in the file.
    ///
    /// The first field in the item tuple is the address
    /// that the relocation applies to.
    type DynamicRelocationIterator<'file>: Iterator<Item = (u64, Relocation)>
    where
        Self: 'file,
        'data: 'file;

    /// Get the architecture type of the file.
    fn architecture(&self) -> Architecture;

    /// Get the sub-architecture type of the file if known.
    ///
    /// A value of `None` has a range of meanings: the file supports all
    /// sub-architectures, the file does not explicitly specify a
    /// sub-architecture, or the sub-architecture is currently unrecognized.
    fn sub_architecture(&self) -> Option<SubArchitecture> {
        None
    }

    /// Get the endianness of the file.
    #[inline]
    fn endianness(&self) -> Endianness {
        if self.is_little_endian() {
            Endianness::Little
        } else {
            Endianness::Big
        }
    }

    /// Return true if the file is little endian, false if it is big endian.
    fn is_little_endian(&self) -> bool;

    /// Return true if the file can contain 64-bit addresses.
    fn is_64(&self) -> bool;

    /// Return the kind of this object.
    fn kind(&self) -> ObjectKind;

    /// Get an iterator for the loadable segments in the file.
    ///
    /// For ELF, this is program headers with type [`PT_LOAD`](crate::elf::PT_LOAD).
    /// For Mach-O, this is load commands with type [`LC_SEGMENT`](crate::macho::LC_SEGMENT)
    /// or [`LC_SEGMENT_64`](crate::macho::LC_SEGMENT_64).
    /// For PE, this is all sections.
    fn segments(&self) -> Self::SegmentIterator<'_>;

    /// Get the section named `section_name`, if such a section exists.
    ///
    /// If `section_name` starts with a '.' then it is treated as a system
    /// section name, and is compared using the conventions specific to the
    /// object file format. This includes:
    /// - if ".debug_str_offsets" is requested for a Mach-O object file, then
    ///   the actual section name that is searched for is "__debug_str_offs".
    /// - if ".debug_info" is requested for an ELF object file, then
    ///   ".zdebug_info" may be returned (and similarly for other debug
    ///   sections). Similarly, if ".debug_info" is requested for a Mach-O
    ///   object file, then "__zdebug_info" may be returned.
    ///
    /// For some object files, multiple segments may contain sections with the
    /// same name. In this case, the first matching section will be used.
    ///
    /// This method skips over sections with invalid names.
    fn section_by_name(&self, section_name: &str) -> Option<Self::Section<'_>> {
        self.section_by_name_bytes(section_name.as_bytes())
    }

    /// Like [`Self::section_by_name`], but allows names that are not UTF-8.
    fn section_by_name_bytes<'file>(
        &'file self,
        section_name: &[u8],
    ) -> Option<Self::Section<'file>>;

    /// Get the section at the given index.
    ///
    /// The meaning of the index depends on the object file.
    ///
    /// For some object files, this requires iterating through all sections.
    ///
    /// Returns an error if the index is invalid.
    fn section_by_index(&self, index: SectionIndex) -> Result<Self::Section<'_>>;

    /// Get an iterator for the sections in the file.
    fn sections(&self) -> Self::SectionIterator<'_>;

    /// Get an iterator for the COMDAT section groups in the file.
    fn comdats(&self) -> Self::ComdatIterator<'_>;

    /// Get the debugging symbol table, if any.
    fn symbol_table(&self) -> Option<Self::SymbolTable<'_>>;

    /// Get the debugging symbol at the given index.
    ///
    /// The meaning of the index depends on the object file.
    ///
    /// Returns an error if the index is invalid.
    fn symbol_by_index(&self, index: SymbolIndex) -> Result<Self::Symbol<'_>>;

    /// Get an iterator for the debugging symbols in the file.
    ///
    /// This may skip over symbols that are malformed or unsupported.
    ///
    /// For Mach-O files, this does not include STAB entries.
    fn symbols(&self) -> Self::SymbolIterator<'_>;

    /// Get the symbol named `symbol_name`, if the symbol exists.
    fn symbol_by_name<'file>(&'file self, symbol_name: &str) -> Option<Self::Symbol<'file>> {
        self.symbol_by_name_bytes(symbol_name.as_bytes())
    }

    /// Like [`Self::symbol_by_name`], but allows names that are not UTF-8.
    fn symbol_by_name_bytes<'file>(&'file self, symbol_name: &[u8]) -> Option<Self::Symbol<'file>> {
        self.symbols()
            .find(|sym| sym.name_bytes() == Ok(symbol_name))
    }

    /// Get the dynamic linking symbol table, if any.
    ///
    /// Only ELF has a separate dynamic linking symbol table.
    /// Consider using [`Self::exports`] or [`Self::imports`] instead.
    fn dynamic_symbol_table(&self) -> Option<Self::SymbolTable<'_>>;

    /// Get an iterator for the dynamic linking symbols in the file.
    ///
    /// This may skip over symbols that are malformed or unsupported.
    ///
    /// Only ELF has dynamic linking symbols.
    /// Other file formats will return an empty iterator.
    /// Consider using [`Self::exports`] or [`Self::imports`] instead.
    fn dynamic_symbols(&self) -> Self::SymbolIterator<'_>;

    /// Get the dynamic relocations for this file.
    ///
    /// Symbol indices in these relocations refer to the dynamic symbol table.
    ///
    /// Only ELF has dynamic relocations.
    fn dynamic_relocations(&self) -> Option<Self::DynamicRelocationIterator<'_>>;

    /// Construct a map from addresses to symbol names.
    ///
    /// The map will only contain defined text and data symbols.
    /// The dynamic symbol table will only be used if there are no debugging symbols.
    fn symbol_map(&self) -> SymbolMap<SymbolMapName<'data>> {
        let mut symbols = Vec::new();
        if let Some(table) = self.symbol_table().or_else(|| self.dynamic_symbol_table()) {
            // Sometimes symbols share addresses. Collect them all then choose the "best".
            let mut all_symbols = Vec::new();
            for symbol in table.symbols() {
                // Must have an address.
                if !symbol.is_definition() {
                    continue;
                }
                // Must have a name.
                let name = match symbol.name() {
                    Ok(name) => name,
                    _ => continue,
                };
                if name.is_empty() {
                    continue;
                }

                // Lower is better.
                let mut priority = 0u32;

                // Prefer known kind.
                match symbol.kind() {
                    SymbolKind::Text | SymbolKind::Data => {}
                    SymbolKind::Unknown => priority += 1,
                    _ => continue,
                }
                priority *= 2;

                // Prefer global visibility.
                priority += match symbol.scope() {
                    SymbolScope::Unknown => 3,
                    SymbolScope::Compilation => 2,
                    SymbolScope::Linkage => 1,
                    SymbolScope::Dynamic => 0,
                };
                priority *= 4;

                // Prefer later entries (earlier symbol is likely to be less specific).
                let index = !0 - symbol.index().0;

                // Tuple is ordered for sort.
                all_symbols.push((symbol.address(), priority, index, name));
            }
            // Unstable sort is okay because tuple includes index.
            all_symbols.sort_unstable();

            let mut previous_address = !0;
            for (address, _priority, _index, name) in all_symbols {
                if address != previous_address {
                    symbols.push(SymbolMapName::new(address, name));
                    previous_address = address;
                }
            }
        }
        SymbolMap::new(symbols)
    }

    /// Construct a map from addresses to symbol names and object file names.
    ///
    /// This is derived from Mach-O STAB entries.
    fn object_map(&self) -> ObjectMap<'data> {
        ObjectMap::default()
    }

    /// Get the imported symbols.
    fn imports(&self) -> Result<Vec<Import<'data>>>;

    /// Get the exported symbols that expose both a name and an address.
    ///
    /// Some file formats may provide other kinds of symbols that can be retrieved using
    /// the low level API.
    fn exports(&self) -> Result<Vec<Export<'data>>>;

    /// Return true if the file contains DWARF debug information sections, false if not.
    fn has_debug_symbols(&self) -> bool;

    /// The UUID from a Mach-O [`LC_UUID`](crate::macho::LC_UUID) load command.
    #[inline]
    fn mach_uuid(&self) -> Result<Option<[u8; 16]>> {
        Ok(None)
    }

    /// The build ID from an ELF [`NT_GNU_BUILD_ID`](crate::elf::NT_GNU_BUILD_ID) note.
    #[inline]
    fn build_id(&self) -> Result<Option<&'data [u8]>> {
        Ok(None)
    }

    /// The filename and CRC from a `.gnu_debuglink` section.
    #[inline]
    fn gnu_debuglink(&self) -> Result<Option<(&'data [u8], u32)>> {
        Ok(None)
    }

    /// The filename and build ID from a `.gnu_debugaltlink` section.
    #[inline]
    fn gnu_debugaltlink(&self) -> Result<Option<(&'data [u8], &'data [u8])>> {
        Ok(None)
    }

    /// The filename and GUID from the PE CodeView section.
    #[inline]
    fn pdb_info(&self) -> Result<Option<CodeView<'_>>> {
        Ok(None)
    }

    /// Get the base address used for relative virtual addresses.
    ///
    /// Currently this is only non-zero for PE.
    fn relative_address_base(&self) -> u64;

    /// Get the virtual address of the entry point of the binary.
    fn entry(&self) -> u64;

    /// File flags that are specific to each file format.
    fn flags(&self) -> FileFlags;
}

/// A loadable segment in an [`Object`].
///
/// This trait is part of the unified read API.
pub trait ObjectSegment<'data>: read::private::Sealed {
    /// Returns the virtual address of the segment.
    fn address(&self) -> u64;

    /// Returns the size of the segment in memory.
    fn size(&self) -> u64;

    /// Returns the alignment of the segment in memory.
    fn align(&self) -> u64;

    /// Returns the offset and size of the segment in the file.
    fn file_range(&self) -> (u64, u64);

    /// Returns a reference to the file contents of the segment.
    ///
    /// The length of this data may be different from the size of the
    /// segment in memory.
    fn data(&self) -> Result<&'data [u8]>;

    /// Return the segment data in the given range.
    ///
    /// Returns `Ok(None)` if the segment does not contain the given range.
    fn data_range(&self, address: u64, size: u64) -> Result<Option<&'data [u8]>>;

    /// Returns the name of the segment.
    fn name_bytes(&self) -> Result<Option<&[u8]>>;

    /// Returns the name of the segment.
    ///
    /// Returns an error if the name is not UTF-8.
    fn name(&self) -> Result<Option<&str>>;

    /// Return the flags of segment.
    fn flags(&self) -> SegmentFlags;
}

/// A section in an [`Object`].
///
/// This trait is part of the unified read API.
pub trait ObjectSection<'data>: read::private::Sealed {
    /// An iterator for the relocations for a section.
    ///
    /// The first field in the item tuple is the section offset
    /// that the relocation applies to.
    type RelocationIterator: Iterator<Item = (u64, Relocation)>;

    /// Returns the section index.
    fn index(&self) -> SectionIndex;

    /// Returns the address of the section.
    fn address(&self) -> u64;

    /// Returns the size of the section in memory.
    fn size(&self) -> u64;

    /// Returns the alignment of the section in memory.
    fn align(&self) -> u64;

    /// Returns offset and size of on-disk segment (if any).
    fn file_range(&self) -> Option<(u64, u64)>;

    /// Returns the raw contents of the section.
    ///
    /// The length of this data may be different from the size of the
    /// section in memory.
    ///
    /// This does not do any decompression.
    fn data(&self) -> Result<&'data [u8]>;

    /// Return the raw contents of the section data in the given range.
    ///
    /// This does not do any decompression.
    ///
    /// Returns `Ok(None)` if the section does not contain the given range.
    fn data_range(&self, address: u64, size: u64) -> Result<Option<&'data [u8]>>;

    /// Returns the potentially compressed file range of the section,
    /// along with information about the compression.
    fn compressed_file_range(&self) -> Result<CompressedFileRange>;

    /// Returns the potentially compressed contents of the section,
    /// along with information about the compression.
    fn compressed_data(&self) -> Result<CompressedData<'data>>;

    /// Returns the uncompressed contents of the section.
    ///
    /// The length of this data may be different from the size of the
    /// section in memory.
    ///
    /// If no compression is detected, then returns the data unchanged.
    /// Returns `Err` if decompression fails.
    fn uncompressed_data(&self) -> Result<Cow<'data, [u8]>> {
        self.compressed_data()?.decompress()
    }

    /// Returns the name of the section.
    fn name_bytes(&self) -> Result<&'data [u8]>;

    /// Returns the name of the section.
    ///
    /// Returns an error if the name is not UTF-8.
    fn name(&self) -> Result<&'data str>;

    /// Returns the name of the segment for this section.
    fn segment_name_bytes(&self) -> Result<Option<&[u8]>>;

    /// Returns the name of the segment for this section.
    ///
    /// Returns an error if the name is not UTF-8.
    fn segment_name(&self) -> Result<Option<&str>>;

    /// Return the kind of this section.
    fn kind(&self) -> SectionKind;

    /// Get the relocations for this section.
    fn relocations(&self) -> Self::RelocationIterator;

    /// Construct a relocation map for this section.
    fn relocation_map(&self) -> Result<RelocationMap>;

    /// Section flags that are specific to each file format.
    fn flags(&self) -> SectionFlags;
}

/// A COMDAT section group in an [`Object`].
///
/// This trait is part of the unified read API.
pub trait ObjectComdat<'data>: read::private::Sealed {
    /// An iterator for the sections in the section group.
    type SectionIterator: Iterator<Item = SectionIndex>;

    /// Returns the COMDAT selection kind.
    fn kind(&self) -> ComdatKind;

    /// Returns the index of the symbol used for the name of COMDAT section group.
    fn symbol(&self) -> SymbolIndex;

    /// Returns the name of the COMDAT section group.
    fn name_bytes(&self) -> Result<&'data [u8]>;

    /// Returns the name of the COMDAT section group.
    ///
    /// Returns an error if the name is not UTF-8.
    fn name(&self) -> Result<&'data str>;

    /// Get the sections in this section group.
    fn sections(&self) -> Self::SectionIterator;
}

/// A symbol table in an [`Object`].
///
/// This trait is part of the unified read API.
pub trait ObjectSymbolTable<'data>: read::private::Sealed {
    /// A symbol table entry.
    type Symbol: ObjectSymbol<'data>;

    /// An iterator for the symbols in a symbol table.
    type SymbolIterator: Iterator<Item = Self::Symbol>;

    /// Get an iterator for the symbols in the table.
    ///
    /// This may skip over symbols that are malformed or unsupported.
    fn symbols(&self) -> Self::SymbolIterator;

    /// Get the symbol at the given index.
    ///
    /// The meaning of the index depends on the object file.
    ///
    /// Returns an error if the index is invalid.
    fn symbol_by_index(&self, index: SymbolIndex) -> Result<Self::Symbol>;
}

/// A symbol table entry in an [`Object`].
///
/// This trait is part of the unified read API.
pub trait ObjectSymbol<'data>: read::private::Sealed {
    /// The index of the symbol.
    fn index(&self) -> SymbolIndex;

    /// The name of the symbol.
    fn name_bytes(&self) -> Result<&'data [u8]>;

    /// The name of the symbol.
    ///
    /// Returns an error if the name is not UTF-8.
    fn name(&self) -> Result<&'data str>;

    /// The address of the symbol. May be zero if the address is unknown.
    fn address(&self) -> u64;

    /// The size of the symbol. May be zero if the size is unknown.
    fn size(&self) -> u64;

    /// Return the kind of this symbol.
    fn kind(&self) -> SymbolKind;

    /// Returns the section where the symbol is defined.
    fn section(&self) -> SymbolSection;

    /// Returns the section index for the section containing this symbol.
    ///
    /// May return `None` if the symbol is not defined in a section.
    fn section_index(&self) -> Option<SectionIndex> {
        self.section().index()
    }

    /// Return true if the symbol is undefined.
    fn is_undefined(&self) -> bool;

    /// Return true if the symbol is a definition of a function or data object
    /// that has a known address.
    ///
    /// This is primarily used to implement [`Object::symbol_map`].
    fn is_definition(&self) -> bool;

    /// Return true if the symbol is common data.
    ///
    /// Note: does not check for [`SymbolSection::Section`] with [`SectionKind::Common`].
    fn is_common(&self) -> bool;

    /// Return true if the symbol is weak.
    fn is_weak(&self) -> bool;

    /// Returns the symbol scope.
    fn scope(&self) -> SymbolScope;

    /// Return true if the symbol visible outside of the compilation unit.
    ///
    /// This treats [`SymbolScope::Unknown`] as global.
    fn is_global(&self) -> bool;

    /// Return true if the symbol is only visible within the compilation unit.
    fn is_local(&self) -> bool;

    /// Symbol flags that are specific to each file format.
    fn flags(&self) -> SymbolFlags<SectionIndex, SymbolIndex>;
}

/// An iterator for files that don't have dynamic relocations.
#[derive(Debug)]
pub struct NoDynamicRelocationIterator;

impl Iterator for NoDynamicRelocationIterator {
    type Item = (u64, Relocation);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}
