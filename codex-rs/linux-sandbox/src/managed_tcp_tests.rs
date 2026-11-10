#![cfg(test)]

//! Check DNS identity across address families, cache exhaustion and literal-address requests.

use super::*;
use pretty_assertions::assert_eq;

fn query(name: &str, kind: RecordType) -> Vec<u8> {
    let mut packet = Message::new(/*id*/ 1, MessageType::Query, OpCode::Query);
    packet.add_query(Query::query(name.parse().unwrap(), kind));
    packet.to_vec().unwrap()
}

#[test]
fn both_families_retain_the_original_name_and_literal_ips_are_separate() {
    let mut names = Names::default();
    let a = names.answer(&query("Db.Example", RecordType::A)).unwrap();
    let aaaa = names
        .answer(&query("db.example", RecordType::AAAA))
        .unwrap();
    let ip4 = Ipv4Addr::from(<[u8; 4]>::try_from(&a[a.len() - 4..]).unwrap());
    let ip6 = Ipv6Addr::from(<[u8; 16]>::try_from(&aaaa[aaaa.len() - 16..]).unwrap());
    for address in [
        IpAddr::V4(ip4),
        IpAddr::V6(ip6),
        IpAddr::V6(ip4.to_ipv6_mapped()),
    ] {
        assert_eq!(
            names.lookup(address).unwrap().as_deref(),
            Some("db.example")
        );
    }
    assert_eq!(names.lookup("203.0.113.19".parse().unwrap()).unwrap(), None);
    assert_eq!(names.lookup("2001:db8::19".parse().unwrap()).unwrap(), None);
}

#[test]
fn capacity_exhaustion_never_reassigns_an_existing_address() {
    let mut names = Names::default();
    let first = names
        .answer(&query("first.example", RecordType::A))
        .unwrap();
    for n in 1..MAX_NAMES {
        names
            .answer(&query(&format!("n{n}.example"), RecordType::A))
            .unwrap();
    }
    assert_eq!(
        names.answer(&query("new.example", RecordType::A)).unwrap()[3] & 15,
        2
    );
    assert_eq!(
        names
            .answer(&query("first.example", RecordType::A))
            .unwrap(),
        first
    );
}

#[test]
fn malformed_and_unsupported_questions_cannot_allocate_an_alias() {
    let mut names = Names::default();
    assert!(names.answer(&[0; 11]).is_none());
    let mut packet = query("valid.example", RecordType::A);
    packet[12] = 0xc0;
    assert!(names.answer(&packet).is_none());
    let mut counts = query("valid.example", RecordType::A);
    counts[6..12].fill(0xff);
    assert!(names.answer(&counts).is_none());
    let mut header_pointer = query("valid.example", RecordType::A);
    header_pointer.splice(12..27, [0xc0, 0x00]);
    assert!(names.answer(&header_pointer).is_none());
    assert_eq!(
        names
            .answer(&query("valid.example", RecordType::MX))
            .unwrap()[7],
        0
    );
    assert!(names.names.is_empty());
}
