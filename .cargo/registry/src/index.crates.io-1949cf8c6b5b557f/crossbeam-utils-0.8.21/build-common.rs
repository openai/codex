// The target triplets have the form of 'arch-vendor-system'.
//
// When building for Linux (e.g. the 'system' part is
// 'linux-something'), replace the vendor with 'unknown'
// so that mapping to rust standard targets happens correctly.
fn convert_custom_linux_target(target: String) -> String {
    let mut parts: Vec<&str> = target.split('-').collect();
    let system = parts.get(2);
    if system == Some(&"linux") {
        parts[1] = "unknown";
    };
    parts.join("-")
}
