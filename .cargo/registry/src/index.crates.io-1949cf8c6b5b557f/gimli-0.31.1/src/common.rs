/// Whether the format of a compilation unit is 32- or 64-bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    /// 64-bit DWARF
    Dwarf64 = 8,
    /// 32-bit DWARF
    Dwarf32 = 4,
}

impl Format {
    /// Return the serialized size of an initial length field for the format.
    #[inline]
    pub fn initial_length_size(self) -> u8 {
        match self {
            Format::Dwarf32 => 4,
            Format::Dwarf64 => 12,
        }
    }

    /// Return the natural word size for the format
    #[inline]
    pub fn word_size(self) -> u8 {
        match self {
            Format::Dwarf32 => 4,
            Format::Dwarf64 => 8,
        }
    }
}

/// Which vendor extensions to support.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Vendor {
    /// A default set of extensions, including some common GNU extensions.
    Default,
    /// AAarch64 extensions.
    AArch64,
}

/// Encoding parameters that are commonly used for multiple DWARF sections.
///
/// This is intended to be small enough to pass by value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
// `address_size` and `format` are used more often than `version`, so keep
// them first.
#[repr(C)]
pub struct Encoding {
    /// The size of an address.
    pub address_size: u8,

    /// Whether the DWARF format is 32- or 64-bit.
    pub format: Format,

    /// The DWARF version of the header.
    pub version: u16,
}

/// Encoding parameters for a line number program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LineEncoding {
    /// The size in bytes of the smallest target machine instruction.
    pub minimum_instruction_length: u8,

    /// The maximum number of individual operations that may be encoded in an
    /// instruction.
    pub maximum_operations_per_instruction: u8,

    /// The initial value of the `is_stmt` register.
    pub default_is_stmt: bool,

    /// The minimum value which a special opcode can add to the line register.
    pub line_base: i8,

    /// The range of values which a special opcode can add to the line register.
    pub line_range: u8,
}

impl Default for LineEncoding {
    fn default() -> Self {
        // Values from LLVM.
        LineEncoding {
            minimum_instruction_length: 1,
            maximum_operations_per_instruction: 1,
            default_is_stmt: true,
            line_base: -5,
            line_range: 14,
        }
    }
}

/// A DWARF register number.
///
/// The meaning of this value is ABI dependent. This is generally encoded as
/// a ULEB128, but supported architectures need 16 bits at most.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Register(pub u16);

/// An offset into the `.debug_abbrev` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DebugAbbrevOffset<T = usize>(pub T);

/// An offset to a set of entries in the `.debug_addr` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugAddrBase<T = usize>(pub T);

/// An index into a set of addresses in the `.debug_addr` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugAddrIndex<T = usize>(pub T);

/// An offset into the `.debug_aranges` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugArangesOffset<T = usize>(pub T);

/// An offset into the `.debug_info` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct DebugInfoOffset<T = usize>(pub T);

/// An offset into the `.debug_line` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugLineOffset<T = usize>(pub T);

/// An offset into the `.debug_line_str` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugLineStrOffset<T = usize>(pub T);

/// An offset into either the `.debug_loc` section or the `.debug_loclists` section,
/// depending on the version of the unit the offset was contained in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocationListsOffset<T = usize>(pub T);

/// An offset to a set of location list offsets in the `.debug_loclists` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugLocListsBase<T = usize>(pub T);

/// An index into a set of location list offsets in the `.debug_loclists` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugLocListsIndex<T = usize>(pub T);

/// An offset into the `.debug_macinfo` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DebugMacinfoOffset<T = usize>(pub T);

/// An offset into the `.debug_macro` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DebugMacroOffset<T = usize>(pub T);

/// An offset into either the `.debug_ranges` section or the `.debug_rnglists` section,
/// depending on the version of the unit the offset was contained in.
///
/// If this is from a DWARF 4 DWO file, then it must additionally be offset by the
/// value of `DW_AT_GNU_ranges_base`. You can use `Dwarf::ranges_offset_from_raw` to do this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RawRangeListsOffset<T = usize>(pub T);

/// An offset into either the `.debug_ranges` section or the `.debug_rnglists` section,
/// depending on the version of the unit the offset was contained in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RangeListsOffset<T = usize>(pub T);

/// An offset to a set of range list offsets in the `.debug_rnglists` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugRngListsBase<T = usize>(pub T);

/// An index into a set of range list offsets in the `.debug_rnglists` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugRngListsIndex<T = usize>(pub T);

/// An offset into the `.debug_str` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugStrOffset<T = usize>(pub T);

/// An offset to a set of entries in the `.debug_str_offsets` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugStrOffsetsBase<T = usize>(pub T);

/// An index into a set of entries in the `.debug_str_offsets` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugStrOffsetsIndex<T = usize>(pub T);

/// An offset into the `.debug_types` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct DebugTypesOffset<T = usize>(pub T);

/// A type signature as used in the `.debug_types` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DebugTypeSignature(pub u64);

/// An offset into the `.debug_frame` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DebugFrameOffset<T = usize>(pub T);

impl<T> From<T> for DebugFrameOffset<T> {
    #[inline]
    fn from(o: T) -> Self {
        DebugFrameOffset(o)
    }
}

/// An offset into the `.eh_frame` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EhFrameOffset<T = usize>(pub T);

impl<T> From<T> for EhFrameOffset<T> {
    #[inline]
    fn from(o: T) -> Self {
        EhFrameOffset(o)
    }
}

/// An offset into the `.debug_info` or `.debug_types` sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub enum UnitSectionOffset<T = usize> {
    /// An offset into the `.debug_info` section.
    DebugInfoOffset(DebugInfoOffset<T>),
    /// An offset into the `.debug_types` section.
    DebugTypesOffset(DebugTypesOffset<T>),
}

impl<T> From<DebugInfoOffset<T>> for UnitSectionOffset<T> {
    fn from(offset: DebugInfoOffset<T>) -> Self {
        UnitSectionOffset::DebugInfoOffset(offset)
    }
}

impl<T> From<DebugTypesOffset<T>> for UnitSectionOffset<T> {
    fn from(offset: DebugTypesOffset<T>) -> Self {
        UnitSectionOffset::DebugTypesOffset(offset)
    }
}

impl<T> UnitSectionOffset<T>
where
    T: Clone,
{
    /// Returns the `DebugInfoOffset` inside, or `None` otherwise.
    pub fn as_debug_info_offset(&self) -> Option<DebugInfoOffset<T>> {
        match self {
            UnitSectionOffset::DebugInfoOffset(offset) => Some(offset.clone()),
            UnitSectionOffset::DebugTypesOffset(_) => None,
        }
    }
    /// Returns the `DebugTypesOffset` inside, or `None` otherwise.
    pub fn as_debug_types_offset(&self) -> Option<DebugTypesOffset<T>> {
        match self {
            UnitSectionOffset::DebugInfoOffset(_) => None,
            UnitSectionOffset::DebugTypesOffset(offset) => Some(offset.clone()),
        }
    }
}

/// An identifier for a DWARF section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub enum SectionId {
    /// The `.debug_abbrev` section.
    DebugAbbrev,
    /// The `.debug_addr` section.
    DebugAddr,
    /// The `.debug_aranges` section.
    DebugAranges,
    /// The `.debug_cu_index` section.
    DebugCuIndex,
    /// The `.debug_frame` section.
    DebugFrame,
    /// The `.eh_frame` section.
    EhFrame,
    /// The `.eh_frame_hdr` section.
    EhFrameHdr,
    /// The `.debug_info` section.
    DebugInfo,
    /// The `.debug_line` section.
    DebugLine,
    /// The `.debug_line_str` section.
    DebugLineStr,
    /// The `.debug_loc` section.
    DebugLoc,
    /// The `.debug_loclists` section.
    DebugLocLists,
    /// The `.debug_macinfo` section.
    DebugMacinfo,
    /// The `.debug_macro` section.
    DebugMacro,
    /// The `.debug_pubnames` section.
    DebugPubNames,
    /// The `.debug_pubtypes` section.
    DebugPubTypes,
    /// The `.debug_ranges` section.
    DebugRanges,
    /// The `.debug_rnglists` section.
    DebugRngLists,
    /// The `.debug_str` section.
    DebugStr,
    /// The `.debug_str_offsets` section.
    DebugStrOffsets,
    /// The `.debug_tu_index` section.
    DebugTuIndex,
    /// The `.debug_types` section.
    DebugTypes,
}

impl SectionId {
    /// Returns the ELF section name for this kind.
    pub fn name(self) -> &'static str {
        match self {
            SectionId::DebugAbbrev => ".debug_abbrev",
            SectionId::DebugAddr => ".debug_addr",
            SectionId::DebugAranges => ".debug_aranges",
            SectionId::DebugCuIndex => ".debug_cu_index",
            SectionId::DebugFrame => ".debug_frame",
            SectionId::EhFrame => ".eh_frame",
            SectionId::EhFrameHdr => ".eh_frame_hdr",
            SectionId::DebugInfo => ".debug_info",
            SectionId::DebugLine => ".debug_line",
            SectionId::DebugLineStr => ".debug_line_str",
            SectionId::DebugLoc => ".debug_loc",
            SectionId::DebugLocLists => ".debug_loclists",
            SectionId::DebugMacinfo => ".debug_macinfo",
            SectionId::DebugMacro => ".debug_macro",
            SectionId::DebugPubNames => ".debug_pubnames",
            SectionId::DebugPubTypes => ".debug_pubtypes",
            SectionId::DebugRanges => ".debug_ranges",
            SectionId::DebugRngLists => ".debug_rnglists",
            SectionId::DebugStr => ".debug_str",
            SectionId::DebugStrOffsets => ".debug_str_offsets",
            SectionId::DebugTuIndex => ".debug_tu_index",
            SectionId::DebugTypes => ".debug_types",
        }
    }

    /// Returns the ELF section name for this kind, when found in a .dwo or .dwp file.
    pub fn dwo_name(self) -> Option<&'static str> {
        Some(match self {
            SectionId::DebugAbbrev => ".debug_abbrev.dwo",
            SectionId::DebugCuIndex => ".debug_cu_index",
            SectionId::DebugInfo => ".debug_info.dwo",
            SectionId::DebugLine => ".debug_line.dwo",
            // The debug_loc section can be present in the dwo when using the
            // GNU split-dwarf extension to DWARF4.
            SectionId::DebugLoc => ".debug_loc.dwo",
            SectionId::DebugLocLists => ".debug_loclists.dwo",
            SectionId::DebugMacinfo => ".debug_macinfo.dwo",
            SectionId::DebugMacro => ".debug_macro.dwo",
            SectionId::DebugRngLists => ".debug_rnglists.dwo",
            SectionId::DebugStr => ".debug_str.dwo",
            SectionId::DebugStrOffsets => ".debug_str_offsets.dwo",
            SectionId::DebugTuIndex => ".debug_tu_index",
            SectionId::DebugTypes => ".debug_types.dwo",
            _ => return None,
        })
    }

    /// Returns the XCOFF section name for this kind.
    pub fn xcoff_name(self) -> Option<&'static str> {
        Some(match self {
            SectionId::DebugAbbrev => ".dwabrev",
            SectionId::DebugAranges => ".dwarnge",
            SectionId::DebugFrame => ".dwframe",
            SectionId::DebugInfo => ".dwinfo",
            SectionId::DebugLine => ".dwline",
            SectionId::DebugLoc => ".dwloc",
            SectionId::DebugMacinfo => ".dwmac",
            SectionId::DebugPubNames => ".dwpbnms",
            SectionId::DebugPubTypes => ".dwpbtyp",
            SectionId::DebugRanges => ".dwrnges",
            SectionId::DebugStr => ".dwstr",
            _ => return None,
        })
    }

    /// Returns true if this is a mergeable string section.
    ///
    /// This is useful for determining the correct section flags.
    pub fn is_string(self) -> bool {
        matches!(self, SectionId::DebugStr | SectionId::DebugLineStr)
    }
}

/// An optionally-provided implementation-defined compilation unit ID to enable
/// split DWARF and linking a split compilation unit back together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DwoId(pub u64);

/// The "type" of file with DWARF debugging information. This determines, among other things,
/// which files DWARF sections should be loaded from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DwarfFileType {
    /// A normal executable or object file.
    Main,
    /// A .dwo split DWARF file.
    Dwo,
    // TODO: Supplementary files, .dwps?
}

impl Default for DwarfFileType {
    fn default() -> Self {
        DwarfFileType::Main
    }
}
