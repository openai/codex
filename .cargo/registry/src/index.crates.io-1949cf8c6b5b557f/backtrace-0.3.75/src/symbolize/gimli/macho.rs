use super::mystd::path::Path;
use super::{gimli, Context, Endian, EndianSlice, Mapping, Stash};
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::convert::TryInto;
use object::macho;
use object::read::macho::{MachHeader, Nlist, Section, Segment as _};
use object::{Bytes, NativeEndian};

#[cfg(target_pointer_width = "32")]
type Mach = object::macho::MachHeader32<NativeEndian>;
#[cfg(target_pointer_width = "64")]
type Mach = object::macho::MachHeader64<NativeEndian>;
type MachSegment = <Mach as MachHeader>::Segment;
type MachSection = <Mach as MachHeader>::Section;
type MachNlist = <Mach as MachHeader>::Nlist;

impl Mapping {
    // The loading path for macOS is so different we just have a completely
    // different implementation of the function here. On macOS we need to go
    // probing the filesystem for a bunch of files.
    pub fn new(path: &Path) -> Option<Mapping> {
        // First up we need to load the unique UUID which is stored in the macho
        // header of the file we're reading, specified at `path`.
        let map = super::mmap(path)?;
        let (macho, data) = find_header(&map)?;
        let endian = macho.endian().ok()?;
        let uuid = macho.uuid(endian, data, 0).ok()?;

        // Next we need to look for a `*.dSYM` file. For now we just probe the
        // containing directory and look around for something that matches
        // `*.dSYM`. Once it's found we root through the dwarf resources that it
        // contains and try to find a macho file which has a matching UUID as
        // the one of our own file. If we find a match that's the dwarf file we
        // want to return.
        if let Some(uuid) = uuid {
            if let Some(parent) = path.parent() {
                if let Some(mapping) = Mapping::load_dsym(parent, uuid) {
                    return Some(mapping);
                }
            }
        }

        // Looks like nothing matched our UUID, so let's at least return our own
        // file. This should have the symbol table for at least some
        // symbolication purposes.
        Mapping::mk(map, |data, stash| {
            let (macho, data) = find_header(data)?;
            let endian = macho.endian().ok()?;
            let obj = Object::parse(macho, endian, data)?;
            Context::new(stash, obj, None, None)
        })
    }

    fn load_dsym(dir: &Path, uuid: [u8; 16]) -> Option<Mapping> {
        for entry in dir.read_dir().ok()? {
            let entry = entry.ok()?;
            let filename = match entry.file_name().into_string() {
                Ok(name) => name,
                Err(_) => continue,
            };
            if !filename.ends_with(".dSYM") {
                continue;
            }
            let candidates = entry.path().join("Contents/Resources/DWARF");
            if let Some(mapping) = Mapping::try_dsym_candidate(&candidates, uuid) {
                return Some(mapping);
            }
        }
        None
    }

    fn try_dsym_candidate(dir: &Path, uuid: [u8; 16]) -> Option<Mapping> {
        // Look for files in the `DWARF` directory which have a matching uuid to
        // the original object file. If we find one then we found the debug
        // information.
        for entry in dir.read_dir().ok()? {
            let entry = entry.ok()?;
            let map = super::mmap(&entry.path())?;
            let candidate = Mapping::mk(map, |data, stash| {
                let (macho, data) = find_header(data)?;
                let endian = macho.endian().ok()?;
                let entry_uuid = macho.uuid(endian, data, 0).ok()??;
                if entry_uuid != uuid {
                    return None;
                }
                let obj = Object::parse(macho, endian, data)?;
                Context::new(stash, obj, None, None)
            });
            if let Some(candidate) = candidate {
                return Some(candidate);
            }
        }

        None
    }
}

fn find_header(data: &'_ [u8]) -> Option<(&'_ Mach, &'_ [u8])> {
    use object::endian::BigEndian;

    let desired_cpu = || {
        if cfg!(target_arch = "x86") {
            Some(macho::CPU_TYPE_X86)
        } else if cfg!(target_arch = "x86_64") {
            Some(macho::CPU_TYPE_X86_64)
        } else if cfg!(target_arch = "arm") {
            Some(macho::CPU_TYPE_ARM)
        } else if cfg!(target_arch = "aarch64") {
            Some(macho::CPU_TYPE_ARM64)
        } else {
            None
        }
    };

    let mut data = Bytes(data);
    match data
        .clone()
        .read::<object::endian::U32<NativeEndian>>()
        .ok()?
        .get(NativeEndian)
    {
        macho::MH_MAGIC_64 | macho::MH_CIGAM_64 | macho::MH_MAGIC | macho::MH_CIGAM => {}

        macho::FAT_MAGIC | macho::FAT_CIGAM => {
            let mut header_data = data;
            let endian = BigEndian;
            let header = header_data.read::<macho::FatHeader>().ok()?;
            let nfat = header.nfat_arch.get(endian);
            let arch = (0..nfat)
                .filter_map(|_| header_data.read::<macho::FatArch32>().ok())
                .find(|arch| desired_cpu() == Some(arch.cputype.get(endian)))?;
            let offset = arch.offset.get(endian);
            let size = arch.size.get(endian);
            data = data
                .read_bytes_at(offset.try_into().ok()?, size.try_into().ok()?)
                .ok()?;
        }

        macho::FAT_MAGIC_64 | macho::FAT_CIGAM_64 => {
            let mut header_data = data;
            let endian = BigEndian;
            let header = header_data.read::<macho::FatHeader>().ok()?;
            let nfat = header.nfat_arch.get(endian);
            let arch = (0..nfat)
                .filter_map(|_| header_data.read::<macho::FatArch64>().ok())
                .find(|arch| desired_cpu() == Some(arch.cputype.get(endian)))?;
            let offset = arch.offset.get(endian);
            let size = arch.size.get(endian);
            data = data
                .read_bytes_at(offset.try_into().ok()?, size.try_into().ok()?)
                .ok()?;
        }

        _ => return None,
    }

    Mach::parse(data.0, 0).ok().map(|h| (h, data.0))
}

// This is used both for executables/libraries and source object files.
pub struct Object<'a> {
    endian: NativeEndian,
    data: &'a [u8],
    dwarf: Option<&'a [MachSection]>,
    syms: Vec<(&'a [u8], u64)>,
    syms_sort_by_name: bool,
    // Only set for executables/libraries, and not the source object files.
    object_map: Option<object::ObjectMap<'a>>,
    // The outer Option is for lazy loading, and the inner Option allows load errors to be cached.
    object_mappings: Box<[Option<Option<Mapping>>]>,
}

impl<'a> Object<'a> {
    fn parse(mach: &'a Mach, endian: NativeEndian, data: &'a [u8]) -> Option<Object<'a>> {
        let is_object = mach.filetype(endian) == object::macho::MH_OBJECT;
        let mut dwarf = None;
        let mut syms = Vec::new();
        let mut syms_sort_by_name = false;
        let mut commands = mach.load_commands(endian, data, 0).ok()?;
        let mut object_map = None;
        let mut object_mappings = Vec::new();
        while let Ok(Some(command)) = commands.next() {
            if let Some((segment, section_data)) = MachSegment::from_command(command).ok()? {
                // Object files should have all sections in a single unnamed segment load command.
                if segment.name() == b"__DWARF" || (is_object && segment.name() == b"") {
                    dwarf = segment.sections(endian, section_data).ok();
                }
            } else if let Some(symtab) = command.symtab().ok()? {
                let symbols = symtab.symbols::<Mach, _>(endian, data).ok()?;
                syms = symbols
                    .iter()
                    .filter_map(|nlist: &MachNlist| {
                        let name = nlist.name(endian, symbols.strings()).ok()?;
                        if name.len() > 0 && nlist.is_definition() {
                            Some((name, u64::from(nlist.n_value(endian))))
                        } else {
                            None
                        }
                    })
                    .collect();
                if is_object {
                    // We never search object file symbols by address.
                    // Instead, we already know the symbol name from the executable, and we
                    // need to search by name to find the matching symbol in the object file.
                    syms.sort_unstable_by_key(|(name, _)| *name);
                    syms_sort_by_name = true;
                } else {
                    syms.sort_unstable_by_key(|(_, addr)| *addr);
                    let map = symbols.object_map(endian);
                    object_mappings.resize_with(map.objects().len(), || None);
                    object_map = Some(map);
                }
            }
        }

        Some(Object {
            endian,
            data,
            dwarf,
            syms,
            syms_sort_by_name,
            object_map,
            object_mappings: object_mappings.into_boxed_slice(),
        })
    }

    pub fn section(&self, _: &Stash, name: &str) -> Option<&'a [u8]> {
        let name = name.as_bytes();
        let dwarf = self.dwarf?;
        let section = dwarf.into_iter().find(|section| {
            let section_name = section.name();
            section_name == name || {
                section_name.starts_with(b"__")
                    && name.starts_with(b".")
                    && &section_name[2..] == &name[1..]
            }
        })?;
        Some(section.data(self.endian, self.data).ok()?)
    }

    pub fn search_symtab<'b>(&'b self, addr: u64) -> Option<&'b [u8]> {
        debug_assert!(!self.syms_sort_by_name);
        let i = match self.syms.binary_search_by_key(&addr, |(_, addr)| *addr) {
            Ok(i) => i,
            Err(i) => i.checked_sub(1)?,
        };
        let (sym, _addr) = self.syms.get(i)?;
        Some(sym)
    }

    /// Try to load a context for an object file.
    ///
    /// If dsymutil was not run, then the DWARF may be found in the source object files.
    pub(super) fn search_object_map<'b>(&'b mut self, addr: u64) -> Option<(&'b Context<'b>, u64)> {
        // `object_map` contains a map from addresses to symbols and object paths.
        // Look up the address and get a mapping for the object.
        let object_map = self.object_map.as_ref()?;
        let symbol = object_map.get(addr)?;
        let object_index = symbol.object_index();
        let mapping = self.object_mappings.get_mut(object_index)?;
        if mapping.is_none() {
            // No cached mapping, so create it.
            *mapping = Some(object_mapping(object_map.objects().get(object_index)?));
        }
        let cx: &'b Context<'static> = &mapping.as_ref()?.as_ref()?.cx;
        // Don't leak the `'static` lifetime, make sure it's scoped to just ourselves.
        let cx = unsafe { core::mem::transmute::<&'b Context<'static>, &'b Context<'b>>(cx) };

        // We must translate the address in order to be able to look it up
        // in the DWARF in the object file.
        debug_assert!(cx.object.syms.is_empty() || cx.object.syms_sort_by_name);
        let i = cx
            .object
            .syms
            .binary_search_by_key(&symbol.name(), |(name, _)| *name)
            .ok()?;
        let object_symbol = cx.object.syms.get(i)?;
        let object_addr = addr
            .wrapping_sub(symbol.address())
            .wrapping_add(object_symbol.1);
        Some((cx, object_addr))
    }
}

fn object_mapping(file: &object::read::ObjectMapFile<'_>) -> Option<Mapping> {
    use super::mystd::ffi::OsStr;
    use super::mystd::os::unix::prelude::*;

    let map = super::mmap(Path::new(OsStr::from_bytes(file.path())))?;
    let member_name = file.member();
    Mapping::mk(map, |data, stash| {
        let data = match member_name {
            Some(member_name) => {
                let archive = object::read::archive::ArchiveFile::parse(data).ok()?;
                let member = archive
                    .members()
                    .filter_map(Result::ok)
                    .find(|m| m.name() == member_name)?;
                member.data(data).ok()?
            }
            None => data,
        };
        let (macho, data) = find_header(data)?;
        let endian = macho.endian().ok()?;
        let obj = Object::parse(macho, endian, data)?;
        Context::new(stash, obj, None, None)
    })
}

pub(super) fn handle_split_dwarf<'data>(
    _package: Option<&gimli::DwarfPackage<EndianSlice<'data, Endian>>>,
    _stash: &'data Stash,
    _load: addr2line::SplitDwarfLoad<EndianSlice<'data, Endian>>,
) -> Option<Arc<gimli::Dwarf<EndianSlice<'data, Endian>>>> {
    None
}
