use core::fmt::{self, Write};
use core::mem::{size_of, transmute};
use core::slice::from_raw_parts;
use libc::c_char;

unsafe extern "C" {
    // dl_iterate_phdr takes a callback that will receive a dl_phdr_info pointer
    // for every DSO that has been linked into the process. dl_iterate_phdr also
    // ensures that the dynamic linker is locked from start to finish of the
    // iteration. If the callback returns a non-zero value the iteration is
    // terminated early. 'data' will be passed as the third argument to the
    // callback on each call. 'size' gives the size of the dl_phdr_info.
    #[allow(improper_ctypes)]
    fn dl_iterate_phdr(
        f: extern "C" fn(info: &dl_phdr_info, size: usize, data: &mut DsoPrinter<'_, '_>) -> i32,
        data: &mut DsoPrinter<'_, '_>,
    ) -> i32;
}

// We need to parse out the build ID and some basic program header data
// which means that we need a bit of stuff from the ELF spec as well.

const PT_LOAD: u32 = 1;
const PT_NOTE: u32 = 4;

// Now we have to replicate, bit for bit, the structure of the dl_phdr_info
// type used by fuchsia's current dynamic linker. Chromium also has this ABI
// boundary as well as crashpad. Eventually we'd like to move these cases to
// use elf-search but we'd need to provide that in the SDK and that has not
// yet been done. Thus we (and they) are stuck having to use this method
// which incurs a tight coupling with the fuchsia libc.

#[allow(non_camel_case_types)]
#[repr(C)]
struct dl_phdr_info {
    addr: *const u8,
    name: *const c_char,
    phdr: *const Elf_Phdr,
    phnum: u16,
    adds: u64,
    subs: u64,
    tls_modid: usize,
    tls_data: *const u8,
}

impl dl_phdr_info {
    fn program_headers(&self) -> PhdrIter<'_> {
        PhdrIter {
            phdrs: self.phdr_slice(),
            base: self.addr,
        }
    }
    // We have no way of knowing of checking if e_phoff and e_phnum are valid.
    // libc should ensure this for us however so it's safe to form a slice here.
    fn phdr_slice(&self) -> &[Elf_Phdr] {
        unsafe { from_raw_parts(self.phdr, self.phnum as usize) }
    }
}

struct PhdrIter<'a> {
    phdrs: &'a [Elf_Phdr],
    base: *const u8,
}

impl<'a> Iterator for PhdrIter<'a> {
    type Item = Phdr<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        self.phdrs.split_first().map(|(phdr, new_phdrs)| {
            self.phdrs = new_phdrs;
            Phdr {
                phdr,
                base: self.base,
            }
        })
    }
}

// Elf_Phdr represents a 64-bit ELF program header in the endianness of the target
// architecture.
#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
#[repr(C)]
struct Elf_Phdr {
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_paddr: u64,
    p_filesz: u64,
    p_memsz: u64,
    p_align: u64,
}

// Phdr represents a valid ELF program header and its contents.
struct Phdr<'a> {
    phdr: &'a Elf_Phdr,
    base: *const u8,
}

impl<'a> Phdr<'a> {
    // We have no way of checking if p_addr or p_memsz are valid. Fuchsia's libc
    // parses the notes first however so by virtue of being here these headers
    // must be valid. NoteIter does not require the underlying data to be valid
    // but it does require the bounds to be valid. We trust that libc has ensured
    // that this is the case for us here.
    fn notes(&self) -> NoteIter<'a> {
        unsafe {
            NoteIter::new(
                self.base.add(self.phdr.p_offset as usize),
                self.phdr.p_memsz as usize,
            )
        }
    }
}

// The note type for build IDs.
const NT_GNU_BUILD_ID: u32 = 3;

// Elf_Nhdr represents an ELF note header in the endianness of the target.
#[allow(non_camel_case_types)]
#[repr(C)]
struct Elf_Nhdr {
    n_namesz: u32,
    n_descsz: u32,
    n_type: u32,
}

// Note represents an ELF note (header + contents). The name is left as a u8
// slice because it is not always null terminated and rust makes it easy enough
// to check that the bytes match eitherway.
struct Note<'a> {
    name: &'a [u8],
    desc: &'a [u8],
    tipe: u32,
}

// NoteIter lets you safely iterate over a note segment. It terminates as soon
// as an error occurs or there are no more notes. If you iterate over invalid
// data it will function as though no notes were found.
struct NoteIter<'a> {
    base: &'a [u8],
    error: bool,
}

impl<'a> NoteIter<'a> {
    // It is an invariant of function that the pointer and size given denote a
    // valid range of bytes that can all be read. The contents of these bytes
    // can be anything but the range must be valid for this to be safe.
    unsafe fn new(base: *const u8, size: usize) -> Self {
        NoteIter {
            base: unsafe { from_raw_parts(base, size) },
            error: false,
        }
    }
}

// align_to aligns 'x' to 'to'-byte alignment assuming 'to' is a power of 2.
// This follows a standard pattern in C/C++ ELF parsing code where
// (x + to - 1) & -to is used. Rust does not let you negate usize so I use
// 2's-complement conversion to recreate that.
fn align_to(x: usize, to: usize) -> usize {
    (x + to - 1) & (!to + 1)
}

// take_bytes_align4 consumes num bytes from the slice (if present) and
// additionally ensures that the final slice is properlly aligned. If an
// either the number of bytes requested is too large or the slice can't be
// realigned afterwards due to not enough remaining bytes existing, None is
// returned and the slice is not modified.
fn take_bytes_align4<'a>(num: usize, bytes: &mut &'a [u8]) -> Option<&'a [u8]> {
    if bytes.len() < align_to(num, 4) {
        return None;
    }
    let (out, bytes_new) = bytes.split_at(num);
    *bytes = &bytes_new[align_to(num, 4) - num..];
    Some(out)
}

// This function has no real invariants the caller must uphold other than
// perhaps that 'bytes' should be aligned for performance (and on some
// architectures correctness). The values in the Elf_Nhdr fields might
// be nonsense but this function ensures no such thing.
fn take_nhdr<'a>(bytes: &mut &'a [u8]) -> Option<&'a Elf_Nhdr> {
    if size_of::<Elf_Nhdr>() > bytes.len() {
        return None;
    }
    // This is safe as long as there is enough space and we just confirmed that
    // in the if statement above so this should not be unsafe.
    let out = unsafe { transmute::<*const u8, &'a Elf_Nhdr>(bytes.as_ptr()) };
    // Note that sice_of::<Elf_Nhdr>() is always 4-byte aligned.
    *bytes = &bytes[size_of::<Elf_Nhdr>()..];
    Some(out)
}

impl<'a> Iterator for NoteIter<'a> {
    type Item = Note<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        // Check if we've reached the end.
        if self.base.is_empty() || self.error {
            return None;
        }
        // We transmute out an nhdr but we carefully consider the resulting
        // struct. We don't trust the namesz or descsz and we make no unsafe
        // decisions based on the type. So even if we get out complete garbage
        // we should still be safe.
        let nhdr = take_nhdr(&mut self.base)?;
        let name = take_bytes_align4(nhdr.n_namesz as usize, &mut self.base)?;
        let desc = take_bytes_align4(nhdr.n_descsz as usize, &mut self.base)?;
        Some(Note {
            name: name,
            desc: desc,
            tipe: nhdr.n_type,
        })
    }
}

struct Perm(u32);

/// Indicates that a segment is executable.
const PERM_X: u32 = 0b00000001;
/// Indicates that a segment is writable.
const PERM_W: u32 = 0b00000010;
/// Indicates that a segment is readable.
const PERM_R: u32 = 0b00000100;

impl core::fmt::Display for Perm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let v = self.0;
        if v & PERM_R != 0 {
            f.write_char('r')?
        }
        if v & PERM_W != 0 {
            f.write_char('w')?
        }
        if v & PERM_X != 0 {
            f.write_char('x')?
        }
        Ok(())
    }
}

/// Represents an ELF segment at runtime.
struct Segment {
    /// Gives the runtime virtual address of this segment's contents.
    addr: usize,
    /// Gives the memory size of this segment's contents.
    size: usize,
    /// Gives the module virtual address of this segment with the ELF file.
    mod_rel_addr: usize,
    /// Gives the permissions found in the ELF file. These permissions are not
    /// necessarily the permissions present at runtime however.
    flags: Perm,
}

/// Lets one iterate over Segments from a DSO.
struct SegmentIter<'a> {
    phdrs: &'a [Elf_Phdr],
    base: usize,
}

impl Iterator for SegmentIter<'_> {
    type Item = Segment;

    fn next(&mut self) -> Option<Self::Item> {
        self.phdrs.split_first().and_then(|(phdr, new_phdrs)| {
            self.phdrs = new_phdrs;
            if phdr.p_type != PT_LOAD {
                self.next()
            } else {
                Some(Segment {
                    addr: phdr.p_vaddr as usize + self.base,
                    size: phdr.p_memsz as usize,
                    mod_rel_addr: phdr.p_vaddr as usize,
                    flags: Perm(phdr.p_flags),
                })
            }
        })
    }
}

/// Represents an ELF DSO (Dynamic Shared Object). This type references
/// the data stored in the actual DSO rather than making its own copy.
struct Dso<'a> {
    /// The dynamic linker always gives us a name, even if the name is empty.
    /// In the case of the main executable this name will be empty. In the case
    /// of a shared object it will be the soname (see DT_SONAME).
    name: &'a str,
    /// On Fuchsia virtually all binaries have build IDs but this is not a strict
    /// requirement. There's no way to match up DSO information with a real ELF
    /// file afterwards if there is no build_id so we require that every DSO
    /// have one here. DSO's without a build_id are ignored.
    build_id: &'a [u8],

    base: usize,
    phdrs: &'a [Elf_Phdr],
}

impl Dso<'_> {
    /// Returns an iterator over Segments in this DSO.
    fn segments(&self) -> SegmentIter<'_> {
        SegmentIter {
            phdrs: self.phdrs.as_ref(),
            base: self.base,
        }
    }
}

struct HexSlice<'a> {
    bytes: &'a [u8],
}

impl fmt::Display for HexSlice<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.bytes {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

fn get_build_id<'a>(info: &'a dl_phdr_info) -> Option<&'a [u8]> {
    for phdr in info.program_headers() {
        if phdr.phdr.p_type == PT_NOTE {
            for note in phdr.notes() {
                if note.tipe == NT_GNU_BUILD_ID && (note.name == b"GNU\0" || note.name == b"GNU") {
                    return Some(note.desc);
                }
            }
        }
    }
    None
}

/// These errors encode issues that arise while parsing information about
/// each DSO.
enum Error {
    /// NameError means that an error occurred while converting a C style string
    /// into a rust string.
    NameError,
    /// BuildIDError means that we didn't find a build ID. This could either be
    /// because the DSO had no build ID or because the segment containing the
    /// build ID was malformed.
    BuildIDError,
}

/// Calls either 'dso' or 'error' for each DSO linked into the process by the
/// dynamic linker.
///
/// # Arguments
///
/// * `visitor` - A DsoPrinter that will have one of eats methods called foreach DSO.
fn for_each_dso(mut visitor: &mut DsoPrinter<'_, '_>) {
    extern "C" fn callback(
        info: &dl_phdr_info,
        _size: usize,
        visitor: &mut DsoPrinter<'_, '_>,
    ) -> i32 {
        // dl_iterate_phdr ensures that info.name will point to a valid
        // location.
        let name_len = unsafe { libc::strlen(info.name) };
        let name_slice: &[u8] =
            unsafe { core::slice::from_raw_parts(info.name.cast::<u8>(), name_len) };
        let name = match core::str::from_utf8(name_slice) {
            Ok(name) => name,
            Err(_) => {
                return visitor.error(Error::NameError) as i32;
            }
        };
        let build_id = match get_build_id(info) {
            Some(build_id) => build_id,
            None => {
                return visitor.error(Error::BuildIDError) as i32;
            }
        };
        visitor.dso(Dso {
            name: name,
            build_id: build_id,
            phdrs: info.phdr_slice(),
            base: info.addr as usize,
        }) as i32
    }
    unsafe { dl_iterate_phdr(callback, &mut visitor) };
}

struct DsoPrinter<'a, 'b> {
    writer: &'a mut core::fmt::Formatter<'b>,
    module_count: usize,
    error: core::fmt::Result,
}

impl DsoPrinter<'_, '_> {
    fn dso(&mut self, dso: Dso<'_>) -> bool {
        let mut write = || {
            write!(
                self.writer,
                "{{{{{{module:{:#x}:{}:elf:{}}}}}}}\n",
                self.module_count,
                dso.name,
                HexSlice {
                    bytes: dso.build_id.as_ref()
                }
            )?;
            for seg in dso.segments() {
                write!(
                    self.writer,
                    "{{{{{{mmap:{:#x}:{:#x}:load:{:#x}:{}:{:#x}}}}}}}\n",
                    seg.addr, seg.size, self.module_count, seg.flags, seg.mod_rel_addr
                )?;
            }
            self.module_count += 1;
            Ok(())
        };
        match write() {
            Ok(()) => false,
            Err(err) => {
                self.error = Err(err);
                true
            }
        }
    }
    fn error(&mut self, _error: Error) -> bool {
        false
    }
}

/// This function prints the Fuchsia symbolizer markup for all information contained in a DSO.
pub fn print_dso_context(out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    out.write_str("{{{reset:begin}}}\n")?;
    let mut visitor = DsoPrinter {
        writer: out,
        module_count: 0,
        error: Ok(()),
    };
    for_each_dso(&mut visitor);
    visitor.error
}

/// This function prints the Fuchsia symbolizer markup to end the backtrace.
pub fn finish_context(out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    out.write_str("{{{reset:end}}}\n")
}
