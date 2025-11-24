use core::{fmt, slice};

use crate::endian::Endianness;
use crate::macho;
use crate::read::{
    ReadRef, Relocation, RelocationEncoding, RelocationFlags, RelocationKind, RelocationTarget,
    SectionIndex, SymbolIndex,
};

use super::{MachHeader, MachOFile};

/// An iterator for the relocations in a [`MachOSection32`](super::MachOSection32).
pub type MachORelocationIterator32<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    MachORelocationIterator<'data, 'file, macho::MachHeader32<Endian>, R>;
/// An iterator for the relocations in a [`MachOSection64`](super::MachOSection64).
pub type MachORelocationIterator64<'data, 'file, Endian = Endianness, R = &'data [u8]> =
    MachORelocationIterator<'data, 'file, macho::MachHeader64<Endian>, R>;

/// An iterator for the relocations in a [`MachOSection`](super::MachOSection).
pub struct MachORelocationIterator<'data, 'file, Mach, R = &'data [u8]>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    pub(super) file: &'file MachOFile<'data, Mach, R>,
    pub(super) relocations: slice::Iter<'data, macho::Relocation<Mach::Endian>>,
}

impl<'data, 'file, Mach, R> Iterator for MachORelocationIterator<'data, 'file, Mach, R>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    type Item = (u64, Relocation);

    fn next(&mut self) -> Option<Self::Item> {
        use RelocationEncoding as E;
        use RelocationKind as K;

        let mut paired_addend = 0;
        loop {
            let reloc = self.relocations.next()?;
            let endian = self.file.endian;
            let cputype = self.file.header.cputype(endian);
            if reloc.r_scattered(endian, cputype) {
                // FIXME: handle scattered relocations
                // We need to add `RelocationTarget::Address` for this.
                continue;
            }
            let reloc = reloc.info(self.file.endian);
            let flags = RelocationFlags::MachO {
                r_type: reloc.r_type,
                r_pcrel: reloc.r_pcrel,
                r_length: reloc.r_length,
            };
            let g = E::Generic;
            let unknown = (K::Unknown, E::Generic);
            let (kind, encoding) = match cputype {
                macho::CPU_TYPE_ARM => match (reloc.r_type, reloc.r_pcrel) {
                    (macho::ARM_RELOC_VANILLA, false) => (K::Absolute, g),
                    _ => unknown,
                },
                macho::CPU_TYPE_ARM64 | macho::CPU_TYPE_ARM64_32 => {
                    match (reloc.r_type, reloc.r_pcrel) {
                        (macho::ARM64_RELOC_UNSIGNED, false) => (K::Absolute, g),
                        (macho::ARM64_RELOC_ADDEND, _) => {
                            paired_addend = i64::from(reloc.r_symbolnum)
                                .wrapping_shl(64 - 24)
                                .wrapping_shr(64 - 24);
                            continue;
                        }
                        _ => unknown,
                    }
                }
                macho::CPU_TYPE_X86 => match (reloc.r_type, reloc.r_pcrel) {
                    (macho::GENERIC_RELOC_VANILLA, false) => (K::Absolute, g),
                    _ => unknown,
                },
                macho::CPU_TYPE_X86_64 => match (reloc.r_type, reloc.r_pcrel) {
                    (macho::X86_64_RELOC_UNSIGNED, false) => (K::Absolute, g),
                    (macho::X86_64_RELOC_SIGNED, true) => (K::Relative, E::X86RipRelative),
                    (macho::X86_64_RELOC_BRANCH, true) => (K::Relative, E::X86Branch),
                    (macho::X86_64_RELOC_GOT, true) => (K::GotRelative, g),
                    (macho::X86_64_RELOC_GOT_LOAD, true) => (K::GotRelative, E::X86RipRelativeMovq),
                    _ => unknown,
                },
                _ => unknown,
            };
            let size = 8 << reloc.r_length;
            let target = if reloc.r_extern {
                RelocationTarget::Symbol(SymbolIndex(reloc.r_symbolnum as usize))
            } else {
                RelocationTarget::Section(SectionIndex(reloc.r_symbolnum as usize))
            };
            let implicit_addend = paired_addend == 0;
            let mut addend = paired_addend;
            if reloc.r_pcrel {
                // For PC relative relocations on some architectures, the
                // addend does not include the offset required due to the
                // PC being different from the place of the relocation.
                // This differs from other file formats, so adjust the
                // addend here to account for this.
                match cputype {
                    macho::CPU_TYPE_X86 => {
                        addend -= 1 << reloc.r_length;
                    }
                    macho::CPU_TYPE_X86_64 => {
                        addend -= 1 << reloc.r_length;
                        match reloc.r_type {
                            macho::X86_64_RELOC_SIGNED_1 => addend -= 1,
                            macho::X86_64_RELOC_SIGNED_2 => addend -= 2,
                            macho::X86_64_RELOC_SIGNED_4 => addend -= 4,
                            _ => {}
                        }
                    }
                    // TODO: maybe missing support for some architectures and relocations
                    _ => {}
                }
            }
            return Some((
                reloc.r_address as u64,
                Relocation {
                    kind,
                    encoding,
                    size,
                    target,
                    addend,
                    implicit_addend,
                    flags,
                },
            ));
        }
    }
}

impl<'data, 'file, Mach, R> fmt::Debug for MachORelocationIterator<'data, 'file, Mach, R>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MachORelocationIterator").finish()
    }
}
