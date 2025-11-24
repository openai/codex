use quote::quote;

struct Ipv4Addr;

fn main() {
    let ip = Ipv4Addr;
    let _ = quote! { #(#ip)* };
}
