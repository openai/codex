/*
SPDX-License-Identifier: Apache-2.0 OR MIT

Copyright 2022 The Arboard contributors

The project to which this file belongs is licensed under either of
the Apache 2.0 or the MIT license at the licensee's choice. The terms
and conditions of the chosen license apply to this file.
*/
#![warn(unreachable_pub)]

mod common;
use std::{borrow::Cow, path::PathBuf};

pub use common::Error;
#[cfg(feature = "image-data")]
pub use common::ImageData;

mod platform;

#[cfg(all(
	unix,
	not(any(target_os = "macos", target_os = "android", target_os = "emscripten")),
))]
pub use platform::{ClearExtLinux, GetExtLinux, LinuxClipboardKind, SetExtLinux};

#[cfg(windows)]
pub use platform::SetExtWindows;

#[cfg(target_os = "macos")]
pub use platform::SetExtApple;

/// The OS independent struct for accessing the clipboard.
///
/// Any number of `Clipboard` instances are allowed to exist at a single point in time. Note however
/// that all `Clipboard`s must be 'dropped' before the program exits. In most scenarios this happens
/// automatically but there are frameworks (for example, `winit`) that take over the execution
/// and where the objects don't get dropped when the application exits. In these cases you have to
/// make sure the object is dropped by taking ownership of it in a confined scope when detecting
/// that your application is about to quit.
///
/// It is also valid to have these multiple `Clipboards` on separate threads at once but note that
/// executing multiple clipboard operations in parallel might fail with a `ClipboardOccupied` error.
///
/// # Platform-specific behavior
///
/// `arboard` does its best to abstract over different platforms, but sometimes the platform-specific
/// behavior leaks through unsolvably. These differences, depending on which platforms are being targeted,
/// may affect your app's clipboard architecture (ex, opening and closing a [`Clipboard`] every time
/// or keeping one open in some application/global state).
///
/// ## Linux
///
/// Using either Wayland and X11, the clipboard and its content is "hosted" inside of the application
/// that last put data onto it. This means that when the last `Clipboard` instance is dropped, the contents
/// may become unavailable to other apps. See [SetExtLinux] for more details.
///
/// ## Windows
///
/// The clipboard on Windows is a global object, which may only be opened on one thread at once.
/// This means that `arboard` only truly opens the clipboard during each operation to prevent
/// multiple `Clipboard`s from existing at once.
///
/// This means that attempting operations in parallel has a high likelihood to return an error or
/// deadlock. As such, it is recommended to avoid creating/operating clipboard objects on >1 thread.
#[allow(rustdoc::broken_intra_doc_links)]
pub struct Clipboard {
	pub(crate) platform: platform::Clipboard,
}

impl Clipboard {
	/// Creates an instance of the clipboard.
	///
	/// # Errors
	///
	/// On some platforms or desktop environments, an error can be returned if clipboards are not
	/// supported. This may be retried.
	pub fn new() -> Result<Self, Error> {
		Ok(Clipboard { platform: platform::Clipboard::new()? })
	}

	/// Fetches UTF-8 text from the clipboard and returns it.
	///
	/// # Errors
	///
	/// Returns error if clipboard is empty or contents are not UTF-8 text.
	pub fn get_text(&mut self) -> Result<String, Error> {
		self.get().text()
	}

	/// Places the text onto the clipboard. Any valid UTF-8 string is accepted.
	///
	/// # Errors
	///
	/// Returns error if `text` failed to be stored on the clipboard.
	pub fn set_text<'a, T: Into<Cow<'a, str>>>(&mut self, text: T) -> Result<(), Error> {
		self.set().text(text)
	}

	/// Places the HTML as well as a plain-text alternative onto the clipboard.
	///
	/// Any valid UTF-8 string is accepted.
	///
	/// # Errors
	///
	/// Returns error if both `html` and `alt_text` failed to be stored on the clipboard.
	pub fn set_html<'a, T: Into<Cow<'a, str>>>(
		&mut self,
		html: T,
		alt_text: Option<T>,
	) -> Result<(), Error> {
		self.set().html(html, alt_text)
	}

	/// Fetches image data from the clipboard, and returns the decoded pixels.
	///
	/// Any image data placed on the clipboard with `set_image` will be possible read back, using
	/// this function. However it's of not guaranteed that an image placed on the clipboard by any
	/// other application will be of a supported format.
	///
	/// # Errors
	///
	/// Returns error if clipboard is empty, contents are not an image, or the contents cannot be
	/// converted to an appropriate format and stored in the [`ImageData`] type.
	#[cfg(feature = "image-data")]
	pub fn get_image(&mut self) -> Result<ImageData<'static>, Error> {
		self.get().image()
	}

	/// Places an image to the clipboard.
	///
	/// The chosen output format, depending on the platform is the following:
	///
	/// - On macOS: `NSImage` object
	/// - On Linux: PNG, under the atom `image/png`
	/// - On Windows: In order of priority `CF_DIB` and `CF_BITMAP`
	///
	/// # Errors
	///
	/// Returns error if `image` cannot be converted to an appropriate format or if it failed to be
	/// stored on the clipboard.
	#[cfg(feature = "image-data")]
	pub fn set_image(&mut self, image: ImageData) -> Result<(), Error> {
		self.set().image(image)
	}

	/// Clears any contents that may be present from the platform's default clipboard,
	/// regardless of the format of the data.
	///
	/// # Errors
	///
	/// Returns error on Windows or Linux if clipboard cannot be cleared.
	pub fn clear(&mut self) -> Result<(), Error> {
		self.clear_with().default()
	}

	/// Begins a "clear" option to remove data from the clipboard.
	pub fn clear_with(&mut self) -> Clear<'_> {
		Clear { platform: platform::Clear::new(&mut self.platform) }
	}

	/// Begins a "get" operation to retrieve data from the clipboard.
	pub fn get(&mut self) -> Get<'_> {
		Get { platform: platform::Get::new(&mut self.platform) }
	}

	/// Begins a "set" operation to set the clipboard's contents.
	pub fn set(&mut self) -> Set<'_> {
		Set { platform: platform::Set::new(&mut self.platform) }
	}
}

/// A builder for an operation that gets a value from the clipboard.
#[must_use]
pub struct Get<'clipboard> {
	pub(crate) platform: platform::Get<'clipboard>,
}

impl Get<'_> {
	/// Completes the "get" operation by fetching UTF-8 text from the clipboard.
	pub fn text(self) -> Result<String, Error> {
		self.platform.text()
	}

	/// Completes the "get" operation by fetching image data from the clipboard and returning the
	/// decoded pixels.
	///
	/// Any image data placed on the clipboard with `set_image` will be possible read back, using
	/// this function. However it's of not guaranteed that an image placed on the clipboard by any
	/// other application will be of a supported format.
	#[cfg(feature = "image-data")]
	pub fn image(self) -> Result<ImageData<'static>, Error> {
		self.platform.image()
	}

	/// Completes the "get" operation by fetching HTML from the clipboard.
	pub fn html(self) -> Result<String, Error> {
		self.platform.html()
	}

	/// Completes the "get" operation by fetching a list of file paths from the clipboard.
	pub fn file_list(self) -> Result<Vec<PathBuf>, Error> {
		self.platform.file_list()
	}
}

/// A builder for an operation that sets a value to the clipboard.
#[must_use]
pub struct Set<'clipboard> {
	pub(crate) platform: platform::Set<'clipboard>,
}

impl Set<'_> {
	/// Completes the "set" operation by placing text onto the clipboard. Any valid UTF-8 string
	/// is accepted.
	pub fn text<'a, T: Into<Cow<'a, str>>>(self, text: T) -> Result<(), Error> {
		let text = text.into();
		self.platform.text(text)
	}

	/// Completes the "set" operation by placing HTML as well as a plain-text alternative onto the
	/// clipboard.
	///
	/// Any valid UTF-8 string is accepted.
	pub fn html<'a, T: Into<Cow<'a, str>>>(
		self,
		html: T,
		alt_text: Option<T>,
	) -> Result<(), Error> {
		let html = html.into();
		let alt_text = alt_text.map(|e| e.into());
		self.platform.html(html, alt_text)
	}

	/// Completes the "set" operation by placing an image onto the clipboard.
	///
	/// The chosen output format, depending on the platform is the following:
	///
	/// - On macOS: `NSImage` object
	/// - On Linux: PNG, under the atom `image/png`
	/// - On Windows: In order of priority `CF_DIB` and `CF_BITMAP`
	#[cfg(feature = "image-data")]
	pub fn image(self, image: ImageData) -> Result<(), Error> {
		self.platform.image(image)
	}
}

/// A builder for an operation that clears the data from the clipboard.
#[must_use]
pub struct Clear<'clipboard> {
	pub(crate) platform: platform::Clear<'clipboard>,
}

impl Clear<'_> {
	/// Completes the "clear" operation by deleting any existing clipboard data,
	/// regardless of the format.
	pub fn default(self) -> Result<(), Error> {
		self.platform.clear()
	}
}

/// All tests grouped in one because the windows clipboard cannot be open on
/// multiple threads at once.
#[cfg(test)]
mod tests {
	use super::*;
	use std::{sync::Arc, thread, time::Duration};

	#[test]
	fn all_tests() {
		let _ = env_logger::builder().is_test(true).try_init();
		{
			let mut ctx = Clipboard::new().unwrap();
			let text = "some string";
			ctx.set_text(text).unwrap();
			assert_eq!(ctx.get_text().unwrap(), text);

			// We also need to check that the content persists after the drop; this is
			// especially important on X11
			drop(ctx);

			// Give any external mechanism a generous amount of time to take over
			// responsibility for the clipboard, in case that happens asynchronously
			// (it appears that this is the case on X11 plus Mutter 3.34+, see #4)
			thread::sleep(Duration::from_millis(300));

			let mut ctx = Clipboard::new().unwrap();
			assert_eq!(ctx.get_text().unwrap(), text);
		}
		{
			let mut ctx = Clipboard::new().unwrap();
			let text = "Some utf8: 🤓 ∑φ(n)<ε 🐔";
			ctx.set_text(text).unwrap();
			assert_eq!(ctx.get_text().unwrap(), text);
		}
		{
			let mut ctx = Clipboard::new().unwrap();
			let text = "hello world";

			ctx.set_text(text).unwrap();
			assert_eq!(ctx.get_text().unwrap(), text);

			ctx.clear().unwrap();

			match ctx.get_text() {
				Ok(text) => assert!(text.is_empty()),
				Err(Error::ContentNotAvailable) => {}
				Err(e) => panic!("unexpected error: {e}"),
			};

			// confirm it is OK to clear when already empty.
			ctx.clear().unwrap();
		}
		{
			let mut ctx = Clipboard::new().unwrap();
			let html = "<b>hello</b> <i>world</i>!";

			ctx.set_html(html, None).unwrap();

			match ctx.get_text() {
				Ok(text) => assert!(text.is_empty()),
				Err(Error::ContentNotAvailable) => {}
				Err(e) => panic!("unexpected error: {e}"),
			};
		}
		{
			let mut ctx = Clipboard::new().unwrap();

			let html = "<b>hello</b> <i>world</i>!";
			let alt_text = "hello world!";

			ctx.set_html(html, Some(alt_text)).unwrap();
			assert_eq!(ctx.get_text().unwrap(), alt_text);
		}
		{
			let mut ctx = Clipboard::new().unwrap();

			let html = "<b>hello</b> <i>world</i>!";

			ctx.set().html(html, None).unwrap();

			if cfg!(target_os = "macos") {
				// Copying HTML on macOS adds wrapper content to work around
				// historical platform bugs. We control this wrapper, so we are
				// able to check that the full user data still appears and at what
				// position in the final copy contents.
				let content = ctx.get().html().unwrap();
				assert!(content.ends_with(&format!("{html}</body></html>")));
			} else {
				assert_eq!(ctx.get().html().unwrap(), html);
			}
		}
		#[cfg(feature = "image-data")]
		{
			let mut ctx = Clipboard::new().unwrap();
			#[rustfmt::skip]
			let bytes = [
				255, 100, 100, 255,
				100, 255, 100, 100,
				100, 100, 255, 100,
				0, 0, 0, 255,
			];
			let img_data = ImageData { width: 2, height: 2, bytes: bytes.as_ref().into() };

			// Make sure that setting one format overwrites the other.
			ctx.set_image(img_data.clone()).unwrap();
			assert!(matches!(ctx.get_text(), Err(Error::ContentNotAvailable)));

			ctx.set_text("clipboard test").unwrap();
			assert!(matches!(ctx.get_image(), Err(Error::ContentNotAvailable)));

			// Test if we get the same image that we put onto the clipboard
			ctx.set_image(img_data.clone()).unwrap();
			let got = ctx.get_image().unwrap();
			assert_eq!(img_data.bytes, got.bytes);

			#[rustfmt::skip]
			let big_bytes = vec![
				255, 100, 100, 255,
				100, 255, 100, 100,
				100, 100, 255, 100,

				0, 1, 2, 255,
				0, 1, 2, 255,
				0, 1, 2, 255,
			];
			let bytes_cloned = big_bytes.clone();
			let big_img_data = ImageData { width: 3, height: 2, bytes: big_bytes.into() };
			ctx.set_image(big_img_data).unwrap();
			let got = ctx.get_image().unwrap();
			assert_eq!(bytes_cloned.as_slice(), got.bytes.as_ref());
		}
		#[cfg(all(
			unix,
			not(any(target_os = "macos", target_os = "android", target_os = "emscripten")),
		))]
		{
			use crate::{LinuxClipboardKind, SetExtLinux};
			use std::sync::atomic::{self, AtomicBool};

			let mut ctx = Clipboard::new().unwrap();

			const TEXT1: &str = "I'm a little teapot,";
			const TEXT2: &str = "short and stout,";
			const TEXT3: &str = "here is my handle";

			ctx.set().clipboard(LinuxClipboardKind::Clipboard).text(TEXT1.to_string()).unwrap();

			ctx.set().clipboard(LinuxClipboardKind::Primary).text(TEXT2.to_string()).unwrap();

			// The secondary clipboard is not available under wayland
			if !cfg!(feature = "wayland-data-control")
				|| std::env::var_os("WAYLAND_DISPLAY").is_none()
			{
				ctx.set().clipboard(LinuxClipboardKind::Secondary).text(TEXT3.to_string()).unwrap();
			}

			assert_eq!(TEXT1, &ctx.get().clipboard(LinuxClipboardKind::Clipboard).text().unwrap());

			assert_eq!(TEXT2, &ctx.get().clipboard(LinuxClipboardKind::Primary).text().unwrap());

			// The secondary clipboard is not available under wayland
			if !cfg!(feature = "wayland-data-control")
				|| std::env::var_os("WAYLAND_DISPLAY").is_none()
			{
				assert_eq!(
					TEXT3,
					&ctx.get().clipboard(LinuxClipboardKind::Secondary).text().unwrap()
				);
			}

			let was_replaced = Arc::new(AtomicBool::new(false));

			let setter = thread::spawn({
				let was_replaced = was_replaced.clone();
				move || {
					thread::sleep(Duration::from_millis(100));
					let mut ctx = Clipboard::new().unwrap();
					ctx.set_text("replacement text".to_owned()).unwrap();
					was_replaced.store(true, atomic::Ordering::Release);
				}
			});

			ctx.set().wait().text("initial text".to_owned()).unwrap();

			assert!(was_replaced.load(atomic::Ordering::Acquire));

			setter.join().unwrap();
		}
	}

	// The cross-platform abstraction should allow any number of clipboards
	// to be open at once without issue, as documented under [Clipboard].
	#[test]
	fn multiple_clipboards_at_once() {
		const THREAD_COUNT: usize = 100;

		let mut handles = Vec::with_capacity(THREAD_COUNT);
		let barrier = Arc::new(std::sync::Barrier::new(THREAD_COUNT));

		for _ in 0..THREAD_COUNT {
			let barrier = barrier.clone();
			handles.push(thread::spawn(move || {
				// As long as the clipboard isn't used multiple times at once, multiple instances
				// are perfectly fine.
				let _ctx = Clipboard::new().unwrap();

				thread::sleep(Duration::from_millis(10));

				barrier.wait();
			}));
		}

		for thread_handle in handles {
			thread_handle.join().unwrap();
		}
	}

	#[test]
	fn clipboard_trait_consistently() {
		fn assert_send_sync<T: Send + Sync + 'static>() {}

		assert_send_sync::<Clipboard>();
		assert!(std::mem::needs_drop::<Clipboard>());
	}
}
