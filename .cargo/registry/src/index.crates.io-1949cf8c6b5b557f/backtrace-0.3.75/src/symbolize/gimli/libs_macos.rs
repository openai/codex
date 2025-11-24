#![allow(deprecated)]

use super::mystd::ffi::OsStr;
use super::mystd::os::unix::prelude::*;
use super::mystd::prelude::v1::*;
use super::{Library, LibrarySegment};
use core::convert::TryInto;
use core::ffi::CStr;
use core::mem;

// FIXME: replace with ptr::from_ref once MSRV is high enough
#[inline(always)]
#[must_use]
const fn ptr_from_ref<T: ?Sized>(r: &T) -> *const T {
    r
}

pub(super) fn native_libraries() -> Vec<Library> {
    let mut ret = Vec::new();
    let images = unsafe { libc::_dyld_image_count() };
    for i in 0..images {
        ret.extend(native_library(i));
    }
    return ret;
}

fn native_library(i: u32) -> Option<Library> {
    use object::macho;
    use object::read::macho::{MachHeader, Segment};
    use object::NativeEndian;

    // Fetch the name of this library which corresponds to the path of
    // where to load it as well.
    let name = unsafe {
        let name = libc::_dyld_get_image_name(i);
        if name.is_null() {
            return None;
        }
        CStr::from_ptr(name)
    };

    // Load the image header of this library and delegate to `object` to
    // parse all the load commands so we can figure out all the segments
    // involved here.
    let (mut load_commands, endian) = unsafe {
        let header = libc::_dyld_get_image_header(i);
        if header.is_null() {
            return None;
        }
        match (*header).magic {
            macho::MH_MAGIC => {
                let endian = NativeEndian;
                let header = &*header.cast::<macho::MachHeader32<NativeEndian>>();
                let data = core::slice::from_raw_parts(
                    ptr_from_ref(header).cast::<u8>(),
                    mem::size_of_val(header) + header.sizeofcmds.get(endian) as usize,
                );
                (header.load_commands(endian, data, 0).ok()?, endian)
            }
            macho::MH_MAGIC_64 => {
                let endian = NativeEndian;
                let header = &*header.cast::<macho::MachHeader64<NativeEndian>>();
                let data = core::slice::from_raw_parts(
                    ptr_from_ref(header).cast::<u8>(),
                    mem::size_of_val(header) + header.sizeofcmds.get(endian) as usize,
                );
                (header.load_commands(endian, data, 0).ok()?, endian)
            }
            _ => return None,
        }
    };

    // Iterate over the segments and register known regions for segments
    // that we find. Additionally record information bout text segments
    // for processing later, see comments below.
    let mut segments = Vec::new();
    let mut first_text = 0;
    let mut text_fileoff_zero = false;
    while let Some(cmd) = load_commands.next().ok()? {
        if let Some((seg, _)) = cmd.segment_32().ok()? {
            if seg.name() == b"__TEXT" {
                first_text = segments.len();
                if seg.fileoff(endian) == 0 && seg.filesize(endian) > 0 {
                    text_fileoff_zero = true;
                }
            }
            segments.push(LibrarySegment {
                len: seg.vmsize(endian).try_into().ok()?,
                stated_virtual_memory_address: seg.vmaddr(endian).try_into().ok()?,
            });
        }
        if let Some((seg, _)) = cmd.segment_64().ok()? {
            if seg.name() == b"__TEXT" {
                first_text = segments.len();
                if seg.fileoff(endian) == 0 && seg.filesize(endian) > 0 {
                    text_fileoff_zero = true;
                }
            }
            segments.push(LibrarySegment {
                len: seg.vmsize(endian).try_into().ok()?,
                stated_virtual_memory_address: seg.vmaddr(endian).try_into().ok()?,
            });
        }
    }

    // Determine the "slide" for this library which ends up being the
    // bias we use to figure out where in memory objects are loaded.
    // This is a bit of a weird computation though and is the result of
    // trying a few things in the wild and seeing what sticks.
    //
    // The general idea is that the `bias` plus a segment's
    // `stated_virtual_memory_address` is going to be where in the
    // actual address space the segment resides. The other thing we rely
    // on though is that a real address minus the `bias` is the index to
    // look up in the symbol table and debuginfo.
    //
    // It turns out, though, that for system loaded libraries these
    // calculations are incorrect. For native executables, however, it
    // appears correct. Lifting some logic from LLDB's source it has
    // some special-casing for the first `__TEXT` section loaded from
    // file offset 0 with a nonzero size. For whatever reason when this
    // is present it appears to mean that the symbol table is relative
    // to just the vmaddr slide for the library. If it's *not* present
    // then the symbol table is relative to the vmaddr slide plus the
    // segment's stated address.
    //
    // To handle this situation if we *don't* find a text section at
    // file offset zero then we increase the bias by the first text
    // sections's stated address and decrease all stated addresses by
    // that amount as well. That way the symbol table is always appears
    // relative to the library's bias amount. This appears to have the
    // right results for symbolizing via the symbol table.
    //
    // Honestly I'm not entirely sure whether this is right or if
    // there's something else that should indicate how to do this. For
    // now though this seems to work well enough (?) and we should
    // always be able to tweak this over time if necessary.
    //
    // For some more information see #318
    let mut slide = unsafe { libc::_dyld_get_image_vmaddr_slide(i) as usize };
    if !text_fileoff_zero {
        let adjust = segments[first_text].stated_virtual_memory_address;
        for segment in segments.iter_mut() {
            segment.stated_virtual_memory_address -= adjust;
        }
        slide += adjust;
    }

    Some(Library {
        name: OsStr::from_bytes(name.to_bytes()).to_owned(),
        segments,
        bias: slide,
    })
}
