extern crate tiny_http;

use std::sync::Arc;
use std::thread;

fn main() {
    let server = Arc::new(tiny_http::Server::http("0.0.0.0:9975").unwrap());
    println!("Now listening on port 9975");

    let mut handles = Vec::new();

    for _ in 0..4 {
        let server = server.clone();

        handles.push(thread::spawn(move || {
            for rq in server.incoming_requests() {
                let response = tiny_http::Response::from_string("hello world".to_string());
                let _ = rq.respond(response);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}
