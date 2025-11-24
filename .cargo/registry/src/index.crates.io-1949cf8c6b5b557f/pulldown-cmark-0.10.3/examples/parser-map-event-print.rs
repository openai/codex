use pulldown_cmark::{html, Event, Parser};

fn main() {
    let markdown_input = "# Example Heading\nExample paragraph with **lorem** _ipsum_ text.";
    println!(
        "\nParsing the following markdown string:\n{}\n",
        markdown_input
    );

    // Set up the parser. We can treat is as any other iterator.
    // For each event, we print its details, such as the tag or string.
    // This filter simply returns the same event without any changes;
    // you can compare the `event-filter` example which alters the output.
    let parser = Parser::new(markdown_input).map(|event| {
        match &event {
            Event::Start(tag) => println!("Start: {:?}", tag),
            Event::End(tag) => println!("End: {:?}", tag),
            Event::Html(s) => println!("Html: {:?}", s),
            Event::InlineHtml(s) => println!("InlineHtml: {:?}", s),
            Event::Text(s) => println!("Text: {:?}", s),
            Event::Code(s) => println!("Code: {:?}", s),
            Event::FootnoteReference(s) => println!("FootnoteReference: {:?}", s),
            Event::TaskListMarker(b) => println!("TaskListMarker: {:?}", b),
            Event::SoftBreak => println!("SoftBreak"),
            Event::HardBreak => println!("HardBreak"),
            Event::Rule => println!("Rule"),
        };
        event
    });

    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    println!("\nHTML output:\n{}\n", &html_output);
}
