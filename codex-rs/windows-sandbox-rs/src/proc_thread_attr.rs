use std::ffi::c_void;
use std::io;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::System::Threading::DeleteProcThreadAttributeList;
use windows_sys::Win32::System::Threading::InitializeProcThreadAttributeList;
use windows_sys::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST;
use windows_sys::Win32::System::Threading::UpdateProcThreadAttribute;

const PROC_THREAD_ATTRIBUTE_HANDLE_LIST: usize = 0x0002_0002;
const PROC_THREAD_ATTRIBUTE_JOB_LIST: usize = 0x0002_000D;
const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: usize = 0x0002_0016;

pub struct ProcThreadAttributeList {
    buffer: Vec<u8>,
    handle_list: Vec<HANDLE>,
    job_list: Vec<HANDLE>,
}

impl ProcThreadAttributeList {
    pub fn new(attr_count: u32) -> io::Result<Self> {
        let mut size: usize = 0;
        unsafe {
            InitializeProcThreadAttributeList(std::ptr::null_mut(), attr_count, 0, &mut size);
        }
        if size == 0 {
            return Err(io::Error::from_raw_os_error(unsafe {
                GetLastError() as i32
            }));
        }
        let mut buffer = vec![0u8; size];
        let list = buffer.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
        let ok = unsafe { InitializeProcThreadAttributeList(list, attr_count, 0, &mut size) };
        if ok == 0 {
            return Err(io::Error::from_raw_os_error(unsafe {
                GetLastError() as i32
            }));
        }
        Ok(Self {
            buffer,
            handle_list: Vec::new(),
            job_list: Vec::new(),
        })
    }

    pub fn as_mut_ptr(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.buffer.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST
    }

    pub fn set_pseudoconsole(&mut self, hpc: isize) -> io::Result<()> {
        // SAFETY: `hpc` is the Windows-defined value and size for this attribute.
        unsafe {
            self.update(
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
                hpc as *mut c_void,
                std::mem::size_of::<HANDLE>(),
            )
        }
    }

    pub fn set_handle_list(&mut self, handles: Vec<HANDLE>) -> io::Result<()> {
        self.handle_list = handles;
        let value = self.handle_list.as_mut_ptr().cast();
        let size = std::mem::size_of_val(self.handle_list.as_slice());
        // SAFETY: `value` points to `self.handle_list`, which remains alive
        // while the attribute list can reference it, and `size` covers that slice.
        unsafe { self.update(PROC_THREAD_ATTRIBUTE_HANDLE_LIST, value, size) }
    }

    pub fn set_job(&mut self, job: HANDLE) -> io::Result<()> {
        // Sandboxed processes must enter the job atomically. If Windows cannot
        // honor the job list (for example, because a parent job forbids
        // nesting), fail the spawn rather than briefly run an uncontained
        // sandbox process tree.
        self.job_list = vec![job];
        let value = self.job_list.as_mut_ptr().cast();
        let size = std::mem::size_of_val(self.job_list.as_slice());
        // SAFETY: `value` points to `self.job_list`, which remains alive while
        // the attribute list can reference it, and `size` covers that slice.
        unsafe { self.update(PROC_THREAD_ATTRIBUTE_JOB_LIST, value, size) }
    }

    unsafe fn update(
        &mut self,
        attribute: usize,
        value: *mut c_void,
        size: usize,
    ) -> io::Result<()> {
        let ok = unsafe {
            UpdateProcThreadAttribute(
                self.as_mut_ptr(),
                0,
                attribute,
                value,
                size,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::from_raw_os_error(unsafe {
                GetLastError() as i32
            }));
        }
        Ok(())
    }
}

impl Drop for ProcThreadAttributeList {
    fn drop(&mut self) {
        unsafe {
            DeleteProcThreadAttributeList(self.as_mut_ptr());
        }
    }
}
