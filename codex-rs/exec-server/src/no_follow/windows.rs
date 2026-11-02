use crate::regular_file;
use std::ffi::OsStr;
use std::ffi::c_void;
use std::io;
use std::io::Write;
use std::mem::size_of;
use std::mem::size_of_val;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::path::Prefix;
use std::ptr;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Foundation::NTSTATUS;
use windows_sys::Win32::Foundation::OBJ_CASE_INSENSITIVE;
use windows_sys::Win32::Foundation::OBJ_DONT_REPARSE;
use windows_sys::Win32::Foundation::RtlNtStatusToDosError;
use windows_sys::Win32::Foundation::UNICODE_STRING;
use windows_sys::Win32::Security::SECURITY_QUALITY_OF_SERVICE;
use windows_sys::Win32::Security::SecurityIdentification;
use windows_sys::Win32::Storage::FileSystem::BY_HANDLE_FILE_INFORMATION;
use windows_sys::Win32::Storage::FileSystem::DELETE;
use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
use windows_sys::Win32::Storage::FileSystem::FILE_DISPOSITION_INFO;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_ID_INFO;
use windows_sys::Win32::Storage::FileSystem::FILE_READ_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_DELETE;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
use windows_sys::Win32::Storage::FileSystem::FILE_WRITE_DATA;
use windows_sys::Win32::Storage::FileSystem::FileDispositionInfo;
use windows_sys::Win32::Storage::FileSystem::FileIdInfo;
use windows_sys::Win32::Storage::FileSystem::GetFileInformationByHandle;
use windows_sys::Win32::Storage::FileSystem::GetFileInformationByHandleEx;
use windows_sys::Win32::Storage::FileSystem::GetVolumeInformationByHandleW;
use windows_sys::Win32::Storage::FileSystem::SetFileInformationByHandle;
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK_0;

#[path = "windows_volume_fallback.rs"]
mod volume_fallback;

const SHARE_ALL: u32 = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE;
const FILE_DIRECTORY_FILE: u32 = 0x0000_0001;
const FILE_SYNCHRONOUS_IO_NONALERT: u32 = 0x0000_0020;
const FILE_NON_DIRECTORY_FILE: u32 = 0x0000_0040;
const FILE_OPEN: u32 = 1;
const FILE_CREATE: u32 = 2;
const FILE_OPEN_IF: u32 = 3;
const SYNCHRONIZE_ACCESS: u32 = 0x0010_0000;
const STATUS_REPARSE_POINT_ENCOUNTERED: NTSTATUS = 0xC000_050B_u32 as i32;
const SECURITY_STATIC_TRACKING: u8 = 0;

#[repr(C)]
struct ObjectAttributes {
    length: u32,
    root_directory: HANDLE,
    object_name: *const UNICODE_STRING,
    attributes: u32,
    security_descriptor: *const c_void,
    security_quality_of_service: *const c_void,
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtCreateFile(
        file_handle: *mut HANDLE,
        desired_access: u32,
        object_attributes: *const ObjectAttributes,
        io_status_block: *mut IO_STATUS_BLOCK,
        allocation_size: *const i64,
        file_attributes: u32,
        share_access: u32,
        create_disposition: u32,
        create_options: u32,
        ea_buffer: *const c_void,
        ea_length: u32,
    ) -> NTSTATUS;
}

fn nt_path(path: &Path) -> io::Result<Vec<u16>> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "no-follow filesystem operations require an absolute path",
        ));
    }

    let (prefix, source_offset) = match path.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(_) => ("\\??\\", 0),
            Prefix::VerbatimDisk(_) => ("\\??\\", 4),
            Prefix::UNC(_, _) => ("\\??\\UNC\\", 2),
            Prefix::VerbatimUNC(_, _) => ("\\??\\UNC\\", 8),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "no-follow filesystem operations require a local disk or UNC path",
                ));
            }
        },
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "no-follow filesystem operations require an absolute Windows path",
            ));
        }
    };

    let mut result: Vec<u16> = OsStr::new(prefix).encode_wide().collect();
    result.extend(
        path.as_os_str()
            .encode_wide()
            .skip(source_offset)
            .map(|unit| {
                if unit == b'/' as u16 {
                    b'\\' as u16
                } else {
                    unit
                }
            }),
    );
    result.push(0);
    Ok(result)
}

fn open_handle(
    path: &Path,
    desired_access: u32,
    create_disposition: u32,
    create_options: u32,
) -> io::Result<OwnedHandle> {
    open_handle_with_sharing(
        path,
        desired_access,
        create_disposition,
        create_options,
        SHARE_ALL,
    )
}

fn open_handle_with_sharing(
    path: &Path,
    desired_access: u32,
    create_disposition: u32,
    create_options: u32,
    share_access: u32,
) -> io::Result<OwnedHandle> {
    let mut name = nt_path(path)?;
    if let Some(handle) = open_nt_handle(
        &mut name,
        /*root_directory*/ None,
        desired_access,
        create_disposition,
        create_options,
        OBJ_DONT_REPARSE,
        share_access,
    )? {
        return Ok(handle);
    }

    // Windows 10 also rejects the DOS drive alias itself. Only for drive-letter
    // paths, open its verified volume root and keep strict checks below it.
    if matches!(path.components().next(), Some(Component::Prefix(prefix))
        if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
    {
        return volume_fallback::open_relative_to_volume(
            &mut name,
            desired_access,
            create_disposition,
            create_options,
            share_access,
        );
    }
    Err(reparse_error())
}

fn reparse_error() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "path contains a reparse point")
}

/// Returns `None` only for the native reparse status; other errors propagate.
fn open_nt_handle(
    path: &mut [u16],
    root_directory: Option<&OwnedHandle>,
    desired_access: u32,
    create_disposition: u32,
    create_options: u32,
    attributes: u32,
    share_access: u32,
) -> io::Result<Option<OwnedHandle>> {
    let maximum_length = u16::try_from(size_of_val(path))
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "filesystem path is too long"))?;
    let object_name = UNICODE_STRING {
        Length: maximum_length - size_of::<u16>() as u16,
        MaximumLength: maximum_length,
        Buffer: path.as_mut_ptr(),
    };
    let security_quality_of_service = SECURITY_QUALITY_OF_SERVICE {
        Length: size_of::<SECURITY_QUALITY_OF_SERVICE>() as u32,
        ImpersonationLevel: SecurityIdentification,
        ContextTrackingMode: SECURITY_STATIC_TRACKING,
        EffectiveOnly: true,
    };
    let object_attributes = ObjectAttributes {
        length: size_of::<ObjectAttributes>() as u32,
        root_directory: root_directory.map_or(ptr::null_mut(), AsRawHandle::as_raw_handle),
        object_name: &object_name,
        attributes: OBJ_CASE_INSENSITIVE | attributes,
        security_descriptor: ptr::null(),
        security_quality_of_service: (&raw const security_quality_of_service).cast(),
    };
    let mut io_status_block = IO_STATUS_BLOCK {
        Anonymous: IO_STATUS_BLOCK_0 { Status: 0 },
        Information: 0,
    };
    let mut handle = ptr::null_mut();
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            desired_access | SYNCHRONIZE_ACCESS,
            &object_attributes,
            &mut io_status_block,
            ptr::null(),
            FILE_ATTRIBUTE_NORMAL,
            share_access,
            create_disposition,
            create_options | FILE_SYNCHRONOUS_IO_NONALERT,
            ptr::null(),
            /*ea_length*/ 0,
        )
    };
    if status == STATUS_REPARSE_POINT_ENCOUNTERED {
        return Ok(None);
    }
    if status < 0 {
        let code = unsafe { RtlNtStatusToDosError(status) };
        return Err(io::Error::from_raw_os_error(code as i32));
    }
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::other(
            "NtCreateFile returned an invalid filesystem handle",
        ));
    }

    Ok(Some(unsafe { OwnedHandle::from_raw_handle(handle) }))
}

fn open_entry(path: &Path) -> io::Result<std::fs::File> {
    let handle = open_handle(
        path,
        FILE_READ_ATTRIBUTES,
        FILE_OPEN,
        /*create_options*/ 0,
    )?;
    Ok(std::fs::File::from(handle))
}

pub(super) fn open_file_sync(path: &Path) -> io::Result<std::fs::File> {
    let handle = open_handle(path, FILE_GENERIC_READ, FILE_OPEN, FILE_NON_DIRECTORY_FILE)?;
    let file = std::fs::File::from(handle);
    validate_regular_file(&file, path)?;
    Ok(file)
}

fn validate_regular_file(file: &std::fs::File, path: &Path) -> io::Result<()> {
    if !regular_file::is_disk_file(file) || !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("path `{}` is not a file", path.display()),
        ));
    }
    Ok(())
}

fn file_information(file: &std::fs::File) -> io::Result<BY_HANDLE_FILE_INFORMATION> {
    let mut info = std::mem::MaybeUninit::uninit();
    // SAFETY: the file owns the handle and a successful call initializes info.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { info.assume_init() })
}

pub(super) fn reject_multiple_links(file: &std::fs::File) -> io::Result<()> {
    if file_information(file)?.nNumberOfLinks > 1 {
        return Err(super::invalid_output("output has multiple hard links"));
    }
    Ok(())
}

fn file_identity(file: &std::fs::File) -> io::Result<(u64, [u8; 16])> {
    // ReFS requires the full 128-bit identifier to distinguish files on the same volume.
    let mut info = std::mem::MaybeUninit::<FILE_ID_INFO>::uninit();
    // SAFETY: the open handle is valid and FileIdInfo fills this exact structure on success.
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileIdInfo,
            info.as_mut_ptr().cast(),
            size_of::<FILE_ID_INFO>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let info = unsafe { info.assume_init() };
    // A filesystem may report success with an all-zero ID when it cannot identify the file.
    if info.FileId.Identifier == [0; 16] {
        return Err(super::invalid_output(
            "filesystem did not provide an output file ID",
        ));
    }
    Ok((info.VolumeSerialNumber, info.FileId.Identifier))
}

pub(super) fn verify_output_path(file: &std::fs::File, path: &Path) -> io::Result<()> {
    verify_output_path_with(file, path, file_identity, filesystem_name)
}

pub(super) fn verify_output_path_with(
    file: &std::fs::File,
    path: &Path,
    file_id: impl Fn(&std::fs::File) -> io::Result<(u64, [u8; 16])>,
    volume_name: impl Fn(&std::fs::File) -> io::Result<String>,
) -> io::Result<()> {
    let current = open_entry(path)?;
    let same = match file_id(file) {
        Ok(id) => id == file_id(&current)?,
        Err(error) => {
            let is_fat = |name: String| {
                ["FAT", "FAT32", "exFAT"]
                    .iter()
                    .any(|fs| name.eq_ignore_ascii_case(fs))
            };
            if !is_fat(volume_name(file)?) || !is_fat(volume_name(&current)?) {
                return Err(error);
            }
            same_fat_file(file, &current)?
        }
    };
    if !same {
        return Err(super::invalid_output("output path was replaced"));
    }
    Ok(())
}

pub(super) fn filesystem_name(file: &std::fs::File) -> io::Result<String> {
    let mut name = [0u16; 32];
    // SAFETY: only the filesystem name is requested, in the supplied writable buffer.
    if unsafe {
        GetVolumeInformationByHandleW(
            file.as_raw_handle(),
            ptr::null_mut(),
            /*nvolumenamesize*/ 0,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            name.as_mut_ptr(),
            name.len() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let name = name.split(|unit| *unit == 0).next().unwrap_or_default();
    Ok(String::from_utf16_lossy(name))
}

fn same_fat_file(file: &std::fs::File, current: &std::fs::File) -> io::Result<bool> {
    // The bound output denies delete sharing, so its FAT directory slot cannot be recycled.
    let id = |file| -> io::Result<_> {
        let info = file_information(file)?;
        if info.nFileIndexHigh == 0 && info.nFileIndexLow == 0 {
            return Err(super::invalid_output(
                "filesystem did not provide an output file ID",
            ));
        }
        Ok((
            info.dwVolumeSerialNumber,
            info.nFileIndexHigh,
            info.nFileIndexLow,
        ))
    };
    Ok(id(file)? == id(current)?)
}

pub(super) fn open_null_output() -> io::Result<std::fs::File> {
    // Use the kernel null device directly so no user-controlled DOS alias is followed.
    let mut name: Vec<_> = r"\Device\Null".encode_utf16().chain([0]).collect();
    let handle = open_nt_handle(
        &mut name,
        /*root_directory*/ None,
        FILE_WRITE_DATA,
        FILE_OPEN,
        /*create_options*/ 0,
        OBJ_DONT_REPARSE,
        SHARE_ALL,
    )?
    .ok_or_else(reparse_error)?;
    Ok(handle.into())
}

fn open_writable_regular_file(path: &Path, share_access: u32) -> io::Result<std::fs::File> {
    let handle = open_handle_with_sharing(
        path,
        FILE_READ_ATTRIBUTES | FILE_WRITE_DATA,
        FILE_OPEN_IF,
        FILE_NON_DIRECTORY_FILE,
        share_access,
    )?;
    let file = std::fs::File::from(handle);
    validate_regular_file(&file, path)?;
    Ok(file)
}

pub(super) fn open_output(path: &Path) -> io::Result<std::fs::File> {
    if matches!(path.components().next(), Some(Component::Prefix(p)) if p.kind().is_verbatim())
        && path.as_os_str().as_encoded_bytes().contains(&b'/')
    {
        return Err(super::invalid_output("invalid verbatim output path"));
    }
    // Full file IDs identify the base file, not a particular data stream. Keep the
    // selected stream bound from the first open, including the unnamed default stream.
    let file = open_writable_regular_file(path, FILE_SHARE_READ | FILE_SHARE_WRITE)?;
    // Fail before starting a turn if this filesystem cannot verify the output.
    verify_output_path(&file, path)?;
    Ok(file)
}

pub(super) async fn write_file(path: PathBuf, contents: Vec<u8>) -> io::Result<()> {
    tokio::task::spawn_blocking(move || {
        let mut file = open_writable_regular_file(&path, SHARE_ALL)?;
        file.set_len(0)?;
        file.write_all(&contents)
    })
    .await
    .map_err(|error| io::Error::other(format!("filesystem task failed: {error}")))?
}

pub(super) async fn metadata(path: PathBuf) -> io::Result<std::fs::Metadata> {
    tokio::task::spawn_blocking(move || open_entry(&path)?.metadata())
        .await
        .map_err(|error| io::Error::other(format!("filesystem task failed: {error}")))?
}

fn create_directory_sync(path: &Path, recursive: bool) -> io::Result<()> {
    if !recursive {
        open_handle(path, FILE_READ_ATTRIBUTES, FILE_CREATE, FILE_DIRECTORY_FILE)?;
        return Ok(());
    }

    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if matches!(component, Component::Normal(_)) {
            open_or_create_directory(&current)?;
        }
    }
    Ok(())
}

fn open_or_create_directory(path: &Path) -> io::Result<()> {
    match open_handle(path, FILE_READ_ATTRIBUTES, FILE_OPEN, FILE_DIRECTORY_FILE) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match open_handle(path, FILE_READ_ATTRIBUTES, FILE_CREATE, FILE_DIRECTORY_FILE) {
                Ok(_) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    open_handle(path, FILE_READ_ATTRIBUTES, FILE_OPEN, FILE_DIRECTORY_FILE)?;
                    Ok(())
                }
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error),
    }
}

pub(super) async fn create_directory(path: PathBuf, recursive: bool) -> io::Result<()> {
    tokio::task::spawn_blocking(move || create_directory_sync(&path, recursive))
        .await
        .map_err(|error| io::Error::other(format!("filesystem task failed: {error}")))?
}

fn remove_sync(path: &Path, recursive: bool, force: bool) -> io::Result<()> {
    if recursive {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "recursive no-follow removal is unsupported",
        ));
    }

    let metadata = match open_entry(path).and_then(|file| file.metadata()) {
        Ok(metadata) => metadata,
        Err(error) if force && error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let create_options = if metadata.is_dir() {
        FILE_DIRECTORY_FILE
    } else {
        FILE_NON_DIRECTORY_FILE
    };
    let handle = open_handle(path, DELETE, FILE_OPEN, create_options)?;
    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    let result = unsafe {
        SetFileInformationByHandle(
            handle.as_raw_handle(),
            FileDispositionInfo,
            (&raw const disposition).cast(),
            size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    };
    if result == 0 {
        return Err(io::Error::last_os_error());
    }
    drop(handle);
    Ok(())
}

pub(super) async fn remove(path: PathBuf, recursive: bool, force: bool) -> io::Result<()> {
    tokio::task::spawn_blocking(move || remove_sync(&path, recursive, force))
        .await
        .map_err(|error| io::Error::other(format!("filesystem task failed: {error}")))?
}

#[cfg(test)]
#[path = "windows_tests.rs"]
mod tests;
