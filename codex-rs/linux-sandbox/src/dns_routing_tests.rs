use super::*;
use pretty_assertions::assert_eq;
use std::net::TcpListener;
use std::sync::mpsc;

#[test]
fn parses_loopback_dns_proxy_endpoint() {
    assert_eq!(
        parse_dns_proxy_endpoint("tcp://127.0.0.1:43128").expect("endpoint"),
        SocketAddr::from((Ipv4Addr::LOCALHOST, 43128))
    );
}

#[test]
fn rejects_non_loopback_dns_proxy_endpoint() {
    let err =
        parse_dns_proxy_endpoint("tcp://192.0.2.10:43128").expect_err("non-loopback endpoint");
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn forwards_dns_query_to_private_proxy_session() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind DNS proxy fixture");
    let endpoint = listener.local_addr().expect("fixture local addr");
    let (received_tx, received_rx) = mpsc::channel();
    let expected_response = b"dns-response".to_vec();
    let response = expected_response.clone();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept DNS proxy session");
        let mut preface = [0_u8; DNS_PROXY_SESSION_PREFACE.len()];
        stream.read_exact(&mut preface).expect("read preface");
        assert_eq!(&preface, DNS_PROXY_SESSION_PREFACE);
        stream
            .write_all(DNS_PROXY_SESSION_PREFACE)
            .expect("write preface ack");
        let query = read_frame(&mut stream).expect("read query");
        received_tx.send(query).expect("send query");
        write_frame(&mut stream, &response).expect("write response");
    });

    let response = resolve_through_proxy(endpoint, b"dns-query").expect("proxy response");

    assert_eq!(received_rx.recv().expect("query"), b"dns-query");
    assert_eq!(response, expected_response);
}
