extern crate uname;

use uname::uname;

fn main() {
    let info = uname().unwrap();

    // Print the hostname
    println!("{}", info.nodename);
    // Print everything
    println!("{:?}", info);
}
