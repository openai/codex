use alloc::vec::Vec;
use indexmap::{IndexMap, IndexSet};
use std::ops::{Deref, DerefMut};

use crate::common::{DebugLineOffset, Encoding, Format, LineEncoding, SectionId};
use crate::constants;
use crate::leb128;
use crate::write::{
    Address, DebugLineStrOffsets, DebugStrOffsets, Error, LineStringId, LineStringTable, Result,
    Section, StringId, Writer,
};

/// The number assigned to the first special opcode.
//
// We output all instructions for all DWARF versions, since readers
// should be able to ignore instructions they don't support.
const OPCODE_BASE: u8 = 13;

/// A line number program.
#[derive(Debug, Clone)]
pub struct LineProgram {
    /// True if this line program was created with `LineProgram::none()`.
    none: bool,
    encoding: Encoding,
    line_encoding: LineEncoding,

    /// A list of source directory path names.
    ///
    /// If a path is relative, then the directory is located relative to the working
    /// directory of the compilation unit.
    ///
    /// The first entry is for the working directory of the compilation unit.
    directories: IndexSet<LineString>,

    /// A list of source file entries.
    ///
    /// Each entry has a path name and a directory.
    ///
    /// If a path is a relative, then the file is located relative to the
    /// directory. Otherwise the directory is meaningless.
    ///
    /// Does not include comp_file, even for version >= 5.
    files: IndexMap<(LineString, DirectoryId), FileInfo>,

    /// True if the file entries may have valid timestamps.
    ///
    /// Entries may still have a timestamp of 0 even if this is set.
    /// For version <= 4, this is ignored.
    /// For version 5, this controls whether to emit `DW_LNCT_timestamp`.
    pub file_has_timestamp: bool,

    /// True if the file entries may have valid sizes.
    ///
    /// Entries may still have a size of 0 even if this is set.
    /// For version <= 4, this is ignored.
    /// For version 5, this controls whether to emit `DW_LNCT_size`.
    pub file_has_size: bool,

    /// True if the file entries have valid MD5 checksums.
    ///
    /// For version <= 4, this is ignored.
    /// For version 5, this controls whether to emit `DW_LNCT_MD5`.
    pub file_has_md5: bool,

    /// True if the file entries have embedded source code.
    ///
    /// For version <= 4, this is ignored.
    /// For version 5, this controls whether to emit `DW_LNCT_LLVM_source`.
    pub file_has_source: bool,

    prev_row: LineRow,
    row: LineRow,
    // TODO: this probably should be either rows or sequences instead
    instructions: Vec<LineInstruction>,
    in_sequence: bool,
}

impl LineProgram {
    /// Create a new `LineProgram`.
    ///
    /// `comp_dir` defines the working directory of the compilation unit,
    /// and must be the same as the `DW_AT_comp_dir` attribute
    /// of the compilation unit DIE.
    ///
    /// `comp_file` and `comp_file_info` define the primary source file
    /// of the compilation unit and must be the same as the `DW_AT_name`
    /// attribute of the compilation unit DIE.
    ///
    /// # Panics
    ///
    /// Panics if `line_encoding.line_base` > 0.
    ///
    /// Panics if `line_encoding.line_base` + `line_encoding.line_range` <= 0.
    ///
    /// Panics if `comp_dir` is empty or contains a null byte.
    ///
    /// Panics if `comp_file` is empty or contains a null byte.
    pub fn new(
        encoding: Encoding,
        line_encoding: LineEncoding,
        comp_dir: LineString,
        comp_file: LineString,
        comp_file_info: Option<FileInfo>,
    ) -> LineProgram {
        // We require a special opcode for a line advance of 0.
        // See the debug_asserts in generate_row().
        assert!(line_encoding.line_base <= 0);
        assert!(line_encoding.line_base + line_encoding.line_range as i8 > 0);
        let mut program = LineProgram {
            none: false,
            encoding,
            line_encoding,
            directories: IndexSet::new(),
            files: IndexMap::new(),
            prev_row: LineRow::initial_state(encoding, line_encoding),
            row: LineRow::initial_state(encoding, line_encoding),
            instructions: Vec::new(),
            in_sequence: false,
            file_has_timestamp: false,
            file_has_size: false,
            file_has_md5: false,
            file_has_source: false,
        };
        // For all DWARF versions, directory index 0 is comp_dir.
        // For version <= 4, the entry is implicit. We still add
        // it here so that we use it, but we don't emit it.
        let comp_dir_id = program.add_directory(comp_dir);
        // For DWARF version >= 5, file index 0 is comp_file and must exist.
        if encoding.version >= 5 {
            program.add_file(comp_file, comp_dir_id, comp_file_info);
        }
        program
    }

    /// Create a new `LineProgram` with no fields set.
    ///
    /// This can be used when the `LineProgram` will not be used.
    ///
    /// You should not attempt to add files or line instructions to
    /// this line program, or write it to the `.debug_line` section.
    pub fn none() -> Self {
        let encoding = Encoding {
            format: Format::Dwarf32,
            version: 2,
            address_size: 0,
        };
        let line_encoding = LineEncoding::default();
        LineProgram {
            none: true,
            encoding,
            line_encoding,
            directories: IndexSet::new(),
            files: IndexMap::new(),
            prev_row: LineRow::initial_state(encoding, line_encoding),
            row: LineRow::initial_state(encoding, line_encoding),
            instructions: Vec::new(),
            in_sequence: false,
            file_has_timestamp: false,
            file_has_size: false,
            file_has_md5: false,
            file_has_source: false,
        }
    }

    /// Return true if this line program was created with `LineProgram::none()`.
    #[inline]
    pub fn is_none(&self) -> bool {
        self.none
    }

    /// Return the encoding parameters for this line program.
    #[inline]
    pub fn encoding(&self) -> Encoding {
        self.encoding
    }

    /// Return the DWARF version for this line program.
    #[inline]
    pub fn version(&self) -> u16 {
        self.encoding.version
    }

    /// Return the address size in bytes for this line program.
    #[inline]
    pub fn address_size(&self) -> u8 {
        self.encoding.address_size
    }

    /// Return the DWARF format for this line program.
    #[inline]
    pub fn format(&self) -> Format {
        self.encoding.format
    }

    /// Return the id for the working directory of the compilation unit.
    #[inline]
    pub fn default_directory(&self) -> DirectoryId {
        DirectoryId(0)
    }

    /// Add a directory entry and return its id.
    ///
    /// If the directory already exists, then return the id of the existing entry.
    ///
    /// If the path is relative, then the directory is located relative to the working
    /// directory of the compilation unit.
    ///
    /// # Panics
    ///
    /// Panics if `directory` is empty or contains a null byte.
    pub fn add_directory(&mut self, directory: LineString) -> DirectoryId {
        if let LineString::String(ref val) = directory {
            // For DWARF version <= 4, directories must not be empty.
            // The first directory isn't emitted so skip the check for it.
            if self.encoding.version <= 4 && !self.directories.is_empty() {
                assert!(!val.is_empty());
            }
            assert!(!val.contains(&0));
        }
        let (index, _) = self.directories.insert_full(directory);
        DirectoryId(index)
    }

    /// Get a reference to a directory entry.
    ///
    /// # Panics
    ///
    /// Panics if `id` is invalid.
    pub fn get_directory(&self, id: DirectoryId) -> &LineString {
        self.directories.get_index(id.0).unwrap()
    }

    /// Add a file entry and return its id.
    ///
    /// If the file already exists, then return the id of the existing entry.
    ///
    /// If the file path is relative, then the file is located relative
    /// to the directory. Otherwise the directory is meaningless, but it
    /// is still used as a key for file entries.
    ///
    /// If `info` is `None`, then new entries are assigned
    /// default information, and existing entries are unmodified.
    ///
    /// If `info` is not `None`, then it is always assigned to the
    /// entry, even if the entry already exists.
    ///
    /// # Panics
    ///
    /// Panics if 'file' is empty or contains a null byte.
    pub fn add_file(
        &mut self,
        file: LineString,
        directory: DirectoryId,
        info: Option<FileInfo>,
    ) -> FileId {
        if let LineString::String(ref val) = file {
            if self.encoding.version <= 4 {
                assert!(!val.is_empty());
            }
            assert!(!val.contains(&0));
        }

        let key = (file, directory);
        let index = if let Some(info) = info {
            let (index, _) = self.files.insert_full(key, info);
            index
        } else {
            let entry = self.files.entry(key);
            let index = entry.index();
            entry.or_default();
            index
        };
        FileId::new(index)
    }

    /// Get a reference to a file entry.
    ///
    /// # Panics
    ///
    /// Panics if `id` is invalid.
    pub fn get_file(&self, id: FileId) -> (&LineString, DirectoryId) {
        self.files
            .get_index(id.index())
            .map(|entry| (&(entry.0).0, (entry.0).1))
            .unwrap()
    }

    /// Get a reference to the info for a file entry.
    ///
    /// # Panics
    ///
    /// Panics if `id` is invalid.
    pub fn get_file_info(&self, id: FileId) -> &FileInfo {
        self.files
            .get_index(id.index())
            .map(|entry| entry.1)
            .unwrap()
    }

    /// Get a mutable reference to the info for a file entry.
    ///
    /// # Panics
    ///
    /// Panics if `id` is invalid.
    pub fn get_file_info_mut(&mut self, id: FileId) -> &mut FileInfo {
        self.files
            .get_index_mut(id.index())
            .map(|entry| entry.1)
            .unwrap()
    }

    /// Begin a new sequence and set its base address.
    ///
    /// # Panics
    ///
    /// Panics if a sequence has already begun.
    pub fn begin_sequence(&mut self, address: Option<Address>) {
        assert!(!self.in_sequence);
        self.in_sequence = true;
        if let Some(address) = address {
            self.instructions.push(LineInstruction::SetAddress(address));
        }
    }

    /// End the sequence, and reset the row to its default values.
    ///
    /// Only the `address_offset` and op_index` fields of the current row are used.
    ///
    /// # Panics
    ///
    /// Panics if a sequence has not begun.
    pub fn end_sequence(&mut self, address_offset: u64) {
        assert!(self.in_sequence);
        self.in_sequence = false;
        self.row.address_offset = address_offset;
        let op_advance = self.op_advance();
        if op_advance != 0 {
            self.instructions
                .push(LineInstruction::AdvancePc(op_advance));
        }
        self.instructions.push(LineInstruction::EndSequence);
        self.prev_row = LineRow::initial_state(self.encoding, self.line_encoding);
        self.row = LineRow::initial_state(self.encoding, self.line_encoding);
    }

    /// Return true if a sequence has begun.
    #[inline]
    pub fn in_sequence(&self) -> bool {
        self.in_sequence
    }

    /// Returns a reference to the data for the current row.
    #[inline]
    pub fn row(&mut self) -> &mut LineRow {
        &mut self.row
    }

    /// Generates the line number information instructions for the current row.
    ///
    /// After the instructions are generated, it sets `discriminator` to 0, and sets
    /// `basic_block`, `prologue_end`, and `epilogue_begin` to false.
    ///
    /// # Panics
    ///
    /// Panics if a sequence has not begun.
    /// Panics if the address_offset decreases.
    pub fn generate_row(&mut self) {
        assert!(self.in_sequence);

        // Output fields that are reset on every row.
        if self.row.discriminator != 0 {
            self.instructions
                .push(LineInstruction::SetDiscriminator(self.row.discriminator));
            self.row.discriminator = 0;
        }
        if self.row.basic_block {
            self.instructions.push(LineInstruction::SetBasicBlock);
            self.row.basic_block = false;
        }
        if self.row.prologue_end {
            self.instructions.push(LineInstruction::SetPrologueEnd);
            self.row.prologue_end = false;
        }
        if self.row.epilogue_begin {
            self.instructions.push(LineInstruction::SetEpilogueBegin);
            self.row.epilogue_begin = false;
        }

        // Output fields that are not reset on every row.
        if self.row.is_statement != self.prev_row.is_statement {
            self.instructions.push(LineInstruction::NegateStatement);
        }
        if self.row.file != self.prev_row.file {
            self.instructions
                .push(LineInstruction::SetFile(self.row.file));
        }
        if self.row.column != self.prev_row.column {
            self.instructions
                .push(LineInstruction::SetColumn(self.row.column));
        }
        if self.row.isa != self.prev_row.isa {
            self.instructions
                .push(LineInstruction::SetIsa(self.row.isa));
        }

        // Advance the line, address, and operation index.
        let line_base = i64::from(self.line_encoding.line_base) as u64;
        let line_range = u64::from(self.line_encoding.line_range);
        let line_advance = self.row.line as i64 - self.prev_row.line as i64;
        let op_advance = self.op_advance();

        // Default to special advances of 0.
        let special_base = u64::from(OPCODE_BASE);
        // TODO: handle lack of special opcodes for 0 line advance
        debug_assert!(self.line_encoding.line_base <= 0);
        debug_assert!(self.line_encoding.line_base + self.line_encoding.line_range as i8 >= 0);
        let special_default = special_base.wrapping_sub(line_base);
        let mut special = special_default;
        let mut use_special = false;

        if line_advance != 0 {
            let special_line = (line_advance as u64).wrapping_sub(line_base);
            if special_line < line_range {
                special = special_base + special_line;
                use_special = true;
            } else {
                self.instructions
                    .push(LineInstruction::AdvanceLine(line_advance));
            }
        }

        if op_advance != 0 {
            // Using ConstAddPc can save a byte.
            let (special_op_advance, const_add_pc) = if special + op_advance * line_range <= 255 {
                (op_advance, false)
            } else {
                let op_range = (255 - special_base) / line_range;
                (op_advance - op_range, true)
            };

            let special_op = special_op_advance * line_range;
            if special + special_op <= 255 {
                special += special_op;
                use_special = true;
                if const_add_pc {
                    self.instructions.push(LineInstruction::ConstAddPc);
                }
            } else {
                self.instructions
                    .push(LineInstruction::AdvancePc(op_advance));
            }
        }

        if use_special && special != special_default {
            debug_assert!(special >= special_base);
            debug_assert!(special <= 255);
            self.instructions
                .push(LineInstruction::Special(special as u8));
        } else {
            self.instructions.push(LineInstruction::Copy);
        }

        self.prev_row = self.row;
    }

    fn op_advance(&self) -> u64 {
        debug_assert!(self.row.address_offset >= self.prev_row.address_offset);
        let mut address_advance = self.row.address_offset - self.prev_row.address_offset;
        if self.line_encoding.minimum_instruction_length != 1 {
            debug_assert_eq!(
                self.row.address_offset % u64::from(self.line_encoding.minimum_instruction_length),
                0
            );
            address_advance /= u64::from(self.line_encoding.minimum_instruction_length);
        }
        address_advance * u64::from(self.line_encoding.maximum_operations_per_instruction)
            + self.row.op_index
            - self.prev_row.op_index
    }

    /// Returns true if the line number program has no instructions.
    ///
    /// Does not check the file or directory entries.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.instructions.is_empty()
    }

    /// Write the line number program to the given section.
    ///
    /// # Panics
    ///
    /// Panics if `self.is_none()`.
    pub fn write<W: Writer>(
        &self,
        w: &mut DebugLine<W>,
        encoding: Encoding,
        debug_line_str_offsets: &DebugLineStrOffsets,
        debug_str_offsets: &DebugStrOffsets,
    ) -> Result<DebugLineOffset> {
        assert!(!self.is_none());

        if encoding.version < self.version()
            || encoding.format != self.format()
            || encoding.address_size != self.address_size()
        {
            return Err(Error::IncompatibleLineProgramEncoding);
        }

        let offset = w.offset();

        let length_offset = w.write_initial_length(self.format())?;
        let length_base = w.len();

        if self.version() < 2 || self.version() > 5 {
            return Err(Error::UnsupportedVersion(self.version()));
        }
        w.write_u16(self.version())?;

        if self.version() >= 5 {
            w.write_u8(self.address_size())?;
            // Segment selector size.
            w.write_u8(0)?;
        }

        let header_length_offset = w.len();
        w.write_udata(0, self.format().word_size())?;
        let header_length_base = w.len();

        w.write_u8(self.line_encoding.minimum_instruction_length)?;
        if self.version() >= 4 {
            w.write_u8(self.line_encoding.maximum_operations_per_instruction)?;
        } else if self.line_encoding.maximum_operations_per_instruction != 1 {
            return Err(Error::NeedVersion(4));
        };
        w.write_u8(if self.line_encoding.default_is_stmt {
            1
        } else {
            0
        })?;
        w.write_u8(self.line_encoding.line_base as u8)?;
        w.write_u8(self.line_encoding.line_range)?;
        w.write_u8(OPCODE_BASE)?;
        w.write(&[0, 1, 1, 1, 1, 0, 0, 0, 1, 0, 0, 1])?;

        if self.version() <= 4 {
            // The first directory is stored as DW_AT_comp_dir.
            for dir in self.directories.iter().skip(1) {
                dir.write(
                    w,
                    constants::DW_FORM_string,
                    self.encoding,
                    debug_line_str_offsets,
                    debug_str_offsets,
                )?;
            }
            w.write_u8(0)?;

            for ((file, dir), info) in self.files.iter() {
                file.write(
                    w,
                    constants::DW_FORM_string,
                    self.encoding,
                    debug_line_str_offsets,
                    debug_str_offsets,
                )?;
                w.write_uleb128(dir.0 as u64)?;
                w.write_uleb128(info.timestamp)?;
                w.write_uleb128(info.size)?;
            }
            w.write_u8(0)?;
        } else {
            // Directory entry formats (only ever 1).
            w.write_u8(1)?;
            w.write_uleb128(u64::from(constants::DW_LNCT_path.0))?;
            let dir_form = self.directories.get_index(0).unwrap().form();
            w.write_uleb128(dir_form.0.into())?;

            // Directory entries.
            w.write_uleb128(self.directories.len() as u64)?;
            for dir in self.directories.iter() {
                dir.write(
                    w,
                    dir_form,
                    self.encoding,
                    debug_line_str_offsets,
                    debug_str_offsets,
                )?;
            }

            // File name entry formats.
            let count = 2
                + if self.file_has_timestamp { 1 } else { 0 }
                + if self.file_has_size { 1 } else { 0 }
                + if self.file_has_md5 { 1 } else { 0 }
                + if self.file_has_source { 1 } else { 0 };
            w.write_u8(count)?;
            w.write_uleb128(u64::from(constants::DW_LNCT_path.0))?;
            let file_form = (self.files.get_index(0).unwrap().0).0.form();
            w.write_uleb128(file_form.0.into())?;
            w.write_uleb128(u64::from(constants::DW_LNCT_directory_index.0))?;
            w.write_uleb128(constants::DW_FORM_udata.0.into())?;
            if self.file_has_timestamp {
                w.write_uleb128(u64::from(constants::DW_LNCT_timestamp.0))?;
                w.write_uleb128(constants::DW_FORM_udata.0.into())?;
            }
            if self.file_has_size {
                w.write_uleb128(u64::from(constants::DW_LNCT_size.0))?;
                w.write_uleb128(constants::DW_FORM_udata.0.into())?;
            }
            if self.file_has_md5 {
                w.write_uleb128(u64::from(constants::DW_LNCT_MD5.0))?;
                w.write_uleb128(constants::DW_FORM_data16.0.into())?;
            }
            if self.file_has_source {
                w.write_uleb128(u64::from(constants::DW_LNCT_LLVM_source.0))?;
                w.write_uleb128(constants::DW_FORM_string.0.into())?;
            }

            // File name entries.
            w.write_uleb128(self.files.len() as u64)?;
            let mut write_file = |file: &LineString, dir: DirectoryId, info: &FileInfo| {
                file.write(
                    w,
                    file_form,
                    self.encoding,
                    debug_line_str_offsets,
                    debug_str_offsets,
                )?;
                w.write_uleb128(dir.0 as u64)?;
                if self.file_has_timestamp {
                    w.write_uleb128(info.timestamp)?;
                }
                if self.file_has_size {
                    w.write_uleb128(info.size)?;
                }
                if self.file_has_md5 {
                    w.write(&info.md5)?;
                }
                if self.file_has_source {
                    // Note: An empty DW_LNCT_LLVM_source is interpreted as missing
                    // source code. Included source code should always be
                    // terminated by a "\n" line ending.
                    let empty_str = LineString::String(Vec::new());
                    let source = info.source.as_ref().unwrap_or(&empty_str);
                    source.write(
                        w,
                        constants::DW_FORM_string,
                        self.encoding,
                        debug_line_str_offsets,
                        debug_str_offsets,
                    )?;
                }
                Ok(())
            };
            for ((file, dir), info) in self.files.iter() {
                write_file(file, *dir, info)?;
            }
        }

        let header_length = (w.len() - header_length_base) as u64;
        w.write_udata_at(
            header_length_offset,
            header_length,
            self.format().word_size(),
        )?;

        for instruction in &self.instructions {
            instruction.write(w, self.encoding)?;
        }

        let length = (w.len() - length_base) as u64;
        w.write_initial_length_at(length_offset, length, self.format())?;

        Ok(offset)
    }
}

/// A row in the line number table that corresponds to a machine instruction.
#[derive(Debug, Clone, Copy)]
pub struct LineRow {
    /// The offset of the instruction from the start address of the sequence.
    pub address_offset: u64,
    /// The index of an operation within a VLIW instruction.
    ///
    /// The index of the first operation is 0.
    /// Set to 0 for non-VLIW instructions.
    pub op_index: u64,

    /// The source file corresponding to the instruction.
    pub file: FileId,
    /// The line number within the source file.
    ///
    /// Lines are numbered beginning at 1. Set to 0 if there is no source line.
    pub line: u64,
    /// The column number within the source line.
    ///
    /// Columns are numbered beginning at 1. Set to 0 for the "left edge" of the line.
    pub column: u64,
    /// An additional discriminator used to distinguish between source locations.
    /// This value is assigned arbitrarily by the DWARF producer.
    pub discriminator: u64,

    /// Set to true if the instruction is a recommended breakpoint for a statement.
    pub is_statement: bool,
    /// Set to true if the instruction is the beginning of a basic block.
    pub basic_block: bool,
    /// Set to true if the instruction is a recommended breakpoint at the entry of a
    /// function.
    pub prologue_end: bool,
    /// Set to true if the instruction is a recommended breakpoint prior to the exit of
    /// a function.
    pub epilogue_begin: bool,

    /// The instruction set architecture of the instruction.
    ///
    /// Set to 0 for the default ISA. Other values are defined by the architecture ABI.
    pub isa: u64,
}

impl LineRow {
    /// Return the initial state as specified in the DWARF standard.
    fn initial_state(encoding: Encoding, line_encoding: LineEncoding) -> Self {
        LineRow {
            address_offset: 0,
            op_index: 0,

            file: FileId::initial_state(encoding.version),
            line: 1,
            column: 0,
            discriminator: 0,

            is_statement: line_encoding.default_is_stmt,
            basic_block: false,
            prologue_end: false,
            epilogue_begin: false,

            isa: 0,
        }
    }
}

/// An instruction in a line number program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineInstruction {
    // Special opcodes
    Special(u8),

    // Standard opcodes
    Copy,
    AdvancePc(u64),
    AdvanceLine(i64),
    SetFile(FileId),
    SetColumn(u64),
    NegateStatement,
    SetBasicBlock,
    ConstAddPc,
    // DW_LNS_fixed_advance_pc is not supported.
    SetPrologueEnd,
    SetEpilogueBegin,
    SetIsa(u64),

    // Extended opcodes
    EndSequence,
    // TODO: this doubles the size of this enum.
    SetAddress(Address),
    // DW_LNE_define_file is not supported.
    SetDiscriminator(u64),
}

impl LineInstruction {
    /// Write the line number instruction to the given section.
    fn write<W: Writer>(self, w: &mut DebugLine<W>, encoding: Encoding) -> Result<()> {
        use self::LineInstruction::*;
        match self {
            Special(val) => w.write_u8(val)?,
            Copy => w.write_u8(constants::DW_LNS_copy.0)?,
            AdvancePc(val) => {
                w.write_u8(constants::DW_LNS_advance_pc.0)?;
                w.write_uleb128(val)?;
            }
            AdvanceLine(val) => {
                w.write_u8(constants::DW_LNS_advance_line.0)?;
                w.write_sleb128(val)?;
            }
            SetFile(val) => {
                w.write_u8(constants::DW_LNS_set_file.0)?;
                w.write_uleb128(val.raw(encoding.version))?;
            }
            SetColumn(val) => {
                w.write_u8(constants::DW_LNS_set_column.0)?;
                w.write_uleb128(val)?;
            }
            NegateStatement => w.write_u8(constants::DW_LNS_negate_stmt.0)?,
            SetBasicBlock => w.write_u8(constants::DW_LNS_set_basic_block.0)?,
            ConstAddPc => w.write_u8(constants::DW_LNS_const_add_pc.0)?,
            SetPrologueEnd => w.write_u8(constants::DW_LNS_set_prologue_end.0)?,
            SetEpilogueBegin => w.write_u8(constants::DW_LNS_set_epilogue_begin.0)?,
            SetIsa(val) => {
                w.write_u8(constants::DW_LNS_set_isa.0)?;
                w.write_uleb128(val)?;
            }
            EndSequence => {
                w.write_u8(0)?;
                w.write_uleb128(1)?;
                w.write_u8(constants::DW_LNE_end_sequence.0)?;
            }
            SetAddress(address) => {
                w.write_u8(0)?;
                w.write_uleb128(1 + u64::from(encoding.address_size))?;
                w.write_u8(constants::DW_LNE_set_address.0)?;
                w.write_address(address, encoding.address_size)?;
            }
            SetDiscriminator(val) => {
                let mut bytes = [0u8; 10];
                // bytes is long enough so this will never fail.
                let len = leb128::write::unsigned(&mut { &mut bytes[..] }, val).unwrap();
                w.write_u8(0)?;
                w.write_uleb128(1 + len as u64)?;
                w.write_u8(constants::DW_LNE_set_discriminator.0)?;
                w.write(&bytes[..len])?;
            }
        }
        Ok(())
    }
}

/// A string value for use in defining paths in line number programs.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LineString {
    /// A slice of bytes representing a string. Must not include null bytes.
    /// Not guaranteed to be UTF-8 or anything like that.
    String(Vec<u8>),

    /// A reference to a string in the `.debug_str` section.
    StringRef(StringId),

    /// A reference to a string in the `.debug_line_str` section.
    LineStringRef(LineStringId),
}

impl LineString {
    /// Create a `LineString` using the normal form for the given encoding.
    pub fn new<T>(val: T, encoding: Encoding, line_strings: &mut LineStringTable) -> Self
    where
        T: Into<Vec<u8>>,
    {
        let val = val.into();
        if encoding.version <= 4 {
            LineString::String(val)
        } else {
            LineString::LineStringRef(line_strings.add(val))
        }
    }

    fn form(&self) -> constants::DwForm {
        match *self {
            LineString::String(..) => constants::DW_FORM_string,
            LineString::StringRef(..) => constants::DW_FORM_strp,
            LineString::LineStringRef(..) => constants::DW_FORM_line_strp,
        }
    }

    fn write<W: Writer>(
        &self,
        w: &mut DebugLine<W>,
        form: constants::DwForm,
        encoding: Encoding,
        debug_line_str_offsets: &DebugLineStrOffsets,
        debug_str_offsets: &DebugStrOffsets,
    ) -> Result<()> {
        if form != self.form() {
            return Err(Error::LineStringFormMismatch);
        }

        match *self {
            LineString::String(ref val) => {
                if encoding.version <= 4 {
                    debug_assert!(!val.is_empty());
                }
                w.write(val)?;
                w.write_u8(0)?;
            }
            LineString::StringRef(val) => {
                if encoding.version < 5 {
                    return Err(Error::NeedVersion(5));
                }
                w.write_offset(
                    debug_str_offsets.get(val).0,
                    SectionId::DebugStr,
                    encoding.format.word_size(),
                )?;
            }
            LineString::LineStringRef(val) => {
                if encoding.version < 5 {
                    return Err(Error::NeedVersion(5));
                }
                w.write_offset(
                    debug_line_str_offsets.get(val).0,
                    SectionId::DebugLineStr,
                    encoding.format.word_size(),
                )?;
            }
        }
        Ok(())
    }
}

/// An identifier for a directory in a `LineProgram`.
///
/// Defaults to the working directory of the compilation unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DirectoryId(usize);

// Force FileId access via the methods.
mod id {
    /// An identifier for a file in a `LineProgram`.
    //
    // We internally use a 0-based index for all versions, but
    // emit a 1-based index for DWARF version <= 4.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct FileId(usize);

    impl FileId {
        /// Create a `FileId` given a 0-based index into `LineProgram::files`.
        pub(crate) fn new(index: usize) -> Self {
            FileId(index)
        }

        /// The 0-based index of the file in `LineProgram::files`.
        pub(super) fn index(self) -> usize {
            self.0
        }

        /// The initial state of the file register.
        pub(super) fn initial_state(version: u16) -> Self {
            if version == 5 {
                // For version 5, the files are 0-based and the default file is 1,
                // which is a 0-based index of 1.
                FileId(1)
            } else {
                // For version <= 4, the files are 1-based and the default file is 1,
                // which is a 0-based index of 0.
                // For version >= 6, the files are 0-based and the default file is 0,
                // which is a 0-based index of 0.
                FileId(0)
            }
        }

        /// Convert to a raw value used for writing.
        ///
        /// This converts to a 1-based index for DWARF version <= 4.
        pub(crate) fn raw(self, version: u16) -> u64 {
            if version <= 4 {
                self.0 as u64 + 1
            } else {
                self.0 as u64
            }
        }
    }
}
pub use self::id::*;

/// Extra information for file in a `LineProgram`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FileInfo {
    /// The implementation defined timestamp of the last modification of the file,
    /// or 0 if not available.
    pub timestamp: u64,

    /// The size of the file in bytes, or 0 if not available.
    pub size: u64,

    /// A 16-byte MD5 digest of the file contents.
    ///
    /// Only used if version >= 5 and `LineProgram::file_has_md5` is `true`.
    pub md5: [u8; 16],

    /// Optionally some embedded sourcecode.
    ///
    /// Only used if version >= 5 and `LineProgram::file_has_source` is `true`.
    ///
    /// NOTE: This currently only supports the `LineString::String` variant,
    /// since we're encoding the string with `DW_FORM_string`.
    /// Other variants will result in an `LineStringFormMismatch` error.
    pub source: Option<LineString>,
}

define_section!(
    DebugLine,
    DebugLineOffset,
    "A writable `.debug_line` section."
);

#[cfg(feature = "read")]
mod convert {
    use super::*;
    use crate::read::{self, Reader};
    use crate::write::{self, ConvertError, ConvertResult};

    impl LineProgram {
        /// Create a line number program by reading the data from the given program.
        ///
        /// Return the program and a mapping from file index to `FileId`.
        pub fn from<R: Reader<Offset = usize>>(
            mut from_program: read::IncompleteLineProgram<R>,
            dwarf: &read::Dwarf<R>,
            line_strings: &mut write::LineStringTable,
            strings: &mut write::StringTable,
            convert_address: &dyn Fn(u64) -> Option<Address>,
        ) -> ConvertResult<(LineProgram, Vec<FileId>)> {
            // Create mappings in case the source has duplicate files or directories.
            let mut dirs = Vec::new();
            let mut files = Vec::new();

            let mut program = {
                let from_header = from_program.header();
                let encoding = from_header.encoding();

                let comp_dir = match from_header.directory(0) {
                    Some(comp_dir) => LineString::from(comp_dir, dwarf, line_strings, strings)?,
                    None => LineString::new(&[][..], encoding, line_strings),
                };

                let comp_name = match from_header.file(0) {
                    Some(comp_file) => {
                        if comp_file.directory_index() != 0 {
                            return Err(ConvertError::InvalidDirectoryIndex);
                        }
                        LineString::from(comp_file.path_name(), dwarf, line_strings, strings)?
                    }
                    None => LineString::new(&[][..], encoding, line_strings),
                };

                if from_header.line_base() > 0 {
                    return Err(ConvertError::InvalidLineBase);
                }
                let mut program = LineProgram::new(
                    encoding,
                    from_header.line_encoding(),
                    comp_dir,
                    comp_name,
                    None, // We'll set this later if needed when we add the file again.
                );

                if from_header.version() <= 4 {
                    // The first directory is implicit.
                    dirs.push(DirectoryId(0));
                    // A file index of 0 is invalid for version <= 4, but putting
                    // something there makes the indexing easier.
                    files.push(FileId::new(0));
                }

                for from_dir in from_header.include_directories() {
                    let from_dir =
                        LineString::from(from_dir.clone(), dwarf, line_strings, strings)?;
                    dirs.push(program.add_directory(from_dir));
                }

                program.file_has_timestamp = from_header.file_has_timestamp();
                program.file_has_size = from_header.file_has_size();
                program.file_has_md5 = from_header.file_has_md5();
                program.file_has_source = from_header.file_has_source();
                for from_file in from_header.file_names().iter() {
                    let from_name =
                        LineString::from(from_file.path_name(), dwarf, line_strings, strings)?;
                    let from_dir = from_file.directory_index();
                    if from_dir >= dirs.len() as u64 {
                        return Err(ConvertError::InvalidDirectoryIndex);
                    }
                    let from_dir = dirs[from_dir as usize];
                    let from_info = Some(FileInfo {
                        timestamp: from_file.timestamp(),
                        size: from_file.size(),
                        md5: *from_file.md5(),
                        source: match from_file.source() {
                            Some(source) => {
                                Some(LineString::from(source, dwarf, line_strings, strings)?)
                            }
                            None => None,
                        },
                    });
                    files.push(program.add_file(from_name, from_dir, from_info));
                }

                program
            };

            // We can't use the `from_program.rows()` because that wouldn't let
            // us preserve address relocations.
            let mut from_row = read::LineRow::new(from_program.header());
            let mut instructions = from_program.header().instructions();
            let mut address = None;
            while let Some(instruction) = instructions.next_instruction(from_program.header())? {
                match instruction {
                    read::LineInstruction::SetAddress(val) => {
                        if program.in_sequence() {
                            return Err(ConvertError::UnsupportedLineInstruction);
                        }
                        match convert_address(val) {
                            Some(val) => address = Some(val),
                            None => return Err(ConvertError::InvalidAddress),
                        }
                        from_row
                            .execute(read::LineInstruction::SetAddress(0), &mut from_program)?;
                    }
                    read::LineInstruction::DefineFile(_) => {
                        return Err(ConvertError::UnsupportedLineInstruction);
                    }
                    _ => {
                        if from_row.execute(instruction, &mut from_program)? {
                            if !program.in_sequence() {
                                program.begin_sequence(address);
                                address = None;
                            }
                            if from_row.end_sequence() {
                                program.end_sequence(from_row.address());
                            } else {
                                program.row().address_offset = from_row.address();
                                program.row().op_index = from_row.op_index();
                                program.row().file = {
                                    let file = from_row.file_index();
                                    if file >= files.len() as u64 {
                                        return Err(ConvertError::InvalidFileIndex);
                                    }
                                    if file == 0 && program.version() <= 4 {
                                        return Err(ConvertError::InvalidFileIndex);
                                    }
                                    files[file as usize]
                                };
                                program.row().line = match from_row.line() {
                                    Some(line) => line.get(),
                                    None => 0,
                                };
                                program.row().column = match from_row.column() {
                                    read::ColumnType::LeftEdge => 0,
                                    read::ColumnType::Column(val) => val.get(),
                                };
                                program.row().discriminator = from_row.discriminator();
                                program.row().is_statement = from_row.is_stmt();
                                program.row().basic_block = from_row.basic_block();
                                program.row().prologue_end = from_row.prologue_end();
                                program.row().epilogue_begin = from_row.epilogue_begin();
                                program.row().isa = from_row.isa();
                                program.generate_row();
                            }
                            from_row.reset(from_program.header());
                        }
                    }
                };
            }
            Ok((program, files))
        }
    }

    impl LineString {
        fn from<R: Reader<Offset = usize>>(
            from_attr: read::AttributeValue<R>,
            dwarf: &read::Dwarf<R>,
            line_strings: &mut write::LineStringTable,
            strings: &mut write::StringTable,
        ) -> ConvertResult<LineString> {
            Ok(match from_attr {
                read::AttributeValue::String(r) => LineString::String(r.to_slice()?.to_vec()),
                read::AttributeValue::DebugStrRef(offset) => {
                    let r = dwarf.debug_str.get_str(offset)?;
                    let id = strings.add(r.to_slice()?);
                    LineString::StringRef(id)
                }
                read::AttributeValue::DebugLineStrRef(offset) => {
                    let r = dwarf.debug_line_str.get_str(offset)?;
                    let id = line_strings.add(r.to_slice()?);
                    LineString::LineStringRef(id)
                }
                _ => return Err(ConvertError::UnsupportedLineStringForm),
            })
        }
    }
}

#[cfg(test)]
#[cfg(feature = "read")]
mod tests {
    use super::*;
    use crate::read;
    use crate::write::{AttributeValue, Dwarf, EndianVec, Sections, Unit};
    use crate::LittleEndian;

    #[test]
    fn test_line_program() {
        let dir1 = LineString::String(b"dir1".to_vec());
        let file1 = LineString::String(b"file1".to_vec());
        let dir2 = LineString::String(b"dir2".to_vec());
        let file2 = LineString::String(b"file2".to_vec());

        let mut dwarf = Dwarf::new();

        for &version in &[2, 3, 4, 5] {
            for &address_size in &[4, 8] {
                for &format in &[Format::Dwarf32, Format::Dwarf64] {
                    let encoding = Encoding {
                        format,
                        version,
                        address_size,
                    };
                    let mut program = LineProgram::new(
                        encoding,
                        LineEncoding::default(),
                        dir1.clone(),
                        file1.clone(),
                        None,
                    );

                    assert_eq!(&dir1, program.get_directory(program.default_directory()));
                    program.file_has_timestamp = true;
                    program.file_has_size = true;
                    if encoding.version >= 5 {
                        program.file_has_md5 = true;
                    }

                    // Note: Embedded source code is an accepted extension
                    // that will become part of DWARF v6. We're using the LLVM extension
                    // here for v5.
                    if encoding.version >= 5 {
                        program.file_has_source = true;
                    }

                    let dir_id = program.add_directory(dir2.clone());
                    assert_eq!(&dir2, program.get_directory(dir_id));
                    assert_eq!(dir_id, program.add_directory(dir2.clone()));

                    let file_info = FileInfo {
                        timestamp: 1,
                        size: 2,
                        md5: if encoding.version >= 5 {
                            [3; 16]
                        } else {
                            [0; 16]
                        },
                        source: (encoding.version >= 5)
                            .then(|| LineString::String(b"the source code\n".to_vec())),
                    };
                    let file_id = program.add_file(file2.clone(), dir_id, Some(file_info.clone()));
                    assert_eq!((&file2, dir_id), program.get_file(file_id));
                    assert_eq!(file_info, *program.get_file_info(file_id));

                    program.get_file_info_mut(file_id).size = 3;
                    assert_ne!(file_info, *program.get_file_info(file_id));
                    assert_eq!(file_id, program.add_file(file2.clone(), dir_id, None));
                    assert_ne!(file_info, *program.get_file_info(file_id));
                    assert_eq!(
                        file_id,
                        program.add_file(file2.clone(), dir_id, Some(file_info.clone()))
                    );
                    assert_eq!(file_info, *program.get_file_info(file_id));

                    let mut unit = Unit::new(encoding, program);
                    let root = unit.get_mut(unit.root());
                    root.set(
                        constants::DW_AT_comp_dir,
                        AttributeValue::String(b"dir1".to_vec()),
                    );
                    root.set(
                        constants::DW_AT_name,
                        AttributeValue::String(b"file1".to_vec()),
                    );
                    root.set(constants::DW_AT_stmt_list, AttributeValue::LineProgramRef);
                    root.set(
                        constants::DW_AT_decl_file,
                        AttributeValue::FileIndex(Some(file_id)),
                    );

                    let mut dwarf = Dwarf::new();
                    dwarf.units.add(unit);
                }
            }
        }

        let mut sections = Sections::new(EndianVec::new(LittleEndian));
        dwarf.write(&mut sections).unwrap();
        let read_dwarf = sections.read(LittleEndian);
        let convert_dwarf =
            Dwarf::from(&read_dwarf, &|address| Some(Address::Constant(address))).unwrap();

        let mut convert_units = convert_dwarf.units.iter();
        for (_, unit) in dwarf.units.iter() {
            let program = &unit.line_program;
            let root = unit.get(unit.root());
            let Some(AttributeValue::FileIndex(Some(file_id))) =
                root.get(constants::DW_AT_decl_file)
            else {
                panic!("missing DW_AT_decl_file");
            };

            let (_, convert_unit) = convert_units.next().unwrap();
            let convert_program = &convert_unit.line_program;
            let convert_root = convert_unit.get(convert_unit.root());
            let Some(AttributeValue::FileIndex(Some(convert_file_id))) =
                convert_root.get(constants::DW_AT_decl_file)
            else {
                panic!("missing DW_AT_decl_file");
            };

            assert_eq!(convert_program.version(), program.version());
            assert_eq!(convert_program.address_size(), program.address_size());
            assert_eq!(convert_program.format(), program.format());

            let (file, dir) = program.get_file(*file_id);
            let (convert_file, convert_dir) = convert_program.get_file(*convert_file_id);
            assert_eq!(file, convert_file);
            assert_eq!(
                program.get_directory(dir),
                convert_program.get_directory(convert_dir)
            );
            assert_eq!(
                program.get_file_info(*file_id),
                convert_program.get_file_info(*convert_file_id)
            );
        }
    }

    #[test]
    fn test_line_row() {
        let dir1 = &b"dir1"[..];
        let file1 = &b"file1"[..];
        let file2 = &b"file2"[..];

        for &version in &[2, 3, 4, 5] {
            for &address_size in &[4, 8] {
                for &format in &[Format::Dwarf32, Format::Dwarf64] {
                    let encoding = Encoding {
                        format,
                        version,
                        address_size,
                    };
                    let line_base = -5;
                    let line_range = 14;
                    let neg_line_base = (-line_base) as u8;
                    let mut program = LineProgram::new(
                        encoding,
                        LineEncoding {
                            line_base,
                            line_range,
                            ..Default::default()
                        },
                        LineString::String(dir1.to_vec()),
                        LineString::String(file1.to_vec()),
                        None,
                    );
                    let dir_id = program.default_directory();
                    let file1_id =
                        program.add_file(LineString::String(file1.to_vec()), dir_id, None);
                    let file2_id =
                        program.add_file(LineString::String(file2.to_vec()), dir_id, None);

                    // Test sequences.
                    {
                        let mut program = program.clone();
                        let address = Address::Constant(0x12);
                        program.begin_sequence(Some(address));
                        assert_eq!(
                            program.instructions,
                            vec![LineInstruction::SetAddress(address)]
                        );
                    }

                    {
                        let mut program = program.clone();
                        program.begin_sequence(None);
                        assert_eq!(program.instructions, Vec::new());
                    }

                    {
                        let mut program = program.clone();
                        program.begin_sequence(None);
                        program.end_sequence(0x1234);
                        assert_eq!(
                            program.instructions,
                            vec![
                                LineInstruction::AdvancePc(0x1234),
                                LineInstruction::EndSequence
                            ]
                        );
                    }

                    // Create a base program.
                    program.begin_sequence(None);
                    program.row.line = 0x1000;
                    program.generate_row();
                    let base_row = program.row;
                    let base_instructions = program.instructions.clone();

                    // Create test cases.
                    let mut tests = Vec::new();

                    let row = base_row;
                    tests.push((row, vec![LineInstruction::Copy]));

                    let mut row = base_row;
                    row.line -= u64::from(neg_line_base);
                    tests.push((row, vec![LineInstruction::Special(OPCODE_BASE)]));

                    let mut row = base_row;
                    row.line += u64::from(line_range) - 1;
                    row.line -= u64::from(neg_line_base);
                    tests.push((
                        row,
                        vec![LineInstruction::Special(OPCODE_BASE + line_range - 1)],
                    ));

                    let mut row = base_row;
                    row.line += u64::from(line_range);
                    row.line -= u64::from(neg_line_base);
                    tests.push((
                        row,
                        vec![
                            LineInstruction::AdvanceLine(i64::from(line_range - neg_line_base)),
                            LineInstruction::Copy,
                        ],
                    ));

                    let mut row = base_row;
                    row.address_offset = 1;
                    row.line -= u64::from(neg_line_base);
                    tests.push((
                        row,
                        vec![LineInstruction::Special(OPCODE_BASE + line_range)],
                    ));

                    let op_range = (255 - OPCODE_BASE) / line_range;
                    let mut row = base_row;
                    row.address_offset = u64::from(op_range);
                    row.line -= u64::from(neg_line_base);
                    tests.push((
                        row,
                        vec![LineInstruction::Special(
                            OPCODE_BASE + op_range * line_range,
                        )],
                    ));

                    let mut row = base_row;
                    row.address_offset = u64::from(op_range);
                    row.line += u64::from(255 - OPCODE_BASE - op_range * line_range);
                    row.line -= u64::from(neg_line_base);
                    tests.push((row, vec![LineInstruction::Special(255)]));

                    let mut row = base_row;
                    row.address_offset = u64::from(op_range);
                    row.line += u64::from(255 - OPCODE_BASE - op_range * line_range) + 1;
                    row.line -= u64::from(neg_line_base);
                    tests.push((
                        row,
                        vec![LineInstruction::ConstAddPc, LineInstruction::Copy],
                    ));

                    let mut row = base_row;
                    row.address_offset = u64::from(op_range);
                    row.line += u64::from(255 - OPCODE_BASE - op_range * line_range) + 2;
                    row.line -= u64::from(neg_line_base);
                    tests.push((
                        row,
                        vec![
                            LineInstruction::ConstAddPc,
                            LineInstruction::Special(OPCODE_BASE + 6),
                        ],
                    ));

                    let mut row = base_row;
                    row.address_offset = u64::from(op_range) * 2;
                    row.line += u64::from(255 - OPCODE_BASE - op_range * line_range);
                    row.line -= u64::from(neg_line_base);
                    tests.push((
                        row,
                        vec![LineInstruction::ConstAddPc, LineInstruction::Special(255)],
                    ));

                    let mut row = base_row;
                    row.address_offset = u64::from(op_range) * 2;
                    row.line += u64::from(255 - OPCODE_BASE - op_range * line_range) + 1;
                    row.line -= u64::from(neg_line_base);
                    tests.push((
                        row,
                        vec![
                            LineInstruction::AdvancePc(row.address_offset),
                            LineInstruction::Copy,
                        ],
                    ));

                    let mut row = base_row;
                    row.address_offset = u64::from(op_range) * 2;
                    row.line += u64::from(255 - OPCODE_BASE - op_range * line_range) + 2;
                    row.line -= u64::from(neg_line_base);
                    tests.push((
                        row,
                        vec![
                            LineInstruction::AdvancePc(row.address_offset),
                            LineInstruction::Special(OPCODE_BASE + 6),
                        ],
                    ));

                    let mut row = base_row;
                    row.address_offset = 0x1234;
                    tests.push((
                        row,
                        vec![LineInstruction::AdvancePc(0x1234), LineInstruction::Copy],
                    ));

                    let mut row = base_row;
                    row.line += 0x1234;
                    tests.push((
                        row,
                        vec![LineInstruction::AdvanceLine(0x1234), LineInstruction::Copy],
                    ));

                    let mut row = base_row;
                    row.file = file1_id;
                    if version == 5 {
                        // Version 5 is 0-based, but the default file is 1, so this row
                        // will need to set the file.
                        tests.push((
                            row,
                            vec![LineInstruction::SetFile(file1_id), LineInstruction::Copy],
                        ));
                    } else {
                        // This is the first file, so normally this is already the default.
                        tests.push((row, vec![LineInstruction::Copy]));
                    }

                    let mut row = base_row;
                    row.file = file2_id;
                    if version == 5 {
                        tests.push((row, vec![LineInstruction::Copy]));
                    } else {
                        tests.push((
                            row,
                            vec![LineInstruction::SetFile(file2_id), LineInstruction::Copy],
                        ));
                    }

                    let mut row = base_row;
                    row.column = 0x1234;
                    tests.push((
                        row,
                        vec![LineInstruction::SetColumn(0x1234), LineInstruction::Copy],
                    ));

                    let mut row = base_row;
                    row.discriminator = 0x1234;
                    tests.push((
                        row,
                        vec![
                            LineInstruction::SetDiscriminator(0x1234),
                            LineInstruction::Copy,
                        ],
                    ));

                    let mut row = base_row;
                    row.is_statement = !row.is_statement;
                    tests.push((
                        row,
                        vec![LineInstruction::NegateStatement, LineInstruction::Copy],
                    ));

                    let mut row = base_row;
                    row.basic_block = true;
                    tests.push((
                        row,
                        vec![LineInstruction::SetBasicBlock, LineInstruction::Copy],
                    ));

                    let mut row = base_row;
                    row.prologue_end = true;
                    tests.push((
                        row,
                        vec![LineInstruction::SetPrologueEnd, LineInstruction::Copy],
                    ));

                    let mut row = base_row;
                    row.epilogue_begin = true;
                    tests.push((
                        row,
                        vec![LineInstruction::SetEpilogueBegin, LineInstruction::Copy],
                    ));

                    let mut row = base_row;
                    row.isa = 0x1234;
                    tests.push((
                        row,
                        vec![LineInstruction::SetIsa(0x1234), LineInstruction::Copy],
                    ));

                    for test in tests {
                        // Test generate_row().
                        let mut program = program.clone();
                        program.row = test.0;
                        program.generate_row();
                        assert_eq!(
                            &program.instructions[base_instructions.len()..],
                            &test.1[..]
                        );

                        // Test LineProgram::from().
                        let mut unit = Unit::new(encoding, program);
                        let root = unit.get_mut(unit.root());
                        root.set(constants::DW_AT_stmt_list, AttributeValue::LineProgramRef);

                        let mut dwarf = Dwarf::new();
                        dwarf.units.add(unit);

                        let mut sections = Sections::new(EndianVec::new(LittleEndian));
                        dwarf.write(&mut sections).unwrap();
                        let read_dwarf = sections.read(LittleEndian);

                        let convert_dwarf =
                            Dwarf::from(&read_dwarf, &|address| Some(Address::Constant(address)))
                                .unwrap();
                        let convert_unit = convert_dwarf.units.iter().next().unwrap().1;
                        let convert_program = &convert_unit.line_program;

                        assert_eq!(
                            &convert_program.instructions[base_instructions.len()..],
                            &test.1[..]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_line_instruction() {
        let dir1 = &b"dir1"[..];
        let file1 = &b"file1"[..];

        for &version in &[2, 3, 4, 5] {
            for &address_size in &[4, 8] {
                for &format in &[Format::Dwarf32, Format::Dwarf64] {
                    let encoding = Encoding {
                        format,
                        version,
                        address_size,
                    };
                    let mut program = LineProgram::new(
                        encoding,
                        LineEncoding::default(),
                        LineString::String(dir1.to_vec()),
                        LineString::String(file1.to_vec()),
                        None,
                    );
                    let dir_id = program.default_directory();
                    let file_id =
                        program.add_file(LineString::String(file1.to_vec()), dir_id, None);

                    for (inst, expect_inst) in &[
                        (
                            LineInstruction::Special(OPCODE_BASE),
                            read::LineInstruction::Special(OPCODE_BASE),
                        ),
                        (
                            LineInstruction::Special(255),
                            read::LineInstruction::Special(255),
                        ),
                        (LineInstruction::Copy, read::LineInstruction::Copy),
                        (
                            LineInstruction::AdvancePc(0x12),
                            read::LineInstruction::AdvancePc(0x12),
                        ),
                        (
                            LineInstruction::AdvanceLine(0x12),
                            read::LineInstruction::AdvanceLine(0x12),
                        ),
                        (
                            LineInstruction::SetFile(file_id),
                            read::LineInstruction::SetFile(file_id.raw(encoding.version)),
                        ),
                        (
                            LineInstruction::SetColumn(0x12),
                            read::LineInstruction::SetColumn(0x12),
                        ),
                        (
                            LineInstruction::NegateStatement,
                            read::LineInstruction::NegateStatement,
                        ),
                        (
                            LineInstruction::SetBasicBlock,
                            read::LineInstruction::SetBasicBlock,
                        ),
                        (
                            LineInstruction::ConstAddPc,
                            read::LineInstruction::ConstAddPc,
                        ),
                        (
                            LineInstruction::SetPrologueEnd,
                            read::LineInstruction::SetPrologueEnd,
                        ),
                        (
                            LineInstruction::SetEpilogueBegin,
                            read::LineInstruction::SetEpilogueBegin,
                        ),
                        (
                            LineInstruction::SetIsa(0x12),
                            read::LineInstruction::SetIsa(0x12),
                        ),
                        (
                            LineInstruction::EndSequence,
                            read::LineInstruction::EndSequence,
                        ),
                        (
                            LineInstruction::SetAddress(Address::Constant(0x12)),
                            read::LineInstruction::SetAddress(0x12),
                        ),
                        (
                            LineInstruction::SetDiscriminator(0x12),
                            read::LineInstruction::SetDiscriminator(0x12),
                        ),
                    ][..]
                    {
                        let mut program = program.clone();
                        program.instructions.push(*inst);

                        let mut unit = Unit::new(encoding, program);
                        let root = unit.get_mut(unit.root());
                        root.set(constants::DW_AT_stmt_list, AttributeValue::LineProgramRef);

                        let mut dwarf = Dwarf::new();
                        dwarf.units.add(unit);
                        let mut sections = Sections::new(EndianVec::new(LittleEndian));
                        dwarf.write(&mut sections).unwrap();

                        let read_dwarf = sections.read(LittleEndian);
                        let read_unit_header = read_dwarf.units().next().unwrap().unwrap();
                        let read_unit = read_dwarf.unit(read_unit_header).unwrap();
                        let read_unit = read_unit.unit_ref(&read_dwarf);
                        let read_header = read_unit.line_program.as_ref().unwrap().header();
                        let mut read_insts = read_header.instructions();
                        assert_eq!(
                            *expect_inst,
                            read_insts.next_instruction(read_header).unwrap().unwrap()
                        );
                        assert_eq!(None, read_insts.next_instruction(read_header).unwrap());
                    }
                }
            }
        }
    }

    // Test that the address/line advance is correct. We don't test for optimality.
    #[test]
    fn test_advance() {
        let encoding = Encoding {
            format: Format::Dwarf32,
            version: 4,
            address_size: 8,
        };

        let dir1 = &b"dir1"[..];
        let file1 = &b"file1"[..];

        let addresses = 0..50;
        let lines = -10..25i64;

        for minimum_instruction_length in [1, 4] {
            for maximum_operations_per_instruction in [1, 3] {
                for line_base in [-5, 0] {
                    for line_range in [10, 20] {
                        let line_encoding = LineEncoding {
                            minimum_instruction_length,
                            maximum_operations_per_instruction,
                            line_base,
                            line_range,
                            default_is_stmt: true,
                        };
                        let mut program = LineProgram::new(
                            encoding,
                            line_encoding,
                            LineString::String(dir1.to_vec()),
                            LineString::String(file1.to_vec()),
                            None,
                        );
                        for address_advance in addresses.clone() {
                            program.begin_sequence(Some(Address::Constant(0x1000)));
                            program.row().line = 0x10000;
                            program.generate_row();
                            for line_advance in lines.clone() {
                                {
                                    let row = program.row();
                                    row.address_offset +=
                                        address_advance * u64::from(minimum_instruction_length);
                                    row.line = row.line.wrapping_add(line_advance as u64);
                                }
                                program.generate_row();
                            }
                            let address_offset = program.row().address_offset
                                + u64::from(minimum_instruction_length);
                            program.end_sequence(address_offset);
                        }

                        let mut unit = Unit::new(encoding, program);
                        let root = unit.get_mut(unit.root());
                        root.set(constants::DW_AT_stmt_list, AttributeValue::LineProgramRef);

                        let mut dwarf = Dwarf::new();
                        dwarf.units.add(unit);
                        let mut sections = Sections::new(EndianVec::new(LittleEndian));
                        dwarf.write(&mut sections).unwrap();

                        let read_dwarf = sections.read(LittleEndian);
                        let read_unit_header = read_dwarf.units().next().unwrap().unwrap();
                        let read_unit = read_dwarf.unit(read_unit_header).unwrap();
                        let read_unit = read_unit.unit_ref(&read_dwarf);
                        let read_program = read_unit.line_program.clone().unwrap();

                        let mut rows = read_program.rows();
                        for address_advance in addresses.clone() {
                            let mut address;
                            let mut line;
                            {
                                let row = rows.next_row().unwrap().unwrap().1;
                                address = row.address();
                                line = row.line().unwrap().get();
                            }
                            assert_eq!(address, 0x1000);
                            assert_eq!(line, 0x10000);
                            for line_advance in lines.clone() {
                                let row = rows.next_row().unwrap().unwrap().1;
                                assert_eq!(
                                    row.address() - address,
                                    address_advance * u64::from(minimum_instruction_length)
                                );
                                assert_eq!(
                                    (row.line().unwrap().get() as i64) - (line as i64),
                                    line_advance
                                );
                                address = row.address();
                                line = row.line().unwrap().get();
                            }
                            let row = rows.next_row().unwrap().unwrap().1;
                            assert!(row.end_sequence());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_line_string() {
        let version = 5;

        let file1 = "file1";

        for &address_size in &[4, 8] {
            for &format in &[Format::Dwarf32, Format::Dwarf64] {
                let encoding = Encoding {
                    format,
                    version,
                    address_size,
                };

                let files: &mut [&mut dyn Fn(&mut Dwarf) -> LineString] = &mut [
                    &mut |_dwarf| LineString::String(file1.as_bytes().to_vec()),
                    &mut |dwarf| LineString::StringRef(dwarf.strings.add(file1)),
                    &mut |dwarf| LineString::LineStringRef(dwarf.line_strings.add(file1)),
                ];

                for file in files {
                    let mut dwarf = Dwarf::new();
                    let file = file(&mut dwarf);

                    let mut program = LineProgram::new(
                        encoding,
                        LineEncoding::default(),
                        LineString::String(b"dir".to_vec()),
                        file.clone(),
                        None,
                    );
                    program.begin_sequence(Some(Address::Constant(0x1000)));
                    program.row().line = 0x10000;
                    program.generate_row();

                    let mut unit = Unit::new(encoding, program);
                    let root = unit.get_mut(unit.root());
                    root.set(constants::DW_AT_stmt_list, AttributeValue::LineProgramRef);
                    dwarf.units.add(unit);

                    let mut sections = Sections::new(EndianVec::new(LittleEndian));
                    dwarf.write(&mut sections).unwrap();

                    let read_dwarf = sections.read(LittleEndian);
                    let read_unit_header = read_dwarf.units().next().unwrap().unwrap();
                    let read_unit = read_dwarf.unit(read_unit_header).unwrap();
                    let read_unit = read_unit.unit_ref(&read_dwarf);
                    let read_program = read_unit.line_program.clone().unwrap();
                    let read_header = read_program.header();
                    let read_file = read_header.file(0).unwrap();
                    let read_path = read_unit.attr_string(read_file.path_name()).unwrap();
                    assert_eq!(read_path.slice(), file1.as_bytes());
                }
            }
        }
    }

    #[test]
    fn test_missing_comp_dir() {
        for &version in &[2, 3, 4, 5] {
            for &address_size in &[4, 8] {
                for &format in &[Format::Dwarf32, Format::Dwarf64] {
                    let encoding = Encoding {
                        format,
                        version,
                        address_size,
                    };
                    let mut program = LineProgram::new(
                        encoding,
                        LineEncoding::default(),
                        LineString::String(Vec::new()),
                        LineString::String(Vec::new()),
                        None,
                    );
                    // Ensure the program is not empty.
                    let dir_id = program.default_directory();
                    let file_id =
                        program.add_file(LineString::String(b"file1".to_vec()), dir_id, None);
                    program.begin_sequence(Some(Address::Constant(0x1000)));
                    program.row().file = file_id;
                    program.row().line = 0x10000;
                    program.generate_row();

                    let mut unit = Unit::new(encoding, program);
                    let root = unit.get_mut(unit.root());
                    // Testing missing DW_AT_comp_dir/DW_AT_name.
                    root.set(constants::DW_AT_stmt_list, AttributeValue::LineProgramRef);

                    let mut dwarf = Dwarf::new();
                    dwarf.units.add(unit);
                    let mut sections = Sections::new(EndianVec::new(LittleEndian));
                    dwarf.write(&mut sections).unwrap();
                    let read_dwarf = sections.read(LittleEndian);
                    let _convert_dwarf =
                        Dwarf::from(&read_dwarf, &|address| Some(Address::Constant(address)))
                            .unwrap();
                }
            }
        }
    }
}
