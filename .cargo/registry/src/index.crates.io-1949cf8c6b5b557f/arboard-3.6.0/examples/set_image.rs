use arboard::{Clipboard, ImageData};

fn main() {
	let mut ctx = Clipboard::new().unwrap();

	#[rustfmt::skip]
	let bytes = [
		255, 100, 100, 255,
		100, 255, 100, 100,
		100, 100, 255, 100,
		0, 0, 0, 255,
	];
	let img_data = ImageData { width: 2, height: 2, bytes: bytes.as_ref().into() };
	ctx.set_image(img_data).unwrap();
}
