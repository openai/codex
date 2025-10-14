#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
fn main() -> ! {
    windows::run()
}

#[cfg(not(target_os = "windows"))]
fn main() -> ! {
    eprintln!("Windows sandbox is only available on Windows");
    std::process::exit(1);
}
