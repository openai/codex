use quote::quote;
use std::net::Ipv4Addr;

fn main() {
    let ip = Ipv4Addr::LOCALHOST;
    let _ = quote! { #ip };
}
