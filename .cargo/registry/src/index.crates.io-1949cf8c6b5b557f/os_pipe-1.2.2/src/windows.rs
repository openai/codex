use crate::PipeReader;
use crate::PipeWriter;
use std::fs::File;
use std::io;
use std::os::windows::prelude::*;
use std::ptr;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::Pipes::CreatePipe;

pub(crate) fn pipe() -> io::Result<(PipeReader, PipeWriter)> {
    let mut read_pipe = INVALID_HANDLE_VALUE;
    let mut write_pipe = INVALID_HANDLE_VALUE;

    let ret = unsafe {
        // NOTE: These pipes do not support IOCP. We might want to emulate
        // anonymous pipes with CreateNamedPipe, as Rust's stdlib does.
        CreatePipe(&mut read_pipe, &mut write_pipe, ptr::null_mut(), 0)
    };

    if ret == 0 {
        Err(io::Error::last_os_error())
    } else {
        unsafe {
            Ok((
                PipeReader::from_raw_handle(read_pipe as _),
                PipeWriter::from_raw_handle(write_pipe as _),
            ))
        }
    }
}

pub(crate) fn dup(handle: impl AsHandle) -> io::Result<OwnedHandle> {
    handle.as_handle().try_clone_to_owned()
}

impl IntoRawHandle for PipeReader {
    fn into_raw_handle(self) -> RawHandle {
        self.0.into_raw_handle()
    }
}

impl AsRawHandle for PipeReader {
    fn as_raw_handle(&self) -> RawHandle {
        self.0.as_raw_handle()
    }
}

impl FromRawHandle for PipeReader {
    unsafe fn from_raw_handle(handle: RawHandle) -> PipeReader {
        unsafe { PipeReader(File::from_raw_handle(handle)) }
    }
}

impl From<PipeReader> for OwnedHandle {
    fn from(reader: PipeReader) -> Self {
        reader.0.into()
    }
}

impl AsHandle for PipeReader {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.0.as_handle()
    }
}

impl From<OwnedHandle> for PipeReader {
    fn from(handle: OwnedHandle) -> Self {
        PipeReader(handle.into())
    }
}

impl IntoRawHandle for PipeWriter {
    fn into_raw_handle(self) -> RawHandle {
        self.0.into_raw_handle()
    }
}

impl AsRawHandle for PipeWriter {
    fn as_raw_handle(&self) -> RawHandle {
        self.0.as_raw_handle()
    }
}

impl FromRawHandle for PipeWriter {
    unsafe fn from_raw_handle(handle: RawHandle) -> PipeWriter {
        unsafe { PipeWriter(File::from_raw_handle(handle)) }
    }
}

impl From<PipeWriter> for OwnedHandle {
    fn from(writer: PipeWriter) -> Self {
        writer.0.into()
    }
}

impl AsHandle for PipeWriter {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.0.as_handle()
    }
}

impl From<OwnedHandle> for PipeWriter {
    fn from(handle: OwnedHandle) -> Self {
        PipeWriter(handle.into())
    }
}
