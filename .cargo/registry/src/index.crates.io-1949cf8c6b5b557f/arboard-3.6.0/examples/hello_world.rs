use arboard::Clipboard;

fn main() {
	env_logger::init();
	let mut clipboard = Clipboard::new().unwrap();
	println!("Clipboard text was: {:?}", clipboard.get_text());

	let the_string = "Hello, world!";
	clipboard.set_text(the_string).unwrap();
	println!("But now the clipboard text should be: \"{the_string}\"");
}
