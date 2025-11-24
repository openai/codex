extern crate autocfg;

fn main() {
    // Normally, cargo will set `OUT_DIR` for build scripts.
    let ac = autocfg::AutoCfg::with_dir("target").unwrap();

    // When this feature was stabilized, it also renamed the method to
    // `chunk_by`, so it's important to *use* the feature in your probe.
    let code = r#"
        #![feature(slice_group_by)]
        pub fn probe(slice: &[i32]) -> impl Iterator<Item = &[i32]> {
            slice.group_by(|a, b| a == b)
        }
    "#;
    if ac.probe_raw(code).is_ok() {
        autocfg::emit("has_slice_group_by");
    }
}
