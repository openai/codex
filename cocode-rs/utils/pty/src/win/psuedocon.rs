#![allow(clippy::expect_used)]
#![allow(clippy::upper_case_acronyms)]

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

use super::WinChild;
use crate::win::procthreadattr::ProcThreadAttributeList;
use anyhow::bail;
use anyhow::ensure;
use anyhow::Error;
use filedescriptor::FileDescriptor;
use filedescriptor::OwnedHandle;
use lazy_static::lazy_static;
use portable_pty::cmdbuilder::CommandBuilder;
use shared_library::shared_library;
use std::env;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io::Error as IoError;
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::FromRawHandle;
use std::path::Path;
use std::ptr;
use std::sync::Mutex;
use winapi::shared::minwindef::DWORD;
use winapi::shared::ntdef::NTSTATUS;
use winapi::shared::ntstatus::STATUS_SUCCESS;
use winapi::shared::winerror::HRESULT;
use winapi::shared::winerror::S_OK;
use winapi::um::handleapi::*;
use winapi::um::processthreadsapi::*;
use winapi::um::winbase::CREATE_UNICODE_ENVIRONMENT;
use winapi::um::winbase::EXTENDED_STARTUPINFO_PRESENT;
use winapi::um::winbase::STARTF_USESTDHANDLES;
use winapi::um::winbase::STARTUPINFOEXW;
use winapi::um::wincon::COORD;
use winapi::um::winnt::HANDLE;
use winapi::um::winnt::OSVERSIONINFOW;

pub type HPCON = HANDLE;

pub const PSEUDOCONSOLE_RESIZE_QUIRK: DWORD = 0x2;
#[allow(dead_code)]
pub const PSEUDOCONSOLE_PASSTHROUGH_MODE: DWORD = 0x8;

// https://learn.microsoft.com/en-gb/windows/console/createpseudoconsole
// https://learn.microsoft.com/en-gb/windows/release-health/release-information
const MIN_CONPTY_BUILD: u32 = 17_763;

shared_library!(ConPtyFuncs,
    pub fn CreatePseudoConsole(
        size: COORD,
        hInput: HANDLE,
        hOutput: HANDLE,
        flags: DWORD,
        hpc: *mut HPCON
    ) -> HRESULT,
    pub fn ResizePseudoConsole(hpc: HPCON, size: COORD) -> HRESULT,
    pub fn ClosePseudoConsole(hpc: HPCON),
);

shared_library!(Ntdll,
    pub fn RtlGetVersion(
        version_info: *mut OSVERSIONINFOW
    ) -> NTSTATUS,
);

fn load_conpty() -> ConPtyFuncs {
    let kernel = ConPtyFuncs::open(Path::new("kernel32.dll")).expect(
        "this system does not support conpty.  Windows 10 October 2018 or newer is required",
    );

    if let Ok(sideloaded) = ConPtyFuncs::open(Path::new("conpty.dll")) {
        sideloaded
    } else {
        kernel
    }
}

lazy_static! {
    static ref CONPTY: ConPtyFuncs = load_conpty();
}

pub fn conpty_supported() -> bool {
    windows_build_number().is_some_and(|build| build >= MIN_CONPTY_BUILD)
}

fn windows_build_number() -> Option<u32> {
    let ntdll = Ntdll::open(Path::new("ntdll.dll")).ok()?;
    let mut info: OSVERSIONINFOW = unsafe { mem::zeroed() };
    info.dwOSVersionInfoSize = mem::size_of::<OSVERSIONINFOW>() as u32;
    let status = unsafe { (ntdll.RtlGetVersion)(&mut info) };
    if status == STATUS_SUCCESS {
        Some(info.dwBuildNumber)
    } else {
        None
    }
}

pub struct PsuedoCon {
    con: HPCON,
}

unsafe impl Send for PsuedoCon {}
unsafe impl Sync for PsuedoCon {}

impl Drop for PsuedoCon {
    fn drop(&mut self) {
        unsafe { (CONPTY.ClosePseudoConsole)(self.con) };
    }
}

impl PsuedoCon {
    pub fn new(size: COORD, input: FileDescriptor, output: FileDescriptor) -> Result<Self, Error> {
        let mut con: HPCON = INVALID_HANDLE_VALUE;
        let result = unsafe {
            (CONPTY.CreatePseudoConsole)(
                size,
                input.as_raw_handle() as _,
                output.as_raw_handle() as _,
                PSEUDOCONSOLE_RESIZE_QUIRK,
                &mut con,
            )
        };
        ensure!(
            result == S_OK,
            "failed to create psuedo console: HRESULT {result}"
        );
        Ok(Self { con })
    }

    pub fn resize(&self, size: COORD) -> Result<(), Error> {
        let result = unsafe { (CONPTY.ResizePseudoConsole)(self.con, size) };
        ensure!(
            result == S_OK,
            "failed to resize console to {}x{}: HRESULT: {}",
            size.X,
            size.Y,
            result
        );
        Ok(())
    }

    pub fn spawn_command(&self, cmd: CommandBuilder) -> anyhow::Result<WinChild> {
        let mut si: STARTUPINFOEXW = unsafe { mem::zeroed() };
        si.StartupInfo.cb = mem::size_of::<STARTUPINFOEXW>() as u32;
        si.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        si.StartupInfo.hStdInput = INVALID_HANDLE_VALUE;
        si.StartupInfo.hStdOutput = INVALID_HANDLE_VALUE;
        si.StartupInfo.hStdError = INVALID_HANDLE_VALUE;

        let mut attrs = ProcThreadAttributeList::with_capacity(1)?;
        attrs.set_pty(self.con)?;
        si.lpAttributeList = attrs.as_mut_ptr();

        let mut pi: PROCESS_INFORMATION = unsafe { mem::zeroed() };

        let (mut exe, mut cmdline) = build_cmdline(&cmd)?;
        let cmd_os = OsString::from_wide(&cmdline);

        let cwd = resolve_current_directory(&cmd);
        let mut env_block = build_environment_block(&cmd);

        let res = unsafe {
            CreateProcessW(
                exe.as_mut_ptr(),
                cmdline.as_mut_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
                env_block.as_mut_ptr() as *mut _,
                cwd.as_ref().map_or(ptr::null(), std::vec::Vec::as_ptr),
                &mut si.StartupInfo,
                &mut pi,
            )
        };
        if res == 0 {
            let err = IoError::last_os_error();
            let msg = format!(
                "CreateProcessW `{:?}` in cwd `{:?}` failed: {}",
                cmd_os,
                cwd.as_ref().map(|c| OsString::from_wide(c)),
                err
            );
            log::error!("{msg}");
            bail!("{msg}");
        }

        let _main_thread = unsafe { OwnedHandle::from_raw_handle(pi.hThread as _) };
        let proc = unsafe { OwnedHandle::from_raw_handle(pi.hProcess as _) };

        Ok(WinChild {
            proc: Mutex::new(proc),
        })
    }
}

fn resolve_current_directory(cmd: &CommandBuilder) -> Option<Vec<u16>> {
    let home = cmd
        .get_env("USERPROFILE")
        .and_then(|path| Path::new(path).is_dir().then(|| path.to_owned()));
    let cwd = cmd
        .get_cwd()
        .and_then(|path| Path::new(path).is_dir().then(|| path.to_owned()));
    let dir = cwd.or(home)?;

    let mut wide = Vec::new();
    if Path::new(&dir).is_relative() {
        if let Ok(current_dir) = env::current_dir() {
            wide.extend(current_dir.join(&dir).as_os_str().encode_wide());
        } else {
            wide.extend(dir.encode_wide());
        }
    } else {
        wide.extend(dir.encode_wide());
    }
    wide.push(0);
    Some(wide)
}

fn build_environment_block(cmd: &CommandBuilder) -> Vec<u16> {
    let mut block = Vec::new();
    for (key, value) in cmd.iter_full_env_as_str() {
        block.extend(OsStr::new(key).encode_wide());
        block.push(b'=' as u16);
        block.extend(OsStr::new(value).encode_wide());
        block.push(0);
    }
    block.push(0);
    block
}

fn build_cmdline(cmd: &CommandBuilder) -> anyhow::Result<(Vec<u16>, Vec<u16>)> {
    let exe_os: OsString = if cmd.is_default_prog() {
        cmd.get_env("ComSpec")
            .unwrap_or(OsStr::new("cmd.exe"))
            .to_os_string()
    } else {
        let argv = cmd.get_argv();
        let Some(first) = argv.first() else {
            anyhow::bail!("missing program name");
        };
        search_path(cmd, first)
    };

    let mut cmdline = Vec::new();
    append_quoted(&exe_os, &mut cmdline);
    for arg in cmd.get_argv().iter().skip(1) {
        cmdline.push(' ' as u16);
        ensure!(
            !arg.encode_wide().any(|c| c == 0),
            "invalid encoding for command line argument {arg:?}"
        );
        append_quoted(arg, &mut cmdline);
    }
    cmdline.push(0);

    let mut exe: Vec<u16> = exe_os.encode_wide().collect();
    exe.push(0);

    Ok((exe, cmdline))
}

fn search_path(cmd: &CommandBuilder, exe: &OsStr) -> OsString {
    if let Some(path) = cmd.get_env("PATH") {
        let extensions = cmd.get_env("PATHEXT").unwrap_or(OsStr::new(".EXE"));
        for path in env::split_paths(path) {
            let candidate = path.join(exe);
            if candidate.exists() {
                return candidate.into_os_string();
            }

            for ext in env::split_paths(extensions) {
                let ext = ext.to_str().unwrap_or("");
                let path = path
                    .join(exe)
                    .with_extension(ext.strip_prefix('.').unwrap_or(ext));
                if path.exists() {
                    return path.into_os_string();
                }
            }
        }
    }

    exe.to_os_string()
}

fn append_quoted(arg: &OsStr, cmdline: &mut Vec<u16>) {
    if !arg.is_empty()
        && !arg.encode_wide().any(|c| {
            c == ' ' as u16
                || c == '\t' as u16
                || c == '\n' as u16
                || c == '\x0b' as u16
                || c == '\"' as u16
        })
    {
        cmdline.extend(arg.encode_wide());
        return;
    }
    cmdline.push('"' as u16);

    let arg: Vec<_> = arg.encode_wide().collect();
    let mut i = 0;
    while i < arg.len() {
        let mut num_backslashes = 0;
        while i < arg.len() && arg[i] == '\\' as u16 {
            i += 1;
            num_backslashes += 1;
        }

        if i == arg.len() {
            for _ in 0..num_backslashes * 2 {
                cmdline.push('\\' as u16);
            }
            break;
        } else if arg[i] == b'"' as u16 {
            for _ in 0..num_backslashes * 2 + 1 {
                cmdline.push('\\' as u16);
            }
            cmdline.push(arg[i]);
        } else {
            for _ in 0..num_backslashes {
                cmdline.push('\\' as u16);
            }
            cmdline.push(arg[i]);
        }
        i += 1;
    }
    cmdline.push('"' as u16);
}

#[cfg(test)]
mod tests {
    use super::windows_build_number;
    use super::MIN_CONPTY_BUILD;

    #[test]
    fn windows_build_number_returns_value() {
        // We can't stably check the version of the GH workers, but we can
        // at least check that this.
        let version = windows_build_number().unwrap();
        assert!(version > MIN_CONPTY_BUILD);
    }
}
