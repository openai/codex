use alloc::vec::Vec;
use indexmap::IndexSet;
use std::ops::{Deref, DerefMut};

use crate::common::{DebugFrameOffset, EhFrameOffset, Encoding, Format, Register, SectionId};
use crate::constants;
use crate::write::{Address, BaseId, Error, Expression, Result, Section, Writer};

define_section!(
    DebugFrame,
    DebugFrameOffset,
    "A writable `.debug_frame` section."
);

define_section!(EhFrame, EhFrameOffset, "A writable `.eh_frame` section.");

define_id!(CieId, "An identifier for a CIE in a `FrameTable`.");

/// A table of frame description entries.
#[derive(Debug, Default)]
pub struct FrameTable {
    /// Base id for CIEs.
    base_id: BaseId,
    /// The common information entries.
    cies: IndexSet<CommonInformationEntry>,
    /// The frame description entries.
    fdes: Vec<(CieId, FrameDescriptionEntry)>,
}

impl FrameTable {
    /// Add a CIE and return its id.
    ///
    /// If the CIE already exists, then return the id of the existing CIE.
    pub fn add_cie(&mut self, cie: CommonInformationEntry) -> CieId {
        let (index, _) = self.cies.insert_full(cie);
        CieId::new(self.base_id, index)
    }

    /// The number of CIEs.
    pub fn cie_count(&self) -> usize {
        self.cies.len()
    }

    /// Add a FDE.
    ///
    /// Does not check for duplicates.
    ///
    /// # Panics
    ///
    /// Panics if the CIE id is invalid.
    pub fn add_fde(&mut self, cie: CieId, fde: FrameDescriptionEntry) {
        debug_assert_eq!(self.base_id, cie.base_id);
        self.fdes.push((cie, fde));
    }

    /// The number of FDEs.
    pub fn fde_count(&self) -> usize {
        self.fdes.len()
    }

    /// Write the frame table entries to the given `.debug_frame` section.
    pub fn write_debug_frame<W: Writer>(&self, w: &mut DebugFrame<W>) -> Result<()> {
        self.write(&mut w.0, false)
    }

    /// Write the frame table entries to the given `.eh_frame` section.
    pub fn write_eh_frame<W: Writer>(&self, w: &mut EhFrame<W>) -> Result<()> {
        self.write(&mut w.0, true)
    }

    fn write<W: Writer>(&self, w: &mut W, eh_frame: bool) -> Result<()> {
        let mut cie_offsets = vec![None; self.cies.len()];
        for (cie_id, fde) in &self.fdes {
            let cie_index = cie_id.index;
            let cie = self.cies.get_index(cie_index).unwrap();
            let cie_offset = match cie_offsets[cie_index] {
                Some(offset) => offset,
                None => {
                    // Only write CIEs as they are referenced.
                    let offset = cie.write(w, eh_frame)?;
                    cie_offsets[cie_index] = Some(offset);
                    offset
                }
            };

            fde.write(w, eh_frame, cie_offset, cie)?;
        }
        // TODO: write length 0 terminator for eh_frame?
        Ok(())
    }
}

/// A common information entry. This contains information that is shared between FDEs.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommonInformationEntry {
    encoding: Encoding,

    /// A constant that is factored out of code offsets.
    ///
    /// This should be set to the minimum instruction length.
    /// Writing a code offset that is not a multiple of this factor will generate an error.
    code_alignment_factor: u8,

    /// A constant that is factored out of data offsets.
    ///
    /// This should be set to the minimum data alignment for the frame.
    /// Writing a data offset that is not a multiple of this factor will generate an error.
    data_alignment_factor: i8,

    /// The return address register. This might not correspond to an actual machine register.
    return_address_register: Register,

    /// The address of the personality function and its encoding.
    pub personality: Option<(constants::DwEhPe, Address)>,

    /// The encoding to use for the LSDA address in FDEs.
    ///
    /// If set then all FDEs which use this CIE must have a LSDA address.
    pub lsda_encoding: Option<constants::DwEhPe>,

    /// The encoding to use for addresses in FDEs.
    pub fde_address_encoding: constants::DwEhPe,

    /// True for signal trampolines.
    pub signal_trampoline: bool,

    /// The initial instructions upon entry to this function.
    instructions: Vec<CallFrameInstruction>,
}

impl CommonInformationEntry {
    /// Create a new common information entry.
    ///
    /// The encoding version must be a CFI version, not a DWARF version.
    pub fn new(
        encoding: Encoding,
        code_alignment_factor: u8,
        data_alignment_factor: i8,
        return_address_register: Register,
    ) -> Self {
        CommonInformationEntry {
            encoding,
            code_alignment_factor,
            data_alignment_factor,
            return_address_register,
            personality: None,
            lsda_encoding: None,
            fde_address_encoding: constants::DW_EH_PE_absptr,
            signal_trampoline: false,
            instructions: Vec::new(),
        }
    }

    /// Add an initial instruction.
    pub fn add_instruction(&mut self, instruction: CallFrameInstruction) {
        self.instructions.push(instruction);
    }

    fn has_augmentation(&self) -> bool {
        self.personality.is_some()
            || self.lsda_encoding.is_some()
            || self.signal_trampoline
            || self.fde_address_encoding != constants::DW_EH_PE_absptr
    }

    /// Returns the section offset of the CIE.
    fn write<W: Writer>(&self, w: &mut W, eh_frame: bool) -> Result<usize> {
        let encoding = self.encoding;
        let offset = w.len();

        let length_offset = w.write_initial_length(encoding.format)?;
        let length_base = w.len();

        if eh_frame {
            w.write_u32(0)?;
        } else {
            match encoding.format {
                Format::Dwarf32 => w.write_u32(0xffff_ffff)?,
                Format::Dwarf64 => w.write_u64(0xffff_ffff_ffff_ffff)?,
            }
        }

        if eh_frame {
            if encoding.version != 1 {
                return Err(Error::UnsupportedVersion(encoding.version));
            };
        } else {
            match encoding.version {
                1 | 3 | 4 => {}
                _ => return Err(Error::UnsupportedVersion(encoding.version)),
            };
        }
        w.write_u8(encoding.version as u8)?;

        let augmentation = self.has_augmentation();
        if augmentation {
            w.write_u8(b'z')?;
            if self.lsda_encoding.is_some() {
                w.write_u8(b'L')?;
            }
            if self.personality.is_some() {
                w.write_u8(b'P')?;
            }
            if self.fde_address_encoding != constants::DW_EH_PE_absptr {
                w.write_u8(b'R')?;
            }
            if self.signal_trampoline {
                w.write_u8(b'S')?;
            }
        }
        w.write_u8(0)?;

        if encoding.version >= 4 {
            w.write_u8(encoding.address_size)?;
            w.write_u8(0)?; // segment_selector_size
        }

        w.write_uleb128(self.code_alignment_factor.into())?;
        w.write_sleb128(self.data_alignment_factor.into())?;

        if !eh_frame && encoding.version == 1 {
            let register = self.return_address_register.0 as u8;
            if u16::from(register) != self.return_address_register.0 {
                return Err(Error::ValueTooLarge);
            }
            w.write_u8(register)?;
        } else {
            w.write_uleb128(self.return_address_register.0.into())?;
        }

        if augmentation {
            let augmentation_length_offset = w.len();
            w.write_u8(0)?;
            let augmentation_length_base = w.len();

            if let Some(eh_pe) = self.lsda_encoding {
                w.write_u8(eh_pe.0)?;
            }
            if let Some((eh_pe, address)) = self.personality {
                w.write_u8(eh_pe.0)?;
                w.write_eh_pointer(address, eh_pe, encoding.address_size)?;
            }
            if self.fde_address_encoding != constants::DW_EH_PE_absptr {
                w.write_u8(self.fde_address_encoding.0)?;
            }

            let augmentation_length = (w.len() - augmentation_length_base) as u64;
            debug_assert!(augmentation_length < 0x80);
            w.write_udata_at(augmentation_length_offset, augmentation_length, 1)?;
        }

        for instruction in &self.instructions {
            instruction.write(w, encoding, self)?;
        }

        write_nop(
            w,
            encoding.format.word_size() as usize + w.len() - length_base,
            encoding.address_size,
        )?;

        let length = (w.len() - length_base) as u64;
        w.write_initial_length_at(length_offset, length, encoding.format)?;

        Ok(offset)
    }
}

/// A frame description entry. There should be one FDE per function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameDescriptionEntry {
    /// The initial address of the function.
    address: Address,

    /// The length in bytes of the function.
    length: u32,

    /// The address of the LSDA.
    pub lsda: Option<Address>,

    /// The instructions for this function, ordered by offset.
    instructions: Vec<(u32, CallFrameInstruction)>,
}

impl FrameDescriptionEntry {
    /// Create a new frame description entry for a function.
    pub fn new(address: Address, length: u32) -> Self {
        FrameDescriptionEntry {
            address,
            length,
            lsda: None,
            instructions: Vec::new(),
        }
    }

    /// Add an instruction.
    ///
    /// Instructions must be added in increasing order of offset, or writing will fail.
    pub fn add_instruction(&mut self, offset: u32, instruction: CallFrameInstruction) {
        debug_assert!(self.instructions.last().map(|x| x.0).unwrap_or(0) <= offset);
        self.instructions.push((offset, instruction));
    }

    fn write<W: Writer>(
        &self,
        w: &mut W,
        eh_frame: bool,
        cie_offset: usize,
        cie: &CommonInformationEntry,
    ) -> Result<()> {
        let encoding = cie.encoding;
        let length_offset = w.write_initial_length(encoding.format)?;
        let length_base = w.len();

        if eh_frame {
            // .eh_frame uses a relative offset which doesn't need relocation.
            w.write_udata((w.len() - cie_offset) as u64, 4)?;
        } else {
            w.write_offset(
                cie_offset,
                SectionId::DebugFrame,
                encoding.format.word_size(),
            )?;
        }

        if cie.fde_address_encoding != constants::DW_EH_PE_absptr {
            w.write_eh_pointer(
                self.address,
                cie.fde_address_encoding,
                encoding.address_size,
            )?;
            w.write_eh_pointer_data(
                self.length.into(),
                cie.fde_address_encoding.format(),
                encoding.address_size,
            )?;
        } else {
            w.write_address(self.address, encoding.address_size)?;
            w.write_udata(self.length.into(), encoding.address_size)?;
        }

        if cie.has_augmentation() {
            let augmentation_length_offset = w.len();
            w.write_u8(0)?;
            let augmentation_length_base = w.len();

            debug_assert_eq!(self.lsda.is_some(), cie.lsda_encoding.is_some());
            if let (Some(lsda), Some(lsda_encoding)) = (self.lsda, cie.lsda_encoding) {
                w.write_eh_pointer(lsda, lsda_encoding, encoding.address_size)?;
            }

            let augmentation_length = (w.len() - augmentation_length_base) as u64;
            debug_assert!(augmentation_length < 0x80);
            w.write_udata_at(augmentation_length_offset, augmentation_length, 1)?;
        }

        let mut prev_offset = 0;
        for (offset, instruction) in &self.instructions {
            write_advance_loc(w, cie.code_alignment_factor, prev_offset, *offset)?;
            prev_offset = *offset;
            instruction.write(w, encoding, cie)?;
        }

        write_nop(
            w,
            encoding.format.word_size() as usize + w.len() - length_base,
            encoding.address_size,
        )?;

        let length = (w.len() - length_base) as u64;
        w.write_initial_length_at(length_offset, length, encoding.format)?;

        Ok(())
    }
}

/// An instruction in a frame description entry.
///
/// This may be a CFA definition, a register rule, or some other directive.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CallFrameInstruction {
    /// Define the CFA rule to use the provided register and offset.
    Cfa(Register, i32),
    /// Update the CFA rule to use the provided register. The offset is unchanged.
    CfaRegister(Register),
    /// Update the CFA rule to use the provided offset. The register is unchanged.
    CfaOffset(i32),
    /// Define the CFA rule to use the provided expression.
    CfaExpression(Expression),

    /// Restore the initial rule for the register.
    Restore(Register),
    /// The previous value of the register is not recoverable.
    Undefined(Register),
    /// The register has not been modified.
    SameValue(Register),
    /// The previous value of the register is saved at address CFA + offset.
    Offset(Register, i32),
    /// The previous value of the register is CFA + offset.
    ValOffset(Register, i32),
    /// The previous value of the register is stored in another register.
    Register(Register, Register),
    /// The previous value of the register is saved at address given by the expression.
    Expression(Register, Expression),
    /// The previous value of the register is given by the expression.
    ValExpression(Register, Expression),

    /// Push all register rules onto a stack.
    RememberState,
    /// Pop all register rules off the stack.
    RestoreState,
    /// The size of the arguments that have been pushed onto the stack.
    ArgsSize(u32),

    /// AAarch64 extension: negate the `RA_SIGN_STATE` pseudo-register.
    NegateRaState,
}

impl CallFrameInstruction {
    fn write<W: Writer>(
        &self,
        w: &mut W,
        encoding: Encoding,
        cie: &CommonInformationEntry,
    ) -> Result<()> {
        match *self {
            CallFrameInstruction::Cfa(register, offset) => {
                if offset < 0 {
                    let offset = factored_data_offset(offset, cie.data_alignment_factor)?;
                    w.write_u8(constants::DW_CFA_def_cfa_sf.0)?;
                    w.write_uleb128(register.0.into())?;
                    w.write_sleb128(offset.into())?;
                } else {
                    // Unfactored offset.
                    w.write_u8(constants::DW_CFA_def_cfa.0)?;
                    w.write_uleb128(register.0.into())?;
                    w.write_uleb128(offset as u64)?;
                }
            }
            CallFrameInstruction::CfaRegister(register) => {
                w.write_u8(constants::DW_CFA_def_cfa_register.0)?;
                w.write_uleb128(register.0.into())?;
            }
            CallFrameInstruction::CfaOffset(offset) => {
                if offset < 0 {
                    let offset = factored_data_offset(offset, cie.data_alignment_factor)?;
                    w.write_u8(constants::DW_CFA_def_cfa_offset_sf.0)?;
                    w.write_sleb128(offset.into())?;
                } else {
                    // Unfactored offset.
                    w.write_u8(constants::DW_CFA_def_cfa_offset.0)?;
                    w.write_uleb128(offset as u64)?;
                }
            }
            CallFrameInstruction::CfaExpression(ref expression) => {
                w.write_u8(constants::DW_CFA_def_cfa_expression.0)?;
                w.write_uleb128(expression.size(encoding, None) as u64)?;
                expression.write(w, None, encoding, None)?;
            }
            CallFrameInstruction::Restore(register) => {
                if register.0 < 0x40 {
                    w.write_u8(constants::DW_CFA_restore.0 | register.0 as u8)?;
                } else {
                    w.write_u8(constants::DW_CFA_restore_extended.0)?;
                    w.write_uleb128(register.0.into())?;
                }
            }
            CallFrameInstruction::Undefined(register) => {
                w.write_u8(constants::DW_CFA_undefined.0)?;
                w.write_uleb128(register.0.into())?;
            }
            CallFrameInstruction::SameValue(register) => {
                w.write_u8(constants::DW_CFA_same_value.0)?;
                w.write_uleb128(register.0.into())?;
            }
            CallFrameInstruction::Offset(register, offset) => {
                let offset = factored_data_offset(offset, cie.data_alignment_factor)?;
                if offset < 0 {
                    w.write_u8(constants::DW_CFA_offset_extended_sf.0)?;
                    w.write_uleb128(register.0.into())?;
                    w.write_sleb128(offset.into())?;
                } else if register.0 < 0x40 {
                    w.write_u8(constants::DW_CFA_offset.0 | register.0 as u8)?;
                    w.write_uleb128(offset as u64)?;
                } else {
                    w.write_u8(constants::DW_CFA_offset_extended.0)?;
                    w.write_uleb128(register.0.into())?;
                    w.write_uleb128(offset as u64)?;
                }
            }
            CallFrameInstruction::ValOffset(register, offset) => {
                let offset = factored_data_offset(offset, cie.data_alignment_factor)?;
                if offset < 0 {
                    w.write_u8(constants::DW_CFA_val_offset_sf.0)?;
                    w.write_uleb128(register.0.into())?;
                    w.write_sleb128(offset.into())?;
                } else {
                    w.write_u8(constants::DW_CFA_val_offset.0)?;
                    w.write_uleb128(register.0.into())?;
                    w.write_uleb128(offset as u64)?;
                }
            }
            CallFrameInstruction::Register(register1, register2) => {
                w.write_u8(constants::DW_CFA_register.0)?;
                w.write_uleb128(register1.0.into())?;
                w.write_uleb128(register2.0.into())?;
            }
            CallFrameInstruction::Expression(register, ref expression) => {
                w.write_u8(constants::DW_CFA_expression.0)?;
                w.write_uleb128(register.0.into())?;
                w.write_uleb128(expression.size(encoding, None) as u64)?;
                expression.write(w, None, encoding, None)?;
            }
            CallFrameInstruction::ValExpression(register, ref expression) => {
                w.write_u8(constants::DW_CFA_val_expression.0)?;
                w.write_uleb128(register.0.into())?;
                w.write_uleb128(expression.size(encoding, None) as u64)?;
                expression.write(w, None, encoding, None)?;
            }
            CallFrameInstruction::RememberState => {
                w.write_u8(constants::DW_CFA_remember_state.0)?;
            }
            CallFrameInstruction::RestoreState => {
                w.write_u8(constants::DW_CFA_restore_state.0)?;
            }
            CallFrameInstruction::ArgsSize(size) => {
                w.write_u8(constants::DW_CFA_GNU_args_size.0)?;
                w.write_uleb128(size.into())?;
            }
            CallFrameInstruction::NegateRaState => {
                w.write_u8(constants::DW_CFA_AARCH64_negate_ra_state.0)?;
            }
        }
        Ok(())
    }
}

fn write_advance_loc<W: Writer>(
    w: &mut W,
    code_alignment_factor: u8,
    prev_offset: u32,
    offset: u32,
) -> Result<()> {
    if offset == prev_offset {
        return Ok(());
    }
    let delta = factored_code_delta(prev_offset, offset, code_alignment_factor)?;
    if delta < 0x40 {
        w.write_u8(constants::DW_CFA_advance_loc.0 | delta as u8)?;
    } else if delta < 0x100 {
        w.write_u8(constants::DW_CFA_advance_loc1.0)?;
        w.write_u8(delta as u8)?;
    } else if delta < 0x10000 {
        w.write_u8(constants::DW_CFA_advance_loc2.0)?;
        w.write_u16(delta as u16)?;
    } else {
        w.write_u8(constants::DW_CFA_advance_loc4.0)?;
        w.write_u32(delta)?;
    }
    Ok(())
}

fn write_nop<W: Writer>(w: &mut W, len: usize, align: u8) -> Result<()> {
    debug_assert_eq!(align & (align - 1), 0);
    let tail_len = (!len + 1) & (align as usize - 1);
    for _ in 0..tail_len {
        w.write_u8(constants::DW_CFA_nop.0)?;
    }
    Ok(())
}

fn factored_code_delta(prev_offset: u32, offset: u32, factor: u8) -> Result<u32> {
    if offset < prev_offset {
        return Err(Error::InvalidFrameCodeOffset(offset));
    }
    let delta = offset - prev_offset;
    let factor = u32::from(factor);
    let factored_delta = delta / factor;
    if delta != factored_delta * factor {
        return Err(Error::InvalidFrameCodeOffset(offset));
    }
    Ok(factored_delta)
}

fn factored_data_offset(offset: i32, factor: i8) -> Result<i32> {
    let factor = i32::from(factor);
    let factored_offset = offset / factor;
    if offset != factored_offset * factor {
        return Err(Error::InvalidFrameDataOffset(offset));
    }
    Ok(factored_offset)
}

#[cfg(feature = "read")]
pub(crate) mod convert {
    use super::*;
    use crate::read::{self, Reader};
    use crate::write::{ConvertError, ConvertResult};
    use std::collections::{hash_map, HashMap};

    impl FrameTable {
        /// Create a frame table by reading the data in the given section.
        ///
        /// `convert_address` is a function to convert read addresses into the `Address`
        /// type. For non-relocatable addresses, this function may simply return
        /// `Address::Constant(address)`. For relocatable addresses, it is the caller's
        /// responsibility to determine the symbol and addend corresponding to the address
        /// and return `Address::Symbol { symbol, addend }`.
        pub fn from<R, Section>(
            frame: &Section,
            convert_address: &dyn Fn(u64) -> Option<Address>,
        ) -> ConvertResult<FrameTable>
        where
            R: Reader<Offset = usize>,
            Section: read::UnwindSection<R>,
            Section::Offset: read::UnwindOffset<usize>,
        {
            let bases = read::BaseAddresses::default().set_eh_frame(0);

            let mut frame_table = FrameTable::default();

            let mut cie_ids = HashMap::new();
            let mut entries = frame.entries(&bases);
            while let Some(entry) = entries.next()? {
                let partial = match entry {
                    read::CieOrFde::Cie(_) => continue,
                    read::CieOrFde::Fde(partial) => partial,
                };

                // TODO: is it worth caching the parsed CIEs? It would be better if FDEs only
                // stored a reference.
                let from_fde = partial.parse(Section::cie_from_offset)?;
                let from_cie = from_fde.cie();
                let cie_id = match cie_ids.entry(from_cie.offset()) {
                    hash_map::Entry::Occupied(o) => *o.get(),
                    hash_map::Entry::Vacant(e) => {
                        let cie =
                            CommonInformationEntry::from(from_cie, frame, &bases, convert_address)?;
                        let cie_id = frame_table.add_cie(cie);
                        e.insert(cie_id);
                        cie_id
                    }
                };
                let fde = FrameDescriptionEntry::from(&from_fde, frame, &bases, convert_address)?;
                frame_table.add_fde(cie_id, fde);
            }

            Ok(frame_table)
        }
    }

    impl CommonInformationEntry {
        fn from<R, Section>(
            from_cie: &read::CommonInformationEntry<R>,
            frame: &Section,
            bases: &read::BaseAddresses,
            convert_address: &dyn Fn(u64) -> Option<Address>,
        ) -> ConvertResult<CommonInformationEntry>
        where
            R: Reader<Offset = usize>,
            Section: read::UnwindSection<R>,
            Section::Offset: read::UnwindOffset<usize>,
        {
            let mut cie = CommonInformationEntry::new(
                from_cie.encoding(),
                from_cie.code_alignment_factor() as u8,
                from_cie.data_alignment_factor() as i8,
                from_cie.return_address_register(),
            );

            cie.personality = match from_cie.personality_with_encoding() {
                // We treat these the same because the encoding already determines
                // whether it is indirect.
                Some((eh_pe, read::Pointer::Direct(p)))
                | Some((eh_pe, read::Pointer::Indirect(p))) => {
                    let address = convert_address(p).ok_or(ConvertError::InvalidAddress)?;
                    Some((eh_pe, address))
                }
                _ => None,
            };
            cie.lsda_encoding = from_cie.lsda_encoding();
            cie.fde_address_encoding = from_cie
                .fde_address_encoding()
                .unwrap_or(constants::DW_EH_PE_absptr);
            cie.signal_trampoline = from_cie.is_signal_trampoline();

            let mut offset = 0;
            let mut from_instructions = from_cie.instructions(frame, bases);
            while let Some(from_instruction) = from_instructions.next()? {
                if let Some(instruction) = CallFrameInstruction::from(
                    from_instruction,
                    from_cie,
                    frame,
                    convert_address,
                    &mut offset,
                )? {
                    cie.instructions.push(instruction);
                }
            }
            Ok(cie)
        }
    }

    impl FrameDescriptionEntry {
        fn from<R, Section>(
            from_fde: &read::FrameDescriptionEntry<R>,
            frame: &Section,
            bases: &read::BaseAddresses,
            convert_address: &dyn Fn(u64) -> Option<Address>,
        ) -> ConvertResult<FrameDescriptionEntry>
        where
            R: Reader<Offset = usize>,
            Section: read::UnwindSection<R>,
            Section::Offset: read::UnwindOffset<usize>,
        {
            let address =
                convert_address(from_fde.initial_address()).ok_or(ConvertError::InvalidAddress)?;
            let length = from_fde.len() as u32;
            let mut fde = FrameDescriptionEntry::new(address, length);

            match from_fde.lsda() {
                // We treat these the same because the encoding already determines
                // whether it is indirect.
                Some(read::Pointer::Direct(p)) | Some(read::Pointer::Indirect(p)) => {
                    let address = convert_address(p).ok_or(ConvertError::InvalidAddress)?;
                    fde.lsda = Some(address);
                }
                None => {}
            }

            let from_cie = from_fde.cie();
            let mut offset = 0;
            let mut from_instructions = from_fde.instructions(frame, bases);
            while let Some(from_instruction) = from_instructions.next()? {
                if let Some(instruction) = CallFrameInstruction::from(
                    from_instruction,
                    from_cie,
                    frame,
                    convert_address,
                    &mut offset,
                )? {
                    fde.instructions.push((offset, instruction));
                }
            }

            Ok(fde)
        }
    }

    impl CallFrameInstruction {
        fn from<R, Section>(
            from_instruction: read::CallFrameInstruction<R::Offset>,
            from_cie: &read::CommonInformationEntry<R>,
            frame: &Section,
            convert_address: &dyn Fn(u64) -> Option<Address>,
            offset: &mut u32,
        ) -> ConvertResult<Option<CallFrameInstruction>>
        where
            R: Reader<Offset = usize>,
            Section: read::UnwindSection<R>,
        {
            let convert_expression =
                |x| Expression::from(x, from_cie.encoding(), None, None, None, convert_address);
            // TODO: validate integer type conversions
            Ok(Some(match from_instruction {
                read::CallFrameInstruction::SetLoc { .. } => {
                    return Err(ConvertError::UnsupportedCfiInstruction);
                }
                read::CallFrameInstruction::AdvanceLoc { delta } => {
                    *offset += delta * from_cie.code_alignment_factor() as u32;
                    return Ok(None);
                }
                read::CallFrameInstruction::DefCfa { register, offset } => {
                    CallFrameInstruction::Cfa(register, offset as i32)
                }
                read::CallFrameInstruction::DefCfaSf {
                    register,
                    factored_offset,
                } => {
                    let offset = factored_offset * from_cie.data_alignment_factor();
                    CallFrameInstruction::Cfa(register, offset as i32)
                }
                read::CallFrameInstruction::DefCfaRegister { register } => {
                    CallFrameInstruction::CfaRegister(register)
                }

                read::CallFrameInstruction::DefCfaOffset { offset } => {
                    CallFrameInstruction::CfaOffset(offset as i32)
                }
                read::CallFrameInstruction::DefCfaOffsetSf { factored_offset } => {
                    let offset = factored_offset * from_cie.data_alignment_factor();
                    CallFrameInstruction::CfaOffset(offset as i32)
                }
                read::CallFrameInstruction::DefCfaExpression { expression } => {
                    let expression = expression.get(frame)?;
                    CallFrameInstruction::CfaExpression(convert_expression(expression)?)
                }
                read::CallFrameInstruction::Undefined { register } => {
                    CallFrameInstruction::Undefined(register)
                }
                read::CallFrameInstruction::SameValue { register } => {
                    CallFrameInstruction::SameValue(register)
                }
                read::CallFrameInstruction::Offset {
                    register,
                    factored_offset,
                } => {
                    let offset = factored_offset as i64 * from_cie.data_alignment_factor();
                    CallFrameInstruction::Offset(register, offset as i32)
                }
                read::CallFrameInstruction::OffsetExtendedSf {
                    register,
                    factored_offset,
                } => {
                    let offset = factored_offset * from_cie.data_alignment_factor();
                    CallFrameInstruction::Offset(register, offset as i32)
                }
                read::CallFrameInstruction::ValOffset {
                    register,
                    factored_offset,
                } => {
                    let offset = factored_offset as i64 * from_cie.data_alignment_factor();
                    CallFrameInstruction::ValOffset(register, offset as i32)
                }
                read::CallFrameInstruction::ValOffsetSf {
                    register,
                    factored_offset,
                } => {
                    let offset = factored_offset * from_cie.data_alignment_factor();
                    CallFrameInstruction::ValOffset(register, offset as i32)
                }
                read::CallFrameInstruction::Register {
                    dest_register,
                    src_register,
                } => CallFrameInstruction::Register(dest_register, src_register),
                read::CallFrameInstruction::Expression {
                    register,
                    expression,
                } => {
                    let expression = expression.get(frame)?;
                    CallFrameInstruction::Expression(register, convert_expression(expression)?)
                }
                read::CallFrameInstruction::ValExpression {
                    register,
                    expression,
                } => {
                    let expression = expression.get(frame)?;
                    CallFrameInstruction::ValExpression(register, convert_expression(expression)?)
                }
                read::CallFrameInstruction::Restore { register } => {
                    CallFrameInstruction::Restore(register)
                }
                read::CallFrameInstruction::RememberState => CallFrameInstruction::RememberState,
                read::CallFrameInstruction::RestoreState => CallFrameInstruction::RestoreState,
                read::CallFrameInstruction::ArgsSize { size } => {
                    CallFrameInstruction::ArgsSize(size as u32)
                }
                read::CallFrameInstruction::NegateRaState => CallFrameInstruction::NegateRaState,
                read::CallFrameInstruction::Nop => return Ok(None),
            }))
        }
    }
}

#[cfg(test)]
#[cfg(feature = "read")]
mod tests {
    use super::*;
    use crate::arch::X86_64;
    use crate::read;
    use crate::write::EndianVec;
    use crate::{LittleEndian, Vendor};

    #[test]
    fn test_frame_table() {
        for &version in &[1, 3, 4] {
            for &address_size in &[4, 8] {
                for &format in &[Format::Dwarf32, Format::Dwarf64] {
                    let encoding = Encoding {
                        format,
                        version,
                        address_size,
                    };
                    let mut frames = FrameTable::default();

                    let cie1 = CommonInformationEntry::new(encoding, 1, 8, X86_64::RA);
                    let cie1_id = frames.add_cie(cie1.clone());
                    assert_eq!(cie1_id, frames.add_cie(cie1.clone()));

                    let mut cie2 = CommonInformationEntry::new(encoding, 1, 8, X86_64::RA);
                    cie2.lsda_encoding = Some(constants::DW_EH_PE_absptr);
                    cie2.personality =
                        Some((constants::DW_EH_PE_absptr, Address::Constant(0x1234)));
                    cie2.signal_trampoline = true;
                    let cie2_id = frames.add_cie(cie2.clone());
                    assert_ne!(cie1_id, cie2_id);
                    assert_eq!(cie2_id, frames.add_cie(cie2.clone()));

                    let fde1 = FrameDescriptionEntry::new(Address::Constant(0x1000), 0x10);
                    frames.add_fde(cie1_id, fde1.clone());

                    let fde2 = FrameDescriptionEntry::new(Address::Constant(0x2000), 0x20);
                    frames.add_fde(cie1_id, fde2.clone());

                    let mut fde3 = FrameDescriptionEntry::new(Address::Constant(0x3000), 0x30);
                    fde3.lsda = Some(Address::Constant(0x3300));
                    frames.add_fde(cie2_id, fde3.clone());

                    let mut fde4 = FrameDescriptionEntry::new(Address::Constant(0x4000), 0x40);
                    fde4.lsda = Some(Address::Constant(0x4400));
                    frames.add_fde(cie2_id, fde4.clone());

                    let mut cie3 = CommonInformationEntry::new(encoding, 1, 8, X86_64::RA);
                    cie3.fde_address_encoding =
                        constants::DW_EH_PE_pcrel | constants::DW_EH_PE_sdata4;
                    cie3.lsda_encoding =
                        Some(constants::DW_EH_PE_pcrel | constants::DW_EH_PE_sdata4);
                    cie3.personality = Some((
                        constants::DW_EH_PE_pcrel | constants::DW_EH_PE_sdata4,
                        Address::Constant(0x1235),
                    ));
                    cie3.signal_trampoline = true;
                    let cie3_id = frames.add_cie(cie3.clone());
                    assert_ne!(cie2_id, cie3_id);
                    assert_eq!(cie3_id, frames.add_cie(cie3.clone()));

                    let mut fde5 = FrameDescriptionEntry::new(Address::Constant(0x5000), 0x50);
                    fde5.lsda = Some(Address::Constant(0x5500));
                    frames.add_fde(cie3_id, fde5.clone());

                    // Test writing `.debug_frame`.
                    let mut debug_frame = DebugFrame::from(EndianVec::new(LittleEndian));
                    frames.write_debug_frame(&mut debug_frame).unwrap();

                    let mut read_debug_frame =
                        read::DebugFrame::new(debug_frame.slice(), LittleEndian);
                    read_debug_frame.set_address_size(address_size);
                    let convert_frames = FrameTable::from(&read_debug_frame, &|address| {
                        Some(Address::Constant(address))
                    })
                    .unwrap();
                    assert_eq!(frames.cies, convert_frames.cies);
                    assert_eq!(frames.fdes.len(), convert_frames.fdes.len());
                    for (a, b) in frames.fdes.iter().zip(convert_frames.fdes.iter()) {
                        assert_eq!(a.1, b.1);
                    }

                    if version == 1 {
                        // Test writing `.eh_frame`.
                        let mut eh_frame = EhFrame::from(EndianVec::new(LittleEndian));
                        frames.write_eh_frame(&mut eh_frame).unwrap();

                        let mut read_eh_frame = read::EhFrame::new(eh_frame.slice(), LittleEndian);
                        read_eh_frame.set_address_size(address_size);
                        let convert_frames = FrameTable::from(&read_eh_frame, &|address| {
                            Some(Address::Constant(address))
                        })
                        .unwrap();
                        assert_eq!(frames.cies, convert_frames.cies);
                        assert_eq!(frames.fdes.len(), convert_frames.fdes.len());
                        for (a, b) in frames.fdes.iter().zip(convert_frames.fdes.iter()) {
                            assert_eq!(a.1, b.1);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_frame_instruction() {
        let mut expression = Expression::new();
        expression.op_constu(0);

        let cie_instructions = [
            CallFrameInstruction::Cfa(X86_64::RSP, 8),
            CallFrameInstruction::Offset(X86_64::RA, -8),
        ];

        let fde_instructions = [
            (0, CallFrameInstruction::Cfa(X86_64::RSP, 0)),
            (0, CallFrameInstruction::Cfa(X86_64::RSP, -8)),
            (2, CallFrameInstruction::CfaRegister(X86_64::RBP)),
            (4, CallFrameInstruction::CfaOffset(8)),
            (4, CallFrameInstruction::CfaOffset(0)),
            (4, CallFrameInstruction::CfaOffset(-8)),
            (6, CallFrameInstruction::CfaExpression(expression.clone())),
            (8, CallFrameInstruction::Restore(Register(1))),
            (8, CallFrameInstruction::Restore(Register(101))),
            (10, CallFrameInstruction::Undefined(Register(2))),
            (12, CallFrameInstruction::SameValue(Register(3))),
            (14, CallFrameInstruction::Offset(Register(4), 16)),
            (14, CallFrameInstruction::Offset(Register(104), 16)),
            (16, CallFrameInstruction::ValOffset(Register(5), -24)),
            (16, CallFrameInstruction::ValOffset(Register(5), 24)),
            (18, CallFrameInstruction::Register(Register(6), Register(7))),
            (
                20,
                CallFrameInstruction::Expression(Register(8), expression.clone()),
            ),
            (
                22,
                CallFrameInstruction::ValExpression(Register(9), expression.clone()),
            ),
            (24 + 0x80, CallFrameInstruction::RememberState),
            (26 + 0x280, CallFrameInstruction::RestoreState),
            (28 + 0x20280, CallFrameInstruction::ArgsSize(23)),
        ];

        let fde_instructions_aarch64 = [(0, CallFrameInstruction::NegateRaState)];

        for &version in &[1, 3, 4] {
            for &address_size in &[4, 8] {
                for &vendor in &[Vendor::Default, Vendor::AArch64] {
                    for &format in &[Format::Dwarf32, Format::Dwarf64] {
                        let encoding = Encoding {
                            format,
                            version,
                            address_size,
                        };
                        let mut frames = FrameTable::default();

                        let mut cie = CommonInformationEntry::new(encoding, 2, 8, X86_64::RA);
                        for i in &cie_instructions {
                            cie.add_instruction(i.clone());
                        }
                        let cie_id = frames.add_cie(cie);

                        let mut fde = FrameDescriptionEntry::new(Address::Constant(0x1000), 0x10);
                        for (o, i) in &fde_instructions {
                            fde.add_instruction(*o, i.clone());
                        }
                        frames.add_fde(cie_id, fde);

                        if vendor == Vendor::AArch64 {
                            let mut fde =
                                FrameDescriptionEntry::new(Address::Constant(0x2000), 0x10);
                            for (o, i) in &fde_instructions_aarch64 {
                                fde.add_instruction(*o, i.clone());
                            }
                            frames.add_fde(cie_id, fde);
                        }

                        let mut debug_frame = DebugFrame::from(EndianVec::new(LittleEndian));
                        frames.write_debug_frame(&mut debug_frame).unwrap();

                        let mut read_debug_frame =
                            read::DebugFrame::new(debug_frame.slice(), LittleEndian);
                        read_debug_frame.set_address_size(address_size);
                        read_debug_frame.set_vendor(vendor);
                        let frames = FrameTable::from(&read_debug_frame, &|address| {
                            Some(Address::Constant(address))
                        })
                        .unwrap();

                        assert_eq!(
                            &frames.cies.get_index(0).unwrap().instructions,
                            &cie_instructions
                        );
                        assert_eq!(&frames.fdes[0].1.instructions, &fde_instructions);
                        if vendor == Vendor::AArch64 {
                            assert_eq!(&frames.fdes[1].1.instructions, &fde_instructions_aarch64);
                        }
                    }
                }
            }
        }
    }
}
