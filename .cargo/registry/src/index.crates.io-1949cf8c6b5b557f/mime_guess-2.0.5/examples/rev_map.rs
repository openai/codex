extern crate mime_guess;

fn main() {
    print_exts("video/*");
    print_exts("video/x-matroska");
}

fn print_exts(mime_type: &str) {
    println!(
        "Exts for {:?}: {:?}",
        mime_type,
        mime_guess::get_mime_extensions_str(mime_type)
    );
}
