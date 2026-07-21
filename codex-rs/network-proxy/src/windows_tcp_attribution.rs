use std::ffi::c_void;
use std::io;
use std::mem::offset_of;
use std::mem::size_of;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::SocketAddrV4;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use std::os::windows::io::RawHandle;

use windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::HLOCAL;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Foundation::NO_ERROR;
use windows_sys::Win32::Foundation::PSID;
use windows_sys::Win32::NetworkManagement::IpHelper::GetExtendedTcpTable;
use windows_sys::Win32::NetworkManagement::IpHelper::MIB_TCPROW_OWNER_PID;
use windows_sys::Win32::NetworkManagement::IpHelper::MIB_TCPTABLE_OWNER_PID;
use windows_sys::Win32::NetworkManagement::IpHelper::TCP_TABLE_OWNER_PID_CONNECTIONS;
use windows_sys::Win32::Networking::WinSock::AF_INET;
use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows_sys::Win32::Security::GetTokenInformation;
use windows_sys::Win32::Security::SID_AND_ATTRIBUTES;
use windows_sys::Win32::Security::TOKEN_GROUPS;
use windows_sys::Win32::Security::TOKEN_QUERY;
use windows_sys::Win32::Security::TokenRestrictedSids;
use windows_sys::Win32::System::Threading::OpenProcess;
use windows_sys::Win32::System::Threading::OpenProcessToken;
use windows_sys::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION;

/// Returns the restricting SIDs on the process that opened an accepted loopback connection.
///
/// `accepted_local_addr` and `accepted_peer_addr` must come from the accepted server socket. The
/// owning-PID table describes the client side in the opposite direction, so the lookup matches the
/// exact reversed four-tuple.
pub(crate) fn restricting_sids_for_tcp_connection(
    accepted_local_addr: SocketAddr,
    accepted_peer_addr: SocketAddr,
) -> io::Result<Vec<String>> {
    let (SocketAddr::V4(accepted_local_addr), SocketAddr::V4(accepted_peer_addr)) =
        (accepted_local_addr, accepted_peer_addr)
    else {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Windows proxy connection attribution currently supports IPv4 only",
        ));
    };

    let process_id = owning_process_id(accepted_local_addr, accepted_peer_addr)?;
    restricting_sids_for_process(process_id)
}

fn owning_process_id(
    accepted_local_addr: SocketAddrV4,
    accepted_peer_addr: SocketAddrV4,
) -> io::Result<u32> {
    let mut byte_len = 0_u32;
    let result = unsafe {
        GetExtendedTcpTable(
            std::ptr::null_mut(),
            &mut byte_len,
            0,
            AF_INET as u32,
            TCP_TABLE_OWNER_PID_CONNECTIONS,
            0,
        )
    };
    if result != ERROR_INSUFFICIENT_BUFFER {
        return Err(win32_error("query IPv4 TCP owner table size", result));
    }

    let buffer = loop {
        let mut buffer = aligned_buffer(byte_len as usize)?;
        let result = unsafe {
            GetExtendedTcpTable(
                buffer.as_mut_ptr().cast::<c_void>(),
                &mut byte_len,
                0,
                AF_INET as u32,
                TCP_TABLE_OWNER_PID_CONNECTIONS,
                0,
            )
        };
        match result {
            NO_ERROR => break buffer,
            ERROR_INSUFFICIENT_BUFFER => continue,
            _ => return Err(win32_error("read IPv4 TCP owner table", result)),
        }
    };

    let rows = parse_tcp_owner_rows(&buffer, byte_len as usize)?;
    unique_client_process_id(rows, accepted_local_addr, accepted_peer_addr)
}

fn parse_tcp_owner_rows(buffer: &[usize], byte_len: usize) -> io::Result<&[MIB_TCPROW_OWNER_PID]> {
    let rows_offset = offset_of!(MIB_TCPTABLE_OWNER_PID, table);
    if byte_len > size_of_val(buffer) || byte_len < rows_offset {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid IPv4 TCP owner table length",
        ));
    }

    let row_count = unsafe { std::ptr::read_unaligned(buffer.as_ptr().cast::<u32>()) } as usize;
    let rows_byte_len = row_count
        .checked_mul(size_of::<MIB_TCPROW_OWNER_PID>())
        .and_then(|len| rows_offset.checked_add(len))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "IPv4 TCP owner table length overflow",
            )
        })?;
    if rows_byte_len > byte_len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "truncated IPv4 TCP owner table",
        ));
    }

    let rows = unsafe {
        let rows_ptr = buffer
            .as_ptr()
            .cast::<u8>()
            .add(rows_offset)
            .cast::<MIB_TCPROW_OWNER_PID>();
        std::slice::from_raw_parts(rows_ptr, row_count)
    };
    Ok(rows)
}

fn unique_client_process_id(
    rows: &[MIB_TCPROW_OWNER_PID],
    accepted_local_addr: SocketAddrV4,
    accepted_peer_addr: SocketAddrV4,
) -> io::Result<u32> {
    let mut matching_process_ids = rows
        .iter()
        .filter(|row| client_row_matches(row, accepted_local_addr, accepted_peer_addr))
        .map(|row| row.dwOwningPid);
    let process_id = matching_process_ids.next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "accepted connection is absent from the IPv4 TCP owner table",
        )
    })?;
    if matching_process_ids.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "accepted connection has multiple IPv4 TCP owner rows",
        ));
    }
    Ok(process_id)
}

fn client_row_matches(
    row: &MIB_TCPROW_OWNER_PID,
    accepted_local_addr: SocketAddrV4,
    accepted_peer_addr: SocketAddrV4,
) -> bool {
    ipv4_addr_matches(row.dwLocalAddr, *accepted_peer_addr.ip())
        && tcp_port(row.dwLocalPort) == accepted_peer_addr.port()
        && ipv4_addr_matches(row.dwRemoteAddr, *accepted_local_addr.ip())
        && tcp_port(row.dwRemotePort) == accepted_local_addr.port()
}

fn ipv4_addr_matches(table_addr: u32, socket_addr: Ipv4Addr) -> bool {
    table_addr.to_ne_bytes() == socket_addr.octets()
}

fn tcp_port(table_port: u32) -> u16 {
    u16::from_be(table_port as u16)
}

fn restricting_sids_for_process(process_id: u32) -> io::Result<Vec<String>> {
    let process_handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
    let process = owned_handle(process_handle, "open proxy client process")?;

    let mut token_handle: HANDLE = 0;
    let opened = unsafe {
        OpenProcessToken(
            process.as_raw_handle() as HANDLE,
            TOKEN_QUERY,
            &mut token_handle,
        )
    };
    if opened == 0 {
        return Err(last_error("open proxy client process token"));
    }
    let token = owned_handle(token_handle, "open proxy client process token")?;

    let mut byte_len = 0_u32;
    let queried = unsafe {
        GetTokenInformation(
            token.as_raw_handle() as HANDLE,
            TokenRestrictedSids,
            std::ptr::null_mut(),
            0,
            &mut byte_len,
        )
    };
    if queried != 0 || unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER {
        return Err(last_error("query proxy client restricting SID buffer size"));
    }

    let mut buffer = aligned_buffer(byte_len as usize)?;
    let queried = unsafe {
        GetTokenInformation(
            token.as_raw_handle() as HANDLE,
            TokenRestrictedSids,
            buffer.as_mut_ptr().cast::<c_void>(),
            byte_len,
            &mut byte_len,
        )
    };
    if queried == 0 {
        return Err(last_error("read proxy client restricting SIDs"));
    }

    parse_token_groups(&buffer, byte_len as usize)?
        .iter()
        .map(|entry| sid_to_string(entry.Sid))
        .collect()
}

fn parse_token_groups(buffer: &[usize], byte_len: usize) -> io::Result<&[SID_AND_ATTRIBUTES]> {
    let groups_offset = offset_of!(TOKEN_GROUPS, Groups);
    if byte_len > size_of_val(buffer) || byte_len < groups_offset {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid restricting SID buffer length",
        ));
    }

    let group_count = unsafe { std::ptr::read_unaligned(buffer.as_ptr().cast::<u32>()) } as usize;
    let groups_byte_len = group_count
        .checked_mul(size_of::<SID_AND_ATTRIBUTES>())
        .and_then(|len| groups_offset.checked_add(len))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "restricting SID buffer length overflow",
            )
        })?;
    if groups_byte_len > byte_len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "truncated restricting SID buffer",
        ));
    }

    let groups = unsafe {
        let groups_ptr = buffer
            .as_ptr()
            .cast::<u8>()
            .add(groups_offset)
            .cast::<SID_AND_ATTRIBUTES>();
        std::slice::from_raw_parts(groups_ptr, group_count)
    };
    Ok(groups)
}

fn sid_to_string(sid: PSID) -> io::Result<String> {
    let mut string_sid = std::ptr::null_mut();
    if unsafe { ConvertSidToStringSidW(sid, &mut string_sid) } == 0 {
        return Err(last_error("convert proxy client restricting SID to string"));
    }

    let value = unsafe {
        let mut len = 0;
        while *string_sid.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(string_sid, len))
    };
    unsafe {
        LocalFree(string_sid as HLOCAL);
    }
    Ok(value)
}

fn aligned_buffer(byte_len: usize) -> io::Result<Vec<usize>> {
    if byte_len == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Windows API returned an empty buffer length",
        ));
    }
    Ok(vec![0; byte_len.div_ceil(size_of::<usize>())])
}

fn owned_handle(handle: HANDLE, operation: &str) -> io::Result<OwnedHandle> {
    if handle == 0 {
        return Err(last_error(operation));
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(handle as RawHandle) })
}

fn win32_error(operation: &str, error_code: u32) -> io::Error {
    let error = io::Error::from_raw_os_error(error_code as i32);
    io::Error::new(error.kind(), format!("{operation}: {error}"))
}

fn last_error(operation: &str) -> io::Error {
    let error = io::Error::last_os_error();
    io::Error::new(error.kind(), format!("{operation}: {error}"))
}

#[cfg(test)]
#[path = "windows_tcp_attribution_tests.rs"]
mod tests;
