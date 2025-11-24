extern crate tiny_http;

fn main() {
    use tiny_http::{Response, Server};

    let server = Server::http("0.0.0.0:8000").unwrap();

    for request in server.incoming_requests() {
        println!(
            "received request! method: {:?}, url: {:?}, headers: {:?}",
            request.method(),
            request.url(),
            request.headers()
        );

        let response = Response::from_string("hello world");
        request.respond(response).expect("Responded");
    }
}
