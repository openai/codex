//! Assign stable per-command addresses to names. Addresses never change owners
//! while the command runs, including after the client's DNS cache expires.

use hickory_proto::op::Header;
use hickory_proto::op::Message;
use hickory_proto::op::MessageType;
use hickory_proto::op::OpCode;
use hickory_proto::op::Query;
use hickory_proto::op::ResponseCode;
use hickory_proto::rr::DNSClass;
use hickory_proto::rr::RData;
use hickory_proto::rr::Record;
use hickory_proto::rr::RecordType;
use hickory_proto::serialize::binary::BinDecodable;
use hickory_proto::serialize::binary::BinDecoder;
use hickory_proto::serialize::binary::BinEncodable;
use indexmap::IndexSet;
use std::io;
use std::io::Read;
use std::io::Write;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::net::TcpListener;
use std::net::TcpStream;
use std::net::UdpSocket;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

const MAX_NAMES: usize = 8192;
const V4_BASE: u32 = u32::from_be_bytes([198, 18, 0, 0]);
const V6_BASE: u128 = 0xfdc0_de00_0000_0000_0000_0000_0000_0000;

#[derive(Default)]
pub(super) struct Names {
    // Append only: both synthetic address families encode the same one-based index.
    names: IndexSet<String>,
}

impl Names {
    pub(super) fn lookup(&self, ip: IpAddr) -> io::Result<Option<String>> {
        let id = match ip.to_canonical() {
            IpAddr::V4(ip) if u32::from(ip) & 0xfffe_0000 == V4_BASE => u32::from(ip) - V4_BASE,
            IpAddr::V6(ip) if u128::from(ip) >> 32 == V6_BASE >> 32 => u128::from(ip) as u32,
            _ => return Ok(None),
        };
        let name = id
            .checked_sub(1)
            .and_then(|index| self.names.get_index(index as usize));
        name.cloned()
            .map(Some)
            .ok_or_else(|| io::Error::other("unassigned synthetic address"))
    }

    fn id(&mut self, host: String) -> Option<u32> {
        let index = match self.names.get_index_of(&host) {
            Some(index) => index,
            None if self.names.len() < MAX_NAMES => self.names.insert_full(host).0,
            None => return None,
        };
        Some(index as u32 + 1)
    }

    fn answer(&mut self, packet: &[u8]) -> Option<Vec<u8>> {
        if packet.len() > 4096 {
            return None;
        }
        // Decode one question only. EDNS and other trailing bytes never go to a resolver
        // or into the reply, and untrusted section counts never size an allocation.
        let mut decoder = BinDecoder::new(packet);
        let header = Header::read(&mut decoder).ok()?;
        if header.message_type != MessageType::Query
            || header.op_code != OpCode::Query
            || header.counts.queries != 1
            || header.counts.answers != 0
            || header.counts.authorities != 0
        {
            return None;
        }
        let start = decoder.index();
        let query = Query::read(&mut decoder).ok()?;
        // The first question has no earlier name to reference. Reject compression
        // and labels that would require escapes when converted to a SOCKS hostname.
        if query.to_bytes().ok()? != decoder.slice_from(start).ok()?
            || !query
                .name
                .iter()
                .flatten()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        {
            return None;
        }

        let mut reply = Message::response(header.id, OpCode::Query);
        reply.metadata.recursion_desired = header.recursion_desired;
        reply.metadata.recursion_available = true;
        if query.query_class == DNSClass::IN
            && matches!(query.query_type, RecordType::A | RecordType::AAAA)
            && !query.name.is_root()
        {
            let host = query
                .name
                .to_ascii()
                .trim_end_matches('.')
                .to_ascii_lowercase();
            if let Some(id) = self.id(host) {
                let ip = if query.query_type == RecordType::A {
                    IpAddr::V4(Ipv4Addr::from(V4_BASE + id))
                } else {
                    IpAddr::V6(Ipv6Addr::from(V6_BASE + u128::from(id)))
                };
                reply.add_answer(Record::from_rdata(query.name.clone(), 60, RData::from(ip)));
            } else {
                reply.metadata.response_code = ResponseCode::ServFail;
            }
        }
        reply.add_query(query);
        reply.to_vec().ok()
    }
}

pub(super) fn serve(names: Arc<Mutex<Names>>, udp: UdpSocket, tcp: TcpListener) -> io::Result<()> {
    let udp_names = Arc::clone(&names);
    thread::Builder::new().spawn(move || {
        let mut buf = [0; 4096];
        while let Ok((n, peer)) = udp.recv_from(&mut buf) {
            let reply = udp_names
                .lock()
                .ok()
                .and_then(|mut table| table.answer(&buf[..n]));
            if let Some(reply) = reply {
                let _ = udp.send_to(&reply, peer);
            }
        }
    })?;
    // Fixed workers bound concurrent DNS-over-TCP exchanges. Some resolvers
    // pipeline A and AAAA queries on the same connection.
    for _ in 0..4 {
        let listener = tcp.try_clone()?;
        let names = Arc::clone(&names);
        thread::Builder::new().spawn(move || {
            while let Ok(stream) = crate::managed_tcp::accept_with_backoff(&listener) {
                let _ = answer_tcp(stream, &names);
            }
        })?;
    }
    Ok(())
}

fn answer_tcp(mut stream: TcpStream, names: &Mutex<Names>) -> io::Result<()> {
    let timeout = Some(Duration::from_secs(2));
    stream.set_read_timeout(timeout)?;
    stream.set_write_timeout(timeout)?;
    let mut buf = [0; 4096];
    for _ in 0..32 {
        let mut size = [0; 2];
        stream.read_exact(&mut size)?;
        let query = buf
            .get_mut(..usize::from(u16::from_be_bytes(size)))
            .ok_or_else(|| io::Error::other("oversized DNS query"))?;
        stream.read_exact(query)?;
        let reply = names
            .lock()
            .map_err(|_| io::Error::other("DNS name table is unavailable"))?
            .answer(query)
            .ok_or_else(|| io::Error::other("invalid DNS query"))?;
        stream.write_all(&(reply.len() as u16).to_be_bytes())?;
        stream.write_all(&reply)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "managed_tcp_tests.rs"]
mod tests;
