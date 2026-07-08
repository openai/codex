use super::*;
use hickory_proto::op::Query;
use pretty_assertions::assert_eq;
use std::time::Duration;

struct ProcessGuard(libc::pid_t);

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        unsafe {
            libc::kill(self.0, libc::SIGTERM);
            libc::waitpid(self.0, std::ptr::null_mut(), /*options*/ 0);
        }
    }
}

fn query(name: &str, record_type: RecordType, id: u16) -> Vec<u8> {
    let mut message = Message::new();
    let name = Name::from_ascii(name).expect("valid name");
    message.set_id(id);
    message.add_query(Query::query(name, record_type));
    message.to_vec().expect("serialize query")
}

fn exchange(stream: &mut UnixStream, query: &[u8]) -> Message {
    write_frame(stream, query).expect("send query");
    Message::from_vec(&read_frame(stream).expect("receive response")).expect("parse response")
}

#[test]
fn host_resolver_enforces_policy_over_udp_and_tcp_channels() {
    let policy = ManagedNetworkDomainPolicy {
        allowed_domains: vec!["localhost".to_string(), "**.test".to_string()],
        denied_domains: vec!["blocked.test".to_string()],
    };
    let (_spec, files, pid) = spawn_host_dns_relay(&policy).expect("start resolver");
    let _guard = ProcessGuard(pid);
    let [udp, tcp]: [File; 2] = files.try_into().expect("two relay channels");
    let mut udp = unsafe { UnixStream::from_raw_fd(udp.into_raw_fd()) };
    let mut tcp = unsafe { UnixStream::from_raw_fd(tcp.into_raw_fd()) };

    let allowed = exchange(&mut udp, &query("localhost", RecordType::A, 1));
    assert!(
        allowed
            .answers()
            .iter()
            .any(|record| matches!(record.data(), RData::A(_)))
    );
    let denied = exchange(&mut udp, &query("blocked.test", RecordType::A, 2));
    assert_eq!(denied.response_code(), ResponseCode::Refused);
    let allowed = exchange(&mut tcp, &query("localhost", RecordType::A, 3));
    assert_eq!(allowed.response_code(), ResponseCode::NoError);
}

#[test]
fn host_resolver_refuses_untrusted_query_shapes() {
    let managed_policy = ManagedNetworkDomainPolicy {
        allowed_domains: vec!["localhost".to_string()],
        denied_domains: Vec::new(),
    };
    let policy = DnsPolicy {
        matcher: NetworkDomainMatcher::new(&managed_policy).expect("valid policy"),
    };
    let mut multi_question = Message::new();
    multi_question.set_id(4);
    multi_question.add_query(Query::query(
        Name::from_ascii("localhost").expect("valid name"),
        RecordType::A,
    ));
    multi_question.add_query(Query::query(
        Name::from_ascii("localhost").expect("valid name"),
        RecordType::AAAA,
    ));

    let responses = [
        query("1.0.0.127.in-addr.arpa.", RecordType::PTR, 1),
        query("localhost", RecordType::TXT, 2),
        vec![0, 3, 0xff],
        multi_question.to_vec().expect("serialize query"),
    ]
    .map(|wire| policy.response(&wire))
    .map(|response| (response.response_code(), response.answers().len()));

    assert_eq!(
        responses,
        [
            (ResponseCode::Refused, 0),
            (ResponseCode::Refused, 0),
            (ResponseCode::Refused, 0),
            (ResponseCode::Refused, 0),
        ]
    );
}

#[test]
fn host_resolver_exits_when_relay_channels_close() {
    let policy = ManagedNetworkDomainPolicy {
        allowed_domains: vec!["localhost".to_string()],
        denied_domains: Vec::new(),
    };
    let (_spec, files, pid) = spawn_host_dns_relay(&policy).expect("start resolver");
    drop(files);

    for _ in 0..100 {
        let exited = unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) };
        if exited == pid {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    unsafe {
        libc::kill(pid, libc::SIGTERM);
        libc::waitpid(pid, std::ptr::null_mut(), /*options*/ 0);
    }
    panic!("host resolver remained alive after its relay channels closed");
}
