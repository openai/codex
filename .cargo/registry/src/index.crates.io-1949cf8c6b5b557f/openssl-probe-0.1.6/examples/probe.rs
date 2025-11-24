fn main() {
    let r = openssl_probe::probe();

    println!("cert_dir: {:?}", r.cert_dir);
    println!("cert_file: {:?}", r.cert_file);
}
