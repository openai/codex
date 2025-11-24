//! Windows-specific implementation of the `SharedLibrary` trait.

use crate::Segment as SegmentTrait;
use crate::SharedLibrary as SharedLibraryTrait;
use crate::{Bias, IterationControl, SharedLibraryId, Svma};

use std::ffi::{CStr, OsStr, OsString};
use std::fmt;
use std::marker::PhantomData;
use std::mem;
use std::os::windows::ffi::OsStringExt;
use std::ptr;
use std::slice;
use std::usize;

use winapi::ctypes::c_char;
use winapi::shared::minwindef::{HMODULE, MAX_PATH};
use winapi::um::libloaderapi::{FreeLibrary, LoadLibraryExW, LOAD_LIBRARY_AS_DATAFILE};
use winapi::um::memoryapi::VirtualQuery;
use winapi::um::processthreadsapi::GetCurrentProcess;
use winapi::um::psapi::{
    EnumProcessModules, GetModuleFileNameExW, GetModuleInformation, MODULEINFO,
};
use winapi::um::winnt::{
    IMAGE_DEBUG_DIRECTORY, IMAGE_DEBUG_TYPE_CODEVIEW, IMAGE_DIRECTORY_ENTRY_DEBUG,
    IMAGE_DOS_HEADER, IMAGE_DOS_SIGNATURE, IMAGE_NT_HEADERS, IMAGE_NT_SIGNATURE,
    IMAGE_SCN_CNT_CODE, IMAGE_SECTION_HEADER, MEMORY_BASIC_INFORMATION, MEM_COMMIT,
};

// This is 'RSDS'.
const CV_SIGNATURE: u32 = 0x5344_5352;

/// An unsupported segment
pub struct Segment<'a> {
    section: &'a IMAGE_SECTION_HEADER,
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
        std::str::from_utf8(&self.section.Name)
            .unwrap_or("")
            .trim_end_matches('\0')
    }

    fn is_code(&self) -> bool {
        (self.section.Characteristics & IMAGE_SCN_CNT_CODE) != 0
    }

    #[inline]
    fn stated_virtual_memory_address(&self) -> Svma {
        Svma(self.section.VirtualAddress as usize)
    }

    #[inline]
    fn len(&self) -> usize {
        *unsafe { self.section.Misc.VirtualSize() } as usize
    }
}

/// An iterator over PE sections.
pub struct SegmentIter<'a> {
    sections: std::slice::Iter<'a, IMAGE_SECTION_HEADER>,
}

impl<'a> fmt::Debug for SegmentIter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SegmentIter").finish()
    }
}

impl<'a> Iterator for SegmentIter<'a> {
    type Item = Segment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.sections.next().map(|section| Segment { section })
    }
}

#[repr(C)]
struct CodeViewRecord70 {
    signature: u32,
    pdb_signature: [u8; 16],
    pdb_age: u32,
    // This struct has a flexible array containing a UTF-8 \0-terminated string.
    // This is only represented by its first byte here.
    pdb_filename: c_char,
}

/// A shared library on Windows.
pub struct SharedLibrary<'a> {
    module_info: MODULEINFO,
    module_name: OsString,
    phantom: PhantomData<&'a ()>,
}

impl<'a> fmt::Debug for SharedLibrary<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SharedLibrary")
            .field("module_base", &self.module_base())
            .field("name", &self.name())
            .field("debug_name", &self.debug_name())
            .field("id", &self.id())
            .field("debug_id", &self.debug_id())
            .finish()
    }
}

impl<'a> SharedLibrary<'a> {
    fn new(module_info: MODULEINFO, module_name: OsString) -> SharedLibrary<'a> {
        SharedLibrary {
            module_info,
            module_name,
            phantom: PhantomData,
        }
    }

    #[inline]
    fn module_base(&self) -> *const c_char {
        self.module_info.lpBaseOfDll as *const c_char
    }

    fn dos_header(&self) -> Option<&IMAGE_DOS_HEADER> {
        let header: &IMAGE_DOS_HEADER = unsafe { &*(self.module_base() as *const _) };
        if header.e_magic == IMAGE_DOS_SIGNATURE {
            Some(header)
        } else {
            None
        }
    }

    fn nt_headers(&self) -> Option<&IMAGE_NT_HEADERS> {
        self.dos_header().and_then(|dos_header| {
            let nt_headers: &IMAGE_NT_HEADERS =
                unsafe { &*(self.module_base().offset(dos_header.e_lfanew as isize) as *const _) };
            if nt_headers.Signature == IMAGE_NT_SIGNATURE {
                Some(nt_headers)
            } else {
                None
            }
        })
    }

    fn debug_directories(&self) -> &[IMAGE_DEBUG_DIRECTORY] {
        self.nt_headers().map_or(&[], |nt_headers| {
            if nt_headers.OptionalHeader.NumberOfRvaAndSizes <= IMAGE_DIRECTORY_ENTRY_DEBUG as u32 {
                return &[];
            }
            let data_dir =
                nt_headers.OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_DEBUG as usize];
            if data_dir.VirtualAddress == 0 {
                return &[];
            }
            let size = data_dir.Size as usize;
            if size % mem::size_of::<IMAGE_DEBUG_DIRECTORY>() != 0 {
                return &[];
            }
            let nb_dirs = size / mem::size_of::<IMAGE_DEBUG_DIRECTORY>();
            unsafe {
                slice::from_raw_parts(
                    self.module_base().offset(data_dir.VirtualAddress as isize) as *const _,
                    nb_dirs,
                )
            }
        })
    }

    fn codeview_record70(&self) -> Option<&CodeViewRecord70> {
        self.debug_directories().iter().find_map(|debug_directory| {
            if debug_directory.Type != IMAGE_DEBUG_TYPE_CODEVIEW {
                return None;
            }

            let debug_info: &CodeViewRecord70 = unsafe {
                &*(self
                    .module_base()
                    .offset(debug_directory.AddressOfRawData as isize)
                    as *const _)
            };
            if debug_info.signature == CV_SIGNATURE {
                Some(debug_info)
            } else {
                None
            }
        })
    }
}

impl<'a> SharedLibraryTrait for SharedLibrary<'a> {
    type Segment = Segment<'a>;
    type SegmentIter = SegmentIter<'a>;

    #[inline]
    fn name(&self) -> &OsStr {
        &self.module_name
    }

    #[inline]
    fn debug_name(&self) -> Option<&OsStr> {
        self.codeview_record70().and_then(|codeview| {
            let cstr = unsafe { CStr::from_ptr(&codeview.pdb_filename as *const _) };
            if let Ok(s) = cstr.to_str() {
                Some(OsStr::new(s))
            } else {
                None
            }
        })
    }

    fn id(&self) -> Option<SharedLibraryId> {
        self.nt_headers().map(|nt_headers| {
            SharedLibraryId::PeSignature(
                nt_headers.FileHeader.TimeDateStamp,
                nt_headers.OptionalHeader.SizeOfImage,
            )
        })
    }

    #[inline]
    fn debug_id(&self) -> Option<SharedLibraryId> {
        self.codeview_record70()
            .map(|codeview| SharedLibraryId::PdbSignature(codeview.pdb_signature, codeview.pdb_age))
    }

    fn segments(&self) -> Self::SegmentIter {
        let sections = self.nt_headers().map(|nt_headers| unsafe {
            let base =
                (nt_headers as *const _ as *const u8).add(mem::size_of::<IMAGE_NT_HEADERS>());
            slice::from_raw_parts(
                base as *const IMAGE_SECTION_HEADER,
                nt_headers.FileHeader.NumberOfSections as usize,
            )
        });
        SegmentIter {
            sections: sections.unwrap_or(&[][..]).iter(),
        }
    }

    #[inline]
    fn virtual_memory_bias(&self) -> Bias {
        Bias(self.module_base() as usize)
    }

    fn each<F, C>(mut f: F)
    where
        F: FnMut(&Self) -> C,
        C: Into<IterationControl>,
    {
        let proc = unsafe { GetCurrentProcess() };
        let mut modules_size = 0;
        unsafe {
            if EnumProcessModules(proc, ptr::null_mut(), 0, &mut modules_size) == 0 {
                return;
            }
        }
        let module_count = modules_size / mem::size_of::<HMODULE>() as u32;
        let mut modules = vec![unsafe { mem::zeroed() }; module_count as usize];
        unsafe {
            if EnumProcessModules(proc, modules.as_mut_ptr(), modules_size, &mut modules_size) == 0
            {
                return;
            }
        }

        modules.truncate(modules_size as usize / mem::size_of::<HMODULE>());

        for module in modules {
            unsafe {
                let mut module_path = vec![0u16; MAX_PATH + 1];
                let module_path_len = GetModuleFileNameExW(
                    proc,
                    module,
                    module_path.as_mut_ptr(),
                    MAX_PATH as u32 + 1,
                ) as usize;
                if module_path_len == 0 {
                    continue;
                }

                let mut module_info = mem::zeroed();
                if GetModuleInformation(
                    proc,
                    module,
                    &mut module_info,
                    mem::size_of::<MODULEINFO>() as u32,
                ) == 0
                {
                    continue;
                }

                // to prevent something else from unloading the module while
                // we're poking around in memory we load it a second time.  This
                // will effectively just increment the refcount since it has been
                // loaded before.
                let handle_lock = LoadLibraryExW(
                    module_path.as_ptr(),
                    ptr::null_mut(),
                    LOAD_LIBRARY_AS_DATAFILE,
                );

                let mut vmem_info = mem::zeroed();
                let mut should_break = false;
                if VirtualQuery(
                    module_info.lpBaseOfDll,
                    &mut vmem_info,
                    mem::size_of::<MEMORY_BASIC_INFORMATION>(),
                ) == mem::size_of::<MEMORY_BASIC_INFORMATION>()
                {
                    let module_path = OsString::from_wide(&module_path[..module_path_len]);
                    if vmem_info.State == MEM_COMMIT {
                        let shlib = SharedLibrary::new(module_info, module_path);
                        match f(&shlib).into() {
                            IterationControl::Break => should_break = true,
                            IterationControl::Continue => {}
                        }
                    }
                }

                FreeLibrary(handle_lock);

                if should_break {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{IterationControl, Segment, SharedLibrary};
    use crate::windows;

    #[test]
    fn can_break() {
        let mut first_count = 0;
        windows::SharedLibrary::each(|_| {
            first_count += 1;
        });
        assert!(first_count > 2);

        let mut second_count = 0;
        windows::SharedLibrary::each(|_| {
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
        windows::SharedLibrary::each(|shlib| {
            let _ = shlib.name();
            assert!(shlib.debug_name().is_some());
        });
    }

    #[test]
    fn have_code() {
        windows::SharedLibrary::each(|shlib| {
            println!("shlib = {:?}", shlib.name());

            let mut found_code = false;
            for seg in shlib.segments() {
                println!("    segment = {:?}", seg.name());
                if seg.is_code() {
                    found_code = true;
                }
            }
            assert!(found_code);
        });
    }

    #[test]
    fn get_id() {
        windows::SharedLibrary::each(|shlib| {
            assert!(shlib.id().is_some());
            assert!(shlib.debug_id().is_some());
        });
    }
}
