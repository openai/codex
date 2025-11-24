//! Linux-specific implementation of the `SharedLibrary` trait.

use libc;

use crate::Segment as SegmentTrait;
use crate::SharedLibrary as SharedLibraryTrait;
use crate::{Bias, IterationControl, SharedLibraryId, Svma};

use std::any::Any;
use std::borrow::Cow;
use std::env::current_exe;
use std::ffi::{CStr, CString, OsStr};
use std::fmt;
use std::iter;
use std::marker::PhantomData;
use std::mem;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::ffi::OsStringExt;
use std::panic;
use std::slice;
use std::usize;

#[cfg(target_pointer_width = "32")]
type Phdr = libc::Elf32_Phdr;

#[cfg(target_pointer_width = "64")]
type Phdr = libc::Elf64_Phdr;

const NT_GNU_BUILD_ID: u32 = 3;

// Normally we would use `Elf32_Nhdr` on 32-bit platforms and `Elf64_Nhdr` on
// 64-bit platforms. However, in practice it seems that only `Elf32_Nhdr` is
// used, and reading through binutil's `readelf` source confirms this.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct Nhdr {
    pub n_namesz: libc::Elf32_Word,
    pub n_descsz: libc::Elf32_Word,
    pub n_type: libc::Elf32_Word,
}

/// A mapped segment in an ELF file.
#[derive(Debug)]
pub struct Segment<'a> {
    phdr: *const Phdr,
    shlib: PhantomData<&'a SharedLibrary<'a>>,
}

impl<'a> Segment<'a> {
    fn phdr(&self) -> &'a Phdr {
        unsafe { self.phdr.as_ref().unwrap() }
    }

    /// You must pass this segment's `SharedLibrary` or else this is wild UB.
    unsafe fn data(&self, shlib: &SharedLibrary<'a>) -> &'a [u8] {
        let phdr = self.phdr();
        let avma = (shlib.addr as usize).wrapping_add(phdr.p_vaddr as usize);
        slice::from_raw_parts(avma as *const u8, phdr.p_memsz as usize)
    }

    fn is_note(&self) -> bool {
        self.phdr().p_type == libc::PT_NOTE
    }

    /// Parse the contents of a `PT_NOTE` segment.
    ///
    /// Returns a triple of
    ///
    /// 1. The `NT_*` note type.
    /// 2. The note name.
    /// 3. The note descriptor payload.
    ///
    /// You must pass this segment's `SharedLibrary` or else this is wild UB.
    unsafe fn notes(
        &self,
        shlib: &SharedLibrary<'a>,
    ) -> impl Iterator<Item = (libc::Elf32_Word, &'a [u8], &'a [u8])> {
        // `man 5 readelf` says that all of the `Nhdr`, name, and descriptor are
        // always 4-byte aligned, but we copy this alignment behavior from
        // `readelf` since that seems to match reality in practice.
        let alignment = std::cmp::max(self.phdr().p_align as usize, 4);
        let align_up = move |data: &'a [u8]| {
            if alignment != 4 && alignment != 8 {
                return None;
            }

            let ptr = data.as_ptr() as usize;
            let alignment_minus_one = alignment - 1;
            let aligned_ptr = ptr.checked_add(alignment_minus_one)? & !alignment_minus_one;
            let diff = aligned_ptr - ptr;
            if data.len() < diff {
                None
            } else {
                Some(&data[diff..])
            }
        };

        let mut data = self.data(shlib);

        iter::from_fn(move || {
            if (data.as_ptr() as usize % alignment) != 0 {
                return None;
            }

            // Each entry in a `PT_NOTE` segment begins with a
            // fixed-size header `Nhdr`.
            let nhdr_size = mem::size_of::<Nhdr>();
            let nhdr = try_split_at(&mut data, nhdr_size)?;
            let nhdr = (nhdr.as_ptr() as *const Nhdr).as_ref().unwrap();

            // No need to `align_up` after the `Nhdr`
            // It is followed by a name of size `n_namesz`.
            let name_size = nhdr.n_namesz as usize;
            let name = try_split_at(&mut data, name_size)?;

            // And after that is the note's (aligned) descriptor payload of size
            // `n_descsz`.
            data = align_up(data)?;
            let desc_size = nhdr.n_descsz as usize;
            let desc = try_split_at(&mut data, desc_size)?;

            // Align the data for the next `Nhdr`.
            data = align_up(data)?;

            Some((nhdr.n_type, name, desc))
        })
        .fuse()
    }
}

fn try_split_at<'a>(data: &mut &'a [u8], index: usize) -> Option<&'a [u8]> {
    if data.len() < index {
        None
    } else {
        let (head, tail) = data.split_at(index);
        *data = tail;
        Some(head)
    }
}

impl<'a> SegmentTrait for Segment<'a> {
    type SharedLibrary = SharedLibrary<'a>;

    fn name(&self) -> &str {
        unsafe {
            match self.phdr.as_ref().unwrap().p_type {
                libc::PT_NULL => "NULL",
                libc::PT_LOAD => "LOAD",
                libc::PT_DYNAMIC => "DYNAMIC",
                libc::PT_INTERP => "INTERP",
                libc::PT_NOTE => "NOTE",
                libc::PT_SHLIB => "SHLIB",
                libc::PT_PHDR => "PHDR",
                libc::PT_TLS => "TLS",
                libc::PT_GNU_EH_FRAME => "GNU_EH_FRAME",
                libc::PT_GNU_STACK => "GNU_STACK",
                libc::PT_GNU_RELRO => "GNU_RELRO",
                _ => "(unknown segment type)",
            }
        }
    }

    #[inline]
    fn is_code(&self) -> bool {
        let hdr = self.phdr();
        // 0x1 is PT_X for executable
        hdr.p_type == libc::PT_LOAD && (hdr.p_flags & 0x1) != 0
    }

    #[inline]
    fn is_load(&self) -> bool {
        self.phdr().p_type == libc::PT_LOAD
    }

    #[inline]
    fn stated_virtual_memory_address(&self) -> Svma {
        Svma(self.phdr().p_vaddr as _)
    }

    #[inline]
    fn len(&self) -> usize {
        self.phdr().p_memsz as _
    }
}

/// An iterator of mapped segments in a shared library.
pub struct SegmentIter<'a> {
    inner: std::slice::Iter<'a, Phdr>,
}

impl<'a> Iterator for SegmentIter<'a> {
    type Item = Segment<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|phdr| Segment {
            phdr: phdr,
            shlib: PhantomData,
        })
    }
}

impl<'a> fmt::Debug for SegmentIter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let ref phdr = self.inner.as_slice()[0];

        f.debug_struct("SegmentIter")
            .field("phdr", &DebugPhdr(phdr))
            .finish()
    }
}

/// A shared library on Linux.
pub struct SharedLibrary<'a> {
    size: usize,
    addr: *const u8,
    name: Cow<'a, CStr>,
    headers: &'a [Phdr],
}

struct IterState<F> {
    f: F,
    panic: Option<Box<dyn Any + Send>>,
    idx: usize,
}

const CONTINUE: libc::c_int = 0;
const BREAK: libc::c_int = 1;

impl<'a> SharedLibrary<'a> {
    unsafe fn new(info: &'a libc::dl_phdr_info, size: usize, is_first_lib: bool) -> Self {
        // try to get the name from the dl_phdr_info.  If that fails there are two
        // cases we can and need to deal with.  The first one is if we are the first
        // loaded library in which case the name is the executable which we can
        // discover via env::current_exe (reads the proc/self symlink).
        //
        // Otherwise if we have a no name we might be a dylib that was loaded with
        // dlopen in which case we can use dladdr to recover the name.
        let mut name = Cow::Borrowed(if info.dlpi_name.is_null() {
            CStr::from_bytes_with_nul_unchecked(b"\0")
        } else {
            CStr::from_ptr(info.dlpi_name)
        });
        if name.to_bytes().is_empty() {
            if is_first_lib {
                if let Ok(exe) = current_exe() {
                    name = Cow::Owned(CString::from_vec_unchecked(exe.into_os_string().into_vec()));
                }
            } else {
                let mut dlinfo: libc::Dl_info = mem::zeroed();
                if libc::dladdr(info.dlpi_addr as *const libc::c_void, &mut dlinfo) != 0 {
                    name = Cow::Owned(CString::from(CStr::from_ptr(dlinfo.dli_fname)));
                }
            }
        }

        SharedLibrary {
            size: size,
            addr: info.dlpi_addr as usize as *const _,
            name,
            headers: slice::from_raw_parts(info.dlpi_phdr, info.dlpi_phnum as usize),
        }
    }

    unsafe extern "C" fn callback<F, C>(
        info: *mut libc::dl_phdr_info,
        size: usize,
        state: *mut libc::c_void,
    ) -> libc::c_int
    where
        F: FnMut(&Self) -> C,
        C: Into<IterationControl>,
    {
        if (*info).dlpi_phdr.is_null() {
            return CONTINUE;
        }

        let state = &mut *(state as *mut IterState<F>);
        state.idx += 1;

        match panic::catch_unwind(panic::AssertUnwindSafe(|| {
            let info = info.as_ref().unwrap();
            let shlib = SharedLibrary::new(info, size, state.idx == 1);

            (state.f)(&shlib).into()
        })) {
            Ok(IterationControl::Continue) => CONTINUE,
            Ok(IterationControl::Break) => BREAK,
            Err(panicked) => {
                state.panic = Some(panicked);
                BREAK
            }
        }
    }

    fn note_segments(&self) -> impl Iterator<Item = Segment<'a>> {
        self.segments().filter(|s| s.is_note())
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
        // Search for `PT_NOTE` segments, containing auxilliary information.
        // Such segments contain a series of "notes" and one kind of note is
        // `NT_GNU_BUILD_ID`, whose payload contains a unique identifier
        // generated by the linker. Return the first one we find, if any.
        for segment in self.note_segments() {
            for (note_type, note_name, note_descriptor) in unsafe { segment.notes(self) } {
                if note_type == NT_GNU_BUILD_ID && note_name == b"GNU\0" {
                    return Some(SharedLibraryId::GnuBuildId(note_descriptor.to_vec()));
                }
            }
        }

        None
    }

    #[inline]
    fn segments(&self) -> Self::SegmentIter {
        SegmentIter {
            inner: self.headers.iter(),
        }
    }

    #[inline]
    fn virtual_memory_bias(&self) -> Bias {
        Bias(self.addr as usize)
    }

    #[inline]
    fn each<F, C>(f: F)
    where
        F: FnMut(&Self) -> C,
        C: Into<IterationControl>,
    {
        let mut state = IterState {
            f: f,
            panic: None,
            idx: 0,
        };

        unsafe {
            libc::dl_iterate_phdr(Some(Self::callback::<F, C>), &mut state as *mut _ as *mut _);
        }

        if let Some(panic) = state.panic {
            panic::resume_unwind(panic);
        }
    }
}

impl<'a> fmt::Debug for SharedLibrary<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "SharedLibrary {{ size: {:?}, addr: {:?}, ",
            self.size, self.addr
        )?;
        write!(f, "name: {:?}, headers: [", self.name)?;

        // Debug does not usually have a trailing comma in the list,
        // last element must be formatted separately.
        let l = self.headers.len();
        self.headers[..(l - 1)]
            .into_iter()
            .map(|phdr| write!(f, "{:?}, ", &DebugPhdr(phdr)))
            .collect::<fmt::Result>()?;

        write!(f, "{:?}", &DebugPhdr(&self.headers[l - 1]))?;

        write!(f, "] }}")
    }
}

struct DebugPhdr<'a>(&'a Phdr);

impl<'a> fmt::Debug for DebugPhdr<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let phdr = self.0;

        // The layout is different for 32-bit vs 64-bit,
        // but since the fields are the same, it shouldn't matter much.
        f.debug_struct("Phdr")
            .field("p_type", &phdr.p_type)
            .field("p_flags", &phdr.p_flags)
            .field("p_offset", &phdr.p_offset)
            .field("p_vaddr", &phdr.p_vaddr)
            .field("p_paddr", &phdr.p_paddr)
            .field("p_filesz", &phdr.p_filesz)
            .field("p_memsz", &phdr.p_memsz)
            .field("p_align", &phdr.p_align)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use crate::linux;
    use crate::{IterationControl, Segment, SharedLibrary};

    #[test]
    fn have_libc() {
        let mut found_libc = false;
        linux::SharedLibrary::each(|info| {
            found_libc |= info
                .name
                .to_bytes()
                .split(|c| *c == b'.' || *c == b'/')
                .find(|s| s == b"libc")
                .is_some();
        });
        assert!(found_libc);
    }

    #[test]
    fn can_break() {
        let mut first_count = 0;
        linux::SharedLibrary::each(|_| {
            first_count += 1;
        });
        assert!(first_count > 2);

        let mut second_count = 0;
        linux::SharedLibrary::each(|_| {
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
        use std::ffi::OsStr;
        let mut names = vec![];
        linux::SharedLibrary::each(|shlib| {
            println!("{:?}", shlib);
            let name = shlib.name();
            if name != OsStr::new("") {
                names.push(name.to_str().unwrap().to_string());
            }
        });

        assert!(names.iter().any(|x| x.contains("findshlibs")));
        assert!(names.iter().any(|x| x.contains("libc.so")));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn get_id() {
        use std::path::Path;
        use std::process::Command;

        linux::SharedLibrary::each(|shlib| {
            let name = shlib.name();
            let id = shlib.id();
            if id.is_none() {
                println!("no id found for {:?}", name);
                return;
            }
            let path: &Path = name.as_ref();
            if !path.is_absolute() {
                return;
            }
            let gnu_build_id = id.unwrap().to_string();
            let readelf = Command::new("readelf")
                .arg("-n")
                .arg(path)
                .output()
                .unwrap();
            for line in String::from_utf8(readelf.stdout).unwrap().lines() {
                if let Some(index) = line.find("Build ID: ") {
                    let readelf_build_id = line[index + 9..].trim();
                    assert_eq!(readelf_build_id, gnu_build_id);
                }
            }
            println!("{}: {}", path.display(), gnu_build_id);
        });
    }

    #[test]
    fn have_load_segment() {
        linux::SharedLibrary::each(|shlib| {
            println!("shlib = {:?}", shlib.name());

            let mut found_load = false;
            for seg in shlib.segments() {
                println!("    segment = {:?}", seg.name());

                found_load |= seg.name() == "LOAD";
            }
            assert!(found_load);
        });
    }
}
