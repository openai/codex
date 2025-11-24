use std::fs;

fn main() {
    let mut args = std::env::args().skip(1);
    let a = fs::read(&args.next().unwrap()).unwrap();
    let b = fs::read(&args.next().unwrap()).unwrap();

    for (i, (&a, &b)) in a.iter().zip(b.iter()).enumerate() {
        if a != b {
            println!("mismatch at byte {}: {:08b} vs. {:08b}", i, a, b);
            break;
        }
    }
    if a.len() > b.len() {
        println!("a has additional {:?}", &a[b.len()..]);
    }
    if b.len() > a.len() {
        println!("b has additional {:?}", &b[a.len()..]);
    }
}
