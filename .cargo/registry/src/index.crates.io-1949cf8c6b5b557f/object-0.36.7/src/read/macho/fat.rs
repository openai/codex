use crate::endian::BigEndian;
use crate::macho;
use crate::pod::Pod;
use crate::read::{Architecture, Error, ReadError, ReadRef, Result};

pub use macho::{FatArch32, FatArch64, FatHeader};

/// A 32-bit Mach-O universal binary.
///
/// This is a file that starts with [`macho::FatHeader`], and corresponds
/// to [`crate::FileKind::MachOFat32`].
pub type MachOFatFile32<'data> = MachOFatFile<'data, macho::FatArch32>;

/// A 64-bit Mach-O universal binary.
///
/// This is a file that starts with [`macho::FatHeader`], and corresponds
/// to [`crate::FileKind::MachOFat64`].
pub type MachOFatFile64<'data> = MachOFatFile<'data, macho::FatArch64>;

/// A Mach-O universal binary.
///
/// This is a file that starts with [`macho::FatHeader`], and corresponds
/// to [`crate::FileKind::MachOFat32`] or [`crate::FileKind::MachOFat64`].
#[derive(Debug, Clone)]
pub struct MachOFatFile<'data, Fat: FatArch> {
    header: &'data macho::FatHeader,
    arches: &'data [Fat],
}

impl<'data, Fat: FatArch> MachOFatFile<'data, Fat> {
    /// Attempt to parse the fat header and fat arches.
    pub fn parse<R: ReadRef<'data>>(data: R) -> Result<Self> {
        let mut offset = 0;
        let header = data
            .read::<FatHeader>(&mut offset)
            .read_error("Invalid fat header size or alignment")?;
        if header.magic.get(BigEndian) != Fat::MAGIC {
            return Err(Error("Invalid fat magic"));
        }
        let arches = data
            .read_slice::<Fat>(&mut offset, header.nfat_arch.get(BigEndian) as usize)
            .read_error("Invalid nfat_arch")?;
        Ok(MachOFatFile { header, arches })
    }

    /// Return the fat header
    pub fn header(&self) -> &'data macho::FatHeader {
        self.header
    }

    /// Return the array of fat arches.
    pub fn arches(&self) -> &'data [Fat] {
        self.arches
    }
}

/// A trait for generic access to [`macho::FatArch32`] and [`macho::FatArch64`].
#[allow(missing_docs)]
pub trait FatArch: Pod {
    type Word: Into<u64>;
    const MAGIC: u32;

    fn cputype(&self) -> u32;
    fn cpusubtype(&self) -> u32;
    fn offset(&self) -> Self::Word;
    fn size(&self) -> Self::Word;
    fn align(&self) -> u32;

    fn architecture(&self) -> Architecture {
        match self.cputype() {
            macho::CPU_TYPE_ARM => Architecture::Arm,
            macho::CPU_TYPE_ARM64 => Architecture::Aarch64,
            macho::CPU_TYPE_X86 => Architecture::I386,
            macho::CPU_TYPE_X86_64 => Architecture::X86_64,
            macho::CPU_TYPE_MIPS => Architecture::Mips,
            macho::CPU_TYPE_POWERPC => Architecture::PowerPc,
            macho::CPU_TYPE_POWERPC64 => Architecture::PowerPc64,
            _ => Architecture::Unknown,
        }
    }

    fn file_range(&self) -> (u64, u64) {
        (self.offset().into(), self.size().into())
    }

    fn data<'data, R: ReadRef<'data>>(&self, file: R) -> Result<&'data [u8]> {
        file.read_bytes_at(self.offset().into(), self.size().into())
            .read_error("Invalid fat arch offset or size")
    }
}

impl FatArch for FatArch32 {
    type Word = u32;
    const MAGIC: u32 = macho::FAT_MAGIC;

    fn cputype(&self) -> u32 {
        self.cputype.get(BigEndian)
    }

    fn cpusubtype(&self) -> u32 {
        self.cpusubtype.get(BigEndian)
    }

    fn offset(&self) -> Self::Word {
        self.offset.get(BigEndian)
    }

    fn size(&self) -> Self::Word {
        self.size.get(BigEndian)
    }

    fn align(&self) -> u32 {
        self.align.get(BigEndian)
    }
}

impl FatArch for FatArch64 {
    type Word = u64;
    const MAGIC: u32 = macho::FAT_MAGIC_64;

    fn cputype(&self) -> u32 {
        self.cputype.get(BigEndian)
    }

    fn cpusubtype(&self) -> u32 {
        self.cpusubtype.get(BigEndian)
    }

    fn offset(&self) -> Self::Word {
        self.offset.get(BigEndian)
    }

    fn size(&self) -> Self::Word {
        self.size.get(BigEndian)
    }

    fn align(&self) -> u32 {
        self.align.get(BigEndian)
    }
}
