use std::io::Write as _;

use pulldown_cmark::{html, Event, Options, Parser, Tag, TagEnd};

fn main() {
    let markdown_input: &str = "This is Peter on ![holiday in Greece](pearl_beach.jpg).";
    println!("Parsing the following markdown string:\n{}", markdown_input);

    // Set up parser. We can treat is as any other iterator. We replace Peter by John
    // and image by its alt text.
    let parser = Parser::new_ext(markdown_input, Options::empty())
        .map(|event| match event {
            Event::Text(text) => Event::Text(text.replace("Peter", "John").into()),
            _ => event,
        })
        .filter(|event| match event {
            Event::Start(Tag::Image { .. }) | Event::End(TagEnd::Image) => false,
            _ => true,
        });

    // Write to anything implementing the `Write` trait. This could also be a file
    // or network socket.
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    handle.write_all(b"\nHTML output:\n").unwrap();
    html::write_html(&mut handle, parser).unwrap();
}
