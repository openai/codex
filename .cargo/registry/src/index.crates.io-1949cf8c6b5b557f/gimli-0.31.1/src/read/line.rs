use alloc::vec::Vec;
use core::num::{NonZeroU64, Wrapping};

use crate::common::{
    DebugLineOffset, DebugLineStrOffset, DebugStrOffset, DebugStrOffsetsIndex, Encoding, Format,
    LineEncoding, SectionId,
};
use crate::constants;
use crate::endianity::Endianity;
use crate::read::{
    AttributeValue, EndianSlice, Error, Reader, ReaderAddress, ReaderOffset, Result, Section,
};

/// The `DebugLine` struct contains the source location to instruction mapping
/// found in the `.debug_line` section.
#[derive(Debug, Default, Clone, Copy)]
pub struct DebugLine<R> {
    debug_line_section: R,
}

impl<'input, Endian> DebugLine<EndianSlice<'input, Endian>>
where
    Endian: Endianity,
{
    /// Construct a new `DebugLine` instance from the data in the `.debug_line`
    /// section.
    ///
    /// It is the caller's responsibility to read the `.debug_line` section and
    /// present it as a `&[u8]` slice. That means using some ELF loader on
    /// Linux, a Mach-O loader on macOS, etc.
    ///
    /// ```
    /// use gimli::{DebugLine, LittleEndian};
    ///
    /// # let buf = [0x00, 0x01, 0x02, 0x03];
    /// # let read_debug_line_section_somehow = || &buf;
    /// let debug_line = DebugLine::new(read_debug_line_section_somehow(), LittleEndian);
    /// ```
    pub fn new(debug_line_section: &'input [u8], endian: Endian) -> Self {
        Self::from(EndianSlice::new(debug_line_section, endian))
    }
}

impl<R: Reader> DebugLine<R> {
    /// Parse the line number program whose header is at the given `offset` in the
    /// `.debug_line` section.
    ///
    /// The `address_size` must match the compilation unit that the lines apply to.
    /// The `comp_dir` should be from the `DW_AT_comp_dir` attribute of the compilation
    /// unit. The `comp_name` should be from the `DW_AT_name` attribute of the
    /// compilation unit.
    ///
    /// ```rust,no_run
    /// use gimli::{DebugLine, DebugLineOffset, IncompleteLineProgram, EndianSlice, LittleEndian};
    ///
    /// # let buf = [];
    /// # let read_debug_line_section_somehow = || &buf;
    /// let debug_line = DebugLine::new(read_debug_line_section_somehow(), LittleEndian);
    ///
    /// // In a real example, we'd grab the offset via a compilation unit
    /// // entry's `DW_AT_stmt_list` attribute, and the address size from that
    /// // unit directly.
    /// let offset = DebugLineOffset(0);
    /// let address_size = 8;
    ///
    /// let program = debug_line.program(offset, address_size, None, None)
    ///     .expect("should have found a header at that offset, and parsed it OK");
    /// ```
    pub fn program(
        &self,
        offset: DebugLineOffset<R::Offset>,
        address_size: u8,
        comp_dir: Option<R>,
        comp_name: Option<R>,
    ) -> Result<IncompleteLineProgram<R>> {
        let input = &mut self.debug_line_section.clone();
        input.skip(offset.0)?;
        let header = LineProgramHeader::parse(input, offset, address_size, comp_dir, comp_name)?;
        let program = IncompleteLineProgram { header };
        Ok(program)
    }
}

impl<T> DebugLine<T> {
    /// Create a `DebugLine` section that references the data in `self`.
    ///
    /// This is useful when `R` implements `Reader` but `T` does not.
    ///
    /// Used by `DwarfSections::borrow`.
    pub fn borrow<'a, F, R>(&'a self, mut borrow: F) -> DebugLine<R>
    where
        F: FnMut(&'a T) -> R,
    {
        borrow(&self.debug_line_section).into()
    }
}

impl<R> Section<R> for DebugLine<R> {
    fn id() -> SectionId {
        SectionId::DebugLine
    }

    fn reader(&self) -> &R {
        &self.debug_line_section
    }
}

impl<R> From<R> for DebugLine<R> {
    fn from(debug_line_section: R) -> Self {
        DebugLine { debug_line_section }
    }
}

/// Deprecated. `LineNumberProgram` has been renamed to `LineProgram`.
#[deprecated(note = "LineNumberProgram has been renamed to LineProgram, use that instead.")]
pub type LineNumberProgram<R, Offset> = dyn LineProgram<R, Offset>;

/// A `LineProgram` provides access to a `LineProgramHeader` and
/// a way to add files to the files table if necessary. Gimli consumers should
/// never need to use or see this trait.
pub trait LineProgram<R, Offset = <R as Reader>::Offset>
where
    R: Reader<Offset = Offset>,
    Offset: ReaderOffset,
{
    /// Get a reference to the held `LineProgramHeader`.
    fn header(&self) -> &LineProgramHeader<R, Offset>;
    /// Add a file to the file table if necessary.
    fn add_file(&mut self, file: FileEntry<R, Offset>);
}

impl<R, Offset> LineProgram<R, Offset> for IncompleteLineProgram<R, Offset>
where
    R: Reader<Offset = Offset>,
    Offset: ReaderOffset,
{
    fn header(&self) -> &LineProgramHeader<R, Offset> {
        &self.header
    }
    fn add_file(&mut self, file: FileEntry<R, Offset>) {
        self.header.file_names.push(file);
    }
}

impl<'program, R, Offset> LineProgram<R, Offset> for &'program CompleteLineProgram<R, Offset>
where
    R: Reader<Offset = Offset>,
    Offset: ReaderOffset,
{
    fn header(&self) -> &LineProgramHeader<R, Offset> {
        &self.header
    }
    fn add_file(&mut self, _: FileEntry<R, Offset>) {
        // Nop. Our file table is already complete.
    }
}

/// Deprecated. `StateMachine` has been renamed to `LineRows`.
#[deprecated(note = "StateMachine has been renamed to LineRows, use that instead.")]
pub type StateMachine<R, Program, Offset> = LineRows<R, Program, Offset>;

/// Executes a `LineProgram` to iterate over the rows in the matrix of line number information.
///
/// "The hypothetical machine used by a consumer of the line number information
/// to expand the byte-coded instruction stream into a matrix of line number
/// information." -- Section 6.2.1
#[derive(Debug, Clone)]
pub struct LineRows<R, Program, Offset = <R as Reader>::Offset>
where
    Program: LineProgram<R, Offset>,
    R: Reader<Offset = Offset>,
    Offset: ReaderOffset,
{
    program: Program,
    row: LineRow,
    instructions: LineInstructions<R>,
}

type OneShotLineRows<R, Offset = <R as Reader>::Offset> =
    LineRows<R, IncompleteLineProgram<R, Offset>, Offset>;

type ResumedLineRows<'program, R, Offset = <R as Reader>::Offset> =
    LineRows<R, &'program CompleteLineProgram<R, Offset>, Offset>;

impl<R, Program, Offset> LineRows<R, Program, Offset>
where
    Program: LineProgram<R, Offset>,
    R: Reader<Offset = Offset>,
    Offset: ReaderOffset,
{
    fn new(program: IncompleteLineProgram<R, Offset>) -> OneShotLineRows<R, Offset> {
        let row = LineRow::new(program.header());
        let instructions = LineInstructions {
            input: program.header().program_buf.clone(),
        };
        LineRows {
            program,
            row,
            instructions,
        }
    }

    fn resume<'program>(
        program: &'program CompleteLineProgram<R, Offset>,
        sequence: &LineSequence<R>,
    ) -> ResumedLineRows<'program, R, Offset> {
        let row = LineRow::new(program.header());
        let instructions = sequence.instructions.clone();
        LineRows {
            program,
            row,
            instructions,
        }
    }

    /// Get a reference to the header for this state machine's line number
    /// program.
    #[inline]
    pub fn header(&self) -> &LineProgramHeader<R, Offset> {
        self.program.header()
    }

    /// Parse and execute the next instructions in the line number program until
    /// another row in the line number matrix is computed.
    ///
    /// The freshly computed row is returned as `Ok(Some((header, row)))`.
    /// If the matrix is complete, and there are no more new rows in the line
    /// number matrix, then `Ok(None)` is returned. If there was an error parsing
    /// an instruction, then `Err(e)` is returned.
    ///
    /// Unfortunately, the references mean that this cannot be a
    /// `FallibleIterator`.
    pub fn next_row(&mut self) -> Result<Option<(&LineProgramHeader<R, Offset>, &LineRow)>> {
        // Perform any reset that was required after copying the previous row.
        self.row.reset(self.program.header());

        loop {
            // Split the borrow here, rather than calling `self.header()`.
            match self.instructions.next_instruction(self.program.header()) {
                Err(err) => return Err(err),
                Ok(None) => return Ok(None),
                Ok(Some(instruction)) => {
                    if self.row.execute(instruction, &mut self.program)? {
                        if self.row.tombstone {
                            // Perform any reset that was required for the tombstone row.
                            // Normally this is done when `next_row` is called again, but for
                            // tombstones we loop immediately.
                            self.row.reset(self.program.header());
                        } else {
                            return Ok(Some((self.header(), &self.row)));
                        }
                    }
                    // Fall through, parse the next instruction, and see if that
                    // yields a row.
                }
            }
        }
    }
}

/// Deprecated. `Opcode` has been renamed to `LineInstruction`.
#[deprecated(note = "Opcode has been renamed to LineInstruction, use that instead.")]
pub type Opcode<R> = LineInstruction<R, <R as Reader>::Offset>;

/// A parsed line number program instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineInstruction<R, Offset = <R as Reader>::Offset>
where
    R: Reader<Offset = Offset>,
    Offset: ReaderOffset,
{
    /// > ### 6.2.5.1 Special Opcodes
    /// >
    /// > Each ubyte special opcode has the following effect on the state machine:
    /// >
    /// >   1. Add a signed integer to the line register.
    /// >
    /// >   2. Modify the operation pointer by incrementing the address and
    /// >      op_index registers as described below.
    /// >
    /// >   3. Append a row to the matrix using the current values of the state
    /// >      machine registers.
    /// >
    /// >   4. Set the basic_block register to “false.”
    /// >
    /// >   5. Set the prologue_end register to “false.”
    /// >
    /// >   6. Set the epilogue_begin register to “false.”
    /// >
    /// >   7. Set the discriminator register to 0.
    /// >
    /// > All of the special opcodes do those same seven things; they differ from
    /// > one another only in what values they add to the line, address and
    /// > op_index registers.
    Special(u8),

    /// "[`LineInstruction::Copy`] appends a row to the matrix using the current
    /// values of the state machine registers. Then it sets the discriminator
    /// register to 0, and sets the basic_block, prologue_end and epilogue_begin
    /// registers to “false.”"
    Copy,

    /// "The DW_LNS_advance_pc opcode takes a single unsigned LEB128 operand as
    /// the operation advance and modifies the address and op_index registers
    /// [the same as `LineInstruction::Special`]"
    AdvancePc(u64),

    /// "The DW_LNS_advance_line opcode takes a single signed LEB128 operand and
    /// adds that value to the line register of the state machine."
    AdvanceLine(i64),

    /// "The DW_LNS_set_file opcode takes a single unsigned LEB128 operand and
    /// stores it in the file register of the state machine."
    SetFile(u64),

    /// "The DW_LNS_set_column opcode takes a single unsigned LEB128 operand and
    /// stores it in the column register of the state machine."
    SetColumn(u64),

    /// "The DW_LNS_negate_stmt opcode takes no operands. It sets the is_stmt
    /// register of the state machine to the logical negation of its current
    /// value."
    NegateStatement,

    /// "The DW_LNS_set_basic_block opcode takes no operands. It sets the
    /// basic_block register of the state machine to “true.”"
    SetBasicBlock,

    /// > The DW_LNS_const_add_pc opcode takes no operands. It advances the
    /// > address and op_index registers by the increments corresponding to
    /// > special opcode 255.
    /// >
    /// > When the line number program needs to advance the address by a small
    /// > amount, it can use a single special opcode, which occupies a single
    /// > byte. When it needs to advance the address by up to twice the range of
    /// > the last special opcode, it can use DW_LNS_const_add_pc followed by a
    /// > special opcode, for a total of two bytes. Only if it needs to advance
    /// > the address by more than twice that range will it need to use both
    /// > DW_LNS_advance_pc and a special opcode, requiring three or more bytes.
    ConstAddPc,

    /// > The DW_LNS_fixed_advance_pc opcode takes a single uhalf (unencoded)
    /// > operand and adds it to the address register of the state machine and
    /// > sets the op_index register to 0. This is the only standard opcode whose
    /// > operand is not a variable length number. It also does not multiply the
    /// > operand by the minimum_instruction_length field of the header.
    FixedAddPc(u16),

    /// "[`LineInstruction::SetPrologueEnd`] sets the prologue_end register to “true”."
    SetPrologueEnd,

    /// "[`LineInstruction::SetEpilogueBegin`] sets the epilogue_begin register to
    /// “true”."
    SetEpilogueBegin,

    /// "The DW_LNS_set_isa opcode takes a single unsigned LEB128 operand and
    /// stores that value in the isa register of the state machine."
    SetIsa(u64),

    /// An unknown standard opcode with zero operands.
    UnknownStandard0(constants::DwLns),

    /// An unknown standard opcode with one operand.
    UnknownStandard1(constants::DwLns, u64),

    /// An unknown standard opcode with multiple operands.
    UnknownStandardN(constants::DwLns, R),

    /// > [`LineInstruction::EndSequence`] sets the end_sequence register of the state
    /// > machine to “true” and appends a row to the matrix using the current
    /// > values of the state-machine registers. Then it resets the registers to
    /// > the initial values specified above (see Section 6.2.2). Every line
    /// > number program sequence must end with a DW_LNE_end_sequence instruction
    /// > which creates a row whose address is that of the byte after the last
    /// > target machine instruction of the sequence.
    EndSequence,

    /// > The DW_LNE_set_address opcode takes a single relocatable address as an
    /// > operand. The size of the operand is the size of an address on the target
    /// > machine. It sets the address register to the value given by the
    /// > relocatable address and sets the op_index register to 0.
    /// >
    /// > All of the other line number program opcodes that affect the address
    /// > register add a delta to it. This instruction stores a relocatable value
    /// > into it instead.
    SetAddress(u64),

    /// Defines a new source file in the line number program and appends it to
    /// the line number program header's list of source files.
    DefineFile(FileEntry<R, Offset>),

    /// "The DW_LNE_set_discriminator opcode takes a single parameter, an
    /// unsigned LEB128 integer. It sets the discriminator register to the new
    /// value."
    SetDiscriminator(u64),

    /// An unknown extended opcode and the slice of its unparsed operands.
    UnknownExtended(constants::DwLne, R),
}

impl<R, Offset> LineInstruction<R, Offset>
where
    R: Reader<Offset = Offset>,
    Offset: ReaderOffset,
{
    fn parse<'header>(
        header: &'header LineProgramHeader<R>,
        input: &mut R,
    ) -> Result<LineInstruction<R>>
    where
        R: 'header,
    {
        let opcode = input.read_u8()?;
        if opcode == 0 {
            let length = input.read_uleb128().and_then(R::Offset::from_u64)?;
            let mut instr_rest = input.split(length)?;
            let opcode = instr_rest.read_u8()?;

            match constants::DwLne(opcode) {
                constants::DW_LNE_end_sequence => Ok(LineInstruction::EndSequence),

                constants::DW_LNE_set_address => {
                    let address = instr_rest.read_address(header.address_size())?;
                    Ok(LineInstruction::SetAddress(address))
                }

                constants::DW_LNE_define_file => {
                    if header.version() <= 4 {
                        let path_name = instr_rest.read_null_terminated_slice()?;
                        let entry = FileEntry::parse(&mut instr_rest, path_name)?;
                        Ok(LineInstruction::DefineFile(entry))
                    } else {
                        Ok(LineInstruction::UnknownExtended(
                            constants::DW_LNE_define_file,
                            instr_rest,
                        ))
                    }
                }

                constants::DW_LNE_set_discriminator => {
                    let discriminator = instr_rest.read_uleb128()?;
                    Ok(LineInstruction::SetDiscriminator(discriminator))
                }

                otherwise => Ok(LineInstruction::UnknownExtended(otherwise, instr_rest)),
            }
        } else if opcode >= header.opcode_base {
            Ok(LineInstruction::Special(opcode))
        } else {
            match constants::DwLns(opcode) {
                constants::DW_LNS_copy => Ok(LineInstruction::Copy),

                constants::DW_LNS_advance_pc => {
                    let advance = input.read_uleb128()?;
                    Ok(LineInstruction::AdvancePc(advance))
                }

                constants::DW_LNS_advance_line => {
                    let increment = input.read_sleb128()?;
                    Ok(LineInstruction::AdvanceLine(increment))
                }

                constants::DW_LNS_set_file => {
                    let file = input.read_uleb128()?;
                    Ok(LineInstruction::SetFile(file))
                }

                constants::DW_LNS_set_column => {
                    let column = input.read_uleb128()?;
                    Ok(LineInstruction::SetColumn(column))
                }

                constants::DW_LNS_negate_stmt => Ok(LineInstruction::NegateStatement),

                constants::DW_LNS_set_basic_block => Ok(LineInstruction::SetBasicBlock),

                constants::DW_LNS_const_add_pc => Ok(LineInstruction::ConstAddPc),

                constants::DW_LNS_fixed_advance_pc => {
                    let advance = input.read_u16()?;
                    Ok(LineInstruction::FixedAddPc(advance))
                }

                constants::DW_LNS_set_prologue_end => Ok(LineInstruction::SetPrologueEnd),

                constants::DW_LNS_set_epilogue_begin => Ok(LineInstruction::SetEpilogueBegin),

                constants::DW_LNS_set_isa => {
                    let isa = input.read_uleb128()?;
                    Ok(LineInstruction::SetIsa(isa))
                }

                otherwise => {
                    let mut opcode_lengths = header.standard_opcode_lengths().clone();
                    opcode_lengths.skip(R::Offset::from_u8(opcode - 1))?;
                    let num_args = opcode_lengths.read_u8()? as usize;
                    match num_args {
                        0 => Ok(LineInstruction::UnknownStandard0(otherwise)),
                        1 => {
                            let arg = input.read_uleb128()?;
                            Ok(LineInstruction::UnknownStandard1(otherwise, arg))
                        }
                        _ => {
                            let mut args = input.clone();
                            for _ in 0..num_args {
                                input.read_uleb128()?;
                            }
                            let len = input.offset_from(&args);
                            args.truncate(len)?;
                            Ok(LineInstruction::UnknownStandardN(otherwise, args))
                        }
                    }
                }
            }
        }
    }
}

/// Deprecated. `OpcodesIter` has been renamed to `LineInstructions`.
#[deprecated(note = "OpcodesIter has been renamed to LineInstructions, use that instead.")]
pub type OpcodesIter<R> = LineInstructions<R>;

/// An iterator yielding parsed instructions.
///
/// See
/// [`LineProgramHeader::instructions`](./struct.LineProgramHeader.html#method.instructions)
/// for more details.
#[derive(Clone, Debug)]
pub struct LineInstructions<R: Reader> {
    input: R,
}

impl<R: Reader> LineInstructions<R> {
    fn remove_trailing(&self, other: &LineInstructions<R>) -> Result<LineInstructions<R>> {
        let offset = other.input.offset_from(&self.input);
        let mut input = self.input.clone();
        input.truncate(offset)?;
        Ok(LineInstructions { input })
    }
}

impl<R: Reader> LineInstructions<R> {
    /// Advance the iterator and return the next instruction.
    ///
    /// Returns the newly parsed instruction as `Ok(Some(instruction))`. Returns
    /// `Ok(None)` when iteration is complete and all instructions have already been
    /// parsed and yielded. If an error occurs while parsing the next attribute,
    /// then this error is returned as `Err(e)`, and all subsequent calls return
    /// `Ok(None)`.
    ///
    /// Unfortunately, the `header` parameter means that this cannot be a
    /// `FallibleIterator`.
    #[inline(always)]
    pub fn next_instruction(
        &mut self,
        header: &LineProgramHeader<R>,
    ) -> Result<Option<LineInstruction<R>>> {
        if self.input.is_empty() {
            return Ok(None);
        }

        match LineInstruction::parse(header, &mut self.input) {
            Ok(instruction) => Ok(Some(instruction)),
            Err(e) => {
                self.input.empty();
                Err(e)
            }
        }
    }
}

/// Deprecated. `LineNumberRow` has been renamed to `LineRow`.
#[deprecated(note = "LineNumberRow has been renamed to LineRow, use that instead.")]
pub type LineNumberRow = LineRow;

/// A row in the line number program's resulting matrix.
///
/// Each row is a copy of the registers of the state machine, as defined in section 6.2.2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineRow {
    tombstone: bool,
    address: u64,
    op_index: Wrapping<u64>,
    file: u64,
    line: Wrapping<u64>,
    column: u64,
    is_stmt: bool,
    basic_block: bool,
    end_sequence: bool,
    prologue_end: bool,
    epilogue_begin: bool,
    isa: u64,
    discriminator: u64,
}

impl LineRow {
    /// Create a line number row in the initial state for the given program.
    pub fn new<R: Reader>(header: &LineProgramHeader<R>) -> Self {
        LineRow {
            // "At the beginning of each sequence within a line number program, the
            // state of the registers is:" -- Section 6.2.2
            tombstone: false,
            address: 0,
            op_index: Wrapping(0),
            file: 1,
            line: Wrapping(1),
            column: 0,
            // "determined by default_is_stmt in the line number program header"
            is_stmt: header.line_encoding.default_is_stmt,
            basic_block: false,
            end_sequence: false,
            prologue_end: false,
            epilogue_begin: false,
            // "The isa value 0 specifies that the instruction set is the
            // architecturally determined default instruction set. This may be fixed
            // by the ABI, or it may be specified by other means, for example, by
            // the object file description."
            isa: 0,
            discriminator: 0,
        }
    }

    /// "The program-counter value corresponding to a machine instruction
    /// generated by the compiler."
    #[inline]
    pub fn address(&self) -> u64 {
        self.address
    }

    /// > An unsigned integer representing the index of an operation within a VLIW
    /// > instruction. The index of the first operation is 0. For non-VLIW
    /// > architectures, this register will always be 0.
    /// >
    /// > The address and op_index registers, taken together, form an operation
    /// > pointer that can reference any individual operation with the
    /// > instruction stream.
    #[inline]
    pub fn op_index(&self) -> u64 {
        self.op_index.0
    }

    /// "An unsigned integer indicating the identity of the source file
    /// corresponding to a machine instruction."
    #[inline]
    pub fn file_index(&self) -> u64 {
        self.file
    }

    /// The source file corresponding to the current machine instruction.
    #[inline]
    pub fn file<'header, R: Reader>(
        &self,
        header: &'header LineProgramHeader<R>,
    ) -> Option<&'header FileEntry<R>> {
        header.file(self.file)
    }

    /// "An unsigned integer indicating a source line number. Lines are numbered
    /// beginning at 1. The compiler may emit the value 0 in cases where an
    /// instruction cannot be attributed to any source line."
    /// Line number values of 0 are represented as `None`.
    #[inline]
    pub fn line(&self) -> Option<NonZeroU64> {
        NonZeroU64::new(self.line.0)
    }

    /// "An unsigned integer indicating a column number within a source
    /// line. Columns are numbered beginning at 1. The value 0 is reserved to
    /// indicate that a statement begins at the “left edge” of the line."
    #[inline]
    pub fn column(&self) -> ColumnType {
        NonZeroU64::new(self.column)
            .map(ColumnType::Column)
            .unwrap_or(ColumnType::LeftEdge)
    }

    /// "A boolean indicating that the current instruction is a recommended
    /// breakpoint location. A recommended breakpoint location is intended to
    /// “represent” a line, a statement and/or a semantically distinct subpart
    /// of a statement."
    #[inline]
    pub fn is_stmt(&self) -> bool {
        self.is_stmt
    }

    /// "A boolean indicating that the current instruction is the beginning of a
    /// basic block."
    #[inline]
    pub fn basic_block(&self) -> bool {
        self.basic_block
    }

    /// "A boolean indicating that the current address is that of the first byte
    /// after the end of a sequence of target machine instructions. end_sequence
    /// terminates a sequence of lines; therefore other information in the same
    /// row is not meaningful."
    #[inline]
    pub fn end_sequence(&self) -> bool {
        self.end_sequence
    }

    /// "A boolean indicating that the current address is one (of possibly many)
    /// where execution should be suspended for an entry breakpoint of a
    /// function."
    #[inline]
    pub fn prologue_end(&self) -> bool {
        self.prologue_end
    }

    /// "A boolean indicating that the current address is one (of possibly many)
    /// where execution should be suspended for an exit breakpoint of a
    /// function."
    #[inline]
    pub fn epilogue_begin(&self) -> bool {
        self.epilogue_begin
    }

    /// Tag for the current instruction set architecture.
    ///
    /// > An unsigned integer whose value encodes the applicable instruction set
    /// > architecture for the current instruction.
    /// >
    /// > The encoding of instruction sets should be shared by all users of a
    /// > given architecture. It is recommended that this encoding be defined by
    /// > the ABI authoring committee for each architecture.
    #[inline]
    pub fn isa(&self) -> u64 {
        self.isa
    }

    /// "An unsigned integer identifying the block to which the current
    /// instruction belongs. Discriminator values are assigned arbitrarily by
    /// the DWARF producer and serve to distinguish among multiple blocks that
    /// may all be associated with the same source file, line, and column. Where
    /// only one block exists for a given source position, the discriminator
    /// value should be zero."
    #[inline]
    pub fn discriminator(&self) -> u64 {
        self.discriminator
    }

    /// Execute the given instruction, and return true if a new row in the
    /// line number matrix needs to be generated.
    ///
    /// Unknown opcodes are treated as no-ops.
    #[inline]
    pub fn execute<R, Program>(
        &mut self,
        instruction: LineInstruction<R>,
        program: &mut Program,
    ) -> Result<bool>
    where
        Program: LineProgram<R>,
        R: Reader,
    {
        Ok(match instruction {
            LineInstruction::Special(opcode) => {
                self.exec_special_opcode(opcode, program.header())?;
                true
            }

            LineInstruction::Copy => true,

            LineInstruction::AdvancePc(operation_advance) => {
                self.apply_operation_advance(operation_advance, program.header())?;
                false
            }

            LineInstruction::AdvanceLine(line_increment) => {
                self.apply_line_advance(line_increment);
                false
            }

            LineInstruction::SetFile(file) => {
                self.file = file;
                false
            }

            LineInstruction::SetColumn(column) => {
                self.column = column;
                false
            }

            LineInstruction::NegateStatement => {
                self.is_stmt = !self.is_stmt;
                false
            }

            LineInstruction::SetBasicBlock => {
                self.basic_block = true;
                false
            }

            LineInstruction::ConstAddPc => {
                let adjusted = self.adjust_opcode(255, program.header());
                let operation_advance = adjusted / program.header().line_encoding.line_range;
                self.apply_operation_advance(u64::from(operation_advance), program.header())?;
                false
            }

            LineInstruction::FixedAddPc(operand) => {
                if !self.tombstone {
                    let address_size = program.header().address_size();
                    self.address = self.address.add_sized(u64::from(operand), address_size)?;
                    self.op_index.0 = 0;
                }
                false
            }

            LineInstruction::SetPrologueEnd => {
                self.prologue_end = true;
                false
            }

            LineInstruction::SetEpilogueBegin => {
                self.epilogue_begin = true;
                false
            }

            LineInstruction::SetIsa(isa) => {
                self.isa = isa;
                false
            }

            LineInstruction::EndSequence => {
                self.end_sequence = true;
                true
            }

            LineInstruction::SetAddress(address) => {
                // If the address is a tombstone, then skip instructions until the next address.
                // DWARF specifies a tombstone value of -1, but many linkers use 0.
                // However, 0 may be a valid address, so we only skip that if we have previously
                // seen a higher address. Additionally, gold may keep the relocation addend,
                // so we treat all lower addresses as tombstones instead of just 0.
                // This works because DWARF specifies that addresses are monotonically increasing
                // within a sequence; the alternative is to return an error.
                let tombstone_address = !0 >> (64 - program.header().encoding.address_size * 8);
                self.tombstone = address < self.address || address == tombstone_address;
                if !self.tombstone {
                    self.address = address;
                    self.op_index.0 = 0;
                }
                false
            }

            LineInstruction::DefineFile(entry) => {
                program.add_file(entry);
                false
            }

            LineInstruction::SetDiscriminator(discriminator) => {
                self.discriminator = discriminator;
                false
            }

            // Compatibility with future opcodes.
            LineInstruction::UnknownStandard0(_)
            | LineInstruction::UnknownStandard1(_, _)
            | LineInstruction::UnknownStandardN(_, _)
            | LineInstruction::UnknownExtended(_, _) => false,
        })
    }

    /// Perform any reset that was required after copying the previous row.
    #[inline]
    pub fn reset<R: Reader>(&mut self, header: &LineProgramHeader<R>) {
        if self.end_sequence {
            // Previous instruction was EndSequence, so reset everything
            // as specified in Section 6.2.5.3.
            *self = Self::new(header);
        } else {
            // Previous instruction was one of:
            // - Special - specified in Section 6.2.5.1, steps 4-7
            // - Copy - specified in Section 6.2.5.2
            // The reset behaviour is the same in both cases.
            self.discriminator = 0;
            self.basic_block = false;
            self.prologue_end = false;
            self.epilogue_begin = false;
        }
    }

    /// Step 1 of section 6.2.5.1
    fn apply_line_advance(&mut self, line_increment: i64) {
        if line_increment < 0 {
            let decrement = -line_increment as u64;
            if decrement <= self.line.0 {
                self.line.0 -= decrement;
            } else {
                self.line.0 = 0;
            }
        } else {
            self.line += Wrapping(line_increment as u64);
        }
    }

    /// Step 2 of section 6.2.5.1
    fn apply_operation_advance<R: Reader>(
        &mut self,
        operation_advance: u64,
        header: &LineProgramHeader<R>,
    ) -> Result<()> {
        if self.tombstone {
            return Ok(());
        }

        let operation_advance = Wrapping(operation_advance);

        let minimum_instruction_length = u64::from(header.line_encoding.minimum_instruction_length);
        let minimum_instruction_length = Wrapping(minimum_instruction_length);

        let maximum_operations_per_instruction =
            u64::from(header.line_encoding.maximum_operations_per_instruction);
        let maximum_operations_per_instruction = Wrapping(maximum_operations_per_instruction);

        let address_advance = if maximum_operations_per_instruction.0 == 1 {
            self.op_index.0 = 0;
            minimum_instruction_length * operation_advance
        } else {
            let op_index_with_advance = self.op_index + operation_advance;
            self.op_index = op_index_with_advance % maximum_operations_per_instruction;
            minimum_instruction_length
                * (op_index_with_advance / maximum_operations_per_instruction)
        };
        self.address = self
            .address
            .add_sized(address_advance.0, header.address_size())?;
        Ok(())
    }

    #[inline]
    fn adjust_opcode<R: Reader>(&self, opcode: u8, header: &LineProgramHeader<R>) -> u8 {
        opcode - header.opcode_base
    }

    /// Section 6.2.5.1
    fn exec_special_opcode<R: Reader>(
        &mut self,
        opcode: u8,
        header: &LineProgramHeader<R>,
    ) -> Result<()> {
        let adjusted_opcode = self.adjust_opcode(opcode, header);

        let line_range = header.line_encoding.line_range;
        let line_advance = adjusted_opcode % line_range;
        let operation_advance = adjusted_opcode / line_range;

        // Step 1
        let line_base = i64::from(header.line_encoding.line_base);
        self.apply_line_advance(line_base + i64::from(line_advance));

        // Step 2
        self.apply_operation_advance(u64::from(operation_advance), header)?;
        Ok(())
    }
}

/// The type of column that a row is referring to.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ColumnType {
    /// The `LeftEdge` means that the statement begins at the start of the new
    /// line.
    LeftEdge,
    /// A column number, whose range begins at 1.
    Column(NonZeroU64),
}

/// Deprecated. `LineNumberSequence` has been renamed to `LineSequence`.
#[deprecated(note = "LineNumberSequence has been renamed to LineSequence, use that instead.")]
pub type LineNumberSequence<R> = LineSequence<R>;

/// A sequence within a line number program.  A sequence, as defined in section
/// 6.2.5 of the standard, is a linear subset of a line number program within
/// which addresses are monotonically increasing.
#[derive(Clone, Debug)]
pub struct LineSequence<R: Reader> {
    /// The first address that is covered by this sequence within the line number
    /// program.
    pub start: u64,
    /// The first address that is *not* covered by this sequence within the line
    /// number program.
    pub end: u64,
    instructions: LineInstructions<R>,
}

/// Deprecated. `LineNumberProgramHeader` has been renamed to `LineProgramHeader`.
#[deprecated(
    note = "LineNumberProgramHeader has been renamed to LineProgramHeader, use that instead."
)]
pub type LineNumberProgramHeader<R, Offset> = LineProgramHeader<R, Offset>;

/// A header for a line number program in the `.debug_line` section, as defined
/// in section 6.2.4 of the standard.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineProgramHeader<R, Offset = <R as Reader>::Offset>
where
    R: Reader<Offset = Offset>,
    Offset: ReaderOffset,
{
    encoding: Encoding,
    offset: DebugLineOffset<Offset>,
    unit_length: Offset,

    header_length: Offset,

    line_encoding: LineEncoding,

    /// "The number assigned to the first special opcode."
    opcode_base: u8,

    /// "This array specifies the number of LEB128 operands for each of the
    /// standard opcodes. The first element of the array corresponds to the
    /// opcode whose value is 1, and the last element corresponds to the opcode
    /// whose value is `opcode_base - 1`."
    standard_opcode_lengths: R,

    /// "A sequence of directory entry format descriptions."
    directory_entry_format: Vec<FileEntryFormat>,

    /// > Entries in this sequence describe each path that was searched for
    /// > included source files in this compilation. (The paths include those
    /// > directories specified explicitly by the user for the compiler to search
    /// > and those the compiler searches without explicit direction.) Each path
    /// > entry is either a full path name or is relative to the current directory
    /// > of the compilation.
    /// >
    /// > The last entry is followed by a single null byte.
    include_directories: Vec<AttributeValue<R, Offset>>,

    /// "A sequence of file entry format descriptions."
    file_name_entry_format: Vec<FileEntryFormat>,

    /// "Entries in this sequence describe source files that contribute to the
    /// line number information for this compilation unit or is used in other
    /// contexts."
    file_names: Vec<FileEntry<R, Offset>>,

    /// The encoded line program instructions.
    program_buf: R,

    /// The current directory of the compilation.
    comp_dir: Option<R>,

    /// The primary source file.
    comp_file: Option<FileEntry<R, Offset>>,
}

impl<R, Offset> LineProgramHeader<R, Offset>
where
    R: Reader<Offset = Offset>,
    Offset: ReaderOffset,
{
    /// Return the offset of the line number program header in the `.debug_line` section.
    pub fn offset(&self) -> DebugLineOffset<R::Offset> {
        self.offset
    }

    /// Return the length of the line number program and header, not including
    /// the length of the encoded length itself.
    pub fn unit_length(&self) -> R::Offset {
        self.unit_length
    }

    /// Return the encoding parameters for this header's line program.
    pub fn encoding(&self) -> Encoding {
        self.encoding
    }

    /// Get the version of this header's line program.
    pub fn version(&self) -> u16 {
        self.encoding.version
    }

    /// Get the length of the encoded line number program header, not including
    /// the length of the encoded length itself.
    pub fn header_length(&self) -> R::Offset {
        self.header_length
    }

    /// Get the size in bytes of a target machine address.
    pub fn address_size(&self) -> u8 {
        self.encoding.address_size
    }

    /// Whether this line program is encoded in 64- or 32-bit DWARF.
    pub fn format(&self) -> Format {
        self.encoding.format
    }

    /// Get the line encoding parameters for this header's line program.
    pub fn line_encoding(&self) -> LineEncoding {
        self.line_encoding
    }

    /// Get the minimum instruction length any instruction in this header's line
    /// program may have.
    pub fn minimum_instruction_length(&self) -> u8 {
        self.line_encoding.minimum_instruction_length
    }

    /// Get the maximum number of operations each instruction in this header's
    /// line program may have.
    pub fn maximum_operations_per_instruction(&self) -> u8 {
        self.line_encoding.maximum_operations_per_instruction
    }

    /// Get the default value of the `is_stmt` register for this header's line
    /// program.
    pub fn default_is_stmt(&self) -> bool {
        self.line_encoding.default_is_stmt
    }

    /// Get the line base for this header's line program.
    pub fn line_base(&self) -> i8 {
        self.line_encoding.line_base
    }

    /// Get the line range for this header's line program.
    pub fn line_range(&self) -> u8 {
        self.line_encoding.line_range
    }

    /// Get opcode base for this header's line program.
    pub fn opcode_base(&self) -> u8 {
        self.opcode_base
    }

    /// An array of `u8` that specifies the number of LEB128 operands for
    /// each of the standard opcodes.
    pub fn standard_opcode_lengths(&self) -> &R {
        &self.standard_opcode_lengths
    }

    /// Get the format of a directory entry.
    pub fn directory_entry_format(&self) -> &[FileEntryFormat] {
        &self.directory_entry_format[..]
    }

    /// Get the set of include directories for this header's line program.
    ///
    /// For DWARF version <= 4, the compilation's current directory is not included
    /// in the return value, but is implicitly considered to be in the set per spec.
    pub fn include_directories(&self) -> &[AttributeValue<R, Offset>] {
        &self.include_directories[..]
    }

    /// The include directory with the given directory index.
    ///
    /// A directory index of 0 corresponds to the compilation unit directory.
    pub fn directory(&self, directory: u64) -> Option<AttributeValue<R, Offset>> {
        if self.encoding.version <= 4 {
            if directory == 0 {
                self.comp_dir.clone().map(AttributeValue::String)
            } else {
                let directory = directory as usize - 1;
                self.include_directories.get(directory).cloned()
            }
        } else {
            self.include_directories.get(directory as usize).cloned()
        }
    }

    /// Get the format of a file name entry.
    pub fn file_name_entry_format(&self) -> &[FileEntryFormat] {
        &self.file_name_entry_format[..]
    }

    /// Return true if the file entries may have valid timestamps.
    ///
    /// Only returns false if we definitely know that all timestamp fields
    /// are invalid.
    pub fn file_has_timestamp(&self) -> bool {
        self.encoding.version <= 4
            || self
                .file_name_entry_format
                .iter()
                .any(|x| x.content_type == constants::DW_LNCT_timestamp)
    }

    /// Return true if the file entries may have valid sizes.
    ///
    /// Only returns false if we definitely know that all size fields
    /// are invalid.
    pub fn file_has_size(&self) -> bool {
        self.encoding.version <= 4
            || self
                .file_name_entry_format
                .iter()
                .any(|x| x.content_type == constants::DW_LNCT_size)
    }

    /// Return true if the file name entry format contains an MD5 field.
    pub fn file_has_md5(&self) -> bool {
        self.file_name_entry_format
            .iter()
            .any(|x| x.content_type == constants::DW_LNCT_MD5)
    }

    /// Return true if the file name entry format contains a source field.
    pub fn file_has_source(&self) -> bool {
        self.file_name_entry_format
            .iter()
            .any(|x| x.content_type == constants::DW_LNCT_LLVM_source)
    }

    /// Get the list of source files that appear in this header's line program.
    pub fn file_names(&self) -> &[FileEntry<R, Offset>] {
        &self.file_names[..]
    }

    /// The source file with the given file index.
    ///
    /// A file index of 0 corresponds to the compilation unit file.
    /// Note that a file index of 0 is invalid for DWARF version <= 4,
    /// but we support it anyway.
    pub fn file(&self, file: u64) -> Option<&FileEntry<R, Offset>> {
        if self.encoding.version <= 4 {
            if file == 0 {
                self.comp_file.as_ref()
            } else {
                let file = file as usize - 1;
                self.file_names.get(file)
            }
        } else {
            self.file_names.get(file as usize)
        }
    }

    /// Get the raw, un-parsed `EndianSlice` containing this header's line number
    /// program.
    ///
    /// ```
    /// # fn foo() {
    /// use gimli::{LineProgramHeader, EndianSlice, NativeEndian};
    ///
    /// fn get_line_number_program_header<'a>() -> LineProgramHeader<EndianSlice<'a, NativeEndian>> {
    ///     // Get a line number program header from some offset in a
    ///     // `.debug_line` section...
    /// #   unimplemented!()
    /// }
    ///
    /// let header = get_line_number_program_header();
    /// let raw_program = header.raw_program_buf();
    /// println!("The length of the raw program in bytes is {}", raw_program.len());
    /// # }
    /// ```
    pub fn raw_program_buf(&self) -> R {
        self.program_buf.clone()
    }

    /// Iterate over the instructions in this header's line number program, parsing
    /// them as we go.
    pub fn instructions(&self) -> LineInstructions<R> {
        LineInstructions {
            input: self.program_buf.clone(),
        }
    }

    fn parse(
        input: &mut R,
        offset: DebugLineOffset<Offset>,
        mut address_size: u8,
        mut comp_dir: Option<R>,
        comp_name: Option<R>,
    ) -> Result<LineProgramHeader<R, Offset>> {
        let (unit_length, format) = input.read_initial_length()?;
        let rest = &mut input.split(unit_length)?;

        let version = rest.read_u16()?;
        if version < 2 || version > 5 {
            return Err(Error::UnknownVersion(u64::from(version)));
        }

        if version >= 5 {
            address_size = rest.read_address_size()?;
            let segment_selector_size = rest.read_u8()?;
            if segment_selector_size != 0 {
                return Err(Error::UnsupportedSegmentSize);
            }
        }

        let encoding = Encoding {
            format,
            version,
            address_size,
        };

        let header_length = rest.read_length(format)?;

        let mut program_buf = rest.clone();
        program_buf.skip(header_length)?;
        rest.truncate(header_length)?;

        let minimum_instruction_length = rest.read_u8()?;
        if minimum_instruction_length == 0 {
            return Err(Error::MinimumInstructionLengthZero);
        }

        // This field did not exist before DWARF 4, but is specified to be 1 for
        // non-VLIW architectures, which makes it a no-op.
        let maximum_operations_per_instruction = if version >= 4 { rest.read_u8()? } else { 1 };
        if maximum_operations_per_instruction == 0 {
            return Err(Error::MaximumOperationsPerInstructionZero);
        }

        let default_is_stmt = rest.read_u8()? != 0;
        let line_base = rest.read_i8()?;
        let line_range = rest.read_u8()?;
        if line_range == 0 {
            return Err(Error::LineRangeZero);
        }
        let line_encoding = LineEncoding {
            minimum_instruction_length,
            maximum_operations_per_instruction,
            default_is_stmt,
            line_base,
            line_range,
        };

        let opcode_base = rest.read_u8()?;
        if opcode_base == 0 {
            return Err(Error::OpcodeBaseZero);
        }

        let standard_opcode_count = R::Offset::from_u8(opcode_base - 1);
        let standard_opcode_lengths = rest.split(standard_opcode_count)?;

        let directory_entry_format;
        let mut include_directories = Vec::new();
        if version <= 4 {
            directory_entry_format = Vec::new();
            loop {
                let directory = rest.read_null_terminated_slice()?;
                if directory.is_empty() {
                    break;
                }
                include_directories.push(AttributeValue::String(directory));
            }
        } else {
            comp_dir = None;
            directory_entry_format = FileEntryFormat::parse(rest)?;
            let count = rest.read_uleb128()?;
            for _ in 0..count {
                include_directories.push(parse_directory_v5(
                    rest,
                    encoding,
                    &directory_entry_format,
                )?);
            }
        }

        let comp_file;
        let file_name_entry_format;
        let mut file_names = Vec::new();
        if version <= 4 {
            comp_file = comp_name.map(|name| FileEntry {
                path_name: AttributeValue::String(name),
                directory_index: 0,
                timestamp: 0,
                size: 0,
                md5: [0; 16],
                source: None,
            });

            file_name_entry_format = Vec::new();
            loop {
                let path_name = rest.read_null_terminated_slice()?;
                if path_name.is_empty() {
                    break;
                }
                file_names.push(FileEntry::parse(rest, path_name)?);
            }
        } else {
            comp_file = None;
            file_name_entry_format = FileEntryFormat::parse(rest)?;
            let count = rest.read_uleb128()?;
            for _ in 0..count {
                file_names.push(parse_file_v5(rest, encoding, &file_name_entry_format)?);
            }
        }

        let header = LineProgramHeader {
            encoding,
            offset,
            unit_length,
            header_length,
            line_encoding,
            opcode_base,
            standard_opcode_lengths,
            directory_entry_format,
            include_directories,
            file_name_entry_format,
            file_names,
            program_buf,
            comp_dir,
            comp_file,
        };
        Ok(header)
    }
}

/// Deprecated. `IncompleteLineNumberProgram` has been renamed to `IncompleteLineProgram`.
#[deprecated(
    note = "IncompleteLineNumberProgram has been renamed to IncompleteLineProgram, use that instead."
)]
pub type IncompleteLineNumberProgram<R, Offset> = IncompleteLineProgram<R, Offset>;

/// A line number program that has not been run to completion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncompleteLineProgram<R, Offset = <R as Reader>::Offset>
where
    R: Reader<Offset = Offset>,
    Offset: ReaderOffset,
{
    header: LineProgramHeader<R, Offset>,
}

impl<R, Offset> IncompleteLineProgram<R, Offset>
where
    R: Reader<Offset = Offset>,
    Offset: ReaderOffset,
{
    /// Retrieve the `LineProgramHeader` for this program.
    pub fn header(&self) -> &LineProgramHeader<R, Offset> {
        &self.header
    }

    /// Construct a new `LineRows` for executing this program to iterate
    /// over rows in the line information matrix.
    pub fn rows(self) -> OneShotLineRows<R, Offset> {
        OneShotLineRows::new(self)
    }

    /// Execute the line number program, completing the `IncompleteLineProgram`
    /// into a `CompleteLineProgram` and producing an array of sequences within
    /// the line number program that can later be used with
    /// `CompleteLineProgram::resume_from`.
    ///
    /// ```
    /// # fn foo() {
    /// use gimli::{IncompleteLineProgram, EndianSlice, NativeEndian};
    ///
    /// fn get_line_number_program<'a>() -> IncompleteLineProgram<EndianSlice<'a, NativeEndian>> {
    ///     // Get a line number program from some offset in a
    ///     // `.debug_line` section...
    /// #   unimplemented!()
    /// }
    ///
    /// let program = get_line_number_program();
    /// let (program, sequences) = program.sequences().unwrap();
    /// println!("There are {} sequences in this line number program", sequences.len());
    /// # }
    /// ```
    #[allow(clippy::type_complexity)]
    pub fn sequences(self) -> Result<(CompleteLineProgram<R, Offset>, Vec<LineSequence<R>>)> {
        let mut sequences = Vec::new();
        let mut rows = self.rows();
        let mut instructions = rows.instructions.clone();
        let mut sequence_start_addr = None;
        loop {
            let sequence_end_addr;
            if rows.next_row()?.is_none() {
                break;
            }

            let row = &rows.row;
            if row.end_sequence() {
                sequence_end_addr = row.address();
            } else if sequence_start_addr.is_none() {
                sequence_start_addr = Some(row.address());
                continue;
            } else {
                continue;
            }

            // We just finished a sequence.
            sequences.push(LineSequence {
                // In theory one could have multiple DW_LNE_end_sequence instructions
                // in a row.
                start: sequence_start_addr.unwrap_or(0),
                end: sequence_end_addr,
                instructions: instructions.remove_trailing(&rows.instructions)?,
            });
            sequence_start_addr = None;
            instructions = rows.instructions.clone();
        }

        let program = CompleteLineProgram {
            header: rows.program.header,
        };
        Ok((program, sequences))
    }
}

/// Deprecated. `CompleteLineNumberProgram` has been renamed to `CompleteLineProgram`.
#[deprecated(
    note = "CompleteLineNumberProgram has been renamed to CompleteLineProgram, use that instead."
)]
pub type CompleteLineNumberProgram<R, Offset> = CompleteLineProgram<R, Offset>;

/// A line number program that has previously been run to completion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompleteLineProgram<R, Offset = <R as Reader>::Offset>
where
    R: Reader<Offset = Offset>,
    Offset: ReaderOffset,
{
    header: LineProgramHeader<R, Offset>,
}

impl<R, Offset> CompleteLineProgram<R, Offset>
where
    R: Reader<Offset = Offset>,
    Offset: ReaderOffset,
{
    /// Retrieve the `LineProgramHeader` for this program.
    pub fn header(&self) -> &LineProgramHeader<R, Offset> {
        &self.header
    }

    /// Construct a new `LineRows` for executing the subset of the line
    /// number program identified by 'sequence' and  generating the line information
    /// matrix.
    ///
    /// ```
    /// # fn foo() {
    /// use gimli::{IncompleteLineProgram, EndianSlice, NativeEndian};
    ///
    /// fn get_line_number_program<'a>() -> IncompleteLineProgram<EndianSlice<'a, NativeEndian>> {
    ///     // Get a line number program from some offset in a
    ///     // `.debug_line` section...
    /// #   unimplemented!()
    /// }
    ///
    /// let program = get_line_number_program();
    /// let (program, sequences) = program.sequences().unwrap();
    /// for sequence in &sequences {
    ///     let mut sm = program.resume_from(sequence);
    /// }
    /// # }
    /// ```
    pub fn resume_from<'program>(
        &'program self,
        sequence: &LineSequence<R>,
    ) -> ResumedLineRows<'program, R, Offset> {
        ResumedLineRows::resume(self, sequence)
    }
}

/// An entry in the `LineProgramHeader`'s `file_names` set.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FileEntry<R, Offset = <R as Reader>::Offset>
where
    R: Reader<Offset = Offset>,
    Offset: ReaderOffset,
{
    path_name: AttributeValue<R, Offset>,
    directory_index: u64,
    timestamp: u64,
    size: u64,
    md5: [u8; 16],
    source: Option<AttributeValue<R, Offset>>,
}

impl<R, Offset> FileEntry<R, Offset>
where
    R: Reader<Offset = Offset>,
    Offset: ReaderOffset,
{
    // version 2-4
    fn parse(input: &mut R, path_name: R) -> Result<FileEntry<R, Offset>> {
        let directory_index = input.read_uleb128()?;
        let timestamp = input.read_uleb128()?;
        let size = input.read_uleb128()?;

        let entry = FileEntry {
            path_name: AttributeValue::String(path_name),
            directory_index,
            timestamp,
            size,
            md5: [0; 16],
            source: None,
        };

        Ok(entry)
    }

    /// > A slice containing the full or relative path name of
    /// > a source file. If the entry contains a file name or a relative path
    /// > name, the file is located relative to either the compilation directory
    /// > (as specified by the DW_AT_comp_dir attribute given in the compilation
    /// > unit) or one of the directories in the include_directories section.
    pub fn path_name(&self) -> AttributeValue<R, Offset> {
        self.path_name.clone()
    }

    /// > An unsigned LEB128 number representing the directory index of the
    /// > directory in which the file was found.
    /// >
    /// > ...
    /// >
    /// > The directory index represents an entry in the include_directories
    /// > section of the line number program header. The index is 0 if the file
    /// > was found in the current directory of the compilation, 1 if it was found
    /// > in the first directory in the include_directories section, and so
    /// > on. The directory index is ignored for file names that represent full
    /// > path names.
    pub fn directory_index(&self) -> u64 {
        self.directory_index
    }

    /// Get this file's directory.
    ///
    /// A directory index of 0 corresponds to the compilation unit directory.
    pub fn directory(&self, header: &LineProgramHeader<R>) -> Option<AttributeValue<R, Offset>> {
        header.directory(self.directory_index)
    }

    /// The implementation-defined time of last modification of the file,
    /// or 0 if not available.
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// "An unsigned LEB128 number representing the time of last modification of
    /// the file, or 0 if not available."
    // Terminology changed in DWARF version 5.
    #[doc(hidden)]
    pub fn last_modification(&self) -> u64 {
        self.timestamp
    }

    /// The size of the file in bytes, or 0 if not available.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// "An unsigned LEB128 number representing the length in bytes of the file,
    /// or 0 if not available."
    // Terminology changed in DWARF version 5.
    #[doc(hidden)]
    pub fn length(&self) -> u64 {
        self.size
    }

    /// A 16-byte MD5 digest of the file contents.
    ///
    /// Only valid if `LineProgramHeader::file_has_md5` returns `true`.
    pub fn md5(&self) -> &[u8; 16] {
        &self.md5
    }

    /// The source code of this file. (UTF-8 source text string with "\n" line
    /// endings).
    ///
    /// Note: For DWARF v5 files this may return an empty attribute that
    /// indicates that no source code is available, which this function
    /// represents as `Some(<zero-length attr>)`.
    pub fn source(&self) -> Option<AttributeValue<R, Offset>> {
        self.source.clone()
    }
}

/// The format of a component of an include directory or file name entry.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FileEntryFormat {
    /// The type of information that is represented by the component.
    pub content_type: constants::DwLnct,

    /// The encoding form of the component value.
    pub form: constants::DwForm,
}

impl FileEntryFormat {
    fn parse<R: Reader>(input: &mut R) -> Result<Vec<FileEntryFormat>> {
        let format_count = input.read_u8()? as usize;
        let mut format = Vec::with_capacity(format_count);
        let mut path_count = 0;
        for _ in 0..format_count {
            let content_type = input.read_uleb128()?;
            let content_type = if content_type > u64::from(u16::MAX) {
                constants::DwLnct(u16::MAX)
            } else {
                constants::DwLnct(content_type as u16)
            };
            if content_type == constants::DW_LNCT_path {
                path_count += 1;
            }

            let form = constants::DwForm(input.read_uleb128_u16()?);

            format.push(FileEntryFormat { content_type, form });
        }
        if path_count != 1 {
            return Err(Error::MissingFileEntryFormatPath);
        }
        Ok(format)
    }
}

fn parse_directory_v5<R: Reader>(
    input: &mut R,
    encoding: Encoding,
    formats: &[FileEntryFormat],
) -> Result<AttributeValue<R>> {
    let mut path_name = None;

    for format in formats {
        let value = parse_attribute(input, encoding, format.form)?;
        if format.content_type == constants::DW_LNCT_path {
            path_name = Some(value);
        }
    }

    Ok(path_name.unwrap())
}

fn parse_file_v5<R: Reader>(
    input: &mut R,
    encoding: Encoding,
    formats: &[FileEntryFormat],
) -> Result<FileEntry<R>> {
    let mut path_name = None;
    let mut directory_index = 0;
    let mut timestamp = 0;
    let mut size = 0;
    let mut md5 = [0; 16];
    let mut source = None;

    for format in formats {
        let value = parse_attribute(input, encoding, format.form)?;
        match format.content_type {
            constants::DW_LNCT_path => path_name = Some(value),
            constants::DW_LNCT_directory_index => {
                if let Some(value) = value.udata_value() {
                    directory_index = value;
                }
            }
            constants::DW_LNCT_timestamp => {
                if let Some(value) = value.udata_value() {
                    timestamp = value;
                }
            }
            constants::DW_LNCT_size => {
                if let Some(value) = value.udata_value() {
                    size = value;
                }
            }
            constants::DW_LNCT_MD5 => {
                if let AttributeValue::Block(mut value) = value {
                    if value.len().into_u64() == 16 {
                        md5 = value.read_u8_array()?;
                    }
                }
            }
            constants::DW_LNCT_LLVM_source => {
                source = Some(value);
            }
            // Ignore unknown content types.
            _ => {}
        }
    }

    Ok(FileEntry {
        path_name: path_name.unwrap(),
        directory_index,
        timestamp,
        size,
        md5,
        source,
    })
}

// TODO: this should be shared with unit::parse_attribute(), but that is hard to do.
fn parse_attribute<R: Reader>(
    input: &mut R,
    encoding: Encoding,
    form: constants::DwForm,
) -> Result<AttributeValue<R>> {
    Ok(match form {
        constants::DW_FORM_block1 => {
            let len = input.read_u8().map(R::Offset::from_u8)?;
            let block = input.split(len)?;
            AttributeValue::Block(block)
        }
        constants::DW_FORM_block2 => {
            let len = input.read_u16().map(R::Offset::from_u16)?;
            let block = input.split(len)?;
            AttributeValue::Block(block)
        }
        constants::DW_FORM_block4 => {
            let len = input.read_u32().map(R::Offset::from_u32)?;
            let block = input.split(len)?;
            AttributeValue::Block(block)
        }
        constants::DW_FORM_block => {
            let len = input.read_uleb128().and_then(R::Offset::from_u64)?;
            let block = input.split(len)?;
            AttributeValue::Block(block)
        }
        constants::DW_FORM_data1 => {
            let data = input.read_u8()?;
            AttributeValue::Data1(data)
        }
        constants::DW_FORM_data2 => {
            let data = input.read_u16()?;
            AttributeValue::Data2(data)
        }
        constants::DW_FORM_data4 => {
            let data = input.read_u32()?;
            AttributeValue::Data4(data)
        }
        constants::DW_FORM_data8 => {
            let data = input.read_u64()?;
            AttributeValue::Data8(data)
        }
        constants::DW_FORM_data16 => {
            let block = input.split(R::Offset::from_u8(16))?;
            AttributeValue::Block(block)
        }
        constants::DW_FORM_udata => {
            let data = input.read_uleb128()?;
            AttributeValue::Udata(data)
        }
        constants::DW_FORM_sdata => {
            let data = input.read_sleb128()?;
            AttributeValue::Sdata(data)
        }
        constants::DW_FORM_flag => {
            let present = input.read_u8()?;
            AttributeValue::Flag(present != 0)
        }
        constants::DW_FORM_sec_offset => {
            let offset = input.read_offset(encoding.format)?;
            AttributeValue::SecOffset(offset)
        }
        constants::DW_FORM_string => {
            let string = input.read_null_terminated_slice()?;
            AttributeValue::String(string)
        }
        constants::DW_FORM_strp => {
            let offset = input.read_offset(encoding.format)?;
            AttributeValue::DebugStrRef(DebugStrOffset(offset))
        }
        constants::DW_FORM_strp_sup | constants::DW_FORM_GNU_strp_alt => {
            let offset = input.read_offset(encoding.format)?;
            AttributeValue::DebugStrRefSup(DebugStrOffset(offset))
        }
        constants::DW_FORM_line_strp => {
            let offset = input.read_offset(encoding.format)?;
            AttributeValue::DebugLineStrRef(DebugLineStrOffset(offset))
        }
        constants::DW_FORM_strx | constants::DW_FORM_GNU_str_index => {
            let index = input.read_uleb128().and_then(R::Offset::from_u64)?;
            AttributeValue::DebugStrOffsetsIndex(DebugStrOffsetsIndex(index))
        }
        constants::DW_FORM_strx1 => {
            let index = input.read_u8().map(R::Offset::from_u8)?;
            AttributeValue::DebugStrOffsetsIndex(DebugStrOffsetsIndex(index))
        }
        constants::DW_FORM_strx2 => {
            let index = input.read_u16().map(R::Offset::from_u16)?;
            AttributeValue::DebugStrOffsetsIndex(DebugStrOffsetsIndex(index))
        }
        constants::DW_FORM_strx3 => {
            let index = input.read_uint(3).and_then(R::Offset::from_u64)?;
            AttributeValue::DebugStrOffsetsIndex(DebugStrOffsetsIndex(index))
        }
        constants::DW_FORM_strx4 => {
            let index = input.read_u32().map(R::Offset::from_u32)?;
            AttributeValue::DebugStrOffsetsIndex(DebugStrOffsetsIndex(index))
        }
        _ => {
            return Err(Error::UnknownForm(form));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants;
    use crate::endianity::LittleEndian;
    use crate::read::{EndianSlice, Error};
    use crate::test_util::GimliSectionMethods;
    use test_assembler::{Endian, Label, LabelMaker, Section};

    #[test]
    fn test_parse_debug_line_32_ok() {
        #[rustfmt::skip]
        let buf = [
            // 32-bit length = 62.
            0x3e, 0x00, 0x00, 0x00,
            // Version.
            0x04, 0x00,
            // Header length = 40.
            0x28, 0x00, 0x00, 0x00,
            // Minimum instruction length.
            0x01,
            // Maximum operations per byte.
            0x01,
            // Default is_stmt.
            0x01,
            // Line base.
            0x00,
            // Line range.
            0x01,
            // Opcode base.
            0x03,
            // Standard opcode lengths for opcodes 1 .. opcode base - 1.
            0x01, 0x02,
            // Include directories = '/', 'i', 'n', 'c', '\0', '/', 'i', 'n', 'c', '2', '\0', '\0'
            0x2f, 0x69, 0x6e, 0x63, 0x00, 0x2f, 0x69, 0x6e, 0x63, 0x32, 0x00, 0x00,
            // File names
                // foo.rs
                0x66, 0x6f, 0x6f, 0x2e, 0x72, 0x73, 0x00,
                0x00,
                0x00,
                0x00,
                // bar.h
                0x62, 0x61, 0x72, 0x2e, 0x68, 0x00,
                0x01,
                0x00,
                0x00,
            // End file names.
            0x00,

            // Dummy line program data.
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,

            // Dummy next line program.
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];

        let rest = &mut EndianSlice::new(&buf, LittleEndian);
        let comp_dir = EndianSlice::new(b"/comp_dir", LittleEndian);
        let comp_name = EndianSlice::new(b"/comp_name", LittleEndian);

        let header =
            LineProgramHeader::parse(rest, DebugLineOffset(0), 4, Some(comp_dir), Some(comp_name))
                .expect("should parse header ok");

        assert_eq!(
            *rest,
            EndianSlice::new(&buf[buf.len() - 16..], LittleEndian)
        );

        assert_eq!(header.offset, DebugLineOffset(0));
        assert_eq!(header.version(), 4);
        assert_eq!(header.minimum_instruction_length(), 1);
        assert_eq!(header.maximum_operations_per_instruction(), 1);
        assert!(header.default_is_stmt());
        assert_eq!(header.line_base(), 0);
        assert_eq!(header.line_range(), 1);
        assert_eq!(header.opcode_base(), 3);
        assert_eq!(header.directory(0), Some(AttributeValue::String(comp_dir)));
        assert_eq!(
            header.file(0).unwrap().path_name,
            AttributeValue::String(comp_name)
        );

        let expected_lengths = [1, 2];
        assert_eq!(header.standard_opcode_lengths().slice(), &expected_lengths);

        let expected_include_directories = [
            AttributeValue::String(EndianSlice::new(b"/inc", LittleEndian)),
            AttributeValue::String(EndianSlice::new(b"/inc2", LittleEndian)),
        ];
        assert_eq!(header.include_directories(), &expected_include_directories);

        let expected_file_names = [
            FileEntry {
                path_name: AttributeValue::String(EndianSlice::new(b"foo.rs", LittleEndian)),
                directory_index: 0,
                timestamp: 0,
                size: 0,
                md5: [0; 16],
                source: None,
            },
            FileEntry {
                path_name: AttributeValue::String(EndianSlice::new(b"bar.h", LittleEndian)),
                directory_index: 1,
                timestamp: 0,
                size: 0,
                md5: [0; 16],
                source: None,
            },
        ];
        assert_eq!(header.file_names(), &expected_file_names);
    }

    #[test]
    fn test_parse_debug_line_header_length_too_short() {
        #[rustfmt::skip]
        let buf = [
            // 32-bit length = 62.
            0x3e, 0x00, 0x00, 0x00,
            // Version.
            0x04, 0x00,
            // Header length = 20. TOO SHORT!!!
            0x15, 0x00, 0x00, 0x00,
            // Minimum instruction length.
            0x01,
            // Maximum operations per byte.
            0x01,
            // Default is_stmt.
            0x01,
            // Line base.
            0x00,
            // Line range.
            0x01,
            // Opcode base.
            0x03,
            // Standard opcode lengths for opcodes 1 .. opcode base - 1.
            0x01, 0x02,
            // Include directories = '/', 'i', 'n', 'c', '\0', '/', 'i', 'n', 'c', '2', '\0', '\0'
            0x2f, 0x69, 0x6e, 0x63, 0x00, 0x2f, 0x69, 0x6e, 0x63, 0x32, 0x00, 0x00,
            // File names
                // foo.rs
                0x66, 0x6f, 0x6f, 0x2e, 0x72, 0x73, 0x00,
                0x00,
                0x00,
                0x00,
                // bar.h
                0x62, 0x61, 0x72, 0x2e, 0x68, 0x00,
                0x01,
                0x00,
                0x00,
            // End file names.
            0x00,

            // Dummy line program data.
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,

            // Dummy next line program.
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];

        let input = &mut EndianSlice::new(&buf, LittleEndian);

        match LineProgramHeader::parse(input, DebugLineOffset(0), 4, None, None) {
            Err(Error::UnexpectedEof(_)) => {}
            otherwise => panic!("Unexpected result: {:?}", otherwise),
        }
    }

    #[test]
    fn test_parse_debug_line_unit_length_too_short() {
        #[rustfmt::skip]
        let buf = [
            // 32-bit length = 40. TOO SHORT!!!
            0x28, 0x00, 0x00, 0x00,
            // Version.
            0x04, 0x00,
            // Header length = 40.
            0x28, 0x00, 0x00, 0x00,
            // Minimum instruction length.
            0x01,
            // Maximum operations per byte.
            0x01,
            // Default is_stmt.
            0x01,
            // Line base.
            0x00,
            // Line range.
            0x01,
            // Opcode base.
            0x03,
            // Standard opcode lengths for opcodes 1 .. opcode base - 1.
            0x01, 0x02,
            // Include directories = '/', 'i', 'n', 'c', '\0', '/', 'i', 'n', 'c', '2', '\0', '\0'
            0x2f, 0x69, 0x6e, 0x63, 0x00, 0x2f, 0x69, 0x6e, 0x63, 0x32, 0x00, 0x00,
            // File names
                // foo.rs
                0x66, 0x6f, 0x6f, 0x2e, 0x72, 0x73, 0x00,
                0x00,
                0x00,
                0x00,
                // bar.h
                0x62, 0x61, 0x72, 0x2e, 0x68, 0x00,
                0x01,
                0x00,
                0x00,
            // End file names.
            0x00,

            // Dummy line program data.
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,

            // Dummy next line program.
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];

        let input = &mut EndianSlice::new(&buf, LittleEndian);

        match LineProgramHeader::parse(input, DebugLineOffset(0), 4, None, None) {
            Err(Error::UnexpectedEof(_)) => {}
            otherwise => panic!("Unexpected result: {:?}", otherwise),
        }
    }

    const OPCODE_BASE: u8 = 13;
    const STANDARD_OPCODE_LENGTHS: &[u8] = &[0, 1, 1, 1, 1, 0, 0, 0, 1, 0, 0, 1];

    fn make_test_header(
        buf: EndianSlice<'_, LittleEndian>,
    ) -> LineProgramHeader<EndianSlice<'_, LittleEndian>> {
        let encoding = Encoding {
            format: Format::Dwarf32,
            version: 4,
            address_size: 8,
        };
        let line_encoding = LineEncoding {
            line_base: -3,
            line_range: 12,
            ..Default::default()
        };
        LineProgramHeader {
            encoding,
            offset: DebugLineOffset(0),
            unit_length: 1,
            header_length: 1,
            line_encoding,
            opcode_base: OPCODE_BASE,
            standard_opcode_lengths: EndianSlice::new(STANDARD_OPCODE_LENGTHS, LittleEndian),
            file_names: vec![
                FileEntry {
                    path_name: AttributeValue::String(EndianSlice::new(b"foo.c", LittleEndian)),
                    directory_index: 0,
                    timestamp: 0,
                    size: 0,
                    md5: [0; 16],
                    source: None,
                },
                FileEntry {
                    path_name: AttributeValue::String(EndianSlice::new(b"bar.rs", LittleEndian)),
                    directory_index: 0,
                    timestamp: 0,
                    size: 0,
                    md5: [0; 16],
                    source: None,
                },
            ],
            include_directories: vec![],
            directory_entry_format: vec![],
            file_name_entry_format: vec![],
            program_buf: buf,
            comp_dir: None,
            comp_file: None,
        }
    }

    fn make_test_program(
        buf: EndianSlice<'_, LittleEndian>,
    ) -> IncompleteLineProgram<EndianSlice<'_, LittleEndian>> {
        IncompleteLineProgram {
            header: make_test_header(buf),
        }
    }

    #[test]
    fn test_parse_special_opcodes() {
        for i in OPCODE_BASE..u8::MAX {
            let input = [i, 0, 0, 0];
            let input = EndianSlice::new(&input, LittleEndian);
            let header = make_test_header(input);

            let mut rest = input;
            let opcode =
                LineInstruction::parse(&header, &mut rest).expect("Should parse the opcode OK");

            assert_eq!(*rest, *input.range_from(1..));
            assert_eq!(opcode, LineInstruction::Special(i));
        }
    }

    #[test]
    fn test_parse_standard_opcodes() {
        fn test<Operands>(
            raw: constants::DwLns,
            operands: Operands,
            expected: LineInstruction<EndianSlice<'_, LittleEndian>>,
        ) where
            Operands: AsRef<[u8]>,
        {
            let mut input = Vec::new();
            input.push(raw.0);
            input.extend_from_slice(operands.as_ref());

            let expected_rest = [0, 1, 2, 3, 4];
            input.extend_from_slice(&expected_rest);

            let input = EndianSlice::new(&input, LittleEndian);
            let header = make_test_header(input);

            let mut rest = input;
            let opcode =
                LineInstruction::parse(&header, &mut rest).expect("Should parse the opcode OK");

            assert_eq!(opcode, expected);
            assert_eq!(*rest, expected_rest);
        }

        test(constants::DW_LNS_copy, [], LineInstruction::Copy);
        test(
            constants::DW_LNS_advance_pc,
            [42],
            LineInstruction::AdvancePc(42),
        );
        test(
            constants::DW_LNS_advance_line,
            [9],
            LineInstruction::AdvanceLine(9),
        );
        test(constants::DW_LNS_set_file, [7], LineInstruction::SetFile(7));
        test(
            constants::DW_LNS_set_column,
            [1],
            LineInstruction::SetColumn(1),
        );
        test(
            constants::DW_LNS_negate_stmt,
            [],
            LineInstruction::NegateStatement,
        );
        test(
            constants::DW_LNS_set_basic_block,
            [],
            LineInstruction::SetBasicBlock,
        );
        test(
            constants::DW_LNS_const_add_pc,
            [],
            LineInstruction::ConstAddPc,
        );
        test(
            constants::DW_LNS_fixed_advance_pc,
            [42, 0],
            LineInstruction::FixedAddPc(42),
        );
        test(
            constants::DW_LNS_set_prologue_end,
            [],
            LineInstruction::SetPrologueEnd,
        );
        test(
            constants::DW_LNS_set_isa,
            [57 + 0x80, 100],
            LineInstruction::SetIsa(12857),
        );
    }

    #[test]
    fn test_parse_unknown_standard_opcode_no_args() {
        let input = [OPCODE_BASE, 1, 2, 3];
        let input = EndianSlice::new(&input, LittleEndian);
        let mut standard_opcode_lengths = Vec::new();
        let mut header = make_test_header(input);
        standard_opcode_lengths.extend(header.standard_opcode_lengths.slice());
        standard_opcode_lengths.push(0);
        header.opcode_base += 1;
        header.standard_opcode_lengths = EndianSlice::new(&standard_opcode_lengths, LittleEndian);

        let mut rest = input;
        let opcode =
            LineInstruction::parse(&header, &mut rest).expect("Should parse the opcode OK");

        assert_eq!(
            opcode,
            LineInstruction::UnknownStandard0(constants::DwLns(OPCODE_BASE))
        );
        assert_eq!(*rest, *input.range_from(1..));
    }

    #[test]
    fn test_parse_unknown_standard_opcode_one_arg() {
        let input = [OPCODE_BASE, 1, 2, 3];
        let input = EndianSlice::new(&input, LittleEndian);
        let mut standard_opcode_lengths = Vec::new();
        let mut header = make_test_header(input);
        standard_opcode_lengths.extend(header.standard_opcode_lengths.slice());
        standard_opcode_lengths.push(1);
        header.opcode_base += 1;
        header.standard_opcode_lengths = EndianSlice::new(&standard_opcode_lengths, LittleEndian);

        let mut rest = input;
        let opcode =
            LineInstruction::parse(&header, &mut rest).expect("Should parse the opcode OK");

        assert_eq!(
            opcode,
            LineInstruction::UnknownStandard1(constants::DwLns(OPCODE_BASE), 1)
        );
        assert_eq!(*rest, *input.range_from(2..));
    }

    #[test]
    fn test_parse_unknown_standard_opcode_many_args() {
        let input = [OPCODE_BASE, 1, 2, 3];
        let input = EndianSlice::new(&input, LittleEndian);
        let args = input.range_from(1..);
        let mut standard_opcode_lengths = Vec::new();
        let mut header = make_test_header(input);
        standard_opcode_lengths.extend(header.standard_opcode_lengths.slice());
        standard_opcode_lengths.push(3);
        header.opcode_base += 1;
        header.standard_opcode_lengths = EndianSlice::new(&standard_opcode_lengths, LittleEndian);

        let mut rest = input;
        let opcode =
            LineInstruction::parse(&header, &mut rest).expect("Should parse the opcode OK");

        assert_eq!(
            opcode,
            LineInstruction::UnknownStandardN(constants::DwLns(OPCODE_BASE), args)
        );
        assert_eq!(*rest, []);
    }

    #[test]
    fn test_parse_extended_opcodes() {
        fn test<Operands>(
            raw: constants::DwLne,
            operands: Operands,
            expected: LineInstruction<EndianSlice<'_, LittleEndian>>,
        ) where
            Operands: AsRef<[u8]>,
        {
            let mut input = Vec::new();
            input.push(0);

            let operands = operands.as_ref();
            input.push(1 + operands.len() as u8);

            input.push(raw.0);
            input.extend_from_slice(operands);

            let expected_rest = [0, 1, 2, 3, 4];
            input.extend_from_slice(&expected_rest);

            let input = EndianSlice::new(&input, LittleEndian);
            let header = make_test_header(input);

            let mut rest = input;
            let opcode =
                LineInstruction::parse(&header, &mut rest).expect("Should parse the opcode OK");

            assert_eq!(opcode, expected);
            assert_eq!(*rest, expected_rest);
        }

        test(
            constants::DW_LNE_end_sequence,
            [],
            LineInstruction::EndSequence,
        );
        test(
            constants::DW_LNE_set_address,
            [1, 2, 3, 4, 5, 6, 7, 8],
            LineInstruction::SetAddress(578_437_695_752_307_201),
        );
        test(
            constants::DW_LNE_set_discriminator,
            [42],
            LineInstruction::SetDiscriminator(42),
        );

        let mut file = Vec::new();
        // "foo.c"
        let path_name = [b'f', b'o', b'o', b'.', b'c', 0];
        file.extend_from_slice(&path_name);
        // Directory index.
        file.push(0);
        // Last modification of file.
        file.push(1);
        // Size of file.
        file.push(2);

        test(
            constants::DW_LNE_define_file,
            file,
            LineInstruction::DefineFile(FileEntry {
                path_name: AttributeValue::String(EndianSlice::new(b"foo.c", LittleEndian)),
                directory_index: 0,
                timestamp: 1,
                size: 2,
                md5: [0; 16],
                source: None,
            }),
        );

        // Unknown extended opcode.
        let operands = [1, 2, 3, 4, 5, 6];
        let opcode = constants::DwLne(99);
        test(
            opcode,
            operands,
            LineInstruction::UnknownExtended(opcode, EndianSlice::new(&operands, LittleEndian)),
        );
    }

    #[test]
    fn test_file_entry_directory() {
        let path_name = [b'f', b'o', b'o', b'.', b'r', b's', 0];

        let mut file = FileEntry {
            path_name: AttributeValue::String(EndianSlice::new(&path_name, LittleEndian)),
            directory_index: 1,
            timestamp: 0,
            size: 0,
            md5: [0; 16],
            source: None,
        };

        let mut header = make_test_header(EndianSlice::new(&[], LittleEndian));

        let dir = AttributeValue::String(EndianSlice::new(b"dir", LittleEndian));
        header.include_directories.push(dir);

        assert_eq!(file.directory(&header), Some(dir));

        // Now test the compilation's current directory.
        file.directory_index = 0;
        assert_eq!(file.directory(&header), None);
    }

    fn assert_exec_opcode<'input>(
        header: LineProgramHeader<EndianSlice<'input, LittleEndian>>,
        mut registers: LineRow,
        opcode: LineInstruction<EndianSlice<'input, LittleEndian>>,
        expected_registers: LineRow,
        expect_new_row: bool,
    ) {
        let mut program = IncompleteLineProgram { header };
        let is_new_row = registers.execute(opcode, &mut program);

        assert_eq!(is_new_row, Ok(expect_new_row));
        assert_eq!(registers, expected_registers);
    }

    #[test]
    fn test_exec_special_noop() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));

        let initial_registers = LineRow::new(&header);
        let opcode = LineInstruction::Special(16);
        let expected_registers = initial_registers;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, true);
    }

    #[test]
    fn test_exec_special_negative_line_advance() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));

        let mut initial_registers = LineRow::new(&header);
        initial_registers.line.0 = 10;

        let opcode = LineInstruction::Special(13);

        let mut expected_registers = initial_registers;
        expected_registers.line.0 -= 3;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, true);
    }

    #[test]
    fn test_exec_special_positive_line_advance() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));

        let initial_registers = LineRow::new(&header);

        let opcode = LineInstruction::Special(19);

        let mut expected_registers = initial_registers;
        expected_registers.line.0 += 3;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, true);
    }

    #[test]
    fn test_exec_special_positive_address_advance() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));

        let initial_registers = LineRow::new(&header);

        let opcode = LineInstruction::Special(52);

        let mut expected_registers = initial_registers;
        expected_registers.address += 3;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, true);
    }

    #[test]
    fn test_exec_special_positive_address_and_line_advance() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));

        let initial_registers = LineRow::new(&header);

        let opcode = LineInstruction::Special(55);

        let mut expected_registers = initial_registers;
        expected_registers.address += 3;
        expected_registers.line.0 += 3;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, true);
    }

    #[test]
    fn test_exec_special_positive_address_and_negative_line_advance() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));

        let mut initial_registers = LineRow::new(&header);
        initial_registers.line.0 = 10;

        let opcode = LineInstruction::Special(49);

        let mut expected_registers = initial_registers;
        expected_registers.address += 3;
        expected_registers.line.0 -= 3;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, true);
    }

    #[test]
    fn test_exec_special_line_underflow() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));

        let mut initial_registers = LineRow::new(&header);
        initial_registers.line.0 = 2;

        // -3 line advance.
        let opcode = LineInstruction::Special(13);

        let mut expected_registers = initial_registers;
        // Clamp at 0. No idea if this is the best way to handle this situation
        // or not...
        expected_registers.line.0 = 0;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, true);
    }

    #[test]
    fn test_exec_copy() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));

        let mut initial_registers = LineRow::new(&header);
        initial_registers.address = 1337;
        initial_registers.line.0 = 42;

        let opcode = LineInstruction::Copy;

        let expected_registers = initial_registers;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, true);
    }

    #[test]
    fn test_exec_advance_pc() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));
        let initial_registers = LineRow::new(&header);
        let opcode = LineInstruction::AdvancePc(42);

        let mut expected_registers = initial_registers;
        expected_registers.address += 42;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, false);
    }

    #[test]
    fn test_exec_advance_pc_overflow_32() {
        let mut header = make_test_header(EndianSlice::new(&[], LittleEndian));
        header.encoding.address_size = 4;
        let mut registers = LineRow::new(&header);
        registers.address = u32::MAX.into();
        let opcode = LineInstruction::AdvancePc(42);
        let mut program = IncompleteLineProgram { header };
        let result = registers.execute(opcode, &mut program);
        assert_eq!(result, Err(Error::AddressOverflow));
    }

    #[test]
    fn test_exec_advance_pc_overflow_64() {
        let mut header = make_test_header(EndianSlice::new(&[], LittleEndian));
        header.encoding.address_size = 8;
        let mut registers = LineRow::new(&header);
        registers.address = u64::MAX;
        let opcode = LineInstruction::AdvancePc(42);
        let mut program = IncompleteLineProgram { header };
        let result = registers.execute(opcode, &mut program);
        assert_eq!(result, Err(Error::AddressOverflow));
    }

    #[test]
    fn test_exec_advance_line() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));
        let initial_registers = LineRow::new(&header);
        let opcode = LineInstruction::AdvanceLine(42);

        let mut expected_registers = initial_registers;
        expected_registers.line.0 += 42;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, false);
    }

    #[test]
    fn test_exec_advance_line_overflow() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));
        let opcode = LineInstruction::AdvanceLine(42);

        let mut initial_registers = LineRow::new(&header);
        initial_registers.line.0 = u64::MAX;

        let mut expected_registers = initial_registers;
        expected_registers.line.0 = 41;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, false);
    }

    #[test]
    fn test_exec_set_file_in_bounds() {
        for file_idx in 1..3 {
            let header = make_test_header(EndianSlice::new(&[], LittleEndian));
            let initial_registers = LineRow::new(&header);
            let opcode = LineInstruction::SetFile(file_idx);

            let mut expected_registers = initial_registers;
            expected_registers.file = file_idx;

            assert_exec_opcode(header, initial_registers, opcode, expected_registers, false);
        }
    }

    #[test]
    fn test_exec_set_file_out_of_bounds() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));
        let initial_registers = LineRow::new(&header);
        let opcode = LineInstruction::SetFile(100);

        // The spec doesn't say anything about rejecting input programs
        // that set the file register out of bounds of the actual number
        // of files that have been defined. Instead, we cross our
        // fingers and hope that one gets defined before
        // `LineRow::file` gets called and handle the error at
        // that time if need be.
        let mut expected_registers = initial_registers;
        expected_registers.file = 100;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, false);
    }

    #[test]
    fn test_file_entry_file_index_out_of_bounds() {
        // These indices are 1-based, so 0 is invalid. 100 is way more than the
        // number of files defined in the header.
        let out_of_bounds_indices = [0, 100];

        for file_idx in &out_of_bounds_indices[..] {
            let header = make_test_header(EndianSlice::new(&[], LittleEndian));
            let mut row = LineRow::new(&header);

            row.file = *file_idx;

            assert_eq!(row.file(&header), None);
        }
    }

    #[test]
    fn test_file_entry_file_index_in_bounds() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));
        let mut row = LineRow::new(&header);

        row.file = 2;

        assert_eq!(row.file(&header), Some(&header.file_names()[1]));
    }

    #[test]
    fn test_exec_set_column() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));
        let initial_registers = LineRow::new(&header);
        let opcode = LineInstruction::SetColumn(42);

        let mut expected_registers = initial_registers;
        expected_registers.column = 42;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, false);
    }

    #[test]
    fn test_exec_negate_statement() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));
        let initial_registers = LineRow::new(&header);
        let opcode = LineInstruction::NegateStatement;

        let mut expected_registers = initial_registers;
        expected_registers.is_stmt = !initial_registers.is_stmt;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, false);
    }

    #[test]
    fn test_exec_set_basic_block() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));

        let mut initial_registers = LineRow::new(&header);
        initial_registers.basic_block = false;

        let opcode = LineInstruction::SetBasicBlock;

        let mut expected_registers = initial_registers;
        expected_registers.basic_block = true;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, false);
    }

    #[test]
    fn test_exec_const_add_pc() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));
        let initial_registers = LineRow::new(&header);
        let opcode = LineInstruction::ConstAddPc;

        let mut expected_registers = initial_registers;
        expected_registers.address += 20;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, false);
    }

    #[test]
    fn test_exec_const_add_pc_overflow() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));
        let mut registers = LineRow::new(&header);
        registers.address = u64::MAX;
        let opcode = LineInstruction::ConstAddPc;
        let mut program = IncompleteLineProgram { header };
        let result = registers.execute(opcode, &mut program);
        assert_eq!(result, Err(Error::AddressOverflow));
    }

    #[test]
    fn test_exec_fixed_add_pc() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));

        let mut initial_registers = LineRow::new(&header);
        initial_registers.op_index.0 = 1;

        let opcode = LineInstruction::FixedAddPc(10);

        let mut expected_registers = initial_registers;
        expected_registers.address += 10;
        expected_registers.op_index.0 = 0;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, false);
    }

    #[test]
    fn test_exec_fixed_add_pc_overflow() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));
        let mut registers = LineRow::new(&header);
        registers.address = u64::MAX;
        registers.op_index.0 = 1;
        let opcode = LineInstruction::FixedAddPc(10);
        let mut program = IncompleteLineProgram { header };
        let result = registers.execute(opcode, &mut program);
        assert_eq!(result, Err(Error::AddressOverflow));
    }

    #[test]
    fn test_exec_set_prologue_end() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));

        let mut initial_registers = LineRow::new(&header);
        initial_registers.prologue_end = false;

        let opcode = LineInstruction::SetPrologueEnd;

        let mut expected_registers = initial_registers;
        expected_registers.prologue_end = true;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, false);
    }

    #[test]
    fn test_exec_set_isa() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));
        let initial_registers = LineRow::new(&header);
        let opcode = LineInstruction::SetIsa(1993);

        let mut expected_registers = initial_registers;
        expected_registers.isa = 1993;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, false);
    }

    #[test]
    fn test_exec_unknown_standard_0() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));
        let initial_registers = LineRow::new(&header);
        let opcode = LineInstruction::UnknownStandard0(constants::DwLns(111));
        let expected_registers = initial_registers;
        assert_exec_opcode(header, initial_registers, opcode, expected_registers, false);
    }

    #[test]
    fn test_exec_unknown_standard_1() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));
        let initial_registers = LineRow::new(&header);
        let opcode = LineInstruction::UnknownStandard1(constants::DwLns(111), 2);
        let expected_registers = initial_registers;
        assert_exec_opcode(header, initial_registers, opcode, expected_registers, false);
    }

    #[test]
    fn test_exec_unknown_standard_n() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));
        let initial_registers = LineRow::new(&header);
        let opcode = LineInstruction::UnknownStandardN(
            constants::DwLns(111),
            EndianSlice::new(&[2, 2, 2], LittleEndian),
        );
        let expected_registers = initial_registers;
        assert_exec_opcode(header, initial_registers, opcode, expected_registers, false);
    }

    #[test]
    fn test_exec_end_sequence() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));
        let initial_registers = LineRow::new(&header);
        let opcode = LineInstruction::EndSequence;

        let mut expected_registers = initial_registers;
        expected_registers.end_sequence = true;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, true);
    }

    #[test]
    fn test_exec_set_address() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));
        let initial_registers = LineRow::new(&header);
        let opcode = LineInstruction::SetAddress(3030);

        let mut expected_registers = initial_registers;
        expected_registers.address = 3030;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, false);
    }

    #[test]
    fn test_exec_set_address_tombstone() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));
        let initial_registers = LineRow::new(&header);
        let opcode = LineInstruction::SetAddress(!0);

        let mut expected_registers = initial_registers;
        expected_registers.tombstone = true;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, false);
    }

    #[test]
    fn test_exec_set_address_backwards() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));
        let mut initial_registers = LineRow::new(&header);
        initial_registers.address = 1;
        let opcode = LineInstruction::SetAddress(0);

        let mut expected_registers = initial_registers;
        expected_registers.tombstone = true;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, false);
    }

    #[test]
    fn test_exec_define_file() {
        let mut program = make_test_program(EndianSlice::new(&[], LittleEndian));
        let mut row = LineRow::new(program.header());

        let file = FileEntry {
            path_name: AttributeValue::String(EndianSlice::new(b"test.cpp", LittleEndian)),
            directory_index: 0,
            timestamp: 0,
            size: 0,
            md5: [0; 16],
            source: None,
        };

        let opcode = LineInstruction::DefineFile(file);
        let is_new_row = row.execute(opcode, &mut program).unwrap();

        assert!(!is_new_row);
        assert_eq!(Some(&file), program.header().file_names.last());
    }

    #[test]
    fn test_exec_set_discriminator() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));
        let initial_registers = LineRow::new(&header);
        let opcode = LineInstruction::SetDiscriminator(9);

        let mut expected_registers = initial_registers;
        expected_registers.discriminator = 9;

        assert_exec_opcode(header, initial_registers, opcode, expected_registers, false);
    }

    #[test]
    fn test_exec_unknown_extended() {
        let header = make_test_header(EndianSlice::new(&[], LittleEndian));
        let initial_registers = LineRow::new(&header);
        let opcode = LineInstruction::UnknownExtended(
            constants::DwLne(74),
            EndianSlice::new(&[], LittleEndian),
        );
        let expected_registers = initial_registers;
        assert_exec_opcode(header, initial_registers, opcode, expected_registers, false);
    }

    /// Ensure that `LineRows<R,P>` is covariant wrt R.
    /// This only needs to compile.
    #[allow(dead_code, unreachable_code, unused_variables)]
    #[allow(clippy::diverging_sub_expression)]
    fn test_line_rows_variance<'a, 'b>(_: &'a [u8], _: &'b [u8])
    where
        'a: 'b,
    {
        let a: &OneShotLineRows<EndianSlice<'a, LittleEndian>> = unimplemented!();
        let _: &OneShotLineRows<EndianSlice<'b, LittleEndian>> = a;
    }

    #[test]
    fn test_parse_debug_line_v5_ok() {
        let expected_lengths = &[1, 2];
        let expected_program = &[0, 1, 2, 3, 4];
        let expected_rest = &[5, 6, 7, 8, 9];
        let expected_include_directories = [
            AttributeValue::String(EndianSlice::new(b"dir1", LittleEndian)),
            AttributeValue::String(EndianSlice::new(b"dir2", LittleEndian)),
        ];
        let expected_file_names = [
            FileEntry {
                path_name: AttributeValue::String(EndianSlice::new(b"file1", LittleEndian)),
                directory_index: 0,
                timestamp: 0,
                size: 0,
                md5: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
                source: Some(AttributeValue::String(EndianSlice::new(
                    b"foobar",
                    LittleEndian,
                ))),
            },
            FileEntry {
                path_name: AttributeValue::String(EndianSlice::new(b"file2", LittleEndian)),
                directory_index: 1,
                timestamp: 0,
                size: 0,
                md5: [
                    11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
                ],
                source: Some(AttributeValue::String(EndianSlice::new(
                    b"quux",
                    LittleEndian,
                ))),
            },
        ];

        for format in [Format::Dwarf32, Format::Dwarf64] {
            let length = Label::new();
            let header_length = Label::new();
            let start = Label::new();
            let header_start = Label::new();
            let end = Label::new();
            let header_end = Label::new();
            let section = Section::with_endian(Endian::Little)
                .initial_length(format, &length, &start)
                .D16(5)
                // Address size.
                .D8(4)
                // Segment selector size.
                .D8(0)
                .word_label(format.word_size(), &header_length)
                .mark(&header_start)
                // Minimum instruction length.
                .D8(1)
                // Maximum operations per byte.
                .D8(1)
                // Default is_stmt.
                .D8(1)
                // Line base.
                .D8(0)
                // Line range.
                .D8(1)
                // Opcode base.
                .D8(expected_lengths.len() as u8 + 1)
                // Standard opcode lengths for opcodes 1 .. opcode base - 1.
                .append_bytes(expected_lengths)
                // Directory entry format count.
                .D8(1)
                .uleb(constants::DW_LNCT_path.0 as u64)
                .uleb(constants::DW_FORM_string.0 as u64)
                // Directory count.
                .D8(2)
                .append_bytes(b"dir1\0")
                .append_bytes(b"dir2\0")
                // File entry format count.
                .D8(4)
                .uleb(constants::DW_LNCT_path.0 as u64)
                .uleb(constants::DW_FORM_string.0 as u64)
                .uleb(constants::DW_LNCT_directory_index.0 as u64)
                .uleb(constants::DW_FORM_data1.0 as u64)
                .uleb(constants::DW_LNCT_MD5.0 as u64)
                .uleb(constants::DW_FORM_data16.0 as u64)
                .uleb(constants::DW_LNCT_LLVM_source.0 as u64)
                .uleb(constants::DW_FORM_string.0 as u64)
                // File count.
                .D8(2)
                .append_bytes(b"file1\0")
                .D8(0)
                .append_bytes(&expected_file_names[0].md5)
                .append_bytes(b"foobar\0")
                .append_bytes(b"file2\0")
                .D8(1)
                .append_bytes(&expected_file_names[1].md5)
                .append_bytes(b"quux\0")
                .mark(&header_end)
                // Dummy line program data.
                .append_bytes(expected_program)
                .mark(&end)
                // Dummy trailing data.
                .append_bytes(expected_rest);
            length.set_const((&end - &start) as u64);
            header_length.set_const((&header_end - &header_start) as u64);
            let section = section.get_contents().unwrap();

            let input = &mut EndianSlice::new(&section, LittleEndian);

            let header = LineProgramHeader::parse(input, DebugLineOffset(0), 0, None, None)
                .expect("should parse header ok");

            assert_eq!(header.raw_program_buf().slice(), expected_program);
            assert_eq!(input.slice(), expected_rest);

            assert_eq!(header.offset, DebugLineOffset(0));
            assert_eq!(header.version(), 5);
            assert_eq!(header.address_size(), 4);
            assert_eq!(header.minimum_instruction_length(), 1);
            assert_eq!(header.maximum_operations_per_instruction(), 1);
            assert!(header.default_is_stmt());
            assert_eq!(header.line_base(), 0);
            assert_eq!(header.line_range(), 1);
            assert_eq!(header.opcode_base(), expected_lengths.len() as u8 + 1);
            assert_eq!(header.standard_opcode_lengths().slice(), expected_lengths);
            assert_eq!(
                header.directory_entry_format(),
                &[FileEntryFormat {
                    content_type: constants::DW_LNCT_path,
                    form: constants::DW_FORM_string,
                }]
            );
            assert_eq!(header.include_directories(), expected_include_directories);
            assert_eq!(header.directory(0), Some(expected_include_directories[0]));
            assert_eq!(
                header.file_name_entry_format(),
                &[
                    FileEntryFormat {
                        content_type: constants::DW_LNCT_path,
                        form: constants::DW_FORM_string,
                    },
                    FileEntryFormat {
                        content_type: constants::DW_LNCT_directory_index,
                        form: constants::DW_FORM_data1,
                    },
                    FileEntryFormat {
                        content_type: constants::DW_LNCT_MD5,
                        form: constants::DW_FORM_data16,
                    },
                    FileEntryFormat {
                        content_type: constants::DW_LNCT_LLVM_source,
                        form: constants::DW_FORM_string,
                    }
                ]
            );
            assert_eq!(header.file_names(), expected_file_names);
            assert_eq!(header.file(0), Some(&expected_file_names[0]));
        }
    }

    #[test]
    fn test_sequences() {
        #[rustfmt::skip]
        let buf = [
            // 32-bit length
            94, 0x00, 0x00, 0x00,
            // Version.
            0x04, 0x00,
            // Header length = 40.
            0x28, 0x00, 0x00, 0x00,
            // Minimum instruction length.
            0x01,
            // Maximum operations per byte.
            0x01,
            // Default is_stmt.
            0x01,
            // Line base.
            0x00,
            // Line range.
            0x01,
            // Opcode base.
            0x03,
            // Standard opcode lengths for opcodes 1 .. opcode base - 1.
            0x01, 0x02,
            // Include directories = '/', 'i', 'n', 'c', '\0', '/', 'i', 'n', 'c', '2', '\0', '\0'
            0x2f, 0x69, 0x6e, 0x63, 0x00, 0x2f, 0x69, 0x6e, 0x63, 0x32, 0x00, 0x00,
            // File names
                // foo.rs
                0x66, 0x6f, 0x6f, 0x2e, 0x72, 0x73, 0x00,
                0x00,
                0x00,
                0x00,
                // bar.h
                0x62, 0x61, 0x72, 0x2e, 0x68, 0x00,
                0x01,
                0x00,
                0x00,
            // End file names.
            0x00,

            0, 5, constants::DW_LNE_set_address.0, 1, 0, 0, 0,
            constants::DW_LNS_copy.0,
            constants::DW_LNS_advance_pc.0, 1,
            constants::DW_LNS_copy.0,
            constants::DW_LNS_advance_pc.0, 2,
            0, 1, constants::DW_LNE_end_sequence.0,

            // Tombstone
            0, 5, constants::DW_LNE_set_address.0, 0xff, 0xff, 0xff, 0xff,
            constants::DW_LNS_copy.0,
            constants::DW_LNS_advance_pc.0, 1,
            constants::DW_LNS_copy.0,
            constants::DW_LNS_advance_pc.0, 2,
            0, 1, constants::DW_LNE_end_sequence.0,

            0, 5, constants::DW_LNE_set_address.0, 11, 0, 0, 0,
            constants::DW_LNS_copy.0,
            constants::DW_LNS_advance_pc.0, 1,
            constants::DW_LNS_copy.0,
            constants::DW_LNS_advance_pc.0, 2,
            0, 1, constants::DW_LNE_end_sequence.0,
        ];
        assert_eq!(buf[0] as usize, buf.len() - 4);

        let rest = &mut EndianSlice::new(&buf, LittleEndian);

        let header = LineProgramHeader::parse(rest, DebugLineOffset(0), 4, None, None)
            .expect("should parse header ok");
        let program = IncompleteLineProgram { header };

        let sequences = program.sequences().unwrap().1;
        assert_eq!(sequences.len(), 2);
        assert_eq!(sequences[0].start, 1);
        assert_eq!(sequences[0].end, 4);
        assert_eq!(sequences[1].start, 11);
        assert_eq!(sequences[1].end, 14);
    }
}
