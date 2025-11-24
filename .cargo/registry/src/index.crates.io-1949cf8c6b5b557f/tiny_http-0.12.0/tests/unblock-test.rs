extern crate tiny_http;

use std::sync::Arc;
use std::thread;

#[test]
fn unblock_server() {
    let server = tiny_http::Server::http("0.0.0.0:0").unwrap();
    let s = Arc::new(server);

    let s1 = s.clone();
    thread::spawn(move || s1.unblock());

    // Without unblock this would hang forever
    for _rq in s.incoming_requests() {}
}

#[test]
fn unblock_threads() {
    let server = tiny_http::Server::http("0.0.0.0:0").unwrap();
    let s = Arc::new(server);

    let s1 = s.clone();
    let s2 = s.clone();
    let h1 = thread::spawn(move || for _rq in s1.incoming_requests() {});
    let h2 = thread::spawn(move || for _rq in s2.incoming_requests() {});

    // Graceful shutdown; removing even one of the
    // unblock calls prevents termination
    s.unblock();
    s.unblock();
    h1.join().unwrap();
    h2.join().unwrap();
}
