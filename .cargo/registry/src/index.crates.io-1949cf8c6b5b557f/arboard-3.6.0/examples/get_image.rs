use arboard::Clipboard;

fn main() {
	let mut ctx = Clipboard::new().unwrap();

	let img = ctx.get_image().unwrap();

	println!("Image data is:\n{:?}", img.bytes);
}
