extern crate findshlibs;
use findshlibs::{Segment, SharedLibrary, TargetSharedLibrary};

fn main() {
    TargetSharedLibrary::each(|shlib| {
        println!("{}", shlib.name().to_string_lossy());

        for seg in shlib.segments() {
            println!(
                "    {}: segment {}",
                seg.actual_virtual_memory_address(shlib),
                seg.name()
            );
        }
    });
}
