use std::{borrow::Cow, path::PathBuf, time::Instant};

#[cfg(feature = "wayland-data-control")]
use log::{trace, warn};
use percent_encoding::percent_decode_str;

#[cfg(feature = "image-data")]
use crate::ImageData;
use crate::{common::private, Error};

// Magic strings used in `Set::exclude_from_history()` on linux
const KDE_EXCLUSION_MIME: &str = "x-kde-passwordManagerHint";
const KDE_EXCLUSION_HINT: &[u8] = b"secret";

mod x11;

#[cfg(feature = "wayland-data-control")]
mod wayland;

fn into_unknown<E: std::fmt::Display>(error: E) -> Error {
	Error::Unknown { description: error.to_string() }
}

#[cfg(feature = "image-data")]
fn encode_as_png(image: &ImageData) -> Result<Vec<u8>, Error> {
	use image::ImageEncoder as _;

	if image.bytes.is_empty() || image.width == 0 || image.height == 0 {
		return Err(Error::ConversionFailure);
	}

	let mut png_bytes = Vec::new();
	let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
	encoder
		.write_image(
			image.bytes.as_ref(),
			image.width as u32,
			image.height as u32,
			image::ExtendedColorType::Rgba8,
		)
		.map_err(|_| Error::ConversionFailure)?;

	Ok(png_bytes)
}

fn paths_from_uri_list(uri_list: String) -> Vec<PathBuf> {
	uri_list
		.lines()
		.filter_map(|s| s.strip_prefix("file://"))
		.filter_map(|s| percent_decode_str(s).decode_utf8().ok())
		.map(|decoded| PathBuf::from(decoded.as_ref()))
		.collect()
}

/// Clipboard selection
///
/// Linux has a concept of clipboard "selections" which tend to be used in different contexts. This
/// enum provides a way to get/set to a specific clipboard (the default
/// [`Clipboard`](Self::Clipboard) being used for the common platform API). You can choose which
/// clipboard to use with [`GetExtLinux::clipboard`] and [`SetExtLinux::clipboard`].
///
/// See <https://specifications.freedesktop.org/clipboards-spec/clipboards-0.1.txt> for a better
/// description of the different clipboards.
#[derive(Copy, Clone, Debug)]
pub enum LinuxClipboardKind {
	/// Typically used selection for explicit cut/copy/paste actions (ie. windows/macos like
	/// clipboard behavior)
	Clipboard,

	/// Typically used for mouse selections and/or currently selected text. Accessible via middle
	/// mouse click.
	///
	/// *On Wayland, this may not be available for all systems (requires a compositor supporting
	/// version 2 or above) and operations using this will return an error if unsupported.*
	Primary,

	/// The secondary clipboard is rarely used but theoretically available on X11.
	///
	/// *On Wayland, this is not be available and operations using this variant will return an
	/// error.*
	Secondary,
}

pub(crate) enum Clipboard {
	X11(x11::Clipboard),

	#[cfg(feature = "wayland-data-control")]
	WlDataControl(wayland::Clipboard),
}

impl Clipboard {
	pub(crate) fn new() -> Result<Self, Error> {
		#[cfg(feature = "wayland-data-control")]
		{
			if std::env::var_os("WAYLAND_DISPLAY").is_some() {
				// Wayland is available
				match wayland::Clipboard::new() {
					Ok(clipboard) => {
						trace!("Successfully initialized the Wayland data control clipboard.");
						return Ok(Self::WlDataControl(clipboard));
					}
					Err(e) => warn!(
						"Tried to initialize the wayland data control protocol clipboard, but failed. Falling back to the X11 clipboard protocol. The error was: {}",
						e
					),
				}
			}
		}
		Ok(Self::X11(x11::Clipboard::new()?))
	}
}

pub(crate) struct Get<'clipboard> {
	clipboard: &'clipboard mut Clipboard,
	selection: LinuxClipboardKind,
}

impl<'clipboard> Get<'clipboard> {
	pub(crate) fn new(clipboard: &'clipboard mut Clipboard) -> Self {
		Self { clipboard, selection: LinuxClipboardKind::Clipboard }
	}

	pub(crate) fn text(self) -> Result<String, Error> {
		match self.clipboard {
			Clipboard::X11(clipboard) => clipboard.get_text(self.selection),
			#[cfg(feature = "wayland-data-control")]
			Clipboard::WlDataControl(clipboard) => clipboard.get_text(self.selection),
		}
	}

	#[cfg(feature = "image-data")]
	pub(crate) fn image(self) -> Result<ImageData<'static>, Error> {
		match self.clipboard {
			Clipboard::X11(clipboard) => clipboard.get_image(self.selection),
			#[cfg(feature = "wayland-data-control")]
			Clipboard::WlDataControl(clipboard) => clipboard.get_image(self.selection),
		}
	}

	pub(crate) fn html(self) -> Result<String, Error> {
		match self.clipboard {
			Clipboard::X11(clipboard) => clipboard.get_html(self.selection),
			#[cfg(feature = "wayland-data-control")]
			Clipboard::WlDataControl(clipboard) => clipboard.get_html(self.selection),
		}
	}

	pub(crate) fn file_list(self) -> Result<Vec<PathBuf>, Error> {
		match self.clipboard {
			Clipboard::X11(clipboard) => clipboard.get_file_list(self.selection),
			#[cfg(feature = "wayland-data-control")]
			Clipboard::WlDataControl(clipboard) => clipboard.get_file_list(self.selection),
		}
	}
}

/// Linux-specific extensions to the [`Get`](super::Get) builder.
pub trait GetExtLinux: private::Sealed {
	/// Sets the clipboard the operation will retrieve data from.
	///
	/// If wayland support is enabled and available, attempting to use the Secondary clipboard will
	/// return an error.
	fn clipboard(self, selection: LinuxClipboardKind) -> Self;
}

impl GetExtLinux for crate::Get<'_> {
	fn clipboard(mut self, selection: LinuxClipboardKind) -> Self {
		self.platform.selection = selection;
		self
	}
}

/// Configuration on how long to wait for a new X11 copy event is emitted.
#[derive(Default)]
pub(crate) enum WaitConfig {
	/// Waits until the given [`Instant`] has reached.
	Until(Instant),

	/// Waits forever until a new event is reached.
	Forever,

	/// It shouldn't wait.
	#[default]
	None,
}

pub(crate) struct Set<'clipboard> {
	clipboard: &'clipboard mut Clipboard,
	wait: WaitConfig,
	selection: LinuxClipboardKind,
	exclude_from_history: bool,
}

impl<'clipboard> Set<'clipboard> {
	pub(crate) fn new(clipboard: &'clipboard mut Clipboard) -> Self {
		Self {
			clipboard,
			wait: WaitConfig::default(),
			selection: LinuxClipboardKind::Clipboard,
			exclude_from_history: false,
		}
	}

	pub(crate) fn text(self, text: Cow<'_, str>) -> Result<(), Error> {
		match self.clipboard {
			Clipboard::X11(clipboard) => {
				clipboard.set_text(text, self.selection, self.wait, self.exclude_from_history)
			}

			#[cfg(feature = "wayland-data-control")]
			Clipboard::WlDataControl(clipboard) => {
				clipboard.set_text(text, self.selection, self.wait, self.exclude_from_history)
			}
		}
	}

	pub(crate) fn html(self, html: Cow<'_, str>, alt: Option<Cow<'_, str>>) -> Result<(), Error> {
		match self.clipboard {
			Clipboard::X11(clipboard) => {
				clipboard.set_html(html, alt, self.selection, self.wait, self.exclude_from_history)
			}

			#[cfg(feature = "wayland-data-control")]
			Clipboard::WlDataControl(clipboard) => {
				clipboard.set_html(html, alt, self.selection, self.wait, self.exclude_from_history)
			}
		}
	}

	#[cfg(feature = "image-data")]
	pub(crate) fn image(self, image: ImageData<'_>) -> Result<(), Error> {
		match self.clipboard {
			Clipboard::X11(clipboard) => {
				clipboard.set_image(image, self.selection, self.wait, self.exclude_from_history)
			}

			#[cfg(feature = "wayland-data-control")]
			Clipboard::WlDataControl(clipboard) => {
				clipboard.set_image(image, self.selection, self.wait, self.exclude_from_history)
			}
		}
	}
}

/// Linux specific extensions to the [`Set`](super::Set) builder.
pub trait SetExtLinux: private::Sealed {
	/// Whether to wait for the clipboard's contents to be replaced after setting it.
	///
	/// The Wayland and X11 clipboards work by having the clipboard content being, at any given
	/// time, "owned" by a single process, and that process is expected to reply to all the requests
	/// from any other system process that wishes to access the clipboard's contents. As a
	/// consequence, when that process exits the contents of the clipboard will effectively be
	/// cleared since there is no longer anyone around to serve requests for it.
	///
	/// This poses a problem for short-lived programs that just want to copy to the clipboard and
	/// then exit, since they don't want to wait until the user happens to copy something else just
	/// to finish. To resolve that, whenever the user copies something you can offload the actual
	/// work to a newly-spawned daemon process which will run in the background (potentially
	/// outliving the current process) and serve all the requests. That process will then
	/// automatically and silently exit once the user copies something else to their clipboard so it
	/// doesn't take up too many resources.
	///
	/// To support that pattern, this method will not only have the contents of the clipboard be
	/// set, but will also wait and continue to serve requests until the clipboard is overwritten.
	/// As long as you don't exit the current process until that method has returned, you can avoid
	/// all surprising situations where the clipboard's contents seemingly disappear from under your
	/// feet.
	///
	/// See the [daemonize example] for a demo of how you could implement this.
	///
	/// [daemonize example]: https://github.com/1Password/arboard/blob/master/examples/daemonize.rs
	fn wait(self) -> Self;

	/// Whether or not to wait for the clipboard's content to be replaced after setting it. This waits until the
	/// `deadline` has exceeded.
	///
	/// This is useful for short-lived programs so it won't block until new contents on the clipboard
	/// were added.
	///
	/// Note: this is a superset of [`wait()`][SetExtLinux::wait] and will overwrite any state
	/// that was previously set using it.
	fn wait_until(self, deadline: Instant) -> Self;

	/// Sets the clipboard the operation will store its data to.
	///
	/// If wayland support is enabled and available, attempting to use the Secondary clipboard will
	/// return an error.
	///
	/// # Examples
	///
	/// ```
	/// use arboard::{Clipboard, SetExtLinux, LinuxClipboardKind};
	/// # fn main() -> Result<(), arboard::Error> {
	/// let mut ctx = Clipboard::new()?;
	///
	/// let clipboard = "This goes in the traditional (ex. Copy & Paste) clipboard.";
	/// ctx.set().clipboard(LinuxClipboardKind::Clipboard).text(clipboard.to_owned())?;
	///
	/// let primary = "This goes in the primary keyboard. It's typically used via middle mouse click.";
	/// ctx.set().clipboard(LinuxClipboardKind::Primary).text(primary.to_owned())?;
	/// # Ok(())
	/// # }
	/// ```
	fn clipboard(self, selection: LinuxClipboardKind) -> Self;

	/// Excludes the data which will be set on the clipboard from being added to
	/// the desktop clipboard managers' histories by adding the MIME-Type `x-kde-passwordMangagerHint`
	/// to the clipboard's selection data.
	///
	/// This is the most widely adopted convention on Linux.
	fn exclude_from_history(self) -> Self;
}

impl SetExtLinux for crate::Set<'_> {
	fn wait(mut self) -> Self {
		self.platform.wait = WaitConfig::Forever;
		self
	}

	fn clipboard(mut self, selection: LinuxClipboardKind) -> Self {
		self.platform.selection = selection;
		self
	}

	fn wait_until(mut self, deadline: Instant) -> Self {
		self.platform.wait = WaitConfig::Until(deadline);
		self
	}

	fn exclude_from_history(mut self) -> Self {
		self.platform.exclude_from_history = true;
		self
	}
}

pub(crate) struct Clear<'clipboard> {
	clipboard: &'clipboard mut Clipboard,
}

impl<'clipboard> Clear<'clipboard> {
	pub(crate) fn new(clipboard: &'clipboard mut Clipboard) -> Self {
		Self { clipboard }
	}

	pub(crate) fn clear(self) -> Result<(), Error> {
		self.clear_inner(LinuxClipboardKind::Clipboard)
	}

	fn clear_inner(self, selection: LinuxClipboardKind) -> Result<(), Error> {
		match self.clipboard {
			Clipboard::X11(clipboard) => clipboard.clear(selection),
			#[cfg(feature = "wayland-data-control")]
			Clipboard::WlDataControl(clipboard) => clipboard.clear(selection),
		}
	}
}

/// Linux specific extensions to the [Clear] builder.
pub trait ClearExtLinux: private::Sealed {
	/// Performs the "clear" operation on the selected clipboard.
	///
	/// ### Example
	///
	/// ```no_run
	/// # use arboard::{Clipboard, LinuxClipboardKind, ClearExtLinux, Error};
	/// # fn main() -> Result<(), Error> {
	/// let mut clipboard = Clipboard::new()?;
	///
	/// clipboard
	///     .clear_with()
	///     .clipboard(LinuxClipboardKind::Secondary)?;
	/// # Ok(())
	/// # }
	/// ```
	///
	/// If wayland support is enabled and available, attempting to use the Secondary clipboard will
	/// return an error.
	fn clipboard(self, selection: LinuxClipboardKind) -> Result<(), Error>;
}

impl ClearExtLinux for crate::Clear<'_> {
	fn clipboard(self, selection: LinuxClipboardKind) -> Result<(), Error> {
		self.platform.clear_inner(selection)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_decoding_uri_list() {
		// Test that paths_from_uri_list correctly decodes
		// differents percent encoded characters
		let file_list = [
			"file:///tmp/bar.log",
			"file:///tmp/test%5C.txt",
			"file:///tmp/foo%3F.png",
			"file:///tmp/white%20space.txt",
		];

		let paths = vec![
			PathBuf::from("/tmp/bar.log"),
			PathBuf::from("/tmp/test\\.txt"),
			PathBuf::from("/tmp/foo?.png"),
			PathBuf::from("/tmp/white space.txt"),
		];
		assert_eq!(paths_from_uri_list(file_list.join("\n")), paths);
	}
}
