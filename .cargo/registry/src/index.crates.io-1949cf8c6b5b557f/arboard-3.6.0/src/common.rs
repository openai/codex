/*
SPDX-License-Identifier: Apache-2.0 OR MIT

Copyright 2022 The Arboard contributors

The project to which this file belongs is licensed under either of
the Apache 2.0 or the MIT license at the licensee's choice. The terms
and conditions of the chosen license apply to this file.
*/

#[cfg(feature = "image-data")]
use std::borrow::Cow;

/// An error that might happen during a clipboard operation.
///
/// Note that both the `Display` and the `Debug` trait is implemented for this type in such a way
/// that they give a short human-readable description of the error; however the documentation
/// gives a more detailed explanation for each error kind.
#[non_exhaustive]
pub enum Error {
	/// The clipboard contents were not available in the requested format.
	/// This could either be due to the clipboard being empty or the clipboard contents having
	/// an incompatible format to the requested one (eg when calling `get_image` on text)
	ContentNotAvailable,

	/// The selected clipboard is not supported by the current configuration (system and/or environment).
	///
	/// This can be caused by a few conditions:
	/// - Using the Primary clipboard with an older Wayland compositor (that doesn't support version 2)
	/// - Using the Secondary clipboard on Wayland
	ClipboardNotSupported,

	/// The native clipboard is not accessible due to being held by an other party.
	///
	/// This "other party" could be a different process or it could be within
	/// the same program. So for example you may get this error when trying
	/// to interact with the clipboard from multiple threads at once.
	///
	/// Note that it's OK to have multiple `Clipboard` instances. The underlying
	/// implementation will make sure that the native clipboard is only
	/// opened for transferring data and then closed as soon as possible.
	ClipboardOccupied,

	/// The image or the text that was about the be transferred to/from the clipboard could not be
	/// converted to the appropriate format.
	ConversionFailure,

	/// Any error that doesn't fit the other error types.
	///
	/// The `description` field is only meant to help the developer and should not be relied on as a
	/// means to identify an error case during runtime.
	Unknown { description: String },
}

impl std::fmt::Display for Error {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Error::ContentNotAvailable => f.write_str("The clipboard contents were not available in the requested format or the clipboard is empty."),
			Error::ClipboardNotSupported => f.write_str("The selected clipboard is not supported with the current system configuration."),
			Error::ClipboardOccupied => f.write_str("The native clipboard is not accessible due to being held by an other party."),
			Error::ConversionFailure => f.write_str("The image or the text that was about the be transferred to/from the clipboard could not be converted to the appropriate format."),
			Error::Unknown { description } => f.write_fmt(format_args!("Unknown error while interacting with the clipboard: {description}")),
		}
	}
}

impl std::error::Error for Error {}

impl std::fmt::Debug for Error {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		use Error::*;
		macro_rules! kind_to_str {
			($( $e: pat ),*) => {
				match self {
					$(
						$e => stringify!($e),
					)*
				}
			}
		}
		let name = kind_to_str!(
			ContentNotAvailable,
			ClipboardNotSupported,
			ClipboardOccupied,
			ConversionFailure,
			Unknown { .. }
		);
		f.write_fmt(format_args!("{name} - \"{self}\""))
	}
}

impl Error {
	pub(crate) fn unknown<M: Into<String>>(message: M) -> Self {
		Error::Unknown { description: message.into() }
	}
}

/// Stores pixel data of an image.
///
/// Each element in `bytes` stores the value of a channel of a single pixel.
/// This struct stores four channels (red, green, blue, alpha) so
/// a `3*3` image is going to be stored on `3*3*4 = 36` bytes of data.
///
/// The pixels are in row-major order meaning that the second pixel
/// in `bytes` (starting at the fifth byte) corresponds to the pixel that's
/// sitting to the right side of the top-left pixel (x=1, y=0)
///
/// Assigning a `2*1` image would for example look like this
/// ```
/// use arboard::ImageData;
/// use std::borrow::Cow;
/// let bytes = [
///     // A red pixel
///     255, 0, 0, 255,
///
///     // A green pixel
///     0, 255, 0, 255,
/// ];
/// let img = ImageData {
///     width: 2,
///     height: 1,
///     bytes: Cow::from(bytes.as_ref())
/// };
/// ```
#[cfg(feature = "image-data")]
#[derive(Debug, Clone)]
pub struct ImageData<'a> {
	pub width: usize,
	pub height: usize,
	pub bytes: Cow<'a, [u8]>,
}

#[cfg(feature = "image-data")]
impl ImageData<'_> {
	/// Returns a the bytes field in a way that it's guaranteed to be owned.
	/// It moves the bytes if they are already owned and clones them if they are borrowed.
	pub fn into_owned_bytes(self) -> Cow<'static, [u8]> {
		self.bytes.into_owned().into()
	}

	/// Returns an image data that is guaranteed to own its bytes.
	/// It moves the bytes if they are already owned and clones them if they are borrowed.
	pub fn to_owned_img(&self) -> ImageData<'static> {
		ImageData {
			width: self.width,
			height: self.height,
			bytes: self.bytes.clone().into_owned().into(),
		}
	}
}

#[cfg(any(windows, all(unix, not(target_os = "macos"))))]
pub(crate) struct ScopeGuard<F: FnOnce()> {
	callback: Option<F>,
}

#[cfg(any(windows, all(unix, not(target_os = "macos"))))]
impl<F: FnOnce()> ScopeGuard<F> {
	#[cfg_attr(all(windows, not(feature = "image-data")), allow(dead_code))]
	pub(crate) fn new(callback: F) -> Self {
		ScopeGuard { callback: Some(callback) }
	}
}

#[cfg(any(windows, all(unix, not(target_os = "macos"))))]
impl<F: FnOnce()> Drop for ScopeGuard<F> {
	fn drop(&mut self) {
		if let Some(callback) = self.callback.take() {
			(callback)();
		}
	}
}

/// Common trait for sealing platform extension traits.
pub(crate) mod private {
	pub trait Sealed {}

	impl Sealed for crate::Get<'_> {}
	impl Sealed for crate::Set<'_> {}
	impl Sealed for crate::Clear<'_> {}
}
