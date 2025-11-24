use std::net::TcpStream;
use std::thread;
use std::time::Duration;

/// Creates a server and a client connected to the server.
pub fn new_one_server_one_client() -> (tiny_http::Server, TcpStream) {
    let server = tiny_http::Server::http("0.0.0.0:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();
    let client = TcpStream::connect(("127.0.0.1", port)).unwrap();
    (server, client)
}

/// Creates a "hello world" server with a client connected to the server.
///
/// The server will automatically close after 3 seconds.
pub fn new_client_to_hello_world_server() -> TcpStream {
    let server = tiny_http::Server::http("0.0.0.0:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();
    let client = TcpStream::connect(("127.0.0.1", port)).unwrap();

    thread::spawn(move || {
        let mut cycles = 3 * 1000 / 20;

        loop {
            if let Some(rq) = server.try_recv().unwrap() {
                let response = tiny_http::Response::from_string("hello world".to_string());
                rq.respond(response).unwrap();
            }

            thread::sleep(Duration::from_millis(20));

            cycles -= 1;
            if cycles == 0 {
                break;
            }
        }
    });

    client
}
