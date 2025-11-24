use alloc::fmt;
use core::slice;

use crate::endian::LittleEndian as LE;
use crate::pe;
use crate::read::{
    ReadRef, Relocation, RelocationEncoding, RelocationFlags, RelocationKind, RelocationTarget,
    SymbolIndex,
};

use super::{CoffFile, CoffHeader};

/// An iterator for the relocations in a [`CoffBigSection`](super::CoffBigSection).
pub type CoffBigRelocationIterator<'data, 'file, R = &'data [u8]> =
    CoffRelocationIterator<'data, 'file, R, pe::AnonObjectHeaderBigobj>;

/// An iterator for the relocations in a [`CoffSection`](super::CoffSection).
pub struct CoffRelocationIterator<
    'data,
    'file,
    R: ReadRef<'data> = &'data [u8],
    Coff: CoffHeader = pe::ImageFileHeader,
> {
    pub(super) file: &'file CoffFile<'data, R, Coff>,
    pub(super) iter: slice::Iter<'data, pe::ImageRelocation>,
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> Iterator
    for CoffRelocationIterator<'data, 'file, R, Coff>
{
    type Item = (u64, Relocation);

    fn next(&mut self) -> Option<Self::Item> {
        use RelocationEncoding as E;
        use RelocationKind as K;

        self.iter.next().map(|relocation| {
            let typ = relocation.typ.get(LE);
            let flags = RelocationFlags::Coff { typ };
            let g = E::Generic;
            let unknown = (K::Unknown, E::Generic, 0, 0);
            let (kind, encoding, size, addend) = match self.file.header.machine() {
                pe::IMAGE_FILE_MACHINE_ARMNT => match typ {
                    pe::IMAGE_REL_ARM_ADDR32 => (K::Absolute, g, 32, 0),
                    pe::IMAGE_REL_ARM_ADDR32NB => (K::ImageOffset, g, 32, 0),
                    pe::IMAGE_REL_ARM_REL32 => (K::Relative, g, 32, -4),
                    pe::IMAGE_REL_ARM_SECTION => (K::SectionIndex, g, 16, 0),
                    pe::IMAGE_REL_ARM_SECREL => (K::SectionOffset, g, 32, 0),
                    _ => unknown,
                },
                pe::IMAGE_FILE_MACHINE_ARM64 | pe::IMAGE_FILE_MACHINE_ARM64EC => match typ {
                    pe::IMAGE_REL_ARM64_ADDR32 => (K::Absolute, g, 32, 0),
                    pe::IMAGE_REL_ARM64_ADDR32NB => (K::ImageOffset, g, 32, 0),
                    pe::IMAGE_REL_ARM64_SECREL => (K::SectionOffset, g, 32, 0),
                    pe::IMAGE_REL_ARM64_SECTION => (K::SectionIndex, g, 16, 0),
                    pe::IMAGE_REL_ARM64_ADDR64 => (K::Absolute, g, 64, 0),
                    pe::IMAGE_REL_ARM64_REL32 => (K::Relative, g, 32, -4),
                    pe::IMAGE_REL_ARM64_BRANCH26 => (K::Relative, E::AArch64Call, 26, 0),
                    _ => unknown,
                },
                pe::IMAGE_FILE_MACHINE_I386 => match typ {
                    pe::IMAGE_REL_I386_DIR16 => (K::Absolute, g, 16, 0),
                    pe::IMAGE_REL_I386_REL16 => (K::Relative, g, 16, 0),
                    pe::IMAGE_REL_I386_DIR32 => (K::Absolute, g, 32, 0),
                    pe::IMAGE_REL_I386_DIR32NB => (K::ImageOffset, g, 32, 0),
                    pe::IMAGE_REL_I386_SECTION => (K::SectionIndex, g, 16, 0),
                    pe::IMAGE_REL_I386_SECREL => (K::SectionOffset, g, 32, 0),
                    pe::IMAGE_REL_I386_SECREL7 => (K::SectionOffset, g, 7, 0),
                    pe::IMAGE_REL_I386_REL32 => (K::Relative, g, 32, -4),
                    _ => unknown,
                },
                pe::IMAGE_FILE_MACHINE_AMD64 => match typ {
                    pe::IMAGE_REL_AMD64_ADDR64 => (K::Absolute, g, 64, 0),
                    pe::IMAGE_REL_AMD64_ADDR32 => (K::Absolute, g, 32, 0),
                    pe::IMAGE_REL_AMD64_ADDR32NB => (K::ImageOffset, g, 32, 0),
                    pe::IMAGE_REL_AMD64_REL32 => (K::Relative, g, 32, -4),
                    pe::IMAGE_REL_AMD64_REL32_1 => (K::Relative, g, 32, -5),
                    pe::IMAGE_REL_AMD64_REL32_2 => (K::Relative, g, 32, -6),
                    pe::IMAGE_REL_AMD64_REL32_3 => (K::Relative, g, 32, -7),
                    pe::IMAGE_REL_AMD64_REL32_4 => (K::Relative, g, 32, -8),
                    pe::IMAGE_REL_AMD64_REL32_5 => (K::Relative, g, 32, -9),
                    pe::IMAGE_REL_AMD64_SECTION => (K::SectionIndex, g, 16, 0),
                    pe::IMAGE_REL_AMD64_SECREL => (K::SectionOffset, g, 32, 0),
                    pe::IMAGE_REL_AMD64_SECREL7 => (K::SectionOffset, g, 7, 0),
                    _ => unknown,
                },
                _ => unknown,
            };
            let target = RelocationTarget::Symbol(relocation.symbol());
            (
                u64::from(relocation.virtual_address.get(LE)),
                Relocation {
                    kind,
                    encoding,
                    size,
                    target,
                    addend,
                    implicit_addend: true,
                    flags,
                },
            )
        })
    }
}

impl<'data, 'file, R: ReadRef<'data>, Coff: CoffHeader> fmt::Debug
    for CoffRelocationIterator<'data, 'file, R, Coff>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoffRelocationIterator").finish()
    }
}

impl pe::ImageRelocation {
    /// Get the index of the symbol referenced by this relocation.
    pub fn symbol(&self) -> SymbolIndex {
        SymbolIndex(self.symbol_table_index.get(LE) as usize)
    }
}
