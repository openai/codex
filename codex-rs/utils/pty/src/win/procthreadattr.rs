#![allow(clippy::uninit_vec)]

// This file is copied from https://github.com/wezterm/wezterm (MIT license).
// Copyright (c) 2018-Present Wez Furlong
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use super::psuedocon::HPCON;
use anyhow::Error;
use anyhow::ensure;
use std::ffi::c_void;
use std::io::Error as IoError;
use std::mem;
use std::ptr;
use winapi::shared::minwindef::DWORD;
use winapi::um::processthreadsapi::*;
use winapi::um::winnt::HANDLE;

const PROC_THREAD_ATTRIBUTE_JOB_LIST: usize = 0x0002000D;
const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: usize = 0x00020016;

pub struct ProcThreadAttributeList {
    data: Vec<u8>,
    job_list: Vec<HANDLE>,
}

impl ProcThreadAttributeList {
    pub fn with_capacity(num_attributes: DWORD) -> Result<Self, Error> {
        let mut bytes_required: usize = 0;
        unsafe {
            InitializeProcThreadAttributeList(
                ptr::null_mut(),
                num_attributes,
                0,
                &mut bytes_required,
            )
        };
        let mut data = Vec::with_capacity(bytes_required);
        unsafe { data.set_len(bytes_required) };

        let attr_ptr = data.as_mut_slice().as_mut_ptr() as *mut _;
        let res = unsafe {
            InitializeProcThreadAttributeList(attr_ptr, num_attributes, 0, &mut bytes_required)
        };
        ensure!(
            res != 0,
            "InitializeProcThreadAttributeList failed: {}",
            IoError::last_os_error()
        );
        Ok(Self {
            data,
            job_list: Vec::new(),
        })
    }

    pub fn as_mut_ptr(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.data.as_mut_slice().as_mut_ptr() as *mut _
    }

    pub fn set_pty(&mut self, con: HPCON) -> Result<(), Error> {
        // SAFETY: `con` is the Windows-defined value and size for this attribute.
        unsafe {
            self.update(
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
                con,
                mem::size_of::<HPCON>(),
            )
        }
    }

    pub fn set_job(&mut self, job: HANDLE) -> Result<(), Error> {
        // Atomic job attachment is intentionally required for native ConPTY
        // spawns. If Windows cannot honor the job list (for example, because a
        // parent job forbids nesting), fail the spawn rather than briefly run
        // an uncontained process tree.
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
    ) -> Result<(), Error> {
        let res = unsafe {
            UpdateProcThreadAttribute(
                self.as_mut_ptr(),
                0,
                attribute,
                value,
                size,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        ensure!(
            res != 0,
            "UpdateProcThreadAttribute failed: {}",
            IoError::last_os_error()
        );
        Ok(())
    }
}

impl Drop for ProcThreadAttributeList {
    fn drop(&mut self) {
        unsafe { DeleteProcThreadAttributeList(self.as_mut_ptr()) };
    }
}
