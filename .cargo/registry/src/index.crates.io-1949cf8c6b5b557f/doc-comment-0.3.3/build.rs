use std::process::Command;

fn main() {
    if let Ok(v) = Command::new("rustc").arg("--version").output() {
        let s = match String::from_utf8(v.stdout) {
            Ok(s) => s,
            _ => return,
        };
        if !s.starts_with("rustc ") {
            return;
        }
        if let Some(s) = s.split(' ').skip(1).next() {
            let s = s.split('.').collect::<Vec<_>>();
            if s.len() < 3 {
                return;
            }
            if s[0] == "1" && u32::from_str_radix(&s[1], 10)
                                  .map(|nb| nb < 30)
                                  .unwrap_or_else(|_| false) {
                println!("cargo:rustc-cfg=feature=\"old_macros\"");
            }
        }
    }
}