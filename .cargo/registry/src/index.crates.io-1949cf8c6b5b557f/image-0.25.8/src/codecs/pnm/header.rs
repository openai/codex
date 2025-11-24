use std::{fmt, io};

/// The kind of encoding used to store sample values
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SampleEncoding {
    /// Samples are unsigned binary integers in big endian
    Binary,

    /// Samples are encoded as decimal ascii strings separated by whitespace
    Ascii,
}

/// Denotes the category of the magic number
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PnmSubtype {
    /// Magic numbers P1 and P4
    Bitmap(SampleEncoding),

    /// Magic numbers P2 and P5
    Graymap(SampleEncoding),

    /// Magic numbers P3 and P6
    Pixmap(SampleEncoding),

    /// Magic number P7
    ArbitraryMap,
}

/// Stores the complete header data of a file.
///
/// Internally, provides mechanisms for lossless reencoding. After reading a file with the decoder
/// it is possible to recover the header and construct an encoder. Using the encoder on the just
/// loaded image should result in a byte copy of the original file (for single image pnms without
/// additional trailing data).
#[derive(Clone)]
pub struct PnmHeader {
    pub(crate) decoded: HeaderRecord,
    pub(crate) encoded: Option<Vec<u8>>,
}

#[derive(Clone)]
pub(crate) enum HeaderRecord {
    Bitmap(BitmapHeader),
    Graymap(GraymapHeader),
    Pixmap(PixmapHeader),
    Arbitrary(ArbitraryHeader),
}

/// Header produced by a `pbm` file ("Portable Bit Map")
#[derive(Clone, Copy, Debug)]
pub struct BitmapHeader {
    /// Binary or Ascii encoded file
    pub encoding: SampleEncoding,

    /// Height of the image file
    pub height: u32,

    /// Width of the image file
    pub width: u32,
}

/// Header produced by a `pgm` file ("Portable Gray Map")
#[derive(Clone, Copy, Debug)]
pub struct GraymapHeader {
    /// Binary or Ascii encoded file
    pub encoding: SampleEncoding,

    /// Height of the image file
    pub height: u32,

    /// Width of the image file
    pub width: u32,

    /// Maximum sample value within the image
    pub maxwhite: u32,
}

/// Header produced by a `ppm` file ("Portable Pixel Map")
#[derive(Clone, Copy, Debug)]
pub struct PixmapHeader {
    /// Binary or Ascii encoded file
    pub encoding: SampleEncoding,

    /// Height of the image file
    pub height: u32,

    /// Width of the image file
    pub width: u32,

    /// Maximum sample value within the image
    pub maxval: u32,
}

/// Header produced by a `pam` file ("Portable Arbitrary Map")
#[derive(Clone, Debug)]
pub struct ArbitraryHeader {
    /// Height of the image file
    pub height: u32,

    /// Width of the image file
    pub width: u32,

    /// Number of color channels
    pub depth: u32,

    /// Maximum sample value within the image
    pub maxval: u32,

    /// Color interpretation of image pixels
    pub tupltype: Option<ArbitraryTuplType>,
}

/// Standardized tuple type specifiers in the header of a `pam`.
#[derive(Clone, Debug)]
pub enum ArbitraryTuplType {
    /// Pixels are either black (0) or white (1)
    BlackAndWhite,

    /// Pixels are either black (0) or white (1) and a second alpha channel
    BlackAndWhiteAlpha,

    /// Pixels represent the amount of white
    Grayscale,

    /// Grayscale with an additional alpha channel
    GrayscaleAlpha,

    /// Three channels: Red, Green, Blue
    RGB,

    /// Four channels: Red, Green, Blue, Alpha
    RGBAlpha,

    /// An image format which is not standardized
    Custom(String),
}

impl ArbitraryTuplType {
    pub(crate) fn name(&self) -> &str {
        match self {
            ArbitraryTuplType::BlackAndWhite => "BLACKANDWHITE",
            ArbitraryTuplType::BlackAndWhiteAlpha => "BLACKANDWHITE_ALPHA",
            ArbitraryTuplType::Grayscale => "GRAYSCALE",
            ArbitraryTuplType::GrayscaleAlpha => "GRAYSCALE_ALPHA",
            ArbitraryTuplType::RGB => "RGB",
            ArbitraryTuplType::RGBAlpha => "RGB_ALPHA",
            ArbitraryTuplType::Custom(custom) => custom,
        }
    }
}

impl PnmSubtype {
    /// Get the two magic constant bytes corresponding to this format subtype.
    #[must_use]
    pub fn magic_constant(self) -> &'static [u8; 2] {
        match self {
            PnmSubtype::Bitmap(SampleEncoding::Ascii) => b"P1",
            PnmSubtype::Graymap(SampleEncoding::Ascii) => b"P2",
            PnmSubtype::Pixmap(SampleEncoding::Ascii) => b"P3",
            PnmSubtype::Bitmap(SampleEncoding::Binary) => b"P4",
            PnmSubtype::Graymap(SampleEncoding::Binary) => b"P5",
            PnmSubtype::Pixmap(SampleEncoding::Binary) => b"P6",
            PnmSubtype::ArbitraryMap => b"P7",
        }
    }

    /// Whether samples are stored as binary or as decimal ascii
    #[must_use]
    pub fn sample_encoding(self) -> SampleEncoding {
        match self {
            PnmSubtype::ArbitraryMap => SampleEncoding::Binary,
            PnmSubtype::Bitmap(enc) => enc,
            PnmSubtype::Graymap(enc) => enc,
            PnmSubtype::Pixmap(enc) => enc,
        }
    }
}

impl PnmHeader {
    /// Retrieve the format subtype from which the header was created.
    #[must_use]
    pub fn subtype(&self) -> PnmSubtype {
        match self.decoded {
            HeaderRecord::Bitmap(BitmapHeader { encoding, .. }) => PnmSubtype::Bitmap(encoding),
            HeaderRecord::Graymap(GraymapHeader { encoding, .. }) => PnmSubtype::Graymap(encoding),
            HeaderRecord::Pixmap(PixmapHeader { encoding, .. }) => PnmSubtype::Pixmap(encoding),
            HeaderRecord::Arbitrary(ArbitraryHeader { .. }) => PnmSubtype::ArbitraryMap,
        }
    }

    /// The width of the image this header is for.
    #[must_use]
    pub fn width(&self) -> u32 {
        match self.decoded {
            HeaderRecord::Bitmap(BitmapHeader { width, .. }) => width,
            HeaderRecord::Graymap(GraymapHeader { width, .. }) => width,
            HeaderRecord::Pixmap(PixmapHeader { width, .. }) => width,
            HeaderRecord::Arbitrary(ArbitraryHeader { width, .. }) => width,
        }
    }

    /// The height of the image this header is for.
    #[must_use]
    pub fn height(&self) -> u32 {
        match self.decoded {
            HeaderRecord::Bitmap(BitmapHeader { height, .. }) => height,
            HeaderRecord::Graymap(GraymapHeader { height, .. }) => height,
            HeaderRecord::Pixmap(PixmapHeader { height, .. }) => height,
            HeaderRecord::Arbitrary(ArbitraryHeader { height, .. }) => height,
        }
    }

    /// The biggest value a sample can have. In other words, the colour resolution.
    #[must_use]
    pub fn maximal_sample(&self) -> u32 {
        match self.decoded {
            HeaderRecord::Bitmap(BitmapHeader { .. }) => 1,
            HeaderRecord::Graymap(GraymapHeader { maxwhite, .. }) => maxwhite,
            HeaderRecord::Pixmap(PixmapHeader { maxval, .. }) => maxval,
            HeaderRecord::Arbitrary(ArbitraryHeader { maxval, .. }) => maxval,
        }
    }

    /// Retrieve the underlying bitmap header if any
    #[must_use]
    pub fn as_bitmap(&self) -> Option<&BitmapHeader> {
        match self.decoded {
            HeaderRecord::Bitmap(ref bitmap) => Some(bitmap),
            _ => None,
        }
    }

    /// Retrieve the underlying graymap header if any
    #[must_use]
    pub fn as_graymap(&self) -> Option<&GraymapHeader> {
        match self.decoded {
            HeaderRecord::Graymap(ref graymap) => Some(graymap),
            _ => None,
        }
    }

    /// Retrieve the underlying pixmap header if any
    #[must_use]
    pub fn as_pixmap(&self) -> Option<&PixmapHeader> {
        match self.decoded {
            HeaderRecord::Pixmap(ref pixmap) => Some(pixmap),
            _ => None,
        }
    }

    /// Retrieve the underlying arbitrary header if any
    #[must_use]
    pub fn as_arbitrary(&self) -> Option<&ArbitraryHeader> {
        match self.decoded {
            HeaderRecord::Arbitrary(ref arbitrary) => Some(arbitrary),
            _ => None,
        }
    }

    /// Write the header back into a binary stream
    pub fn write(&self, writer: &mut dyn io::Write) -> io::Result<()> {
        writer.write_all(self.subtype().magic_constant())?;
        match *self {
            PnmHeader {
                encoded: Some(ref content),
                ..
            } => writer.write_all(content),
            PnmHeader {
                decoded:
                    HeaderRecord::Bitmap(BitmapHeader {
                        encoding: _encoding,
                        width,
                        height,
                    }),
                ..
            } => writeln!(writer, "\n{width} {height}"),
            PnmHeader {
                decoded:
                    HeaderRecord::Graymap(GraymapHeader {
                        encoding: _encoding,
                        width,
                        height,
                        maxwhite,
                    }),
                ..
            } => writeln!(writer, "\n{width} {height} {maxwhite}"),
            PnmHeader {
                decoded:
                    HeaderRecord::Pixmap(PixmapHeader {
                        encoding: _encoding,
                        width,
                        height,
                        maxval,
                    }),
                ..
            } => writeln!(writer, "\n{width} {height} {maxval}"),
            PnmHeader {
                decoded:
                    HeaderRecord::Arbitrary(ArbitraryHeader {
                        width,
                        height,
                        depth,
                        maxval,
                        ref tupltype,
                    }),
                ..
            } => {
                struct TupltypeWriter<'a>(&'a Option<ArbitraryTuplType>);
                impl fmt::Display for TupltypeWriter<'_> {
                    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        match self.0 {
                            Some(tt) => writeln!(f, "TUPLTYPE {}", tt.name()),
                            None => Ok(()),
                        }
                    }
                }

                writeln!(
                    writer,
                    "\nWIDTH {}\nHEIGHT {}\nDEPTH {}\nMAXVAL {}\n{}ENDHDR",
                    width,
                    height,
                    depth,
                    maxval,
                    TupltypeWriter(tupltype)
                )
            }
        }
    }
}

impl From<BitmapHeader> for PnmHeader {
    fn from(header: BitmapHeader) -> Self {
        PnmHeader {
            decoded: HeaderRecord::Bitmap(header),
            encoded: None,
        }
    }
}

impl From<GraymapHeader> for PnmHeader {
    fn from(header: GraymapHeader) -> Self {
        PnmHeader {
            decoded: HeaderRecord::Graymap(header),
            encoded: None,
        }
    }
}

impl From<PixmapHeader> for PnmHeader {
    fn from(header: PixmapHeader) -> Self {
        PnmHeader {
            decoded: HeaderRecord::Pixmap(header),
            encoded: None,
        }
    }
}

impl From<ArbitraryHeader> for PnmHeader {
    fn from(header: ArbitraryHeader) -> Self {
        PnmHeader {
            decoded: HeaderRecord::Arbitrary(header),
            encoded: None,
        }
    }
}
