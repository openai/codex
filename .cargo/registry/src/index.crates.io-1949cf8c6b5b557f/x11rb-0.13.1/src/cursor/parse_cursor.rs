//! Parse the contents of a cursor file

// This code is loosely based on parse_cursor_file.c from libxcb-cursor, which is:
//   Copyright © 2013 Michael Stapelberg
// and is covered by MIT/X Consortium License

use std::io::{Read, Seek, SeekFrom};

const FILE_MAGIC: u32 = 0x7275_6358;
const IMAGE_TYPE: u32 = 0xfffd_0002;
const IMAGE_MAX_SIZE: u16 = 0x7fff;

/// An error that occurred while parsing
#[derive(Debug)]
pub(crate) enum Error {
    /// An I/O error occurred
    Io,

    /// The file did not begin with the expected magic
    InvalidMagic,

    /// The file contained unrealistically many images
    TooManyEntries,

    /// The file contains no images
    NoImages,

    /// Some image's entry is corrupt and not self-consistent
    CorruptImage,

    /// An entry is larger than the maximum size
    ImageTooLarge,
}

impl From<std::io::Error> for Error {
    fn from(_: std::io::Error) -> Self {
        Error::Io
    }
}

/// Read a single `u32` from a cursor file
///
/// The file stores these entries in little endian.
fn read_u32<R: Read>(read: &mut R) -> Result<u32, Error> {
    let mut buffer = [0; 4];
    read.read_exact(&mut buffer)?;
    Ok(u32::from_le_bytes(buffer))
}

/// A single cursor image
#[derive(Debug)]
pub(crate) struct Image {
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) x_hot: u16,
    pub(crate) y_hot: u16,
    pub(crate) delay: u32,
    pub(crate) pixels: Vec<u32>,
}

impl Image {
    /// Read an `Image` from a reader
    fn read<R: Read>(read: &mut R, expected_kind: u32, expected_size: u32) -> Result<Self, Error> {
        let (_header, kind, size, _version) = (
            read_u32(read)?,
            read_u32(read)?,
            read_u32(read)?,
            read_u32(read)?,
        );
        if (kind, size) != (expected_kind, expected_size) {
            return Err(Error::CorruptImage);
        }

        fn convert_size(size: u32) -> Result<u16, Error> {
            size.try_into()
                .ok()
                .filter(|&size| size <= IMAGE_MAX_SIZE)
                .ok_or(Error::ImageTooLarge)
        }

        let (width, height) = (
            convert_size(read_u32(read)?)?,
            convert_size(read_u32(read)?)?,
        );
        let (x_hot, y_hot) = (read_u32(read)?, read_u32(read)?);
        let delay = read_u32(read)?;
        let x_hot = x_hot.try_into().or(Err(Error::ImageTooLarge))?;
        let y_hot = y_hot.try_into().or(Err(Error::ImageTooLarge))?;

        let num_pixels = u32::from(width) * u32::from(height);
        let pixels = (0..num_pixels)
            .map(|_| read_u32(read))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Image {
            width,
            height,
            x_hot,
            y_hot,
            delay,
            pixels,
        })
    }
}

/// A TOC entry in a cursor file
#[derive(Debug)]
struct TocEntry {
    kind: u32,
    size: u32,
    position: u32,
}

impl TocEntry {
    /// Read a `TocEntry` from a reader
    fn read<R: Read>(read: &mut R) -> Result<Self, Error> {
        Ok(TocEntry {
            kind: read_u32(read)?,
            size: read_u32(read)?,
            position: read_u32(read)?,
        })
    }
}

/// Find the size of the image in the toc with a size as close as possible to the desired size.
fn find_best_size(toc: &[TocEntry], desired_size: u32) -> Result<u32, Error> {
    fn distance(a: u32, b: u32) -> u32 {
        a.max(b) - a.min(b)
    }

    fn is_better(desired_size: u32, entry: &TocEntry, result: &Result<u32, Error>) -> bool {
        match result {
            Err(_) => true,
            Ok(size) => distance(entry.size, desired_size) < distance(*size, desired_size),
        }
    }

    let mut result = Err(Error::NoImages);
    for entry in toc {
        // If this is better than the best so far, replace best
        if entry.kind == IMAGE_TYPE && is_better(desired_size, entry, &result) {
            result = Ok(entry.size)
        }
    }
    result
}

/// Parse a complete cursor file
pub(crate) fn parse_cursor<R: Read + Seek>(
    input: &mut R,
    desired_size: u32,
) -> Result<Vec<Image>, Error> {
    let (magic, header, _version, ntoc) = (
        read_u32(input)?,
        read_u32(input)?,
        read_u32(input)?,
        read_u32(input)?,
    );

    if magic != FILE_MAGIC {
        return Err(Error::InvalidMagic);
    }

    if ntoc > 0x1_0000 {
        return Err(Error::TooManyEntries);
    }

    // Read the table of contents
    let _ = input.seek(SeekFrom::Start(header.into()))?;
    let toc = (0..ntoc)
        .map(|_| TocEntry::read(input))
        .collect::<Result<Vec<_>, _>>()?;

    // Find the cursor size that we should load
    let size = find_best_size(&toc, desired_size)?;

    // Now load all cursors that have the right size
    let mut result = Vec::new();
    for entry in toc {
        if entry.kind != IMAGE_TYPE || entry.size != size {
            continue;
        }
        let _ = input.seek(SeekFrom::Start(entry.position.into()))?;
        result.push(Image::read(input, entry.kind, entry.size)?);
    }

    Ok(result)
}

#[cfg(test)]
mod test {
    use super::{find_best_size, parse_cursor, Error, Image, TocEntry, IMAGE_TYPE};
    use std::io::Cursor;

    #[test]
    fn read_3x5_image() {
        let data = [
            0x00, 0x00, 0x00, 0x00, // header(?)
            0x02, 0x00, 0xfd, 0xff, // IMAGE_TYPE
            0x04, 0x00, 0x00, 0x00, // size 4
            0x00, 0x00, 0x00, 0x00, // version(?)
            0x03, 0x00, 0x00, 0x00, // width 3
            0x05, 0x00, 0x00, 0x00, // height 5
            0x07, 0x00, 0x00, 0x00, // x_hot 7
            0x08, 0x00, 0x00, 0x00, // y_hot 8
            0x2a, 0x00, 0x00, 0x00, // delay 42
            // pixels
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c,
            0x1d, 0x1e, 0x1f, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a,
            0x2b, 0x2c, 0x2d, 0x2e, 0x2f, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38,
            0x39, 0x3a, 0x3b, 0x3c,
        ];
        let image = Image::read(&mut Cursor::new(&data[..]), IMAGE_TYPE, 4).unwrap();
        assert_eq!(image.width, 3);
        assert_eq!(image.height, 5);
        assert_eq!(image.x_hot, 7);
        assert_eq!(image.y_hot, 8);
        assert_eq!(image.delay, 42);
        assert_eq!(
            image.pixels,
            &[
                0x0403_0201,
                0x0807_0605,
                0x0c0b_0a09,
                0x100f_0e0d,
                0x1413_1211,
                0x1817_1615,
                0x1c1b_1a19,
                0x201f_1e1d,
                0x2423_2221,
                0x2827_2625,
                0x2c2b_2a29,
                0x302f_2e2d,
                0x3433_3231,
                0x3837_3635,
                0x3c3b_3a39,
            ]
        );
    }

    #[test]
    fn read_corrupt_image_wrong_kind() {
        let data = [
            0x00, 0x00, 0x00, 0x00, // header(?)
            0x01, 0x00, 0xfd, 0xff, // IMAGE_TYPE - 1
            0x04, 0x00, 0x00, 0x00, // size 4
            0x00, 0x00, 0x00, 0x00, // version(?)
            0x00, 0x00, 0x00, 0x00, // width 0
            0x00, 0x00, 0x00, 0x00, // height 0
            0x00, 0x00, 0x00, 0x00, // x_hot 0
            0x00, 0x00, 0x00, 0x00, // y_hot 0
            0x00, 0x00, 0x00, 0x00, // delay 0
        ];
        match Image::read(&mut Cursor::new(&data[..]), IMAGE_TYPE, 4) {
            Err(Error::CorruptImage) => {}
            r => panic!("Unexpected result {:?}", r),
        }
    }

    #[test]
    fn read_corrupt_image_wrong_size() {
        let data = [
            0x00, 0x00, 0x00, 0x00, // header(?)
            0x02, 0x00, 0xfd, 0xff, // IMAGE_TYPE
            0x04, 0x00, 0x00, 0x00, // size 4
            0x00, 0x00, 0x00, 0x00, // version(?)
            0x00, 0x00, 0x00, 0x00, // width 0
            0x00, 0x00, 0x00, 0x00, // height 0
            0x00, 0x00, 0x00, 0x00, // x_hot 0
            0x00, 0x00, 0x00, 0x00, // y_hot 0
            0x00, 0x00, 0x00, 0x00, // delay 0
        ];
        match Image::read(&mut Cursor::new(&data[..]), IMAGE_TYPE, 42) {
            Err(Error::CorruptImage) => {}
            r => panic!("Unexpected result {:?}", r),
        }
    }

    #[test]
    fn read_image_too_large_width() {
        let data = [
            0x00, 0x00, 0x00, 0x00, // header(?)
            0x02, 0x00, 0xfd, 0xff, // IMAGE_TYPE
            0x04, 0x00, 0x00, 0x00, // size 4
            0x00, 0x00, 0x00, 0x00, // version(?)
            0x00, 0x80, 0x00, 0x00, // width IMAGE_MAX_SIZE + 1
            0x00, 0x00, 0x00, 0x00, // height 0
            0x00, 0x00, 0x00, 0x00, // x_hot 0
            0x00, 0x00, 0x00, 0x00, // y_hot 0
            0x00, 0x00, 0x00, 0x00, // delay 0
        ];
        match Image::read(&mut Cursor::new(&data[..]), IMAGE_TYPE, 4) {
            Err(Error::ImageTooLarge) => {}
            r => panic!("Unexpected result {:?}", r),
        }
    }

    #[test]
    fn read_image_too_large_height() {
        let data = [
            0x00, 0x00, 0x00, 0x00, // header(?)
            0x02, 0x00, 0xfd, 0xff, // IMAGE_TYPE
            0x04, 0x00, 0x00, 0x00, // size 4
            0x00, 0x00, 0x00, 0x00, // version(?)
            0x00, 0x00, 0x00, 0x00, // width 0
            0x00, 0x80, 0x00, 0x00, // height IMAGE_MAX_SIZE + 1
            0x00, 0x00, 0x00, 0x00, // x_hot 0
            0x00, 0x00, 0x00, 0x00, // y_hot 0
            0x00, 0x00, 0x00, 0x00, // delay 0
        ];
        match Image::read(&mut Cursor::new(&data[..]), IMAGE_TYPE, 4) {
            Err(Error::ImageTooLarge) => {}
            r => panic!("Unexpected result {:?}", r),
        }
    }

    #[test]
    fn read_image_too_short() {
        let data = [];
        match Image::read(&mut Cursor::new(&data[..]), IMAGE_TYPE, 4) {
            Err(Error::Io) => {}
            r => panic!("Unexpected result {:?}", r),
        }
    }

    #[test]
    fn find_best_size_empty_input() {
        let res = find_best_size(&[], 42);
        match res {
            Err(Error::NoImages) => {}
            r => panic!("Unexpected result {:?}", r),
        }
    }

    #[test]
    fn find_best_size_one_input() {
        let input = [TocEntry {
            kind: IMAGE_TYPE,
            size: 42,
            position: 42,
        }];
        assert_eq!(42, find_best_size(&input, 10).unwrap());
    }

    #[test]
    fn find_best_size_selects_better() {
        let input = [
            TocEntry {
                kind: IMAGE_TYPE,
                size: 42,
                position: 42,
            },
            TocEntry {
                kind: IMAGE_TYPE,
                size: 32,
                position: 42,
            },
            TocEntry {
                kind: IMAGE_TYPE,
                size: 3,
                position: 42,
            },
            TocEntry {
                kind: IMAGE_TYPE,
                size: 22,
                position: 42,
            },
            TocEntry {
                kind: 0,
                size: 10,
                position: 42,
            },
        ];
        assert_eq!(3, find_best_size(&input, 10).unwrap());
    }

    #[test]
    fn parse_cursor_too_short() {
        let data = [];
        match parse_cursor(&mut Cursor::new(&data[..]), 10) {
            Err(Error::Io) => {}
            r => panic!("Unexpected result {:?}", r),
        }
    }

    #[test]
    fn parse_cursor_incorrect_magic() {
        let data = [
            0x00, 0x00, 0x00, 0x00, // magic
            0x00, 0x00, 0x00, 0x00, // header file offset
            0x00, 0x00, 0x00, 0x00, // version(?)
            0x00, 0x00, 0x00, 0x00, // num TOC entries
        ];
        match parse_cursor(&mut Cursor::new(&data[..]), 10) {
            Err(Error::InvalidMagic) => {}
            r => panic!("Unexpected result {:?}", r),
        }
    }

    #[test]
    fn parse_cursor_too_many_entries() {
        let data = [
            b'X', b'c', b'u', b'r', // magic
            0x00, 0x00, 0x00, 0x00, // header file offset
            0x00, 0x00, 0x00, 0x00, // version(?)
            0x01, 0x00, 0x01, 0x00, // num TOC entries, limit + 1
        ];
        match parse_cursor(&mut Cursor::new(&data[..]), 10) {
            Err(Error::TooManyEntries) => {}
            r => panic!("Unexpected result {:?}", r),
        }
    }

    #[test]
    fn parse_cursor_empty_toc() {
        let data = [
            b'X', b'c', b'u', b'r', // magic
            0x10, 0x00, 0x00, 0x00, // header file offset (16)
            0x00, 0x00, 0x00, 0x00, // version(?)
            0x00, 0x00, 0x00, 0x00, // num TOC entries, 0
        ];
        match parse_cursor(&mut Cursor::new(&data[..]), 10) {
            Err(Error::NoImages) => {}
            r => panic!("Unexpected result {:?}", r),
        }
    }

    #[test]
    fn parse_cursor_one_image() {
        let data = [
            b'X', b'c', b'u', b'r', // magic
            0x10, 0x00, 0x00, 0x00, // header file offset (16)
            0x00, 0x00, 0x00, 0x00, // version(?)
            0x01, 0x00, 0x00, 0x00, // num TOC entries, 1
            // TOC
            0x02, 0x00, 0xfd, 0xff, // IMAGE_TYPE
            0x04, 0x00, 0x00, 0x00, // size 4
            0x1c, 0x00, 0x00, 0x00, // image offset (28)
            // image
            0x00, 0x00, 0x00, 0x00, // header(?)
            0x02, 0x00, 0xfd, 0xff, // IMAGE_TYPE
            0x04, 0x00, 0x00, 0x00, // size 4
            0x00, 0x00, 0x00, 0x00, // version(?)
            0x00, 0x00, 0x00, 0x00, // width 0
            0x00, 0x00, 0x00, 0x00, // height 0
            0x00, 0x00, 0x00, 0x00, // x_hot 0
            0x00, 0x00, 0x00, 0x00, // y_hot 0
            0x00, 0x00, 0x00, 0x00, // delay 0
        ];
        let expected = [Image {
            width: 0,
            height: 0,
            x_hot: 0,
            y_hot: 0,
            delay: 0,
            pixels: vec![],
        }];
        let actual = parse_cursor(&mut Cursor::new(&data[..]), 10).unwrap();
        assert_same_images(&expected, &actual);
    }

    #[test]
    fn parse_cursor_two_images_plus_one_ignored() {
        let data = [
            b'X', b'c', b'u', b'r', // magic
            0x10, 0x00, 0x00, 0x00, // header file offset (16)
            0x00, 0x00, 0x00, 0x00, // version(?)
            0x03, 0x00, 0x00, 0x00, // num TOC entries, 3
            // TOC
            0x02, 0x00, 0xfd, 0xff, // IMAGE_TYPE
            0x04, 0x00, 0x00, 0x00, // size 4
            0x34, 0x00, 0x00, 0x00, // image offset (52)
            0x02, 0x00, 0xfd, 0xff, // IMAGE_TYPE
            0x03, 0x00, 0x00, 0x00, // size 3
            0x34, 0x00, 0x00, 0x00, // image offset (52)
            0x02, 0x00, 0xfd, 0xff, // IMAGE_TYPE
            0x04, 0x00, 0x00, 0x00, // size 4
            0x34, 0x00, 0x00, 0x00, // image offset (52)
            // image
            0x00, 0x00, 0x00, 0x00, // header(?)
            0x02, 0x00, 0xfd, 0xff, // IMAGE_TYPE
            0x04, 0x00, 0x00, 0x00, // size 4
            0x00, 0x00, 0x00, 0x00, // version(?)
            0x00, 0x00, 0x00, 0x00, // width 0
            0x00, 0x00, 0x00, 0x00, // height 0
            0x00, 0x00, 0x00, 0x00, // x_hot 0
            0x00, 0x00, 0x00, 0x00, // y_hot 0
            0x00, 0x00, 0x00, 0x00, // delay 0
        ];
        let expected = [
            Image {
                width: 0,
                height: 0,
                x_hot: 0,
                y_hot: 0,
                delay: 0,
                pixels: vec![],
            },
            Image {
                width: 0,
                height: 0,
                x_hot: 0,
                y_hot: 0,
                delay: 0,
                pixels: vec![],
            },
        ];
        let actual = parse_cursor(&mut Cursor::new(&data[..]), 10).unwrap();
        assert_same_images(&expected, &actual);
    }

    fn assert_same_images(a: &[Image], b: &[Image]) {
        assert_eq!(a.len(), b.len(), "{:?} == {:?}", a, b);
        for (i, (im1, im2)) in a.iter().zip(b.iter()).enumerate() {
            assert_eq!(
                im1.width,
                im2.width,
                "Width image {}: {} == {}",
                i + 1,
                im1.width,
                im2.width
            );
            assert_eq!(
                im1.height,
                im2.height,
                "Height image {}: {} == {}",
                i + 1,
                im1.height,
                im2.height
            );
            assert_eq!(
                im1.x_hot,
                im2.x_hot,
                "X-hot image {}: {} == {}",
                i + 1,
                im1.x_hot,
                im2.x_hot
            );
            assert_eq!(
                im1.y_hot,
                im2.y_hot,
                "Y-hot image {}: {} == {}",
                i + 1,
                im1.y_hot,
                im2.y_hot
            );
            assert_eq!(
                im1.delay,
                im2.delay,
                "Delay image {}: {} == {}",
                i + 1,
                im1.delay,
                im2.delay
            );
            assert_eq!(
                im1.pixels,
                im2.pixels,
                "Pixels image {}: {:?} == {:?}",
                i + 1,
                im1.pixels,
                im2.pixels
            );
        }
    }
}
