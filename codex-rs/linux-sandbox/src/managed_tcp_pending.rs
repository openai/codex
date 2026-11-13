//! Pin the native socket inode before asking for approval. Linux keeps the inode
//! for a live reader after shutdown(SHUT_WR), and clears it on the last close.

use std::collections::HashSet;
use std::fs::File;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::net::IpAddr;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

const INTERVAL: Duration = Duration::from_millis(500);
const TCP4: &str = "/proc/self/net/tcp";
const TCP6: &str = "/proc/self/net/tcp6";

pub(super) struct Owner {
    inode: u64,
    captured: Instant,
    table: &'static str,
}

fn find_inode(
    table: &str,
    mut matches: impl FnMut(&str, &str, u64) -> bool,
) -> io::Result<Option<u64>> {
    let mut reader = BufReader::new(File::open(table)?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    if !line.contains("local_address") || !line.contains("inode") {
        return Err(io::Error::other("missing native TCP socket table"));
    }
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let mut fields = line.split_ascii_whitespace().skip(1);
        let (Some(local), Some(remote), Some(inode)) =
            (fields.next(), fields.next(), fields.nth(6))
        else {
            return Err(io::Error::other("unexpected native TCP socket record"));
        };
        let inode = inode.parse::<u64>().map_err(io::Error::other)?;
        if inode != 0 && matches(local, remote, inode) {
            return Ok(Some(inode));
        }
    }
}

fn v6_key(addr: SocketAddr) -> String {
    let ip: Ipv6Addr = match addr.ip() {
        IpAddr::V4(v4) => v4.to_ipv6_mapped(),
        IpAddr::V6(v6) => v6,
    };
    // procfs renders each 32-bit word in native byte order.
    let address: String = ip
        .octets()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|word| format!("{:08X}", u32::from_ne_bytes(*word)))
        .collect();
    format!("{address}:{:04X}", addr.port())
}

impl Owner {
    pub(super) fn capture(source: SocketAddr, target: SocketAddr) -> io::Result<Self> {
        let lookup = |table, local: String, remote: String| {
            let inode = find_inode(table, |from, to, _| from == local && to == remote)?;
            Ok::<_, io::Error>(inode.map(|inode| Self {
                inode,
                table,
                captured: Instant::now(),
            }))
        };
        if let (SocketAddr::V4(src), SocketAddr::V4(dst)) = (source, target) {
            let key = |addr: std::net::SocketAddrV4| {
                format!(
                    "{:08X}:{:04X}",
                    u32::from_ne_bytes(addr.ip().octets()),
                    addr.port()
                )
            };
            if let Some(owner) = lookup(TCP4, key(src), key(dst))? {
                return Ok(owner);
            }
        }
        lookup(TCP6, v6_key(source), v6_key(target))?.ok_or(io::ErrorKind::ConnectionAborted.into())
    }

    fn still_owned(&self) -> io::Result<bool> {
        // Only slow approvals need polling. Share one bounded snapshot per family.
        type Snapshot = Mutex<Option<(Instant, Option<HashSet<u64>>)>>;
        static V4: Snapshot = Mutex::new(None);
        static V6: Snapshot = Mutex::new(None);
        let cache = if self.table == TCP4 { &V4 } else { &V6 };
        let mut cache = cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if cache
            .as_ref()
            .is_none_or(|(sampled, _)| *sampled < self.captured || sampled.elapsed() >= INTERVAL)
        {
            let sampled = Instant::now();
            let mut inodes = HashSet::new();
            let overflow = find_inode(self.table, |_, _, inode| {
                inodes.insert(inode);
                inodes.len() > 16_384
            })?;
            *cache = Some((sampled, overflow.is_none().then_some(inodes)));
        }
        if let Some((_, Some(inodes))) = &*cache {
            return Ok(inodes.contains(&self.inode));
        }
        drop(cache);
        Ok(find_inode(self.table, |_, _, inode| inode == self.inode)?.is_some())
    }

    pub(super) fn wait_for_reply(&self, stream: &TcpStream) -> io::Result<()> {
        stream.set_read_timeout(Some(INTERVAL))?;
        loop {
            match stream.peek(&mut [0]) {
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) => {}
                result => return result.map(|_| ()),
            }
            if !self.still_owned()? {
                return Err(io::ErrorKind::ConnectionAborted.into());
            }
        }
    }
}
