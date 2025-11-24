use arboard::Clipboard;
use std::{thread, time::Duration};

fn main() {
	env_logger::init();
	let mut ctx = Clipboard::new().unwrap();

	let html = r#"<h1>Hello, World!</h1>
<b>Lorem ipsum</b> dolor sit amet,<br>
<i>consectetur adipiscing elit</i>."#;

	let alt_text = r#"Hello, World!
Lorem ipsum dolor sit amet,
consectetur adipiscing elit."#;

	ctx.set_html(html, Some(alt_text)).unwrap();
	thread::sleep(Duration::from_secs(5));

	let success = ctx.get().html().unwrap() == html;
	println!("Set and Get html operations were successful: {success}");
}
