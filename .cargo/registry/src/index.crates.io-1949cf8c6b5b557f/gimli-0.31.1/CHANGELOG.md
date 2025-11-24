# `gimli` Change Log

--------------------------------------------------------------------------------

## 0.31.1

Released 2024/10/04.

### Changed

* Changed `read::Evaluation::evaluate` to validate `DW_OP_deref_size`.
  [#739](https://github.com/gimli-rs/gimli/pull/739)

* Changed `write::LineProgram` to allow use of file index 0 for DWARF version 5.
  [#740](https://github.com/gimli-rs/gimli/pull/740)

* Improved the workaround for reading zero length entries in `.debug_frame`.
  [#741](https://github.com/gimli-rs/gimli/pull/741)

* Implemented `Default` for `read::DwarfSections` and `read::DwarfPackageSections`.
  [#742](https://github.com/gimli-rs/gimli/pull/742)

* Changed `read::ArangeEntryIter` to handle tombstones in `.debug_aranges`.
  [#743](https://github.com/gimli-rs/gimli/pull/743)

* Improved handling handling of 0 for tombstones in `DW_LNE_set_address`
  and address pairs in ranges and locations.
  [#750](https://github.com/gimli-rs/gimli/pull/750)

* Changed the `read::ArrayLike` trait implementation to use const generics.
  [#752](https://github.com/gimli-rs/gimli/pull/752)

### Added

* Added `MIPS::HI` and `MIPS::LO`.
  [#749](https://github.com/gimli-rs/gimli/pull/749)

--------------------------------------------------------------------------------

## 0.31.0

Released 2024/07/16.

### Breaking changes

* Deleted support for segment selectors.
  [#720](https://github.com/gimli-rs/gimli/pull/720)

* Added `read::FileEntry::source` and deleted `Copy` implementation.
  [#728](https://github.com/gimli-rs/gimli/pull/728)

* Changed `read::LineRow::execute` to return a `Result`.
  [#731](https://github.com/gimli-rs/gimli/pull/731)

* Deleted `Display` implementation for `read::LineInstruction`.
  [#734](https://github.com/gimli-rs/gimli/pull/734)

* Changed `read::Error` to be non-exhaustive.

### Changed

* Fixed `Hash` implementation for `read::EndianReader`.
  [#723](https://github.com/gimli-rs/gimli/pull/723)

* Changed `read::EhFrameHdr::parse` to validate the FDE count encoding.
  [#725](https://github.com/gimli-rs/gimli/pull/725)

* Changed address overflow to be an error for `read::UnwindTableRow`,
  `read::LineRow`, and `read::ArangeEntry`.
  [#730](https://github.com/gimli-rs/gimli/pull/730)
  [#731](https://github.com/gimli-rs/gimli/pull/731)
  [#732](https://github.com/gimli-rs/gimli/pull/732)

* Changed wrapping addition for 32-bit addresses to wrap at 32 bits instead of
  at 64 bits.
  [#733](https://github.com/gimli-rs/gimli/pull/733)

* Added earlier validation of address sizes.
  [#733](https://github.com/gimli-rs/gimli/pull/733)

### Added

* Added `read::IndexSectionId::section_id`.
  [#719](https://github.com/gimli-rs/gimli/pull/719)

* Added `read::FrameDescriptionEntry::end_address`.
  [#727](https://github.com/gimli-rs/gimli/pull/727)

* Added support for `DW_LNCT_LLVM_source`.
  [#728](https://github.com/gimli-rs/gimli/pull/728)

--------------------------------------------------------------------------------

## 0.30.0

Released 2024/05/26.

### Breaking changes

* Added context to some `read::Error` variants.
  [#703](https://github.com/gimli-rs/gimli/pull/703)

* Changed type of `read::UnitIndexSection::section` to `IndexSectionId`.
  [#716](https://github.com/gimli-rs/gimli/pull/716)

### Changed

* Fixed `write::Operation::ImplicitPointer::size`.
  [#712](https://github.com/gimli-rs/gimli/pull/712)

* Changed `read::RngListIter` and `read::LocListIter` to skip ranges where
  the end is before the beginning, instead of returning an error.
  [#715](https://github.com/gimli-rs/gimli/pull/715)

* Fixed clippy warnings.
  [#713](https://github.com/gimli-rs/gimli/pull/713)

### Added

* Added `read::UnitRef`.
  [#711](https://github.com/gimli-rs/gimli/pull/711)

--------------------------------------------------------------------------------

## 0.29.0

Released 2024/04/11.

### Breaking changes

* Changed `Reader` type parameter to `ReaderOffset` for `read::UnwindContext` and related types.
  Replaced `Expression` with `UnwindExpression` in unwind information types.
  [#703](https://github.com/gimli-rs/gimli/pull/703)

### Changed

* Changed `write::Sections::for_each` and `for_each_mut` to specify section lifetime.
  [#699](https://github.com/gimli-rs/gimli/pull/699)

* Fixed writing unwind information with an LSDA encoding that is not `DW_EH_PE_absptr`.
  [#704](https://github.com/gimli-rs/gimli/pull/704)

* Fixed parsing for an empty DWP index.
  [#706](https://github.com/gimli-rs/gimli/pull/706)

* Improved error handling in `read::Unit::dwo_name`.
  [#693](https://github.com/gimli-rs/gimli/pull/693)

* Fixed warnings.
  [#692](https://github.com/gimli-rs/gimli/pull/692)
  [#694](https://github.com/gimli-rs/gimli/pull/694)
  [#695](https://github.com/gimli-rs/gimli/pull/695)
  [#696](https://github.com/gimli-rs/gimli/pull/696)

### Added

* Added MIPS register definitions.
  [#690](https://github.com/gimli-rs/gimli/pull/690)

* Added PowerPC register definitions.
  [#691](https://github.com/gimli-rs/gimli/pull/691)

* Added `read::DwarfSections` and `read::DwarfPackageSections`.
  [#698](https://github.com/gimli-rs/gimli/pull/698)

* Implemented `BitOr` for `DwEhPe`.
  [#709](https://github.com/gimli-rs/gimli/pull/709)

* Added `read::Relocate`, `read::RelocateReader`, and `write::RelocateWriter`.
  [#709](https://github.com/gimli-rs/gimli/pull/709)

--------------------------------------------------------------------------------

## 0.28.1

Released 2023/11/24.

### Changed

* Changed `read::AbbreviationsCache` to require manual population using
  `Dwarf::populate_abbreviations_cache`.
  [#679](https://github.com/gimli-rs/gimli/pull/679)

* Changed the default `read::UnwindContextStorage` to use `Box` instead of `Vec`
  so that its memory usage is limited.
  [#687](https://github.com/gimli-rs/gimli/pull/687)

* Changed `read::UnwindTable::new` to always reset the context, because
  previous errors may have left the context in an invalid state.
  [#684](https://github.com/gimli-rs/gimli/pull/684)

* Changed the `Debug` implementation for `read::EndianSlice` to limit the number
  of bytes it displays.
  [#686](https://github.com/gimli-rs/gimli/pull/686)

### Added

* Added more AArch64 register definitions.
  [#680](https://github.com/gimli-rs/gimli/pull/680)

* Added `read::Unit::new_with_abbreviations`.
  [#677](https://github.com/gimli-rs/gimli/pull/677)

* Added `read::Evaluation::value_result`.
  [#676](https://github.com/gimli-rs/gimli/pull/676)

--------------------------------------------------------------------------------

## 0.28.0

Released 2023/08/12.

### Breaking changes

* Deleted `impl From<EndianSlice> for &[u8]`. Use `EndianSlice::slice` instead.
  [#669](https://github.com/gimli-rs/gimli/pull/669)

* Deleted `impl Index<usize> for EndianSlice` and
  `impl Index<RangeFrom<usize>> for EndianSlice`.
  [#669](https://github.com/gimli-rs/gimli/pull/669)

* Replaced `impl From<Pointer> for u64` with `Pointer::pointer`.
  [#670](https://github.com/gimli-rs/gimli/pull/670)

* Updated `fallible-iterator` to 0.3.0.
  [#672](https://github.com/gimli-rs/gimli/pull/672)

* Changed some optional dependencies to use the `dep:` feature syntax.
  [#672](https://github.com/gimli-rs/gimli/pull/672)

* Added `non_exhaustive` attribute to `read::RegisterRule`,
  `read::CallFrameInstruction`, and `write::CallFrameInstruction`.
  [#673](https://github.com/gimli-rs/gimli/pull/673)

### Changed

* The minimum supported rust version for the `read` feature and its dependencies
  increased to 1.60.0.

* The minimum supported rust version for other features increased to 1.65.0.

### Added

* Added `Vendor`, `read::DebugFrame::set_vendor`, and `read::EhFrame::set_vendor`.
  [#673](https://github.com/gimli-rs/gimli/pull/673)

* Added more ARM and AArch64 register definitions, and
  `DW_CFA_AARCH64_negate_ra_state` support.
  [#673](https://github.com/gimli-rs/gimli/pull/673)

--------------------------------------------------------------------------------

## 0.27.3

Released 2023/06/14.

### Changed

* Excluded test fixtures from published package.
  [#661](https://github.com/gimli-rs/gimli/pull/661)

### Added

* Added `FallibleIterator` implementation for `read::OperationIter`.
  [#649](https://github.com/gimli-rs/gimli/pull/649)

* Added `DW_AT_GNU_deleted` constant.
  [#658](https://github.com/gimli-rs/gimli/pull/658)

--------------------------------------------------------------------------------

## 0.27.2

Released 2023/02/15.

### Added

* Added support for tombstones in `read::LineRows`.
  [#642](https://github.com/gimli-rs/gimli/pull/642)

--------------------------------------------------------------------------------

## 0.27.1

Released 2023/01/23.

### Added

* Added `SectionId::xcoff_name` and `read::Section::xcoff_section_name`.
  [#635](https://github.com/gimli-rs/gimli/pull/635)

* Added `read::Dwarf::make_dwo` and `read::Unit::dwo_name`.
  [#637](https://github.com/gimli-rs/gimli/pull/637)

### Changed

* Changed `read::DwarfPackage::sections` to handle supplementary files.
  [#638](https://github.com/gimli-rs/gimli/pull/638)

--------------------------------------------------------------------------------

## 0.27.0

Released 2022/11/23.

### Breaking changes

* Added `read::Dwarf::abbreviations_cache` to cache abbreviations at offset 0.
  Changed `read::Dwarf::abbreviations` to return `Result<Arc<Abbreviations>>`,
  and changed `read::Unit::abbreviations` to `Arc<Abbreviations>`.
  [#628](https://github.com/gimli-rs/gimli/pull/628)

### Added

* Added LoongArch register definitions.
  [#624](https://github.com/gimli-rs/gimli/pull/624)

* Added support for tombstones in `read::LocListIter` and `read::RngListIter`.
  [#631](https://github.com/gimli-rs/gimli/pull/631)

--------------------------------------------------------------------------------

## 0.26.2

Released 2022/07/16.

### Changed

* Fixed CFI personality encoding when writing.
  [#609](https://github.com/gimli-rs/gimli/pull/609)

* Fixed use of raw pointer for mutation, detected by Miri.
  [#614](https://github.com/gimli-rs/gimli/pull/614)

* Fixed `DW_OP_GNU_implicit_pointer` handling for DWARF version 2.
  [#618](https://github.com/gimli-rs/gimli/pull/618)

### Added

* Added `read::EhHdrTable::iter`.
  [#619](https://github.com/gimli-rs/gimli/pull/619)

--------------------------------------------------------------------------------

## 0.26.1

Released 2021/11/02.

### Changed

* Fixed segmentation fault in `ArrayVec<Vec<T>>::into_vec`, which may be used by
  `read::Evaluation::result`. This regression was introduced in 0.26.0.
  [#601](https://github.com/gimli-rs/gimli/pull/601)

--------------------------------------------------------------------------------

## 0.26.0

Released 2021/10/24.

### Breaking changes

* Removed `read::UninitializedUnwindContext`. Use `Box<UnwindContext>` instead.
  [#593](https://github.com/gimli-rs/gimli/pull/593)

* Renamed `read::Error::CfiStackFull` to `StackFull`.
  [#595](https://github.com/gimli-rs/gimli/pull/595)

* Added `UnwindContextStorage` type parameter to `read::UnwindContext`, `read::UnwindTable`,
  `read::UnwindTableRow`, and `read::RegisterRuleMap`.
  [#595](https://github.com/gimli-rs/gimli/pull/595)

* Added `EvaluationStorage` type parameter to `read::Evaluation`.
  [#595](https://github.com/gimli-rs/gimli/pull/595)

* Added `read::SectionId::DebugCuIndex` and `read::SectionId::DebugTuIndex`.
  [#588](https://github.com/gimli-rs/gimli/pull/588)

### Changed

* Fixed `DW_EH_PE_pcrel` handling in default `write::Writer::write_eh_pointer` implementation.
  [#576](https://github.com/gimli-rs/gimli/pull/576)

* Fixed `read::AttributeSpecification::size` for some forms.
  [#597](https://github.com/gimli-rs/gimli/pull/597)

* Display more unit details in dwarfdump.
  [#584](https://github.com/gimli-rs/gimli/pull/584)

### Added

* Added `write::DebuggingInformationEntry::delete_child`.
  [#570](https://github.com/gimli-rs/gimli/pull/570)

* Added ARM and AArch64 register definitions.
  [#574](https://github.com/gimli-rs/gimli/pull/574)
  [#577](https://github.com/gimli-rs/gimli/pull/577)

* Added RISC-V register definitions.
  [#579](https://github.com/gimli-rs/gimli/pull/579)

* Added `read::DwarfPackage`, `read::DebugCuIndex`, and `read::DebugTuIndex`.
  [#588](https://github.com/gimli-rs/gimli/pull/588)

* Added `read-core` feature to allow building without `liballoc`.
  [#596](https://github.com/gimli-rs/gimli/pull/596)

* Added `read::EntriesRaw::skip_attributes`.
  [#597](https://github.com/gimli-rs/gimli/pull/597)

--------------------------------------------------------------------------------

## 0.25.0

Released 2021/07/26.

### Breaking changes

* `read::FrameDescriptionEntry::unwind_info_for_address` now returns a reference
  instead of cloning.
  [#557](https://github.com/gimli-rs/gimli/pull/557)

* `read::AttributeValue::RangeListsRef` now contains a `RawRangeListsOffset`
  to allow handling of GNU split DWARF extensions.
  Use `read::Dwarf::ranges_offset_from_raw` to handle it.
  [#568](https://github.com/gimli-rs/gimli/pull/568)
  [#569](https://github.com/gimli-rs/gimli/pull/569)

* Added `read::Unit::dwo_id`.
  [#569](https://github.com/gimli-rs/gimli/pull/569)

### Changed

* `.debug_aranges` parsing now accepts version 3.
  [#560](https://github.com/gimli-rs/gimli/pull/560)

* `read::Dwarf::attr_ranges_offset` and its callers now handle GNU split DWARF extensions.
  [#568](https://github.com/gimli-rs/gimli/pull/568)
  [#569](https://github.com/gimli-rs/gimli/pull/569)

### Added

* Added `read::DebugLineStr::new`.
  [#556](https://github.com/gimli-rs/gimli/pull/556)

* Added `read::UnwindTable::into_current_row`.
  [#557](https://github.com/gimli-rs/gimli/pull/557)

* Added more `DW_LANG` constants.
  [#565](https://github.com/gimli-rs/gimli/pull/565)

* dwarfdump: added DWO parent support.
  [#568](https://github.com/gimli-rs/gimli/pull/568)

* Added `read::Dwarf` methods: `ranges_offset_from_raw`, `raw_ranges`, and `raw_locations`.
  [#568](https://github.com/gimli-rs/gimli/pull/568)
  [#569](https://github.com/gimli-rs/gimli/pull/569)

--------------------------------------------------------------------------------

## 0.24.0

Released 2021/05/01.

### Breaking changes

* Minimum Rust version increased to 1.42.0.

* Added `read::Dwarf::debug_aranges`.
  [#539](https://github.com/gimli-rs/gimli/pull/539)

* Replaced `read::DebugAranges::items` with `read::DebugAranges::headers`.
  [#539](https://github.com/gimli-rs/gimli/pull/539)

* Added `read::Operation::Wasm*`.
  [#546](https://github.com/gimli-rs/gimli/pull/546)

* `read::LineRow::line` now returns `Option<NonZeroU64>`.
  The `read::ColumnType::Column` variant now contains a `NonZeroU64`.
  [#551](https://github.com/gimli-rs/gimli/pull/551)

* Replaced `read::Dwarf::debug_str_sup` with `read::Dwarf::sup`.
  Deleted `sup` parameter of `read::Dwarf::load`.
  Added `read::Dwarf::load_sup`.
  [#554](https://github.com/gimli-rs/gimli/pull/554)

### Added

* dwarfdump: Supplementary object file support.
  [#552](https://github.com/gimli-rs/gimli/pull/552)

### Changed

* Support `DW_FORM_addrx*` for `DW_AT_low_pc`/`DW_AT_high_pc` in `read::Dwarf`.
  [#541](https://github.com/gimli-rs/gimli/pull/541)

* Performance improvement in `EndianReader`.
  [#549](https://github.com/gimli-rs/gimli/pull/549)

--------------------------------------------------------------------------------

## 0.23.0

Released 2020/10/27.

### Breaking changes

* Added more variants to `read::UnitType`.
  Added `read::AttributeValue::DwoId`
  [#521](https://github.com/gimli-rs/gimli/pull/521)

* Replaced `CompilationUnitHeader` and `TypeUnitHeader` with `UnitHeader`.
  Replaced `CompilationUnitHeadersIter` with `DebugInfoUnitHeadersIter`.
  Replaced `TypeUnitHeadersIter` with `DebugTypesUnitHeadersIter`.
  [#523](https://github.com/gimli-rs/gimli/pull/523)


### Added

* Added read support for split DWARF.
  [#527](https://github.com/gimli-rs/gimli/pull/527)
  [#529](https://github.com/gimli-rs/gimli/pull/529)

* Added `read::Dwarf::attr_address`.
  [#524](https://github.com/gimli-rs/gimli/pull/524)

* Added read support for `DW_AT_GNU_addr_base` and `DW_AT_GNU_ranges_base`.
  [#525](https://github.com/gimli-rs/gimli/pull/525)

* dwarfdump: Display index values for attributes.
  [#526](https://github.com/gimli-rs/gimli/pull/526)

* Added `name_to_register`.
  [#532](https://github.com/gimli-rs/gimli/pull/532)

--------------------------------------------------------------------------------

## 0.22.0

Released 2020/07/03.

### Breaking changes

* Fixed `UnitHeader::size_of_header` for DWARF 5 units.
  [#518](https://github.com/gimli-rs/gimli/pull/518)

### Added

* Added fuzz targets in CI.
  [#512](https://github.com/gimli-rs/gimli/pull/512)

* Added read support for `DW_OP_GNU_addr_index` and `DW_OP_GNU_const_index`.
  [#516](https://github.com/gimli-rs/gimli/pull/516)

* Added `.dwo` support to dwarfdump.
  [#516](https://github.com/gimli-rs/gimli/pull/516)

* Added `SectionId::dwo_name` and `Section::dwo_section_name`.
  [#517](https://github.com/gimli-rs/gimli/pull/517)

### Fixed

* Fixed panic when reading `DW_FORM_indirect` combined with `DW_FORM_implicit_const`.
  [#502](https://github.com/gimli-rs/gimli/pull/502)

* Fixed panic for `read::Abbreviations::get(0)`.
  [#505](https://github.com/gimli-rs/gimli/pull/505)

* Fixed arithmetic overflow when reading `.debug_line`.
  [#508](https://github.com/gimli-rs/gimli/pull/508)

* Fixed arithmetic overflow when reading CFI.
  [#509](https://github.com/gimli-rs/gimli/pull/509)

* Fixed arithmetic overflow and division by zero when reading `.debug_aranges`.
  [#510](https://github.com/gimli-rs/gimli/pull/510)

* Don't return error from `read::Unit::new` when `DW_AT_name` or `DW_AT_comp_dir` is missing.
  [#515](https://github.com/gimli-rs/gimli/pull/515)

--------------------------------------------------------------------------------

## 0.21.0

Released 2020/05/12.

### Breaking changes

* Minimum Rust version increased to 1.38.0.

* Replaced `read::Operation::Literal` with `Operation::UnsignedConstant` and `Operation::SignedConstant`.
  Changed `read::Operation::Bra` and `read::Operation::Skip` to contain the target offset instead of the bytecode.
  [#479](https://github.com/gimli-rs/gimli/pull/479)

* Changed `write::Expression` to support references. Existing users can convert to use `Expression::raw`.
  [#479](https://github.com/gimli-rs/gimli/pull/479)

* Replaced `write::AttributeValue::AnyUnitEntryRef` with `DebugInfoRef`.
  Renamed `write::AttributeValue::ThisUnitEntryRef` to `UnitRef`.
  [#479](https://github.com/gimli-rs/gimli/pull/479)

* Added more optional features: `endian-reader` and `fallible-iterator`.
  [#495](https://github.com/gimli-rs/gimli/pull/495)
  [#498](https://github.com/gimli-rs/gimli/pull/498)

### Added

* Added `read::Expression::operations`
  [#479](https://github.com/gimli-rs/gimli/pull/479)

### Fixed

* Fixed newlines in `dwarfdump` example.
  [#470](https://github.com/gimli-rs/gimli/pull/470)

* Ignore zero terminators when reading `.debug_frame` sections.
  [#486](https://github.com/gimli-rs/gimli/pull/486)

* Increase the number of CFI register rules supported by `read::UnwindContext`.
  [#487](https://github.com/gimli-rs/gimli/pull/487)

* Fixed version handling and return register encoding when reading `.eh_frame` sections.
  [#493](https://github.com/gimli-rs/gimli/pull/493)

### Changed

* Added `EhFrame` and `DebugFrame` to `write::Sections`.
  [#492](https://github.com/gimli-rs/gimli/pull/492)

* Improved performance of `write::LineProgram::generate_row`.
  [#476](https://github.com/gimli-rs/gimli/pull/476)

* Removed use of the `byteorder`, `arrayvec` and `smallvec` crates.
  [#494](https://github.com/gimli-rs/gimli/pull/494)
  [#496](https://github.com/gimli-rs/gimli/pull/496)
  [#497](https://github.com/gimli-rs/gimli/pull/497)

--------------------------------------------------------------------------------

## 0.20.0

Released 2020/01/11.

### Breaking changes

* Changed type of `DwTag`, `DwAt`, and `DwForm` constants.
  [#451](https://github.com/gimli-rs/gimli/pull/451)

* Added `read/write::AttributeValue::DebugMacroRef`, and returned where
  required in `read::Attribute::value`. Added `SectionId::DebugMacro`.
  [#454](https://github.com/gimli-rs/gimli/pull/454)

* Deleted `alloc` feature, and fixed `no-std` builds with stable rust.
  [#459](https://github.com/gimli-rs/gimli/pull/459)

* Deleted `read::Error::description`, and changed `<read::Error as Display>`
  to display what was previously the description.
  [#462](https://github.com/gimli-rs/gimli/pull/462)

### Added

* Added GNU view constants.
  [#434](https://github.com/gimli-rs/gimli/pull/434)

* Added `read::EntriesRaw` for low level DIE parsing.
  [#455](https://github.com/gimli-rs/gimli/pull/455)

* Added `examples/simple-line.rs`.
  [#460](https://github.com/gimli-rs/gimli/pull/460)

### Fixed

* Fixed handling of CFI augmentations without data.
  [#438](https://github.com/gimli-rs/gimli/pull/438)

* dwarfdump: fix panic for malformed expressions.
  [#447](https://github.com/gimli-rs/gimli/pull/447)

* dwarfdump: fix handling of Mach-O relocations.
  [#449](https://github.com/gimli-rs/gimli/pull/449)

### Changed

* Improved abbreviation parsing performance.
  [#451](https://github.com/gimli-rs/gimli/pull/451)

--------------------------------------------------------------------------------

## 0.19.0

Released 2019/07/08.

### Breaking changes

* Small API changes related to `.debug_loc` and `.debug_loclists`:
  added `read::RawLocListEntry::AddressOrOffsetPair` enum variant,
  added `write::Sections::debug_loc/debug_loclists` public members,
  and replaced `write::AttributeValue::LocationListsRef` with `LocationListRef`.
  [#425](https://github.com/gimli-rs/gimli/pull/425)

### Added

* Added `read::Attribute::exprloc_value` and `read::AttributeValue::exprloc_value`.
  [#422](https://github.com/gimli-rs/gimli/pull/422)

* Added support for writing `.debug_loc` and `.debug_loclists` sections.
  [#425](https://github.com/gimli-rs/gimli/pull/425)

* Added `-G` flag to `dwarfdump` example to display global offsets.
  [#427](https://github.com/gimli-rs/gimli/pull/427)

* Added `examples/simple.rs`.
  [#429](https://github.com/gimli-rs/gimli/pull/429)

### Fixed

* `write::LineProgram::from` no longer requires `DW_AT_name` or `DW_AT_comp_dir`
  attributes to be present in the unit DIE.
  [#430](https://github.com/gimli-rs/gimli/pull/430)

--------------------------------------------------------------------------------

## 0.18.0

Released 2019/04/25.

The focus of this release has been on improving support for reading CFI,
and adding support for writing CFI.

### Breaking changes

* For types which have an `Offset` type parameter, the default `Offset`
  has changed from `usize` to `R::Offset`.
  [#392](https://github.com/gimli-rs/gimli/pull/392)

* Added an `Offset` type parameter to the `read::Unit` type to allow variance.
  [#393](https://github.com/gimli-rs/gimli/pull/393)

* Changed the `UninitializedUnwindContext::initialize` method to borrow `self`,
  and return `&mut UnwindContext`. Deleted the `InitializedUnwindContext` type.
  [#395](https://github.com/gimli-rs/gimli/pull/395)

* Deleted the `UnwindSection` type parameters from the `CommonInformationEntry`,
  `FrameDescriptionEntry`, `UninitializedUnwindContext`,
  `UnwindContext`, and `UnwindTable` types.
  [#399](https://github.com/gimli-rs/gimli/pull/399)

* Changed the signature of the `get_cie` callback parameter for various functions.
  The signature now matches the `UnwindSection::cie_from_offset` method, so
  that method can be used as the parameter.
  [#400](https://github.com/gimli-rs/gimli/pull/400)

* Reduced the number of lifetime parameters for the `UnwindTable` type.
  [#400](https://github.com/gimli-rs/gimli/pull/400)

* Updated `fallible-iterator` to version 0.2.0.
  [#407](https://github.com/gimli-rs/gimli/pull/407)

* Added a parameter to the `Error::UnexpectedEof` enum variant.
  [#408](https://github.com/gimli-rs/gimli/pull/408)

### Added

* Update to 2018 edition.
  [#391](https://github.com/gimli-rs/gimli/pull/391)

* Added the `FrameDescriptionEntry::unwind_info_for_address` method.
  [#396](https://github.com/gimli-rs/gimli/pull/396)

* Added the `FrameDescriptionEntry::rows` method.
  [#396](https://github.com/gimli-rs/gimli/pull/396)

* Added the `EhHdrTable::unwind_info_for_address` method.
  [#400](https://github.com/gimli-rs/gimli/pull/400)

* Added the `EhHdrTable::fde_for_address` method and deprecated the
  `EhHdrTable::lookup_and_parse` method.
  [#400](https://github.com/gimli-rs/gimli/pull/400)

* Added the `EhHdrTable::pointer_to_offset` method.
  [#400](https://github.com/gimli-rs/gimli/pull/400)

* Added the `UnwindSection::fde_for_address` method.
  [#396](https://github.com/gimli-rs/gimli/pull/396)

* Added the `UnwindSection::fde_from_offset` method.
  [#400](https://github.com/gimli-rs/gimli/pull/400)

* Added the `UnwindSection::partial_fde_from_offset` method.
  [#400](https://github.com/gimli-rs/gimli/pull/400)

* Added the `Section::id` method.
  [#406](https://github.com/gimli-rs/gimli/pull/406)

* Added the `Dwarf::load` method, and corresponding methods for individual sections.
  [#406](https://github.com/gimli-rs/gimli/pull/406)

* Added the `Dwarf::borrow` method, and corresponding methods for individual sections.
  [#406](https://github.com/gimli-rs/gimli/pull/406)

* Added the `Dwarf::format_error` method.
  [#408](https://github.com/gimli-rs/gimli/pull/408)

* Added the `Dwarf::die_ranges` method.
  [#417](https://github.com/gimli-rs/gimli/pull/417)

* Added the `Dwarf::unit_ranges` method.
  [#417](https://github.com/gimli-rs/gimli/pull/417)

* Added support for writing `.debug_frame` and `.eh_frame` sections.
  [#412](https://github.com/gimli-rs/gimli/pull/412)
  [#419](https://github.com/gimli-rs/gimli/pull/419)

### Fixed

* The `code_alignment_factor` is now used when evaluating CFI instructions
  that advance the location.
  [#401](https://github.com/gimli-rs/gimli/pull/401)

* Fixed parsing of pointers encoded with `DW_EH_PE_funcrel`.
  [#402](https://github.com/gimli-rs/gimli/pull/402)

* Use the FDE address encoding from the augmentation when parsing `DW_CFA_set_loc`.
  [#403](https://github.com/gimli-rs/gimli/pull/403)

* Fixed setting of `.eh_frame` base addresses in dwarfdump.
  [#410](https://github.com/gimli-rs/gimli/pull/410)

## 0.17.0

Released 2019/02/21.

The focus of this release has been on improving DWARF 5 support, and
adding support for writing DWARF.

### Breaking changes

* Changed register values to a `Register` type instead of `u8`/`u64`.
  [#328](https://github.com/gimli-rs/gimli/pull/328)

* Replaced `BaseAddresses::set_cfi` with `set_eh_frame_hdr` and `set_eh_frame`.
  Replaced `BaseAddresses::set_data` with `set_got`.
  You should now use the same `BaseAddresses` value for parsing both
  `.eh_frame` and `.eh_frame_hdr`.
  [#351](https://github.com/gimli-rs/gimli/pull/351)

* Renamed many types and functions related to `.debug_line`.
  Renamed `LineNumberProgram` to `LineProgram`.
  Renamed `IncompleteLineNumberProgram` to `IncompleteLineProgram`.
  Renamed `CompleteLineNumberProgram` to `CompleteLineProgram`.
  Renamed `LineNumberProgramHeader` to `LineProgramHeader`.
  Renamed `LineNumberRow` to `LineRow`.
  Renamed `StateMachine` to `LineRows`.
  Renamed `Opcode` to `LineInstruction`.
  Renamed `OpcodesIter` to `LineInstructions`.
  Renamed `LineNumberSequence` to `LineSequence`.
  [#359](https://github.com/gimli-rs/gimli/pull/359)

* Added `Offset` type parameter to `AttributeValue`, `LineProgram`,
  `IncompleteLineProgram`, `CompleteLineProgram`, `LineRows`, `LineInstruction`,
  and `FileEntry`.
  [#324](https://github.com/gimli-rs/gimli/pull/324)

* Changed `FileEntry::path_name`, `FileEntry::directory`, and
  `LineProgramHeader::directory` to return an `AttributeValue` instead
  of a `Reader`.
  [#366](https://github.com/gimli-rs/gimli/pull/366)

* Renamed `FileEntry::last_modification` to `FileEntry::timestamp`
  and renamed `FileEntry::length` to `FileEntry::size`.
  [#366](https://github.com/gimli-rs/gimli/pull/366)

* Added an `Encoding` type. Changed many functions that previously accepted
  `Format`, version or address size parameters to accept an `Encoding`
  parameter instead.
  Notable changes are `LocationLists::locations`, `RangeLists::ranges`,
  and `Expression::evaluation`.
  [#364](https://github.com/gimli-rs/gimli/pull/364)

* Changed return type of `LocationLists::new` and `RangeLists::new`.
  [#370](https://github.com/gimli-rs/gimli/pull/370)

* Added parameters to `LocationsLists::locations` and `RangeLists::ranges`
  to support `.debug_addr`.
  [#358](https://github.com/gimli-rs/gimli/pull/358)

* Added more `AttributeValue` variants: `DebugAddrBase`, `DebugAddrIndex`,
  `DebugLocListsBase`, `DebugLocListsIndex`, `DebugRngListsBase`, `DebugRngListsIndex`,
  `DebugStrOffsetsBase`, `DebugStrOffsetsIndex`, `DebugLineStrRef`.
  [#358](https://github.com/gimli-rs/gimli/pull/358)

* Changed `AttributeValue::Data*` attributes to native endian integers instead
  of byte arrays.
  [#365](https://github.com/gimli-rs/gimli/pull/365)

* Replaced `EvaluationResult::TextBase` with
  `EvaluationResult::RequiresRelocatedAddress`. The handling of `TextBase`
  was incorrect.
  [#335](https://github.com/gimli-rs/gimli/pull/335)

* Added `EvaluationResult::IndexedAddress` for operations that require an
  address from `.debug_addr`.
  [#358](https://github.com/gimli-rs/gimli/pull/358)

* Added `Reader::read_slice`. Added a default implementation of
  `Reader::read_u8_array` which uses this.
  [#358](https://github.com/gimli-rs/gimli/pull/358)

### Added

* Added initial support for writing DWARF. This is targeted at supporting
  line number information only.
  [#340](https://github.com/gimli-rs/gimli/pull/340)
  [#344](https://github.com/gimli-rs/gimli/pull/344)
  [#346](https://github.com/gimli-rs/gimli/pull/346)
  [#361](https://github.com/gimli-rs/gimli/pull/361)
  [#362](https://github.com/gimli-rs/gimli/pull/362)
  [#365](https://github.com/gimli-rs/gimli/pull/365)
  [#368](https://github.com/gimli-rs/gimli/pull/368)
  [#382](https://github.com/gimli-rs/gimli/pull/382)

* Added `read` and `write` Cargo features. Both are enabled by default.
  [#343](https://github.com/gimli-rs/gimli/pull/343)

* Added support for reading DWARF 5 `.debug_line` and `.debug_line_str` sections.
  [#366](https://github.com/gimli-rs/gimli/pull/366)

* Added support for reading DWARF 5 `.debug_str_offsets` sections, including
  parsing `DW_FORM_strx*` attributes.
  [#358](https://github.com/gimli-rs/gimli/pull/358)

* Added support for reading DWARF 5 `.debug_addr` sections, including parsing
  `DW_FORM_addrx*` attributes and evaluating `DW_OP_addrx` and `DW_OP_constx`
  operations.
  [#358](https://github.com/gimli-rs/gimli/pull/358)

* Added support for reading DWARF 5 indexed addresses and offsets in
  `.debug_loclists` and `.debug_rnglists`, including parsing `DW_FORM_rnglistx`
  and `DW_FORM_loclistx` attributes.
  [#358](https://github.com/gimli-rs/gimli/pull/358)

* Added high level `Dwarf` and `Unit` types. Existing code does not need to
  switch to using these types, but doing so will make DWARF 5 support simpler.
  [#352](https://github.com/gimli-rs/gimli/pull/352)
  [#380](https://github.com/gimli-rs/gimli/pull/380)
  [#381](https://github.com/gimli-rs/gimli/pull/381)

* Added `EhFrame::set_address_size` and `DebugFrame::set_address_size` methods
  to allow parsing non-native CFI sections. The default address size is still
  the native size.
  [#325](https://github.com/gimli-rs/gimli/pull/325)

* Added architecture specific definitions for `Register` values and names.
  Changed dwarfdump to print them.
  [#328](https://github.com/gimli-rs/gimli/pull/328)

* Added support for reading relocatable DWARF sections.
  [#337](https://github.com/gimli-rs/gimli/pull/337)

* Added parsing of `DW_FORM_data16`.
  [#366](https://github.com/gimli-rs/gimli/pull/366)

### Fixed

* Fixed parsing DWARF 5 ranges with `start == end == 0`.
  [#323](https://github.com/gimli-rs/gimli/pull/323)

* Changed `LineRows` to be covariant in its `Reader` type parameter.
  [#324](https://github.com/gimli-rs/gimli/pull/324)

* Fixed handling of empty units in dwarfdump.
  [#330](https://github.com/gimli-rs/gimli/pull/330)

* Fixed `UnitHeader::length_including_self` for `Dwarf64`.
  [#342](https://github.com/gimli-rs/gimli/pull/342)

* Fixed parsing of `DW_CFA_set_loc`.
  [#355](https://github.com/gimli-rs/gimli/pull/355)

* Fixed handling of multiple headers in `.debug_loclists` and `.debug_rnglists`.
  [#370](https://github.com/gimli-rs/gimli/pull/370)

--------------------------------------------------------------------------------

## 0.16.1

Released 2018/08/28.

### Added

* Added `EhFrameHdr::lookup_and_parse`. [#316][]
* Added support for `DW_CFA_GNU_args_size`. [#319][]

### Fixed

* Implement `Send`/`Sync` for `SubRange`. [#305][]
* Fixed `alloc` support on nightly. [#306][] [#310][]

[#305]: https://github.com/gimli-rs/gimli/pull/305
[#306]: https://github.com/gimli-rs/gimli/pull/306
[#310]: https://github.com/gimli-rs/gimli/pull/310
[#316]: https://github.com/gimli-rs/gimli/pull/316
[#319]: https://github.com/gimli-rs/gimli/pull/319

--------------------------------------------------------------------------------

## 0.16.0

Released 2018/06/01.

### Added

* Added support for building in `#![no_std]` environments, when the `alloc`
  crate is available. Disable the "std" feature and enable the "alloc"
  feature. [#138][] [#271][]

* Added support for DWARF 5 `.debug_rnglists` and `.debug_loclists`
  sections. [#272][]

* Added support for DWARF 5 `DW_FORM_ref_sup` and `DW_FORM_strp_sup` attribute
  forms. [#288][]

* Added support for DWARF 5 operations on typed values. [#293][]

* A `dwarf-validate` example program that checks the integrity of the given
  DWARF and its references between sections. [#290][]

* Added the `EndianReader<T>` type, an easy way to define a custom `Reader`
  implementation with a reference to a generic buffer of bytes and an associated
  endianity. [#298][] [#302][]

### Changed

* Various speed improvements for evaluating `.debug_line` line number
  programs. [#276][]

* The example `dwarfdump` clone is a [whole lot faster
  now][dwarfdump-faster]. [#282][] [#284][] [#285][]

### Deprecated

* `EndianBuf` has been renamed to `EndianSlice`, use that name instead. [#295][]

### Fixed

* Evaluating the `DW_CFA_restore_state` opcode properly maintains the current
  location. Previously it would incorrectly restore the old location when
  popping from evaluation stack. [#274][]

[#271]: https://github.com/gimli-rs/gimli/issues/271
[#138]: https://github.com/gimli-rs/gimli/issues/138
[#274]: https://github.com/gimli-rs/gimli/issues/274
[#272]: https://github.com/gimli-rs/gimli/issues/272
[#276]: https://github.com/gimli-rs/gimli/issues/276
[#282]: https://github.com/gimli-rs/gimli/issues/282
[#285]: https://github.com/gimli-rs/gimli/issues/285
[#284]: https://github.com/gimli-rs/gimli/issues/284
[#288]: https://github.com/gimli-rs/gimli/issues/288
[#290]: https://github.com/gimli-rs/gimli/issues/290
[#293]: https://github.com/gimli-rs/gimli/issues/293
[#295]: https://github.com/gimli-rs/gimli/issues/295
[#298]: https://github.com/gimli-rs/gimli/issues/298
[#302]: https://github.com/gimli-rs/gimli/issues/302
[dwarfdump-faster]: https://robert.ocallahan.org/2018/03/speeding-up-dwarfdump-with-rust.html

--------------------------------------------------------------------------------

## 0.15.0

Released 2017/12/01.

### Added

* Added the `EndianBuf::to_string()` method. [#233][]

* Added more robust error handling in our example `dwarfdump` clone. [#234][]

* Added `FrameDescriptionEntry::initial_address` method. [#237][]

* Added `FrameDescriptionEntry::len` method. [#237][]

* Added the `FrameDescriptionEntry::entry_len` method. [#241][]

* Added the `CommonInformationEntry::offset` method. [#241][]

* Added the `CommonInformationEntry::entry_len` method. [#241][]

* Added the `CommonInformationEntry::version` method. [#241][]

* Added the `CommonInformationEntry::augmentation` method. [#241][]

* Added the `CommonInformationEntry::code_alignment_factor` method. [#241][]

* Added the `CommonInformationEntry::data_alignment_factor` method. [#241][]

* Added the `CommonInformationEntry::return_address_register` method. [#241][]

* Added support for printing `.eh_frame` sections to our example `dwarfdump`
  clone. [#241][]

* Added support for parsing the `.eh_frame_hdr` section. On Linux, the
  `.eh_frame_hdr` section provides a pointer to the already-mapped-in-memory
  `.eh_frame` data, so that it doesn't need to be duplicated, and a binary
  search table of its entries for faster unwinding information lookups. [#250][]

* Added support for parsing DWARF 5 compilation unit headers. [#257][]

* Added support for DWARF 5's `DW_FORM_implicit_const`. [#257][]

### Changed

* Unwinding methods now give ownership of the unwinding context back to the
  caller if errors are encountered, not just on the success path. This allows
  recovering from errors in signal-safe code, where constructing a new unwinding
  context is not an option because it requires allocation. This is a **breaking
  change** affecting `UnwindSection::unwind_info_for_address` and
  `UninitializedUnwindContext::initialize`. [#241][]

* `CfaRule` and `RegisterRule` now expose their `DW_OP` expressions as
  `Expression`. This is a minor **breaking change**. [#241][]

* The `Error::UnknownVersion` variant now contains the unknown version
  number. This is a minor **breaking change**. [#245][]

* `EvaluationResult::RequiresEntryValue` requires an `Expression` instead of a
  `Reader` now. This is a minor **breaking change**. [#256][]


[#233]: https://github.com/gimli-rs/gimli/pull/233
[#234]: https://github.com/gimli-rs/gimli/pull/234
[#237]: https://github.com/gimli-rs/gimli/pull/237
[#241]: https://github.com/gimli-rs/gimli/pull/241
[#245]: https://github.com/gimli-rs/gimli/pull/245
[#250]: https://github.com/gimli-rs/gimli/pull/250
[#256]: https://github.com/gimli-rs/gimli/pull/256
[#257]: https://github.com/gimli-rs/gimli/pull/257

--------------------------------------------------------------------------------

## 0.14.0

Released 2017/08/08.

### Added

* All `pub` types now `derive(Hash)`. [#192][]

* All the constants from DWARF 5 are now defined. [#193][]

* Added support for the `DW_OP_GNU_parameter_ref` GNU extension to parsing and
  evaluation DWARF opcodes. [#208][]

* Improved LEB128 parsing performance. [#216][]

* Improved `.debug_{aranges,pubnames,pubtypes}` parsing performance. [#218][]

* Added the ability to choose endianity dynamically at run time, rather than
  only statically at compile time. [#219][]

### Changed

* The biggest change of this release is that `gimli` no longer requires the
  object file's section be fully loaded into memory. This enables using `gimli`
  on 32 bit platforms where there often isn't enough contiguous virtual memory
  address space to load debugging information into. The default behavior is
  still geared for 64 bit platforms, where address space overfloweth, and you
  can still load the whole sections of the object file (or the entire object
  file) into memory. This is abstracted over with the `gimli::Reader`
  trait. This manifests as small (but many) breaking changes to much of the
  public API. [#182][]

### Fixed

* The `DW_END_*` constants for defining endianity of a compilation unit were
  previously incorrect. [#193][]

* The `DW_OP_addr` opcode is relative to the base address of the `.text` section
  of the binary, but we were incorrectly treating it as an absolute value. [#210][]

[GitHub]: https://github.com/gimli-rs/gimli
[crates.io]: https://crates.io/crates/gimli
[contributing]: https://github.com/gimli-rs/gimli/blob/master/CONTRIBUTING.md
[easy]: https://github.com/gimli-rs/gimli/issues?q=is%3Aopen+is%3Aissue+label%3Aeasy
[#192]: https://github.com/gimli-rs/gimli/pull/192
[#193]: https://github.com/gimli-rs/gimli/pull/193
[#182]: https://github.com/gimli-rs/gimli/issues/182
[#208]: https://github.com/gimli-rs/gimli/pull/208
[#210]: https://github.com/gimli-rs/gimli/pull/210
[#216]: https://github.com/gimli-rs/gimli/pull/216
[#218]: https://github.com/gimli-rs/gimli/pull/218
[#219]: https://github.com/gimli-rs/gimli/pull/219
