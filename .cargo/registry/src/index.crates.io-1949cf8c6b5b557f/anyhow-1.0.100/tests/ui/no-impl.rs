use anyhow::anyhow;

#[derive(Debug)]
struct Error;

fn main() {
    let _ = anyhow!(Error);
}
