use alloc::vec::Vec;
use core::slice;

use crate::endian::{Endian, Endianness};
use crate::macho;
use crate::read::{Architecture, Error, File, ReadError, ReadRef, Result};

/// A parsed representation of the dyld shared cache.
#[derive(Debug)]
pub struct DyldCache<'data, E = Endianness, R = &'data [u8]>
where
    E: Endian,
    R: ReadRef<'data>,
{
    endian: E,
    data: R,
    subcaches: Vec<DyldSubCache<'data, E, R>>,
    mappings: &'data [macho::DyldCacheMappingInfo<E>],
    images: &'data [macho::DyldCacheImageInfo<E>],
    arch: Architecture,
}

/// Information about a subcache.
#[derive(Debug)]
pub struct DyldSubCache<'data, E = Endianness, R = &'data [u8]>
where
    E: Endian,
    R: ReadRef<'data>,
{
    data: R,
    mappings: &'data [macho::DyldCacheMappingInfo<E>],
}

/// A slice of structs describing each subcache. The struct gained
/// an additional field (the file suffix) in dyld-1042.1 (macOS 13 / iOS 16),
/// so this is an enum of the two possible slice types.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum DyldSubCacheSlice<'data, E: Endian> {
    /// V1, used between dyld-940 and dyld-1042.1.
    V1(&'data [macho::DyldSubCacheEntryV1<E>]),
    /// V2, used since dyld-1042.1.
    V2(&'data [macho::DyldSubCacheEntryV2<E>]),
}

// This is the offset of the end of the images_across_all_subcaches_count field.
const MIN_HEADER_SIZE_SUBCACHES_V1: u32 = 0x1c8;

// This is the offset of the end of the cacheSubType field.
// This field comes right after the images_across_all_subcaches_count field,
// and we don't currently have it in our definition of the DyldCacheHeader type.
const MIN_HEADER_SIZE_SUBCACHES_V2: u32 = 0x1d0;

impl<'data, E, R> DyldCache<'data, E, R>
where
    E: Endian,
    R: ReadRef<'data>,
{
    /// Parse the raw dyld shared cache data.
    ///
    /// For shared caches from macOS 12 / iOS 15 and above, the subcache files need to be
    /// supplied as well, in the correct order, with the `.symbols` subcache last (if present).
    /// For example, `data` would be the data for `dyld_shared_cache_x86_64`,
    /// and `subcache_data` would be the data for `[dyld_shared_cache_x86_64.1, dyld_shared_cache_x86_64.2, ...]`.
    pub fn parse(data: R, subcache_data: &[R]) -> Result<Self> {
        let header = macho::DyldCacheHeader::parse(data)?;
        let (arch, endian) = header.parse_magic()?;
        let mappings = header.mappings(endian, data)?;

        let symbols_subcache_uuid = header.symbols_subcache_uuid(endian);
        let subcaches_info = header.subcaches(endian, data)?;
        let subcaches_count = match subcaches_info {
            Some(DyldSubCacheSlice::V1(subcaches)) => subcaches.len(),
            Some(DyldSubCacheSlice::V2(subcaches)) => subcaches.len(),
            None => 0,
        };
        if subcache_data.len() != subcaches_count + symbols_subcache_uuid.is_some() as usize {
            return Err(Error("Incorrect number of SubCaches"));
        }

        // Split out the .symbols subcache data from the other subcaches.
        let (symbols_subcache_data_and_uuid, subcache_data) =
            if let Some(symbols_uuid) = symbols_subcache_uuid {
                let (sym_data, rest_data) = subcache_data.split_last().unwrap();
                (Some((*sym_data, symbols_uuid)), rest_data)
            } else {
                (None, subcache_data)
            };

        // Read the regular SubCaches, if present.
        let mut subcaches = Vec::new();
        if let Some(subcaches_info) = subcaches_info {
            let (v1, v2) = match subcaches_info {
                DyldSubCacheSlice::V1(s) => (s, &[][..]),
                DyldSubCacheSlice::V2(s) => (&[][..], s),
            };
            let uuids = v1.iter().map(|e| &e.uuid).chain(v2.iter().map(|e| &e.uuid));
            for (&data, uuid) in subcache_data.iter().zip(uuids) {
                let sc_header = macho::DyldCacheHeader::<E>::parse(data)?;
                if &sc_header.uuid != uuid {
                    return Err(Error("Unexpected SubCache UUID"));
                }
                let mappings = sc_header.mappings(endian, data)?;
                subcaches.push(DyldSubCache { data, mappings });
            }
        }

        // Read the .symbols SubCache, if present.
        // Other than the UUID verification, the symbols SubCache is currently unused.
        let _symbols_subcache = match symbols_subcache_data_and_uuid {
            Some((data, uuid)) => {
                let sc_header = macho::DyldCacheHeader::<E>::parse(data)?;
                if sc_header.uuid != uuid {
                    return Err(Error("Unexpected .symbols SubCache UUID"));
                }
                let mappings = sc_header.mappings(endian, data)?;
                Some(DyldSubCache { data, mappings })
            }
            None => None,
        };

        let images = header.images(endian, data)?;
        Ok(DyldCache {
            endian,
            data,
            subcaches,
            mappings,
            images,
            arch,
        })
    }

    /// Get the architecture type of the file.
    pub fn architecture(&self) -> Architecture {
        self.arch
    }

    /// Get the endianness of the file.
    #[inline]
    pub fn endianness(&self) -> Endianness {
        if self.is_little_endian() {
            Endianness::Little
        } else {
            Endianness::Big
        }
    }

    /// Return true if the file is little endian, false if it is big endian.
    pub fn is_little_endian(&self) -> bool {
        self.endian.is_little_endian()
    }

    /// Iterate over the images in this cache.
    pub fn images<'cache>(&'cache self) -> DyldCacheImageIterator<'data, 'cache, E, R> {
        DyldCacheImageIterator {
            cache: self,
            iter: self.images.iter(),
        }
    }

    /// Find the address in a mapping and return the cache or subcache data it was found in,
    /// together with the translated file offset.
    pub fn data_and_offset_for_address(&self, address: u64) -> Option<(R, u64)> {
        if let Some(file_offset) = address_to_file_offset(address, self.endian, self.mappings) {
            return Some((self.data, file_offset));
        }
        for subcache in &self.subcaches {
            if let Some(file_offset) =
                address_to_file_offset(address, self.endian, subcache.mappings)
            {
                return Some((subcache.data, file_offset));
            }
        }
        None
    }
}

/// An iterator over all the images (dylibs) in the dyld shared cache.
#[derive(Debug)]
pub struct DyldCacheImageIterator<'data, 'cache, E = Endianness, R = &'data [u8]>
where
    E: Endian,
    R: ReadRef<'data>,
{
    cache: &'cache DyldCache<'data, E, R>,
    iter: slice::Iter<'data, macho::DyldCacheImageInfo<E>>,
}

impl<'data, 'cache, E, R> Iterator for DyldCacheImageIterator<'data, 'cache, E, R>
where
    E: Endian,
    R: ReadRef<'data>,
{
    type Item = DyldCacheImage<'data, 'cache, E, R>;

    fn next(&mut self) -> Option<DyldCacheImage<'data, 'cache, E, R>> {
        let image_info = self.iter.next()?;
        Some(DyldCacheImage {
            cache: self.cache,
            image_info,
        })
    }
}

/// One image (dylib) from inside the dyld shared cache.
#[derive(Debug)]
pub struct DyldCacheImage<'data, 'cache, E = Endianness, R = &'data [u8]>
where
    E: Endian,
    R: ReadRef<'data>,
{
    pub(crate) cache: &'cache DyldCache<'data, E, R>,
    image_info: &'data macho::DyldCacheImageInfo<E>,
}

impl<'data, 'cache, E, R> DyldCacheImage<'data, 'cache, E, R>
where
    E: Endian,
    R: ReadRef<'data>,
{
    /// The file system path of this image.
    pub fn path(&self) -> Result<&'data str> {
        let path = self.image_info.path(self.cache.endian, self.cache.data)?;
        // The path should always be ascii, so from_utf8 should always succeed.
        let path = core::str::from_utf8(path).map_err(|_| Error("Path string not valid utf-8"))?;
        Ok(path)
    }

    /// The subcache data which contains the Mach-O header for this image,
    /// together with the file offset at which this image starts.
    pub fn image_data_and_offset(&self) -> Result<(R, u64)> {
        let address = self.image_info.address.get(self.cache.endian);
        self.cache
            .data_and_offset_for_address(address)
            .ok_or(Error("Address not found in any mapping"))
    }

    /// Parse this image into an Object.
    pub fn parse_object(&self) -> Result<File<'data, R>> {
        File::parse_dyld_cache_image(self)
    }
}

impl<E: Endian> macho::DyldCacheHeader<E> {
    /// Read the dyld cache header.
    pub fn parse<'data, R: ReadRef<'data>>(data: R) -> Result<&'data Self> {
        data.read_at::<macho::DyldCacheHeader<E>>(0)
            .read_error("Invalid dyld cache header size or alignment")
    }

    /// Returns (arch, endian) based on the magic string.
    pub fn parse_magic(&self) -> Result<(Architecture, E)> {
        let (arch, is_big_endian) = match &self.magic {
            b"dyld_v1    i386\0" => (Architecture::I386, false),
            b"dyld_v1  x86_64\0" => (Architecture::X86_64, false),
            b"dyld_v1 x86_64h\0" => (Architecture::X86_64, false),
            b"dyld_v1     ppc\0" => (Architecture::PowerPc, true),
            b"dyld_v1   armv6\0" => (Architecture::Arm, false),
            b"dyld_v1   armv7\0" => (Architecture::Arm, false),
            b"dyld_v1  armv7f\0" => (Architecture::Arm, false),
            b"dyld_v1  armv7s\0" => (Architecture::Arm, false),
            b"dyld_v1  armv7k\0" => (Architecture::Arm, false),
            b"dyld_v1   arm64\0" => (Architecture::Aarch64, false),
            b"dyld_v1  arm64e\0" => (Architecture::Aarch64, false),
            _ => return Err(Error("Unrecognized dyld cache magic")),
        };
        let endian =
            E::from_big_endian(is_big_endian).read_error("Unsupported dyld cache endian")?;
        Ok((arch, endian))
    }

    /// Return the mapping information table.
    pub fn mappings<'data, R: ReadRef<'data>>(
        &self,
        endian: E,
        data: R,
    ) -> Result<&'data [macho::DyldCacheMappingInfo<E>]> {
        data.read_slice_at::<macho::DyldCacheMappingInfo<E>>(
            self.mapping_offset.get(endian).into(),
            self.mapping_count.get(endian) as usize,
        )
        .read_error("Invalid dyld cache mapping size or alignment")
    }

    /// Return the information about subcaches, if present.
    ///
    /// Returns `None` for dyld caches produced before dyld-940 (macOS 12).
    pub fn subcaches<'data, R: ReadRef<'data>>(
        &self,
        endian: E,
        data: R,
    ) -> Result<Option<DyldSubCacheSlice<'data, E>>> {
        let header_size = self.mapping_offset.get(endian);
        if header_size >= MIN_HEADER_SIZE_SUBCACHES_V2 {
            let subcaches = data
                .read_slice_at::<macho::DyldSubCacheEntryV2<E>>(
                    self.subcaches_offset.get(endian).into(),
                    self.subcaches_count.get(endian) as usize,
                )
                .read_error("Invalid dyld subcaches size or alignment")?;
            Ok(Some(DyldSubCacheSlice::V2(subcaches)))
        } else if header_size >= MIN_HEADER_SIZE_SUBCACHES_V1 {
            let subcaches = data
                .read_slice_at::<macho::DyldSubCacheEntryV1<E>>(
                    self.subcaches_offset.get(endian).into(),
                    self.subcaches_count.get(endian) as usize,
                )
                .read_error("Invalid dyld subcaches size or alignment")?;
            Ok(Some(DyldSubCacheSlice::V1(subcaches)))
        } else {
            Ok(None)
        }
    }

    /// Return the UUID for the .symbols subcache, if present.
    pub fn symbols_subcache_uuid(&self, endian: E) -> Option<[u8; 16]> {
        if self.mapping_offset.get(endian) >= MIN_HEADER_SIZE_SUBCACHES_V1 {
            let uuid = self.symbols_subcache_uuid;
            if uuid != [0; 16] {
                return Some(uuid);
            }
        }
        None
    }

    /// Return the image information table.
    pub fn images<'data, R: ReadRef<'data>>(
        &self,
        endian: E,
        data: R,
    ) -> Result<&'data [macho::DyldCacheImageInfo<E>]> {
        if self.mapping_offset.get(endian) >= MIN_HEADER_SIZE_SUBCACHES_V1 {
            data.read_slice_at::<macho::DyldCacheImageInfo<E>>(
                self.images_across_all_subcaches_offset.get(endian).into(),
                self.images_across_all_subcaches_count.get(endian) as usize,
            )
            .read_error("Invalid dyld cache image size or alignment")
        } else {
            data.read_slice_at::<macho::DyldCacheImageInfo<E>>(
                self.images_offset.get(endian).into(),
                self.images_count.get(endian) as usize,
            )
            .read_error("Invalid dyld cache image size or alignment")
        }
    }
}

impl<E: Endian> macho::DyldCacheImageInfo<E> {
    /// The file system path of this image.
    pub fn path<'data, R: ReadRef<'data>>(&self, endian: E, data: R) -> Result<&'data [u8]> {
        let r_start = self.path_file_offset.get(endian).into();
        let r_end = data.len().read_error("Couldn't get data len()")?;
        data.read_bytes_at_until(r_start..r_end, 0)
            .read_error("Couldn't read dyld cache image path")
    }

    /// Find the file offset of the image by looking up its address in the mappings.
    pub fn file_offset(
        &self,
        endian: E,
        mappings: &[macho::DyldCacheMappingInfo<E>],
    ) -> Result<u64> {
        let address = self.address.get(endian);
        address_to_file_offset(address, endian, mappings)
            .read_error("Invalid dyld cache image address")
    }
}

/// Find the file offset of the image by looking up its address in the mappings.
pub fn address_to_file_offset<E: Endian>(
    address: u64,
    endian: E,
    mappings: &[macho::DyldCacheMappingInfo<E>],
) -> Option<u64> {
    for mapping in mappings {
        let mapping_address = mapping.address.get(endian);
        if address >= mapping_address
            && address < mapping_address.wrapping_add(mapping.size.get(endian))
        {
            return Some(address - mapping_address + mapping.file_offset.get(endian));
        }
    }
    None
}
