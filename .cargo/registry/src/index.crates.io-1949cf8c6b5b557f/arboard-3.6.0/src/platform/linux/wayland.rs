use std::{borrow::Cow, io::Read, path::PathBuf};

use wl_clipboard_rs::{
	copy::{self, Error as CopyError, MimeSource, MimeType, Options, Source},
	paste::{self, get_contents, Error as PasteError, Seat},
	utils::is_primary_selection_supported,
};

#[cfg(feature = "image-data")]
use super::encode_as_png;
use super::{
	into_unknown, paths_from_uri_list, LinuxClipboardKind, WaitConfig, KDE_EXCLUSION_HINT,
	KDE_EXCLUSION_MIME,
};
use crate::common::Error;
#[cfg(feature = "image-data")]
use crate::common::ImageData;

#[cfg(feature = "image-data")]
const MIME_PNG: &str = "image/png";

pub(crate) struct Clipboard {}

impl TryInto<copy::ClipboardType> for LinuxClipboardKind {
	type Error = Error;

	fn try_into(self) -> Result<copy::ClipboardType, Self::Error> {
		match self {
			LinuxClipboardKind::Clipboard => Ok(copy::ClipboardType::Regular),
			LinuxClipboardKind::Primary => Ok(copy::ClipboardType::Primary),
			LinuxClipboardKind::Secondary => Err(Error::ClipboardNotSupported),
		}
	}
}

impl TryInto<paste::ClipboardType> for LinuxClipboardKind {
	type Error = Error;

	fn try_into(self) -> Result<paste::ClipboardType, Self::Error> {
		match self {
			LinuxClipboardKind::Clipboard => Ok(paste::ClipboardType::Regular),
			LinuxClipboardKind::Primary => Ok(paste::ClipboardType::Primary),
			LinuxClipboardKind::Secondary => Err(Error::ClipboardNotSupported),
		}
	}
}

fn add_clipboard_exclusions(exclude_from_history: bool, sources: &mut Vec<MimeSource>) {
	if exclude_from_history {
		sources.push(MimeSource {
			source: Source::Bytes(Box::from(KDE_EXCLUSION_HINT)),
			mime_type: MimeType::Specific(String::from(KDE_EXCLUSION_MIME)),
		});
	}
}

fn handle_copy_error(e: copy::Error) -> Error {
	match e {
		CopyError::PrimarySelectionUnsupported => Error::ClipboardNotSupported,
		other => into_unknown(other),
	}
}

fn handle_paste_error(e: paste::Error) -> Error {
	match e {
		PasteError::PrimarySelectionUnsupported => Error::ClipboardNotSupported,
		other => into_unknown(other),
	}
}

impl Clipboard {
	pub(crate) fn new() -> Result<Self, Error> {
		// Check if it's possible to communicate with the wayland compositor
		match is_primary_selection_supported() {
			// We don't care if the primary clipboard is supported or not, `wl-clipboard-rs` will fail
			// if not and we don't want to duplicate more of their logic.
			Ok(_) => Ok(Self {}),
			Err(e) => Err(into_unknown(e)),
		}
	}

	fn string_for_mime(
		&mut self,
		selection: LinuxClipboardKind,
		mime: paste::MimeType,
	) -> Result<String, Error> {
		let result = get_contents(selection.try_into()?, Seat::Unspecified, mime);
		match result {
			Ok((mut pipe, _)) => {
				let mut contents = vec![];
				pipe.read_to_end(&mut contents).map_err(into_unknown)?;
				String::from_utf8(contents).map_err(|_| Error::ConversionFailure)
			}
			Err(PasteError::ClipboardEmpty) | Err(PasteError::NoMimeType) => {
				Err(Error::ContentNotAvailable)
			}
			Err(err) => Err(handle_paste_error(err)),
		}
	}

	pub(crate) fn clear(&mut self, selection: LinuxClipboardKind) -> Result<(), Error> {
		let selection = selection.try_into()?;
		copy::clear(selection, copy::Seat::All).map_err(handle_copy_error)
	}

	pub(crate) fn get_text(&mut self, selection: LinuxClipboardKind) -> Result<String, Error> {
		self.string_for_mime(selection, paste::MimeType::Text)
	}

	pub(crate) fn set_text(
		&self,
		text: Cow<'_, str>,
		selection: LinuxClipboardKind,
		wait: WaitConfig,
		exclude_from_history: bool,
	) -> Result<(), Error> {
		let mut opts = Options::new();
		opts.foreground(matches!(wait, WaitConfig::Forever));
		opts.clipboard(selection.try_into()?);

		let mut sources = Vec::with_capacity(if exclude_from_history { 2 } else { 1 });

		sources.push(MimeSource {
			source: Source::Bytes(text.into_owned().into_bytes().into_boxed_slice()),
			mime_type: MimeType::Text,
		});

		add_clipboard_exclusions(exclude_from_history, &mut sources);

		opts.copy_multi(sources).map_err(handle_copy_error)
	}

	pub(crate) fn get_html(&mut self, selection: LinuxClipboardKind) -> Result<String, Error> {
		self.string_for_mime(selection, paste::MimeType::Specific("text/html"))
	}

	pub(crate) fn set_html(
		&self,
		html: Cow<'_, str>,
		alt: Option<Cow<'_, str>>,
		selection: LinuxClipboardKind,
		wait: WaitConfig,
		exclude_from_history: bool,
	) -> Result<(), Error> {
		let mut opts = Options::new();
		opts.foreground(matches!(wait, WaitConfig::Forever));
		opts.clipboard(selection.try_into()?);

		let mut sources = {
			let cap = [true, alt.is_some(), exclude_from_history]
				.map(|v| usize::from(v as u8))
				.iter()
				.sum();
			Vec::with_capacity(cap)
		};

		if let Some(alt) = alt {
			sources.push(MimeSource {
				source: Source::Bytes(alt.into_owned().into_bytes().into_boxed_slice()),
				mime_type: MimeType::Text,
			});
		}

		sources.push(MimeSource {
			source: Source::Bytes(html.into_owned().into_bytes().into_boxed_slice()),
			mime_type: MimeType::Specific(String::from("text/html")),
		});

		add_clipboard_exclusions(exclude_from_history, &mut sources);

		opts.copy_multi(sources).map_err(handle_copy_error)
	}

	#[cfg(feature = "image-data")]
	pub(crate) fn get_image(
		&mut self,
		selection: LinuxClipboardKind,
	) -> Result<ImageData<'static>, Error> {
		use std::io::Cursor;
		use wl_clipboard_rs::paste::MimeType;

		let result =
			get_contents(selection.try_into()?, Seat::Unspecified, MimeType::Specific(MIME_PNG));

		match result {
			Ok((mut pipe, _mime_type)) => {
				let mut buffer = vec![];
				pipe.read_to_end(&mut buffer).map_err(into_unknown)?;
				let image = image::io::Reader::new(Cursor::new(buffer))
					.with_guessed_format()
					.map_err(|_| Error::ConversionFailure)?
					.decode()
					.map_err(|_| Error::ConversionFailure)?;
				let image = image.into_rgba8();

				Ok(ImageData {
					width: image.width() as usize,
					height: image.height() as usize,
					bytes: image.into_raw().into(),
				})
			}

			Err(PasteError::ClipboardEmpty) | Err(PasteError::NoMimeType) => {
				Err(Error::ContentNotAvailable)
			}

			Err(err) => Err(handle_paste_error(err)),
		}
	}

	#[cfg(feature = "image-data")]
	pub(crate) fn set_image(
		&mut self,
		image: ImageData,
		selection: LinuxClipboardKind,
		wait: WaitConfig,
		exclude_from_history: bool,
	) -> Result<(), Error> {
		let mut opts = Options::new();
		opts.foreground(matches!(wait, WaitConfig::Forever));
		opts.clipboard(selection.try_into()?);

		let image = encode_as_png(&image)?;

		let mut sources = Vec::with_capacity(if exclude_from_history { 2 } else { 1 });

		sources.push(MimeSource {
			source: Source::Bytes(image.into()),
			mime_type: MimeType::Specific(String::from(MIME_PNG)),
		});

		add_clipboard_exclusions(exclude_from_history, &mut sources);

		opts.copy_multi(sources).map_err(handle_copy_error)
	}

	pub(crate) fn get_file_list(
		&mut self,
		selection: LinuxClipboardKind,
	) -> Result<Vec<PathBuf>, Error> {
		self.string_for_mime(selection, paste::MimeType::Specific("text/uri-list"))
			.map(paths_from_uri_list)
	}
}
