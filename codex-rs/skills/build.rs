use std::fs;
use std::path::Path;

fn main() {
    let samples_dir = Path::new("src/assets/samples");
    if !samples_dir.exists() {
        return;
    }

    println!("cargo:rerun-if-changed={}", samples_dir.display());
    visit_dir(samples_dir);
}

fn visit_dir(dir: &Path) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        println!("cargo:rerun-if-changed={}", path.display());
        if path.is_dir() {
            visit_dir(&path);
        }
    }
}
