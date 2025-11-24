use alloc::fmt;
use core::fmt::Debug;
use core::slice;

use crate::endian::BigEndian as BE;
use crate::pod::Pod;
use crate::read::{
    ReadRef, Relocation, RelocationEncoding, RelocationFlags, RelocationKind, RelocationTarget,
    SymbolIndex,
};
use crate::xcoff;

use super::{FileHeader, SectionHeader, XcoffFile};

/// An iterator for the relocations in an [`XcoffSection32`](super::XcoffSection32).
pub type XcoffRelocationIterator32<'data, 'file, R = &'data [u8]> =
    XcoffRelocationIterator<'data, 'file, xcoff::FileHeader32, R>;
/// An iterator for the relocations in an [`XcoffSection64`](super::XcoffSection64).
pub type XcoffRelocationIterator64<'data, 'file, R = &'data [u8]> =
    XcoffRelocationIterator<'data, 'file, xcoff::FileHeader64, R>;

/// An iterator for the relocations in an [`XcoffSection`](super::XcoffSection).
pub struct XcoffRelocationIterator<'data, 'file, Xcoff, R = &'data [u8]>
where
    Xcoff: FileHeader,
    R: ReadRef<'data>,
{
    #[allow(unused)]
    pub(super) file: &'file XcoffFile<'data, Xcoff, R>,
    pub(super) relocations:
        slice::Iter<'data, <<Xcoff as FileHeader>::SectionHeader as SectionHeader>::Rel>,
}

impl<'data, 'file, Xcoff, R> Iterator for XcoffRelocationIterator<'data, 'file, Xcoff, R>
where
    Xcoff: FileHeader,
    R: ReadRef<'data>,
{
    type Item = (u64, Relocation);

    fn next(&mut self) -> Option<Self::Item> {
        self.relocations.next().map(|relocation| {
            let r_rtype = relocation.r_rtype();
            let r_rsize = relocation.r_rsize();
            let flags = RelocationFlags::Xcoff { r_rtype, r_rsize };
            let encoding = RelocationEncoding::Generic;
            let (kind, addend) = match r_rtype {
                xcoff::R_POS
                | xcoff::R_RL
                | xcoff::R_RLA
                | xcoff::R_BA
                | xcoff::R_RBA
                | xcoff::R_TLS => (RelocationKind::Absolute, 0),
                xcoff::R_REL | xcoff::R_BR | xcoff::R_RBR => (RelocationKind::Relative, -4),
                xcoff::R_TOC | xcoff::R_TOCL | xcoff::R_TOCU => (RelocationKind::Got, 0),
                _ => (RelocationKind::Unknown, 0),
            };
            let size = (r_rsize & 0x3F) + 1;
            let target = RelocationTarget::Symbol(relocation.symbol());
            (
                relocation.r_vaddr().into(),
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

impl<'data, 'file, Xcoff, R> fmt::Debug for XcoffRelocationIterator<'data, 'file, Xcoff, R>
where
    Xcoff: FileHeader,
    R: ReadRef<'data>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("XcoffRelocationIterator").finish()
    }
}

/// A trait for generic access to [`xcoff::Rel32`] and [`xcoff::Rel64`].
#[allow(missing_docs)]
pub trait Rel: Debug + Pod {
    type Word: Into<u64>;
    fn r_vaddr(&self) -> Self::Word;
    fn r_symndx(&self) -> u32;
    fn r_rsize(&self) -> u8;
    fn r_rtype(&self) -> u8;

    fn symbol(&self) -> SymbolIndex {
        SymbolIndex(self.r_symndx() as usize)
    }
}

impl Rel for xcoff::Rel32 {
    type Word = u32;

    fn r_vaddr(&self) -> Self::Word {
        self.r_vaddr.get(BE)
    }

    fn r_symndx(&self) -> u32 {
        self.r_symndx.get(BE)
    }

    fn r_rsize(&self) -> u8 {
        self.r_rsize
    }

    fn r_rtype(&self) -> u8 {
        self.r_rtype
    }
}

impl Rel for xcoff::Rel64 {
    type Word = u64;

    fn r_vaddr(&self) -> Self::Word {
        self.r_vaddr.get(BE)
    }

    fn r_symndx(&self) -> u32 {
        self.r_symndx.get(BE)
    }

    fn r_rsize(&self) -> u8 {
        self.r_rsize
    }

    fn r_rtype(&self) -> u8 {
        self.r_rtype
    }
}
