use super::*;
use pretty_assertions::assert_eq;
use std::net::TcpListener;
use std::net::TcpStream;

#[test]
fn parses_owner_table_and_matches_reversed_client_tuple() -> io::Result<()> {
    let proxy_addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 3128);
    let client_addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 49152);
    let rows = [
        tcp_row(proxy_addr, client_addr, 100),
        tcp_row(client_addr, proxy_addr, 200),
    ];
    let (buffer, byte_len) = owner_table_buffer(&rows);

    let parsed = parse_tcp_owner_rows(&buffer, byte_len)?;

    assert_eq!(
        unique_client_process_id(parsed, proxy_addr, client_addr)?,
        200
    );
    Ok(())
}

#[test]
fn rejects_multiple_matching_owner_rows() {
    let proxy_addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 3128);
    let client_addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 49152);
    let rows = [
        tcp_row(client_addr, proxy_addr, 200),
        tcp_row(client_addr, proxy_addr, 201),
    ];

    let error = unique_client_process_id(&rows, proxy_addr, client_addr)
        .expect_err("duplicate connection rows should fail closed");

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn rejects_truncated_owner_table() {
    let byte_len = offset_of!(MIB_TCPTABLE_OWNER_PID, table);
    let mut buffer = aligned_buffer(byte_len).expect("aligned table buffer");
    unsafe {
        std::ptr::write_unaligned(buffer.as_mut_ptr().cast::<u32>(), 1);
    }

    let Err(error) = parse_tcp_owner_rows(&buffer, byte_len) else {
        panic!("truncated connection row should fail closed");
    };

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn resolves_loopback_connection_to_current_process() -> io::Result<()> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let client = TcpStream::connect(listener.local_addr()?)?;
    let (accepted, _) = listener.accept()?;
    let local_addr = accepted.local_addr()?;
    let peer_addr = accepted.peer_addr()?;

    let process_id = owning_process_id(socket_addr_v4(local_addr)?, socket_addr_v4(peer_addr)?)?;
    let restricting_sids = restricting_sids_for_tcp_connection(local_addr, peer_addr)?;

    assert_eq!(process_id, std::process::id());
    assert!(restricting_sids.iter().all(|sid| sid.starts_with("S-")));
    drop(client);
    Ok(())
}

fn socket_addr_v4(addr: SocketAddr) -> io::Result<SocketAddrV4> {
    match addr {
        SocketAddr::V4(addr) => Ok(addr),
        SocketAddr::V6(_) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "test listener unexpectedly used IPv6",
        )),
    }
}

fn tcp_row(
    local_addr: SocketAddrV4,
    remote_addr: SocketAddrV4,
    process_id: u32,
) -> MIB_TCPROW_OWNER_PID {
    MIB_TCPROW_OWNER_PID {
        dwState: 0,
        dwLocalAddr: u32::from_ne_bytes(local_addr.ip().octets()),
        dwLocalPort: local_addr.port().to_be() as u32,
        dwRemoteAddr: u32::from_ne_bytes(remote_addr.ip().octets()),
        dwRemotePort: remote_addr.port().to_be() as u32,
        dwOwningPid: process_id,
    }
}

fn owner_table_buffer(rows: &[MIB_TCPROW_OWNER_PID]) -> (Vec<usize>, usize) {
    let rows_offset = offset_of!(MIB_TCPTABLE_OWNER_PID, table);
    let rows_byte_len = size_of_val(rows);
    let byte_len = rows_offset + rows_byte_len;
    let mut buffer = aligned_buffer(byte_len).expect("aligned table buffer");
    unsafe {
        let buffer_ptr = buffer.as_mut_ptr().cast::<u8>();
        std::ptr::write_unaligned(buffer_ptr.cast::<u32>(), rows.len() as u32);
        std::ptr::copy_nonoverlapping(
            rows.as_ptr().cast::<u8>(),
            buffer_ptr.add(rows_offset),
            rows_byte_len,
        );
    }
    (buffer, byte_len)
}
