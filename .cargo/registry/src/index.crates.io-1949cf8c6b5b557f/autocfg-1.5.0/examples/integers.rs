extern crate autocfg;

fn main() {
    // Normally, cargo will set `OUT_DIR` for build scripts.
    let ac = autocfg::AutoCfg::with_dir("target").unwrap();
    for i in 3..8 {
        ac.emit_has_type(&format!("i{}", 1 << i));
    }
}
