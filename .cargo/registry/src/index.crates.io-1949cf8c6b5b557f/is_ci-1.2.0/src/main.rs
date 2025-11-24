use is_ci::uncached;

pub fn main() {
    let code = if uncached() { 0 } else { 1 };
    std::process::exit(code);
}
