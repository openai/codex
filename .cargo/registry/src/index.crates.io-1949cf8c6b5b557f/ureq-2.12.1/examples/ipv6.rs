use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use ureq::Resolver;
pub struct Ipv6Resolver;

impl Resolver for Ipv6Resolver {
    fn resolve(&self, netloc: &str) -> io::Result<Vec<SocketAddr>> {
        ToSocketAddrs::to_socket_addrs(netloc).map(|iter| {
            let vec = iter
                // only keep ipv6 addresses
                .filter(|s| s.is_ipv6())
                .collect::<Vec<SocketAddr>>();

            if vec.is_empty() {
                println!(
                    "Failed to find any ipv6 addresses. This probably means \
                    the DNS server didn't return any."
                );
            }

            vec
        })
    }
}

pub fn main() {
    let agent = ureq::builder().resolver(Ipv6Resolver).build();

    let result = agent.get("https://www.google.com/").call();

    match result {
        Err(err) => {
            println!("{:?}", err);
            std::process::exit(1);
        }
        Ok(response) => {
            assert_eq!(response.status(), 200);
        }
    }
}
