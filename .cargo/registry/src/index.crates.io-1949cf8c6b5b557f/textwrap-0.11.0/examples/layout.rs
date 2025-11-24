#[cfg(feature = "hyphenation")]
extern crate hyphenation;
extern crate textwrap;

#[cfg(feature = "hyphenation")]
use hyphenation::{Language, Load};
use textwrap::Wrapper;

#[cfg(not(feature = "hyphenation"))]
fn new_wrapper<'a>() -> Wrapper<'a, textwrap::HyphenSplitter> {
    Wrapper::new(0)
}

#[cfg(feature = "hyphenation")]
fn new_wrapper<'a>() -> Wrapper<'a, hyphenation::Standard> {
    let dictionary = hyphenation::Standard::from_embedded(Language::EnglishUS).unwrap();
    Wrapper::with_splitter(0, dictionary)
}

fn main() {
    let example = "Memory safety without garbage collection. \
                   Concurrency without data races. \
                   Zero-cost abstractions.";
    let mut prev_lines = vec![];
    let mut wrapper = new_wrapper();
    for width in 15..60 {
        wrapper.width = width;
        let lines = wrapper.wrap(example);
        if lines != prev_lines {
            let title = format!(" Width: {} ", width);
            println!(".{:-^1$}.", title, width + 2);
            for line in &lines {
                println!("| {:1$} |", line, width);
            }
            prev_lines = lines;
        }
    }
}
