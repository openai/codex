fn main() {
// execpolicy-legacy/build.rs
    println!("cargo:rerun-if-changed=src/default.policy");
}
