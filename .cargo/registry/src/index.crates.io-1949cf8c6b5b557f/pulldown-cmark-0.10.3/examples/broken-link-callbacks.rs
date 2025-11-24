use pulldown_cmark::{html, BrokenLink, Options, Parser};

fn main() {
    let input: &str = "Hello world, check out [my website][].";
    println!("Parsing the following markdown string:\n{}", input);

    // Setup callback that sets the URL and title when it encounters
    // a reference to our home page.
    let callback = |broken_link: BrokenLink| {
        if broken_link.reference.as_ref() == "my website" {
            println!(
                "Replacing the markdown `{}` of type {:?} with a working link",
                &input[broken_link.span], broken_link.link_type,
            );
            Some(("http://example.com".into(), "my example website".into()))
        } else {
            None
        }
    };

    // Create a parser with our callback function for broken links.
    let parser = Parser::new_with_broken_link_callback(input, Options::empty(), Some(callback));

    // Write to String buffer.
    let mut html_output: String = String::with_capacity(input.len() * 3 / 2);
    html::push_html(&mut html_output, parser);

    // Check that the output is what we expected.
    let expected_html: &str =
        "<p>Hello world, check out <a href=\"http://example.com\" title=\"my example website\">my website</a>.</p>\n";
    assert_eq!(expected_html, &html_output);

    // Write result to stdout.
    println!("\nHTML output:\n{}", &html_output);
}
