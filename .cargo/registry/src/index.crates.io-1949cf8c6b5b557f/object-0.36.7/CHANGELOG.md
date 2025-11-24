# `object` Change Log

--------------------------------------------------------------------------------

## 0.36.7

Released 2024/12/21.

### Changed

* Included `build.rs` in package.

--------------------------------------------------------------------------------

## 0.36.6

Released 2024/12/21.

### Added

* Added `Architecuture::M68k`.
  [#742](https://github.com/gimli-rs/object/pull/742)
  [#749](https://github.com/gimli-rs/object/pull/749)

* Added `Architecuture::Mips64_N32`.
  [#743](https://github.com/gimli-rs/object/pull/743)

* Added `elf::SHT_RELR`, `read::elf::SectionHeader::relr`, and
  `write::elf::Writer::write_relative_relocation_section_header`.
  [#746](https://github.com/gimli-rs/object/pull/746)

* Added `core::error::Error` implementation for Rust 1.81 onwards.
  [#747](https://github.com/gimli-rs/object/pull/747)

### Changed

* Changed `build::elf::Builder` to support `.annobin.notes`,
  `SHT_LLVM_DEPENDENT_LIBRARIES`, and `SHT_RELR` sections.
  [#735](https://github.com/gimli-rs/object/pull/735)
  [#737](https://github.com/gimli-rs/object/pull/737)
  [#746](https://github.com/gimli-rs/object/pull/746)

* Changed `write::Object::add_subsection` to omit the subsection name suffix
  if the subsection name is empty.
  [#748](https://github.com/gimli-rs/object/pull/748)

--------------------------------------------------------------------------------

## 0.36.5

Released 2024/10/04.

### Added

* Added `Architecture::E2K32` and `Architecture::E2K64`.
  [#727](https://github.com/gimli-rs/object/pull/727)

* Added read and write support for `pe::IMAGE_REL_ARM64_BRANCH26`.
  [#731](https://github.com/gimli-rs/object/pull/731)

### Changed

* Fixed decompression of multi-frame Zstandard data in `read::CompressedData::decompress`.
  [#730](https://github.com/gimli-rs/object/pull/730)

--------------------------------------------------------------------------------

## 0.36.4

Released 2024/08/30.

### Added

* Added `pe::IMAGE_FILE_MACHINE_ARM64X` and `pe::IMAGE_FILE_MACHINE_CHPE_X86`.
  [#717](https://github.com/gimli-rs/object/pull/717)

* Added `elf::SHF_GNU_RETAIN` and `elf::SHF_GNU_MBIND`.
  [#720](https://github.com/gimli-rs/object/pull/720)

### Changed

* Fixed the checksum for COFF BSS section symbols in `write::Object`.
  [#718](https://github.com/gimli-rs/object/pull/718)

* Changed `read::CompressedData::decompress` to validate the decompressed size.
  [#723](https://github.com/gimli-rs/object/pull/723)

* Updated `wasmparser` dependency.

--------------------------------------------------------------------------------

## 0.36.3

Released 2024/08/07.

### Added

* Added `Iterator` implementations for various types in the low level read API.
  [#713](https://github.com/gimli-rs/object/pull/713)
  [#714](https://github.com/gimli-rs/object/pull/714)

### Changed

* Changed `from_bytes` constructors for integer endian types to `const`.
  [#712](https://github.com/gimli-rs/object/pull/712)

* Changed `next` methods in the low level read API to fuse after returning an
  error.
  [#714](https://github.com/gimli-rs/object/pull/714)

* Updated `wasmparser` dependency.
  [#715](https://github.com/gimli-rs/object/pull/715)

--------------------------------------------------------------------------------

## 0.36.2

Released 2024/07/24.

### Changed

* Improved writing of GNU symbol versioning in `build::elf::Builder`.
  [#705](https://github.com/gimli-rs/object/pull/705)

* Fixed alignment of `SHT_HASH`/`SHT_GNU_verdef`/`SHT_GNU_verneed` sections in
  `write::elf::Writer`.
  [#706](https://github.com/gimli-rs/object/pull/706)

* Fixed writing of GNU hash for absolute symbols in `build::elf::Builder`.
  [#707](https://github.com/gimli-rs/object/pull/707)

* Fixed writing of empty ELF string table in `write::Object`.
  [#710](https://github.com/gimli-rs/object/pull/710)

--------------------------------------------------------------------------------

## 0.36.1

Released 2024/06/29.

### Added

* Added `SectionKind::DebugString`.
  [#694](https://github.com/gimli-rs/object/pull/694)

* Added `Architecture::Sparc` and `Architecture::Sparc32Plus`.
  [#699](https://github.com/gimli-rs/object/pull/699)
  [#700](https://github.com/gimli-rs/object/pull/700)

* Added more RISC-V ELF relocation constants.
  [#701](https://github.com/gimli-rs/object/pull/701)

### Changed

* Changed `read::ElfFile::imports` to return the library for versioned symbols.
  [#693](https://github.com/gimli-rs/object/pull/693)

* Changed `read::MachOFile` to support Go's debug section compression.
  [#697](https://github.com/gimli-rs/object/pull/697)

* Reversed the order of Mach-O relocations emitted by `write::Object`.
  [#702](https://github.com/gimli-rs/object/pull/702)

--------------------------------------------------------------------------------

## 0.36.0

Released 2024/05/26.

### Breaking changes

* Deleted `data` and `align` parameters for `write::Object::add_subsection`.
  Use `add_symbol_data` or `add_symbol_bss` instead.
  [#676](https://github.com/gimli-rs/object/pull/676)

* Changed methods in the lower level read API to accept or return `SectionIndex`
  or `SymbolIndex` instead of `usize`.
  [#677](https://github.com/gimli-rs/object/pull/677)
  [#684](https://github.com/gimli-rs/object/pull/684)
  [#685](https://github.com/gimli-rs/object/pull/685)

* Deleted `SymbolKind::Null`. Changed `read::Object::sections` and `read::Object::symbols`
  to no longer return null entries. This affects ELF and XCOFF.
  [#679](https://github.com/gimli-rs/object/pull/679)

* Changed `read::ObjectMap::object` to return `ObjectMapFile`. This handles
  splitting the object file name into path and member.
  [#686](https://github.com/gimli-rs/object/pull/686)

* Changed `read::coff::ImageSymbol::address` to only return an address for
  symbols that have an address.
  [#689](https://github.com/gimli-rs/object/pull/689)

### Added

* Added `pod::slice_from_all_bytes` and `pod::slice_from_all_bytes_mut`.
  [#672](https://github.com/gimli-rs/object/pull/672)

* Added `write::Object::set_subsections_via_symbols`.
  Changed `write::Object::add_symbol_data` and `write::Object::add_symbol_bss`
  to correctly handle zero size symbols when subsections are enabled.
  [#676](https://github.com/gimli-rs/object/pull/676)

* Added methods in the unified read API to return the lower level API structures.
  Some existing methods were deprecated so that naming of these methods is more consistent.
  [#678](https://github.com/gimli-rs/object/pull/678)

* Added methods in the lower level read API to return a `SectionIndex` or `SymbolIndex`.
  [#684](https://github.com/gimli-rs/object/pull/684)
  [#689](https://github.com/gimli-rs/object/pull/689)

* Implemented `Display` for `read::SymbolIndex` and `read::SectionIndex`.
  [#684](https://github.com/gimli-rs/object/pull/684)

* Added `is_common`, `is_absolute`, `is_local`, and `is_weak` to `read::elf::Sym`.
  [#685](https://github.com/gimli-rs/object/pull/685)

### Changed

* Changed `read::ArchiveFile` to skip the `<ECSYMBOLS>` member.
  [#669](https://github.com/gimli-rs/object/pull/669)

* Fixed handling of segment data in the dyld shared cache.
  [#673](https://github.com/gimli-rs/object/pull/673)

* Changed `read::RelocationMap` to handle Mach-O section relocations.
  [#675](https://github.com/gimli-rs/object/pull/675)

* Changed `read::elf::RelocationSections` to ignore relocations that apply to relocations.
  [#680](https://github.com/gimli-rs/object/pull/680)

* Removed a lifetime bound from an argument in `read::elf::SectionTable::section_name`,
  `read::elf::SymbolTable::symbol_name`, and `read::elf::SymbolTable::symbol_section`.
  [#681](https://github.com/gimli-rs/object/pull/681)

--------------------------------------------------------------------------------

## 0.35.0

Released 2024/04/10.

### Breaking changes

* Moved the `'file` lifetime parameter from `read::Object` to its associated types.
  [#655](https://github.com/gimli-rs/object/pull/655)

### Added

* Added support more section kinds in `build::elf`.
  [#650](https://github.com/gimli-rs/object/pull/650)

* Added thin archive support to `read::ArchiveFile`.
  [#651](https://github.com/gimli-rs/object/pull/651)

* Added `read::ReadCacheOps` and changed `read::ReadCache` bound from `Read + Seek` to `ReadCacheOps`.
  [#652](https://github.com/gimli-rs/object/pull/652)

* Added `read::ObjectSection::relocation_map`
  [#654](https://github.com/gimli-rs/object/pull/654)

* Added `read::ArchiveFile::symbols`.
  [#658](https://github.com/gimli-rs/object/pull/658)

* Added `BinaryFormat::native_object`.
  [#661](https://github.com/gimli-rs/object/pull/661)

### Changed

* The minimum supported rust version for the `read` feature and its dependencies
  has changed to 1.65.0.
  [#655](https://github.com/gimli-rs/object/pull/655)

* Fixed `sh_offset` handling for `SHT_NOBITS` sections in `build::elf`.
  [#645](https://github.com/gimli-rs/object/pull/645)

* Fixed handling of ELF files with dynamic symbols but no dynamic strings.
  [#646](https://github.com/gimli-rs/object/pull/646)

* Fixed potential panics in `read::WasmFile` due to invalid function indices.
  [#649](https://github.com/gimli-rs/object/pull/649)

* Fixed handling of Wasm components in `read::WasmFile`.
  [#649](https://github.com/gimli-rs/object/pull/649)

* Fixed `sh_entsize` for 32-bit hash sections in `write::elf`.
  [#650](https://github.com/gimli-rs/object/pull/650)

* Fixed `sh_size` for attribute sections in `build::elf`.
  [#650](https://github.com/gimli-rs/object/pull/650)

* Fixed `sh_info` for `SHT_DYNSYM` sections in `build::elf`.
  [#650](https://github.com/gimli-rs/object/pull/650)

* Fixed handling of dynamic relocations with invalid `sh_link` in `build::elf`.
  [#650](https://github.com/gimli-rs/object/pull/650)

* Fixed parsing of member names containing '/' in `read::ArchiveFile`.
  [#657](https://github.com/gimli-rs/object/pull/657)

* Fixed handling of load segment alignments in `build::elf::Builder::read`.
  [#659](https://github.com/gimli-rs/object/pull/659)

--------------------------------------------------------------------------------

## 0.34.0

Released 2024/03/11.

### Breaking changes

* Replaced `macho::DyldSubCacheInfo` with `macho::DyldSubCacheEntryV1`.
  Changed the return type of `macho::DyldCacheHeader::subcaches`.
  [#642](https://github.com/gimli-rs/object/pull/642)

### Changed

* Added `macho::DyldSubCacheEntryV2` and changed `read::macho::DyldCache`
  to handle both versions. This is needed for macOS 13 and above.
  [#642](https://github.com/gimli-rs/object/pull/642)

--------------------------------------------------------------------------------

## 0.33.0

Released 2024/03/05.

### Breaking changes

* Deleted file format variants in `RelocationKind`. Replaced their usage
  with `read::Relocation::flags` and `write::Relocation::flags`.
  [#585](https://github.com/gimli-rs/object/pull/585)

* Replaced `kind`, `encoding` and `size` fields in `write::Relocation`
  with `RelocationFlags::Generic` in the `flags` field.
  [#585](https://github.com/gimli-rs/object/pull/585)

* Replaced `macho::FatHeader::parse`, `macho::FatHeader::parse_arch32`,
  and `macho::FatHeader::parse_arch64` with `read::macho::MachOFatFile`,
  `read::macho::MachOFatFile32` and `read::macho::MachOFatFile64`.
  [#623](https://github.com/gimli-rs/object/pull/623)

### Added

* Added `macho::PLATFORM_XROS` and `macho::PLATFORM_XROSSIMULATOR`.
  [#626](https://github.com/gimli-rs/object/pull/626)

* Added `build::elf::Builder` and associated types.
  Extended `write::elf::Writer` to support this.
  [#618](https://github.com/gimli-rs/object/pull/618)

### Changed

* Changed the lifetime to `'data` for the return value of `ObjectSection::name`,
  `ObjectSection::name_bytes`, `ObjectComdat::name`, `ObjectComdat::name_bytes`.
  [#620](https://github.com/gimli-rs/object/pull/620)
  [#622](https://github.com/gimli-rs/object/pull/622)

* Checked that sizes are smaller than the file length in `read::ReadCache`.
  [#630](https://github.com/gimli-rs/object/pull/630)

* Used `Vec::try_reserve_exact` for large allocations.
  [#632](https://github.com/gimli-rs/object/pull/632)

--------------------------------------------------------------------------------

## 0.32.2

Released 2023/12/24.

### Added

* Added ELF relocations for LoongArch ABI v2.20.
  [#578](https://github.com/gimli-rs/object/pull/578)
  [#589](https://github.com/gimli-rs/object/pull/589)

* Added ELF support for SHARC.
  [#593](https://github.com/gimli-rs/object/pull/593)

* Added `write::coff::Writer`.
  [#595](https://github.com/gimli-rs/object/pull/595)

* Added `SubArchitecture::Arm64EC` support for PE/COFF.
  [#607](https://github.com/gimli-rs/object/pull/607)

* Added `SubArchitecture::Arm64E` support for Mach-O.
  [#614](https://github.com/gimli-rs/object/pull/614)

* Added `read::Object::symbol_by_name` and `read::Object::symbol_by_name_bytes`.
  [#602](https://github.com/gimli-rs/object/pull/602)

* Added more functions to the low level API in `read::xcoff`.
  [#608](https://github.com/gimli-rs/object/pull/608)

* Added more functions to the low level API in `read::macho`.
  [#584](https://github.com/gimli-rs/object/pull/584)

### Changed

* Fixes for AArch64 relocation addends for Mach-O.
  [#581](https://github.com/gimli-rs/object/pull/581)

* Changes to `write::Object` output for Mach-O, including the addition of a `LC_DYSYMTAB` load command.
  [#584](https://github.com/gimli-rs/object/pull/584)

* Changed `write::Object` to always use `R_X86_64_PLT32` for x86-64 branches for ELF.
  [#590](https://github.com/gimli-rs/object/pull/590)

* Fixed `read::ObjectSymbol::kind` for undefined section symbols for COFF.
  [#592](https://github.com/gimli-rs/object/pull/592)

* Fixed `write::Object` to accept undefined section symbols for COFF.
  [#594](https://github.com/gimli-rs/object/pull/594)

* Improved parsing of auxiliary section symbols for COFF.
  [#603](https://github.com/gimli-rs/object/pull/603)

* Improved the selection of symbols for `read::Object::symbol_map`.
  This includes changes to `read::Symbol::is_definition`.
  [#601](https://github.com/gimli-rs/object/pull/601)
  [#606](https://github.com/gimli-rs/object/pull/606)

* Changed `read::ObjectSymbol::kind` for ELF `STT_NOTYPE` symbols to `SymbolKind::Unknown`.
  [#604](https://github.com/gimli-rs/object/pull/604)

* Changed `read::ObjectSymbol::scope` for XCOFF `C_HIDEXT` symbols to `SymbolScope::Compilation`.
  [#605](https://github.com/gimli-rs/object/pull/605)

--------------------------------------------------------------------------------

## 0.32.1

Released 2023/09/03.

### Added

* Added `write::Object::set_macho_cpu_subtype`.
  [#574](https://github.com/gimli-rs/object/pull/574)

--------------------------------------------------------------------------------

## 0.32.0

Released 2023/08/12.

### Breaking changes

* Changed `read::elf::Note::name` to exclude all trailing null bytes.
  [#549](https://github.com/gimli-rs/object/pull/549)

* Updated dependencies, and changed some optional dependencies to use the `dep:`
  feature syntax.
  [#558](https://github.com/gimli-rs/object/pull/558)
  [#569](https://github.com/gimli-rs/object/pull/569)

### Changed

* The minimum supported rust version for the `read` feature and its dependencies
  has changed to 1.60.0.

* The minimum supported rust version for other features has changed to 1.65.0.

* Changed many definitions from `static` to `const`.
  [#549](https://github.com/gimli-rs/object/pull/549)

* Fixed Mach-O section alignment padding in `write::Object`.
  [#553](https://github.com/gimli-rs/object/pull/553)

* Changed `read::File` to an enum.
  [#564](https://github.com/gimli-rs/object/pull/564)

### Added

* Added `elf::ELF_NOTE_GO`, `elf::NT_GO_BUILD_ID`, and `read::elf::Note::name_bytes`.
  [#549](https://github.com/gimli-rs/object/pull/549)

* Added `read::FileKind::CoffImport` and `read::coff::ImportFile`.
  [#555](https://github.com/gimli-rs/object/pull/555)
  [#556](https://github.com/gimli-rs/object/pull/556)

* Added `Architecture::Csky` and basic ELF support for C-SKY.
  [#561](https://github.com/gimli-rs/object/pull/561)

* Added `read::elf::ElfSymbol::raw_symbol`.
  [#562](https://github.com/gimli-rs/object/pull/562)

--------------------------------------------------------------------------------

## 0.30.4

Released 2023/06/05.

### Changed

* Fixed Mach-O section alignment padding in `write::Object`.
  [#553](https://github.com/gimli-rs/object/pull/553)

--------------------------------------------------------------------------------

## 0.31.1

Released 2023/05/09.

### Changed

* Fixed address for global symbols in `read::wasm`.
  [#539](https://github.com/gimli-rs/object/pull/539)

* Fixed writing of alignment for empty ELF sections.
  [#540](https://github.com/gimli-rs/object/pull/540)

### Added

* Added more `elf::GNU_PROPERTY_*` definitions.
  Added `read::elf::note::gnu_properties`, `write::StandardSection::GnuProperty`,
  and `write::Object::add_elf_gnu_property_u32`.
  [#537](https://github.com/gimli-rs/object/pull/537)
  [#541](https://github.com/gimli-rs/object/pull/541)

* Added Mach-O support for `Architecture::Aarch64_Ilp32`.
  [#542](https://github.com/gimli-rs/object/pull/542)
  [#545](https://github.com/gimli-rs/object/pull/545)

* Added `Architecture::Wasm64`.
  [#543](https://github.com/gimli-rs/object/pull/543)

--------------------------------------------------------------------------------

## 0.31.0

Released 2023/04/14.

### Breaking changes

* Added a type parameter on existing COFF types to support reading COFF `/bigobj` files.
  [#502](https://github.com/gimli-rs/object/pull/502)

* Changed PE symbols to support COFF `/bigobj`.
  Changed `pe::IMAGE_SYM_*` to `i32`.
  Changed `pe::ImageSymbolEx::section_number` to `I32Bytes`.
  Deleted a number of methods from `pe::ImageSymbol`.
  Use the `read::pe::ImageSymbol` trait instead.
  [#502](https://github.com/gimli-rs/object/pull/502)

* Changed `pe::Guid` to a single array, and added methods to read the individual fields.
  [#502](https://github.com/gimli-rs/object/pull/502)

* Added `Symbol` type parameter to `SymbolFlags` to support `SymbolFlags::Xcoff`.
  [#527](https://github.com/gimli-rs/object/pull/527)

### Changed

* Fix alignment when reserving zero length sections in `write::elf::Write::reserve`.
  [#514](https://github.com/gimli-rs/object/pull/514)

* Validate command size in `read::macho::LoadCommandIterator`.
  [#516](https://github.com/gimli-rs/object/pull/516)

* Handle invalid alignment in `read::macho::MachoSection::align`.
  [#516](https://github.com/gimli-rs/object/pull/516)

* Accept `SymbolKind::Unknown` in `write::Object::macho_write`.
  [#519](https://github.com/gimli-rs/object/pull/519)

* Updated `wasmparser` dependency.
  [#528](https://github.com/gimli-rs/object/pull/528)

### Added

* Added more `elf::EF_RISCV_*` definitions.
  [#507](https://github.com/gimli-rs/object/pull/507)

* Added `read::elf::SectionHeader::gnu_attributes` and associated types.
  Added `.gnu.attributes` support to `write::elf::Writer`.
  [#509](https://github.com/gimli-rs/object/pull/509)
  [#525](https://github.com/gimli-rs/object/pull/525)

* Added `write::Object::set_macho_build_version`.
  [#524](https://github.com/gimli-rs/object/pull/524)

* Added `read::FileKind::Xcoff32`, `read::FileKind::Xcoff64`, `read::XcoffFile`,
  and associated types.
  Added XCOFF support to `write::Object`.
  [#469](https://github.com/gimli-rs/object/pull/469)
  [#476](https://github.com/gimli-rs/object/pull/476)
  [#477](https://github.com/gimli-rs/object/pull/477)
  [#482](https://github.com/gimli-rs/object/pull/482)
  [#484](https://github.com/gimli-rs/object/pull/484)
  [#486](https://github.com/gimli-rs/object/pull/486)
  [#527](https://github.com/gimli-rs/object/pull/527)

* Added `read::FileKind::CoffBig`, `read::pe::CoffHeader` and `read::pe::ImageSymbol`.
  [#502](https://github.com/gimli-rs/object/pull/502)

* Added `elf::PT_GNU_PROPERTY`.
  [#530](https://github.com/gimli-rs/object/pull/530)

* Added `elf::ELFCOMPRESS_ZSTD`, `read::CompressionFormat::Zstandard`,
  and Zstandard decompression in `read::CompressedData::decompress` using
  the `ruzstd` crate.
  [#532](https://github.com/gimli-rs/object/pull/532)

* Added `read::elf::NoteIterator::new`.
  [#533](https://github.com/gimli-rs/object/pull/533)

--------------------------------------------------------------------------------

## 0.30.3

Released 2023/01/23.

### Added

* Added `SectionKind::ReadOnlyDataWithRel` for writing.
  [#504](https://github.com/gimli-rs/object/pull/504)

--------------------------------------------------------------------------------

## 0.30.2

Released 2023/01/11.

### Added

* Added more ELF constants for AVR flags and relocations.
  [#500](https://github.com/gimli-rs/object/pull/500)

--------------------------------------------------------------------------------

## 0.30.1

Released 2023/01/04.

### Changed

* Changed `read::ElfSymbol::kind` to handle `STT_NOTYPE` and `STT_GNU_IFUNC`.
  [#498](https://github.com/gimli-rs/object/pull/498)

### Added

* Added `read::CoffSymbol::raw_symbol`.
  [#494](https://github.com/gimli-rs/object/pull/494)

* Added ELF support for Solana Binary Format.
  [#491](https://github.com/gimli-rs/object/pull/491)

* Added ELF support for AArch64 ILP32.
  [#497](https://github.com/gimli-rs/object/pull/497)

--------------------------------------------------------------------------------

## 0.30.0

Released 2022/11/22.

### Breaking changes

* The minimum supported rust version for the `read` feature has changed to 1.52.0.
  [#458](https://github.com/gimli-rs/object/pull/458)

* The minimum supported rust version for the `write` feature has changed to 1.61.0.

* Fixed endian handling in `read::elf::SymbolTable::shndx`.
  [#458](https://github.com/gimli-rs/object/pull/458)

* Fixed endian handling in `read::pe::ResourceName`.
  [#458](https://github.com/gimli-rs/object/pull/458)

* Changed definitions for LoongArch ELF header flags.
  [#483](https://github.com/gimli-rs/object/pull/483)

### Changed

* Fixed parsing of multiple debug directory entries in `read::pe::PeFile::pdb_info`.
  [#451](https://github.com/gimli-rs/object/pull/451)

* Changed the section name used when writing COFF stub symbols.
  [#475](https://github.com/gimli-rs/object/pull/475)

### Added

* Added `read::pe::DataDirectories::delay_load_import_table`.
  [#448](https://github.com/gimli-rs/object/pull/448)

* Added `read::macho::LoadCommandData::raw_data`.
  [#449](https://github.com/gimli-rs/object/pull/449)

* Added ELF relocations for LoongArch ps ABI v2.
  [#450](https://github.com/gimli-rs/object/pull/450)

* Added PowerPC support for Mach-O.
  [#460](https://github.com/gimli-rs/object/pull/460)

* Added support for reading the AIX big archive format.
  [#462](https://github.com/gimli-rs/object/pull/462)
  [#467](https://github.com/gimli-rs/object/pull/467)
  [#473](https://github.com/gimli-rs/object/pull/473)

* Added support for `RelocationEncoding::AArch64Call` when writing Mach-O files.
  [#465](https://github.com/gimli-rs/object/pull/465)

* Added support for `RelocationKind::Relative` when writing RISC-V ELF files.
  [#470](https://github.com/gimli-rs/object/pull/470)

* Added Xtensa architecture support for ELF.
  [#481](https://github.com/gimli-rs/object/pull/481)

* Added `read::pe::ResourceName::raw_data`.
  [#487](https://github.com/gimli-rs/object/pull/487)

--------------------------------------------------------------------------------

## 0.29.0

Released 2022/06/22.

### Breaking changes

* The `write` feature now has a minimum supported rust version of 1.56.1.
  [#444](https://github.com/gimli-rs/object/pull/444)

* Added `os_abi` and `abi_version` fields to `FileFlags::Elf`.
  [#438](https://github.com/gimli-rs/object/pull/438)
  [#441](https://github.com/gimli-rs/object/pull/441)

### Changed

* Fixed handling of empty symbol tables in `read::elf::ElfFile::symbol_table` and
  `read::elf::ElfFile::dynamic_symbol_table`.
  [#443](https://github.com/gimli-rs/object/pull/443)

### Added

* Added more `ELF_OSABI_*` constants.
  [#439](https://github.com/gimli-rs/object/pull/439)

--------------------------------------------------------------------------------

## 0.28.4

Released 2022/05/09.

### Added

* Added `read::pe::DataDirectories::resource_directory`.
  [#425](https://github.com/gimli-rs/object/pull/425)
  [#427](https://github.com/gimli-rs/object/pull/427)

* Added PE support for more ARM relocations.
  [#428](https://github.com/gimli-rs/object/pull/428)

* Added support for `Architecture::LoongArch64`.
  [#430](https://github.com/gimli-rs/object/pull/430)
  [#432](https://github.com/gimli-rs/object/pull/432)

* Added `elf::EF_MIPS_ABI` and associated constants.
  [#433](https://github.com/gimli-rs/object/pull/433)

--------------------------------------------------------------------------------

## 0.28.3

Released 2022/01/19.

### Changed

* For the Mach-O support in `write::Object`, accept `RelocationKind::MachO` for all
  architectures, and accept `RelocationKind::Absolute` for ARM64.
  [#422](https://github.com/gimli-rs/object/pull/422)

### Added

* Added `pe::ImageDataDirectory::file_range`, `read::pe::SectionTable::pe_file_range_at`
  and `pe::ImageSectionHeader::pe_file_range_at`.
  [#421](https://github.com/gimli-rs/object/pull/421)

* Added `write::Object::add_coff_exports`.
  [#423](https://github.com/gimli-rs/object/pull/423)

--------------------------------------------------------------------------------

## 0.28.2

Released 2022/01/09.

### Changed

* Ignored errors for the Wasm extended name section in `read::WasmFile::parse`.
  [#408](https://github.com/gimli-rs/object/pull/408)

* Ignored errors for the COFF symbol table in `read::PeFile::parse`.
  [#410](https://github.com/gimli-rs/object/pull/410)

* Fixed handling of `SectionFlags::Coff` in `write::Object::coff_write`.
  [#412](https://github.com/gimli-rs/object/pull/412)

### Added

* Added `read::ObjectSegment::flags`.
  [#416](https://github.com/gimli-rs/object/pull/416)
  [#418](https://github.com/gimli-rs/object/pull/418)

--------------------------------------------------------------------------------

## 0.28.1

Released 2021/12/12.

### Changed

* Fixed `read::elf::SymbolTable::shndx_section`.
  [#405](https://github.com/gimli-rs/object/pull/405)

* Fixed build warnings.
  [#405](https://github.com/gimli-rs/object/pull/405)
  [#406](https://github.com/gimli-rs/object/pull/406)

--------------------------------------------------------------------------------

## 0.28.0

Released 2021/12/12.

### Breaking changes

* `write_core` feature no longer enables `std` support. Use `write_std` instead.
  [#400](https://github.com/gimli-rs/object/pull/400)

* Multiple changes related to Mach-O split dyld cache support.
  [#398](https://github.com/gimli-rs/object/pull/398)

### Added

* Added `write::pe::Writer::write_file_align`.
  [#397](https://github.com/gimli-rs/object/pull/397)

* Added support for Mach-O split dyld cache.
  [#398](https://github.com/gimli-rs/object/pull/398)

* Added support for `IMAGE_SCN_LNK_NRELOC_OVFL` when reading and writing COFF.
  [#399](https://github.com/gimli-rs/object/pull/399)

* Added `write::elf::Writer::reserve_null_symbol_index`.
  [#402](https://github.com/gimli-rs/object/pull/402)

--------------------------------------------------------------------------------

## 0.27.1

Released 2021/10/22.

### Changed

* Fixed build error with older Rust versions due to cargo resolver version.

--------------------------------------------------------------------------------

## 0.27.0

Released 2021/10/17.

### Breaking changes

* Changed `read::elf` to use `SectionIndex` instead of `usize` in more places.
  [#341](https://github.com/gimli-rs/object/pull/341)

* Changed some `read::elf` section methods to additionally return the linked section index.
  [#341](https://github.com/gimli-rs/object/pull/341)

* Changed `read::pe::ImageNtHeaders::parse` to return `DataDirectories` instead of a slice.
  [#357](https://github.com/gimli-rs/object/pull/357)

* Deleted `value` parameter for `write:WritableBuffer::resize`.
  [#369](https://github.com/gimli-rs/object/pull/369)

* Changed `write::Object` and `write::Section` to use `Cow` for section data.
  This added a lifetime parameter, which existing users can set to `'static`.
  [#370](https://github.com/gimli-rs/object/pull/370)

### Changed

* Fixed parsing when PE import directory has zero size.
  [#341](https://github.com/gimli-rs/object/pull/341)

* Fixed parsing when PE import directory has zero for original first thunk.
  [#385](https://github.com/gimli-rs/object/pull/385)
  [#387](https://github.com/gimli-rs/object/pull/387)

* Fixed parsing when PE export directory has zero number of names.
  [#353](https://github.com/gimli-rs/object/pull/353)

* Fixed parsing when PE export directory has zero number of names and addresses.
  [#362](https://github.com/gimli-rs/object/pull/362)

* Fixed parsing when PE sections are contiguous.
  [#354](https://github.com/gimli-rs/object/pull/354)

* Fixed `std` feature for `indexmap` dependency.
  [#374](https://github.com/gimli-rs/object/pull/374)

* Fixed overflow in COFF section name offset parsing.
  [#390](https://github.com/gimli-rs/object/pull/390)

### Added

* Added `name_bytes` methods to unified `read` traits.
  [#351](https://github.com/gimli-rs/object/pull/351)

* Added `read::Object::kind`.
  [#352](https://github.com/gimli-rs/object/pull/352)

* Added `read::elf::VersionTable` and related helpers.
  [#341](https://github.com/gimli-rs/object/pull/341)

* Added `read::elf::SectionTable::dynamic` and related helpers.
  [#345](https://github.com/gimli-rs/object/pull/345)

* Added `read::coff::SectionTable::max_section_file_offset`.
  [#344](https://github.com/gimli-rs/object/pull/344)

* Added `read::pe::ExportTable` and related helpers.
  [#349](https://github.com/gimli-rs/object/pull/349)
  [#353](https://github.com/gimli-rs/object/pull/353)

* Added `read::pe::ImportTable` and related helpers.
  [#357](https://github.com/gimli-rs/object/pull/357)

* Added `read::pe::DataDirectories` and related helpers.
  [#357](https://github.com/gimli-rs/object/pull/357)
  [#384](https://github.com/gimli-rs/object/pull/384)

* Added `read::pe::RichHeaderInfo` and related helpers.
  [#375](https://github.com/gimli-rs/object/pull/375)
  [#379](https://github.com/gimli-rs/object/pull/379)

* Added `read::pe::RelocationBlocks` and related helpers.
  [#378](https://github.com/gimli-rs/object/pull/378)

* Added `write::elf::Writer`.
  [#350](https://github.com/gimli-rs/object/pull/350)

* Added `write::pe::Writer`.
  [#382](https://github.com/gimli-rs/object/pull/382)
  [#388](https://github.com/gimli-rs/object/pull/388)

* Added `write::Section::data/data_mut`.
  [#367](https://github.com/gimli-rs/object/pull/367)

* Added `write::Object::write_stream`.
  [#369](https://github.com/gimli-rs/object/pull/369)

* Added MIPSr6 ELF header flag definitions.
  [#372](https://github.com/gimli-rs/object/pull/372)

--------------------------------------------------------------------------------

## 0.26.2

Released 2021/08/28.

### Added

* Added support for 64-bit symbol table names to `read::archive`.
  [#366](https://github.com/gimli-rs/object/pull/366)

--------------------------------------------------------------------------------

## 0.26.1

Released 2021/08/19.

### Changed

* Activate `memchr`'s `rustc-dep-of-std` feature
  [#356](https://github.com/gimli-rs/object/pull/356)

--------------------------------------------------------------------------------

## 0.26.0

Released 2021/07/26.

### Breaking changes

* Changed `ReadRef::read_bytes_at_until` to accept a range parameter.
  [#326](https://github.com/gimli-rs/object/pull/326)

* Added `ReadRef` type parameter to `read::StringTable` and types that
  contain it. String table entries are now only read as required.
  [#326](https://github.com/gimli-rs/object/pull/326)

* Changed result type of `read::elf::SectionHeader::data` and `data_as_array`.
  [#332](https://github.com/gimli-rs/object/pull/332)

* Moved `pod::WritableBuffer` to `write::WritableBuffer`.
  Renamed `WritableBuffer::extend` to `write_bytes`.
  Added more provided methods to `WritableBuffer`.
  [#335](https://github.com/gimli-rs/object/pull/335)

* Moved `pod::Bytes` to `read::Bytes`.
  [#336](https://github.com/gimli-rs/object/pull/336)

* Added `is_mips64el` parameter to `elf::Rela64::r_info/set_r_info`.
  [#337](https://github.com/gimli-rs/object/pull/337)

### Changed

* Removed `alloc` dependency when no features are enabled.
  [#336](https://github.com/gimli-rs/object/pull/336)

### Added

* Added `read::pe::PeFile` methods: `section_table`, `data_directory`, and `data`.
  [#324](https://github.com/gimli-rs/object/pull/324)

* Added more ELF definitions.
  [#332](https://github.com/gimli-rs/object/pull/332)

* Added `read::elf::SectionTable` methods for hash tables and symbol version
  information.
  [#332](https://github.com/gimli-rs/object/pull/332)

* Added PE RISC-V definitions.
  [#333](https://github.com/gimli-rs/object/pull/333)

* Added `WritableBuffer` implementation for `Vec`.
  [#335](https://github.com/gimli-rs/object/pull/335)

--------------------------------------------------------------------------------

## 0.25.3

Released 2021/06/12.

### Added

* Added `RelocationEncoding::AArch64Call`.
  [#322](https://github.com/gimli-rs/object/pull/322)

--------------------------------------------------------------------------------

## 0.25.2

Released 2021/06/04.

### Added

* Added `Architecture::X86_64_X32`.
  [#320](https://github.com/gimli-rs/object/pull/320)

--------------------------------------------------------------------------------

## 0.25.1

Released 2021/06/03.

### Changed

* write: Fix choice of `SHT_REL` or `SHT_RELA` for most architectures.
  [#318](https://github.com/gimli-rs/object/pull/318)

* write: Fix relocation encoding for MIPS64EL.
  [#318](https://github.com/gimli-rs/object/pull/318)

--------------------------------------------------------------------------------

## 0.25.0

Released 2021/06/02.

### Breaking changes

* Added `non_exhaustive` to most public enums.
  [#306](https://github.com/gimli-rs/object/pull/306)

* `MachHeader::parse` and `MachHeader::load_commands` now require a header offset.
  [#304](https://github.com/gimli-rs/object/pull/304)

* Added `ReadRef::read_bytes_at_until`.
  [#308](https://github.com/gimli-rs/object/pull/308)

* `PeFile::entry`, `PeSection::address` and `PeSegment::address` now return a
  virtual address instead of a RVA.
  [#315](https://github.com/gimli-rs/object/pull/315)

### Added

* Added `pod::from_bytes_mut`, `pod::slice_from_bytes_mut`, `pod::bytes_of_mut`,
  and `pod::bytes_of_slice_mut`.
  [#296](https://github.com/gimli-rs/object/pull/296)
  [#297](https://github.com/gimli-rs/object/pull/297)

* Added `Object::pdb_info`.
  [#298](https://github.com/gimli-rs/object/pull/298)

* Added `read::macho::DyldCache`, other associated definitions,
  and support for these in the examples.
  [#308](https://github.com/gimli-rs/object/pull/308)

* Added more architecture support.
  [#303](https://github.com/gimli-rs/object/pull/303)
  [#309](https://github.com/gimli-rs/object/pull/309)

* Derive more traits for enums.
  [#311](https://github.com/gimli-rs/object/pull/311)

* Added `Object::relative_address_base`.
  [#315](https://github.com/gimli-rs/object/pull/315)

### Changed

* Improved performance for string parsing.
  [#302](https://github.com/gimli-rs/object/pull/302)

* `objdump` example allows selecting container members.
  [#308](https://github.com/gimli-rs/object/pull/308)
