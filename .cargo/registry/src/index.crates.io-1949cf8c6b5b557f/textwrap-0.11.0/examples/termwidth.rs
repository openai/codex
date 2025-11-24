#[cfg(feature = "hyphenation")]
extern crate hyphenation;
extern crate textwrap;

#[cfg(feature = "hyphenation")]
use hyphenation::{Language, Load, Standard};
#[cfg(feature = "term_size")]
use textwrap::Wrapper;

#[cfg(not(feature = "term_size"))]
fn main() {
    println!("Please enable the term_size feature to run this example.");
}

#[cfg(feature = "term_size")]
fn main() {
    #[cfg(not(feature = "hyphenation"))]
    fn new_wrapper<'a>() -> (&'static str, Wrapper<'a, textwrap::HyphenSplitter>) {
        ("without hyphenation", Wrapper::with_termwidth())
    }

    #[cfg(feature = "hyphenation")]
    fn new_wrapper<'a>() -> (&'static str, Wrapper<'a, Standard>) {
        let dictionary = Standard::from_embedded(Language::EnglishUS).unwrap();
        (
            "with hyphenation",
            Wrapper::with_splitter(textwrap::termwidth(), dictionary),
        )
    }

    let example = "Memory safety without garbage collection. \
                   Concurrency without data races. \
                   Zero-cost abstractions.";
    // Create a new Wrapper -- automatically set the width to the
    // current terminal width.
    let (msg, wrapper) = new_wrapper();
    println!("Formatted {} in {} columns:", msg, wrapper.width);
    println!("----");
    println!("{}", wrapper.fill(example));
    println!("----");
}
