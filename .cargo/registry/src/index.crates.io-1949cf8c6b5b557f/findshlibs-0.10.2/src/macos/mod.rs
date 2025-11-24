//! The MacOS implementation of the [SharedLibrary
//! trait](../trait.SharedLibrary.html).
#![allow(clippy::cast_ptr_alignment)]

use lazy_static::lazy_static;
use libc;

use crate::Segment as SegmentTrait;
use crate::SharedLibrary as SharedLibraryTrait;
use crate::{Bias, IterationControl, SharedLibraryId, Svma};

use std::ffi::{CStr, OsStr};
use std::fmt;
use std::marker::PhantomData;
use std::os::unix::ffi::OsStrExt;
use std::sync::Mutex;
use std::usize;

const LC_UUID: u32 = 27;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct uuid_command {
    cmd: u32,
    cmdsize: u32,
    uuid: [u8; 16usize],
}

lazy_static! {
    /// A lock protecting dyld FFI calls.
    ///
    /// MacOS does not provide an atomic way to iterate shared libraries, so
    /// *you* must take this lock whenever dynamically adding or removing shared
    /// libraries to ensure that there are no races with iterating shared
    /// libraries.
    pub static ref DYLD_LOCK: Mutex<()> = Mutex::new(());
}

/// A Mach-O segment.
pub enum Segment<'a> {
    /// A 32-bit Mach-O segment.
    Segment32(&'a libc::segment_command),
    /// A 64-bit Mach-O segment.
    Segment64(&'a libc::segment_command_64),
}

impl<'a> fmt::Debug for Segment<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Segment")
            .field("name", &self.name())
            .field("is_code", &self.is_code())
            .finish()
    }
}

impl<'a> SegmentTrait for Segment<'a> {
    type SharedLibrary = SharedLibrary<'a>;

    #[inline]
    fn name(&self) -> &str {
        let cstr = match *self {
            Segment::Segment32(seg) => unsafe { CStr::from_ptr(seg.segname.as_ptr()) },
            Segment::Segment64(seg) => unsafe { CStr::from_ptr(seg.segname.as_ptr()) },
        };
        cstr.to_str().unwrap_or("(invalid segment name)")
    }

    #[inline]
    fn is_code(&self) -> bool {
        self.name().as_bytes() == b"__TEXT"
    }

    #[inline]
    fn stated_virtual_memory_address(&self) -> Svma {
        match *self {
            Segment::Segment32(seg) => Svma(seg.vmaddr as usize),
            Segment::Segment64(seg) => {
                assert!(seg.vmaddr <= (usize::MAX as u64));
                Svma(seg.vmaddr as usize)
            }
        }
    }

    #[inline]
    fn len(&self) -> usize {
        match *self {
            Segment::Segment32(seg) => seg.vmsize as usize,
            Segment::Segment64(seg) => {
                assert!(seg.vmsize <= (usize::MAX as u64));
                seg.vmsize as usize
            }
        }
    }
}

/// An iterator over Mach-O segments.
#[derive(Debug)]
pub struct SegmentIter<'a> {
    phantom: PhantomData<&'a SharedLibrary<'a>>,
    commands: *const libc::load_command,
    num_commands: usize,
}

impl<'a> SegmentIter<'a> {
    fn find_uuid(&self) -> Option<[u8; 16]> {
        let mut num_commands = self.num_commands;
        let mut commands = self.commands;

        while num_commands > 0 {
            num_commands -= 1;
            let this_command = unsafe { commands.as_ref().unwrap() };
            let command_size = this_command.cmdsize as isize;
            if let LC_UUID = this_command.cmd {
                let uuid_cmd = commands as *const uuid_command;
                return Some(unsafe { (*uuid_cmd).uuid });
            }
            commands = unsafe { (commands as *const u8).offset(command_size) as *const _ };
        }

        None
    }
}

impl<'a> Iterator for SegmentIter<'a> {
    type Item = Segment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.num_commands > 0 {
            self.num_commands -= 1;

            let this_command = unsafe { self.commands.as_ref().unwrap() };
            let command_size = this_command.cmdsize as isize;

            match this_command.cmd {
                libc::LC_SEGMENT => {
                    let segment = self.commands as *const libc::segment_command;
                    let segment = unsafe { segment.as_ref().unwrap() };
                    self.commands =
                        unsafe { (self.commands as *const u8).offset(command_size) as *const _ };
                    return Some(Segment::Segment32(segment));
                }
                libc::LC_SEGMENT_64 => {
                    let segment = self.commands as *const libc::segment_command_64;
                    let segment = unsafe { segment.as_ref().unwrap() };
                    self.commands =
                        unsafe { (self.commands as *const u8).offset(command_size) as *const _ };
                    return Some(Segment::Segment64(segment));
                }
                _ => {
                    // Some other kind of load command; skip to the next one.
                    self.commands =
                        unsafe { (self.commands as *const u8).offset(command_size) as *const _ };
                    continue;
                }
            }
        }

        None
    }
}

#[derive(Debug)]
enum MachType {
    Mach32,
    Mach64,
}

impl MachType {
    unsafe fn from_header_ptr(header: *const libc::mach_header) -> Option<MachType> {
        header.as_ref().and_then(|header| match header.magic {
            libc::MH_MAGIC => Some(MachType::Mach32),
            libc::MH_MAGIC_64 => Some(MachType::Mach64),
            _ => None,
        })
    }
}

enum MachHeader<'a> {
    Header32(&'a libc::mach_header),
    Header64(&'a libc::mach_header_64),
}

impl<'a> MachHeader<'a> {
    unsafe fn from_header_ptr(header: *const libc::mach_header) -> Option<MachHeader<'a>> {
        MachType::from_header_ptr(header).and_then(|ty| match ty {
            MachType::Mach32 => header.as_ref().map(MachHeader::Header32),
            MachType::Mach64 => (header as *const libc::mach_header_64)
                .as_ref()
                .map(MachHeader::Header64),
        })
    }
}

/// The MacOS implementation of the [SharedLibrary
/// trait](../trait.SharedLibrary.html).
///
/// This wraps the `_dyld_image_count` and
/// `_dyld_get_image_{header,vmaddr_slide,name}` system APIs from the
/// `<mach-o/dyld.h>` header.
pub struct SharedLibrary<'a> {
    header: MachHeader<'a>,
    slide: usize,
    name: &'a CStr,
}

impl<'a> fmt::Debug for SharedLibrary<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SharedLibrary")
            .field("name", &self.name())
            .field("id", &self.id())
            .finish()
    }
}

impl<'a> SharedLibrary<'a> {
    fn new(header: MachHeader<'a>, slide: usize, name: &'a CStr) -> Self {
        SharedLibrary {
            header: header,
            slide: slide,
            name: name,
        }
    }
}

impl<'a> SharedLibraryTrait for SharedLibrary<'a> {
    type Segment = Segment<'a>;
    type SegmentIter = SegmentIter<'a>;

    #[inline]
    fn name(&self) -> &OsStr {
        OsStr::from_bytes(self.name.to_bytes())
    }

    fn id(&self) -> Option<SharedLibraryId> {
        self.segments().find_uuid().map(SharedLibraryId::Uuid)
    }

    fn segments(&self) -> Self::SegmentIter {
        match self.header {
            MachHeader::Header32(header) => {
                let num_commands = header.ncmds;
                let header = header as *const libc::mach_header;
                let commands = unsafe { header.offset(1) as *const libc::load_command };
                SegmentIter {
                    phantom: PhantomData,
                    commands: commands,
                    num_commands: num_commands as usize,
                }
            }
            MachHeader::Header64(header) => {
                let num_commands = header.ncmds;
                let header = header as *const libc::mach_header_64;
                let commands = unsafe { header.offset(1) as *const libc::load_command };
                SegmentIter {
                    phantom: PhantomData,
                    commands: commands,
                    num_commands: num_commands as usize,
                }
            }
        }
    }

    #[inline]
    fn virtual_memory_bias(&self) -> Bias {
        Bias(self.slide)
    }

    fn each<F, C>(mut f: F)
    where
        F: FnMut(&Self) -> C,
        C: Into<IterationControl>,
    {
        // Make sure we have exclusive access to dyld so that (hopefully) no one
        // else adds or removes shared libraries while we are iterating them.
        let _dyld_lock = DYLD_LOCK.lock();

        let count = unsafe { libc::_dyld_image_count() };

        for image_idx in 0..count {
            let (header, slide, name) = unsafe {
                (
                    libc::_dyld_get_image_header(image_idx),
                    libc::_dyld_get_image_vmaddr_slide(image_idx),
                    libc::_dyld_get_image_name(image_idx),
                )
            };

            if let Some(header) = unsafe { MachHeader::from_header_ptr(header) } {
                assert!(
                    !name.is_null(),
                    "If we have a header pointer, name should be valid"
                );

                let name = unsafe { CStr::from_ptr(name) };
                let shlib = SharedLibrary::new(header, slide as usize, name);

                match f(&shlib).into() {
                    IterationControl::Break => break,
                    IterationControl::Continue => continue,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::macos;
    use crate::{IterationControl, Segment, SharedLibrary};

    #[test]
    fn have_libdyld() {
        let mut found_dyld = false;
        macos::SharedLibrary::each(|shlib| {
            found_dyld |= shlib
                .name
                .to_bytes()
                .split(|c| *c == b'.' || *c == b'/')
                .any(|s| s == b"libdyld");
        });
        assert!(found_dyld);
    }

    #[test]
    fn can_break() {
        let mut first_count = 0;
        macos::SharedLibrary::each(|_| {
            first_count += 1;
        });
        assert!(first_count > 2);

        let mut second_count = 0;
        macos::SharedLibrary::each(|_| {
            second_count += 1;

            if second_count == first_count - 1 {
                IterationControl::Break
            } else {
                IterationControl::Continue
            }
        });
        assert_eq!(second_count, first_count - 1);
    }

    #[test]
    fn get_name() {
        macos::SharedLibrary::each(|shlib| {
            let _ = shlib.name();
        });
    }

    #[test]
    fn get_id() {
        macos::SharedLibrary::each(|shlib| {
            assert!(shlib.id().is_some());
        });
    }

    #[test]
    fn have_text_or_pagezero() {
        macos::SharedLibrary::each(|shlib| {
            println!("shlib = {:?}", shlib.name());

            let mut found_text_or_pagezero = false;
            for seg in shlib.segments() {
                println!("    segment = {:?}", seg.name());

                found_text_or_pagezero |= seg.name() == "__TEXT";
                found_text_or_pagezero |= seg.name() == "__PAGEZERO";
            }
            assert!(found_text_or_pagezero);
        });
    }
}
