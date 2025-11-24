//! Utility code for working with images.
//!
//! This module contains the [`Image`] struct which represents an arbitrary image in
//! `ImageFormat::ZPixmap`. If you do not know what `ZPixmap` means, then rest assured that you do
//! not want to know the details. It suffices to know that the values of the individual pixels are
//! saved one after another in memory.
//!
//! An [`Image`] can be converted to a different internal representation. [`Image::native`]
//! converts it to the native format of the X11 server. These conversions do not change the actual
//! content of the image, but only the way that it is laid out in memory (e.g. byte order and
//! padding). Specifically, there is no support for converting an image to another `depth`.
//!
//! The code in this module is only available when the `image` feature of the library is
//! enabled.

// For future readers:
//
// - Z-Pixmap means that the pixels are right next to each other. I.e. first you have the value of
//   the pixel at (0, 0), then (1, 0), .... At the end of the row, there might be some padding
//   before the next row begins.
// - XY-Bitmap consists of individual bits. The bits are assigned colors via a GC's foreground and
//   background color when uploading to the X11 server.
// - XY-Pixmap consists of bit planes. This means you first get the first bit of each pixel,
//   stuffed together into bytes. After all of the first pixels are represented, the second plane
//   begins with the second bit of each pixel etc.

use std::borrow::Cow;

use crate::connection::Connection;
use crate::cookie::VoidCookie;
use crate::errors::{ConnectionError, ParseError, ReplyError};
use crate::protocol::xproto::{
    get_image, put_image, Drawable, Format, Gcontext, GetImageReply, ImageFormat,
    ImageOrder as XprotoImageOrder, Setup, VisualClass, Visualid, Visualtype,
};

/// The description of a single color component.
///
/// For example, in an RGB image, pixels are often saved as `0xRRGGBB`, where each letter
/// represents the respective color component. In the example, green has a `width` of 8 (it takes
/// up 8 bits) and a `shift` of `16` (there are 16 less significant bits beyond it). This info is
/// represented as a `ColorComponent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorComponent {
    /// Number of bits for the component
    width: u8,
    /// Offset in an u32 of the component
    shift: u8,
}

impl ColorComponent {
    /// Create a new color component with the given information.
    ///
    /// The following conditions must be satisfied:
    /// - `width <= 16`: color components have at most 16 bits.
    /// - `shift < 32`: pixel values have at most 32 bits.
    /// - `shift + width <= 32`: pixel values have at most 32 bits.
    pub fn new(width: u8, shift: u8) -> Result<Self, ParseError> {
        if width > 16 || shift >= 32 || shift + width > 32 {
            Err(ParseError::InvalidValue)
        } else {
            Ok(Self { width, shift })
        }
    }

    /// Get the bit width of the color component.
    pub fn width(self) -> u8 {
        self.width
    }

    /// Get the bit shift of the color component.
    pub fn shift(self) -> u8 {
        self.shift
    }

    /// Get the pixel mask representing this color component.
    ///
    /// The mask can be used to mask off other colors in a pixel value. Only the bits that
    /// correspond to this color component are set.
    /// ```
    /// # use x11rb::image::ColorComponent;
    /// let red = ColorComponent::new(8, 16)?;
    /// assert_eq!(red.mask(), 0xff0000);
    /// # Ok::<(), x11rb::errors::ParseError>(())
    /// ```
    pub fn mask(self) -> u32 {
        // Get a mask with 'width' set bits.
        let mask = (1u32 << self.width) - 1;
        // Shift the mask into the right position
        mask << self.shift
    }

    /// Create a new color component from a color mask.
    ///
    /// This turns a color mask into its individual components.
    /// ```
    /// # use x11rb::image::ColorComponent;
    /// let red1 = ColorComponent::new(8, 16);
    /// let red2 = ColorComponent::from_mask(0xff0000);
    /// ```
    ///
    /// # Errors
    ///
    /// This function fails if the given value is not a well-formed mask. This means that at least
    /// one bit must be set and the set bits must be consecutive.
    pub fn from_mask(mask: u32) -> Result<Self, ParseError> {
        let width = mask.count_ones();
        let shift = mask.trailing_zeros();
        // Both width and shift can be at most 32, which should fit into u8.
        let result = Self::new(width.try_into().unwrap(), shift.try_into().unwrap())?;
        if mask != result.mask() {
            Err(ParseError::InvalidValue)
        } else {
            Ok(result)
        }
    }

    /// Get this color component out of a pixel value.
    ///
    /// This function gets a single pixel value and returns this color component's value in that
    /// pixel value, expanded to width 16.
    /// ```
    /// # use x11rb::image::ColorComponent;
    /// let red = ColorComponent::new(8, 16)?;
    /// assert_eq!(0xABAB, red.decode(0x78AB_4321));
    /// # Ok::<(), x11rb::errors::ParseError>(())
    /// ```
    pub fn decode(self, pixel: u32) -> u16 {
        // Get the color component out
        let value = (pixel & self.mask()) >> self.shift;

        // Now expand from with `self.width` to width 16.
        let mut width = self.width;
        let mut value = value << (16 - width);
        // Add some low bits by using the high bits
        while width < 16 {
            value |= value >> width;
            width <<= 1;
        }
        value.try_into().unwrap()
    }

    /// Encode a color value according to this pixel format.
    ///
    /// ```
    /// # use x11rb::image::ColorComponent;
    /// let red = ColorComponent::new(8, 16)?;
    /// assert_eq!(0xAB0000, red.encode(0xABCD));
    /// # Ok::<(), x11rb::errors::ParseError>(())
    /// ```
    pub fn encode(self, intensity: u16) -> u32 {
        // First truncate to width `self.width`, then place at the correct offset.
        (u32::from(intensity) >> (16 - self.width)) << self.shift
    }
}

/// A collection of color components describing the red, green, and blue components of a pixel.
///
/// A [`ColorComponent`] describes a single color component in an image. This structure describes
/// the `red`, `green`, and `blue` color components by containing a [`ColorComponent`] for each of
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelLayout {
    red: ColorComponent,
    green: ColorComponent,
    blue: ColorComponent,
}

impl PixelLayout {
    /// Create a new pixel layout from the description of each component
    pub fn new(red: ColorComponent, green: ColorComponent, blue: ColorComponent) -> Self {
        Self { red, green, blue }
    }

    /// Create a new pixel layout
    ///
    /// This function errors if the visual has a different class than `TrueColor` or `DirectColor`,
    /// because color pallets and grayscales are not supported. This function also errors if the
    /// mask components of the visual are malformed.
    pub fn from_visual_type(visual: Visualtype) -> Result<Self, ParseError> {
        if visual.class != VisualClass::TRUE_COLOR && visual.class != VisualClass::DIRECT_COLOR {
            Err(ParseError::InvalidValue)
        } else {
            Ok(Self::new(
                ColorComponent::from_mask(visual.red_mask)?,
                ColorComponent::from_mask(visual.green_mask)?,
                ColorComponent::from_mask(visual.blue_mask)?,
            ))
        }
    }

    /// Get the depth of this pixel layout.
    ///
    /// The depth is the number of significant bits of each pixel value.
    pub fn depth(self) -> u8 {
        // TODO: I am not quite sure this implementation is correct. The protocol seems to allow
        // unused bits in the middle..?
        self.red.width + self.green.width + self.blue.width
    }

    /// Decode a pixel value into its red, green, and blue components.
    ///
    /// This function returns each component expanded to width 16.
    /// ```
    /// # use x11rb::image::{ColorComponent, PixelLayout};
    /// let layout = PixelLayout::new(
    ///     ColorComponent::new(8, 16)?,
    ///     ColorComponent::new(8, 8)?,
    ///     ColorComponent::new(8, 0)?,
    /// );
    /// assert_eq!((0xABAB, 0x4343, 0x2121), layout.decode(0x78AB_4321));
    /// # Ok::<(), x11rb::errors::ParseError>(())
    /// ```
    pub fn decode(self, pixel: u32) -> (u16, u16, u16) {
        let red = self.red.decode(pixel);
        let green = self.green.decode(pixel);
        let blue = self.blue.decode(pixel);
        (red, green, blue)
    }

    /// Encode a color value according to this layout.
    ///
    /// ```
    /// # use x11rb::image::{ColorComponent, PixelLayout};
    /// let layout = PixelLayout::new(
    ///     ColorComponent::new(8, 16)?,
    ///     ColorComponent::new(8, 8)?,
    ///     ColorComponent::new(8, 0)?,
    /// );
    /// assert_eq!(0x00AB_4321, layout.encode((0xABAB, 0x4343, 0x2121)));
    /// # Ok::<(), x11rb::errors::ParseError>(())
    /// ```
    pub fn encode(self, (red, green, blue): (u16, u16, u16)) -> u32 {
        let red = self.red.encode(red);
        let green = self.green.encode(green);
        let blue = self.blue.encode(blue);
        red | green | blue
    }
}

// Compute the stride based on some information of the image
fn compute_stride(width: u16, bits_per_pixel: BitsPerPixel, scanline_pad: ScanlinePad) -> usize {
    let value = usize::from(width) * usize::from(bits_per_pixel);
    scanline_pad.round_to_multiple(value) / 8
}

#[cfg(test)]
mod test_stride {
    use super::compute_stride;

    #[test]
    fn test_stride() {
        for &(width, bpp, pad, stride) in &[
            // bpp=pad=8
            (0, 8, 8, 0),
            (1, 8, 8, 1),
            (2, 8, 8, 2),
            (3, 8, 8, 3),
            (41, 8, 8, 41),
            // bpp=8, pad=16
            (0, 8, 16, 0),
            (1, 8, 16, 2),
            (2, 8, 16, 2),
            (3, 8, 16, 4),
            (41, 8, 16, 42),
            // bpp=16, pad=16
            (0, 16, 16, 0),
            (1, 16, 16, 2),
            (2, 16, 16, 4),
            (3, 16, 16, 6),
            (41, 16, 16, 82),
            // bpp=16, pad=32
            (0, 16, 32, 0),
            (1, 16, 32, 4),
            (2, 16, 32, 4),
            (3, 16, 32, 8),
            (41, 16, 32, 84),
            // bpp=32, pad=32
            (0, 32, 32, 0),
            (1, 32, 32, 4),
            (2, 32, 32, 8),
            (3, 32, 32, 12),
            (41, 32, 32, 164),
        ] {
            let actual = compute_stride(width, bpp.try_into().unwrap(), pad.try_into().unwrap());
            assert_eq!(stride, actual, "width={}, bpp={}, pad={}", width, bpp, pad);
        }
    }
}

// Find the format with the given depth in `setup.pixmap_formats`.
fn find_format(setup: &Setup, depth: u8) -> Result<&Format, ParseError> {
    setup
        .pixmap_formats
        .iter()
        .find(|f| f.depth == depth)
        .ok_or(ParseError::InvalidValue)
}

macro_rules! number_enum {
    {
        $(#[$meta:meta])*
        $vis:vis enum $enum_name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant_name:ident = $value:literal,
            )*
        }
    } => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
        $vis enum $enum_name {
            $(
                $(#[$variant_meta])*
                $variant_name = $value,
            )*
        }

        impl TryFrom<u8> for $enum_name {
            type Error = ParseError;
            fn try_from(value: u8) -> Result<Self, Self::Error> {
                match value {
                    $($value => Ok(Self::$variant_name),)*
                    _ => Err(ParseError::InvalidValue),
                }
            }
        }

        impl From<$enum_name> for u8 {
            fn from(value: $enum_name) -> u8 {
                match value {
                    $($enum_name::$variant_name => $value,)*
                }
            }
        }

        impl From<$enum_name> for usize {
            fn from(value: $enum_name) -> usize {
                u8::from(value).into()
            }
        }
    }
}

number_enum! {
    /// The padding of scanlines.
    ///
    /// Each line of an image is padded to a multiple of some value. This value is the scanline
    /// padding, which this enum represents.
    #[non_exhaustive]
    pub enum ScanlinePad {
        /// Padding to multiples of 8 bits, i.e. no padding.
        Pad8 = 8,
        /// Padding to multiples of 16 bits, i.e. the next even number of bytes.
        Pad16 = 16,
        /// Padding to multiples of 32 bits, i.e. the next multiple of 4.
        Pad32 = 32,
    }
}

impl ScanlinePad {
    fn round_to_multiple(self, value: usize) -> usize {
        let value = value + usize::from(self) - 1;
        value - value % usize::from(self)
    }
}

#[cfg(test)]
mod test_scanline_pad {
    use super::ScanlinePad;

    #[test]
    fn number_conversions() {
        assert_eq!(8_u8, ScanlinePad::Pad8.into());
        assert_eq!(16_u8, ScanlinePad::Pad16.into());
        assert_eq!(32_u8, ScanlinePad::Pad32.into());
        assert_eq!(8.try_into(), Ok(ScanlinePad::Pad8));
        assert_eq!(16.try_into(), Ok(ScanlinePad::Pad16));
        assert_eq!(32.try_into(), Ok(ScanlinePad::Pad32));
    }

    #[test]
    fn test_round_to_multiple() {
        for &(value, pad8, pad16, pad32) in [
            (0, 0, 0, 0),
            (1, 8, 16, 32),
            (2, 8, 16, 32),
            (3, 8, 16, 32),
            (4, 8, 16, 32),
            (5, 8, 16, 32),
            (6, 8, 16, 32),
            (7, 8, 16, 32),
            (8, 8, 16, 32),
            (9, 16, 16, 32),
            (10, 16, 16, 32),
            (11, 16, 16, 32),
            (12, 16, 16, 32),
            (13, 16, 16, 32),
            (14, 16, 16, 32),
            (15, 16, 16, 32),
            (16, 16, 16, 32),
            (17, 24, 32, 32),
            (33, 40, 48, 64),
            (47, 48, 48, 64),
            (48, 48, 48, 64),
            (49, 56, 64, 64),
        ]
        .iter()
        {
            assert_eq!(
                pad8,
                ScanlinePad::Pad8.round_to_multiple(value),
                "value={} for pad8",
                value,
            );
            assert_eq!(
                pad16,
                ScanlinePad::Pad16.round_to_multiple(value),
                "value={} for pad16",
                value,
            );
            assert_eq!(
                pad32,
                ScanlinePad::Pad32.round_to_multiple(value),
                "value={} for pad32",
                value,
            );
        }
    }
}

number_enum! {
    /// The number of bits required to store one pixel.
    ///
    /// This value is only about the size of one pixel in memory. Other names for it include
    /// `bits_per_pixel` or `bpp`. It may be larger than the number of meaningful bits for a pixel
    /// value, which is its `depth`.
    #[non_exhaustive]
    pub enum BitsPerPixel {
        /// Each pixel takes one bit of memory.
        B1 = 1,
        /// Each pixel takes four bits of memory.
        B4 = 4,
        /// Each pixel takes one byte of memory.
        B8 = 8,
        /// Each pixel takes two bytes of memory.
        B16 = 16,
        /// Each pixel takes three bytes of memory.
        ///
        /// This is most likely not what you want to use, even if you have RGB data with 8 bits per
        /// channel. This corresponds to `depth=24`, but is almost always stored as four bytes
        /// per pixel, which is `BitsPerPixel::B32`.
        B24 = 24,
        /// Each pixel takes four bytes of memory.
        B32 = 32,
    }
}

/// Order in which bytes are stored in memory.
///
/// If the numberof bits per pixel is less than 8, then this is the
/// order in which bits are packed into bytes.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum ImageOrder {
    /// Least significant byte first
    LsbFirst,
    /// Most significant byte first
    MsbFirst,
}

impl TryFrom<XprotoImageOrder> for ImageOrder {
    type Error = ParseError;

    fn try_from(value: XprotoImageOrder) -> Result<Self, ParseError> {
        match value {
            XprotoImageOrder::LSB_FIRST => Ok(Self::LsbFirst),
            XprotoImageOrder::MSB_FIRST => Ok(Self::MsbFirst),
            _ => Err(ParseError::InvalidValue),
        }
    }
}

/// The description of an image.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Image<'a> {
    /// Width in pixels.
    width: u16,

    /// Height in pixels.
    height: u16,

    /// Right padding on each scanline.
    scanline_pad: ScanlinePad,

    /// Color depth in bits.
    depth: u8,

    /// Storage per pixel in bits. Must be >= depth.
    bits_per_pixel: BitsPerPixel,

    /// Byte order of components.
    ///
    /// This is the nibble order when bits_per_pixel is 4.
    byte_order: ImageOrder,

    /// The image data.
    data: Cow<'a, [u8]>,
}

impl<'a> Image<'a> {
    /// The width in pixels.
    pub fn width(&self) -> u16 {
        self.width
    }

    /// The height in pixels.
    pub fn height(&self) -> u16 {
        self.height
    }

    /// The padding on the right side of each scanline.
    ///
    /// Each scanline is rounded up to some multiple of the `scanline_pad`.
    pub fn scanline_pad(&self) -> ScanlinePad {
        self.scanline_pad
    }

    /// Color depth in bits.
    pub fn depth(&self) -> u8 {
        self.depth
    }

    /// Number of bits required to store one pixel.
    ///
    /// This is always `>= depth`.
    pub fn bits_per_pixel(&self) -> BitsPerPixel {
        self.bits_per_pixel
    }

    /// Order in which bytes are stored in memory.
    ///
    /// If `bits_per_pixel()` is smaller than 8, then this is the order in which bits are packed
    /// into bytes.
    pub fn byte_order(&self) -> ImageOrder {
        self.byte_order
    }

    /// Raw pixel data.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Mutable access to the raw pixel data.
    ///
    /// If the `Image` was constructed with `Cow::Borrowed`-access to its pixel data, then a copy
    /// is made when this method is called.
    pub fn data_mut(&mut self) -> &mut [u8] {
        self.data.to_mut()
    }

    /// Construct a new image from existing data.
    ///
    /// This constructs a new `Image` from given `data` without copying this data around. The other
    /// parameters describe the format that the image data is in.
    ///
    /// See [`Image::allocate`] for a variant that allocates memory for you and
    /// [`Image::allocate_native`] for allocating an image that is in an X11 server's native
    /// format.
    ///
    /// # Errors
    ///
    /// The only possible error is that `data.len()` is too short for an image as described by the
    /// other parameters.
    pub fn new(
        width: u16,
        height: u16,
        scanline_pad: ScanlinePad,
        depth: u8,
        bits_per_pixel: BitsPerPixel,
        byte_order: ImageOrder,
        data: Cow<'a, [u8]>,
    ) -> Result<Self, ParseError> {
        let stride = compute_stride(width, bits_per_pixel, scanline_pad);
        let expected_size = usize::from(height) * stride;
        if data.len() < expected_size {
            Err(ParseError::InsufficientData)
        } else {
            Ok(Self {
                width,
                height,
                scanline_pad,
                depth,
                bits_per_pixel,
                byte_order,
                data,
            })
        }
    }

    /// Construct a new, empty image.
    ///
    /// This function allocates memory for a new image in the format that is described by the
    /// parameters.
    ///
    /// See [`Image::new`] for a variant that wraps an existing in-memory image in an `Image` and
    /// [`Image::allocate_native`] for allocating an image that is in an X11 server's native
    /// format.
    pub fn allocate(
        width: u16,
        height: u16,
        scanline_pad: ScanlinePad,
        depth: u8,
        bits_per_pixel: BitsPerPixel,
        byte_order: ImageOrder,
    ) -> Self {
        let stride = compute_stride(width, bits_per_pixel, scanline_pad);
        let data = Cow::Owned(vec![0; usize::from(height) * stride]);
        Self {
            width,
            height,
            scanline_pad,
            depth,
            bits_per_pixel,
            byte_order,
            data,
        }
    }

    /// Construct a new, empty image.
    ///
    /// This function allocates memory for a new image in the format that the X11 server expects.
    /// The image will have size `width`x`height` and a depth of `depth`.
    ///
    /// See [`Image::new`] for a variant that wraps an existing in-memory image in an `Image` and
    /// [`Image::allocate`] for allocating an image that is in a more general format, not
    /// necessarily what the X11 server wants.
    pub fn allocate_native(
        width: u16,
        height: u16,
        depth: u8,
        setup: &Setup,
    ) -> Result<Self, ParseError> {
        let format = find_format(setup, depth)?;
        Ok(Self::allocate(
            width,
            height,
            format.scanline_pad.try_into()?,
            depth,
            format.bits_per_pixel.try_into()?,
            setup.image_byte_order.try_into()?,
        ))
    }

    /// The stride is the number of bytes that each row of pixel data occupies in memory.
    fn stride(&self) -> usize {
        compute_stride(self.width, self.bits_per_pixel, self.scanline_pad)
    }

    /// Get an image from the X11 server.
    ///
    /// This function sends a [`GetImage`](crate::protocol::xproto::GetImageRequest) request, waits
    /// for its response and wraps it in a new `Image`. The image and the corresponding visual id
    /// are returned.
    ///
    /// The returned image contains the rectangle with top left corner `(x, y)` and size `(width,
    /// height)` of the given `drawable`.
    pub fn get(
        conn: &impl Connection,
        drawable: Drawable,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
    ) -> Result<(Self, Visualid), ReplyError> {
        let reply = get_image(
            conn,
            ImageFormat::Z_PIXMAP,
            drawable,
            x,
            y,
            width,
            height,
            !0,
        )?
        .reply()?;
        let visual = reply.visual;
        let image = Self::get_from_reply(conn.setup(), width, height, reply)?;
        Ok((image, visual))
    }

    /// Construct an `Image` from a `GetImageReply`.
    ///
    /// This function takes a `GetImageReply` and wraps it in an `Image`. The given `width` and
    /// `height` describe the corresponding values of the `GetImage` request that was used to get
    /// the `GetImageReply`.
    pub fn get_from_reply(
        setup: &Setup,
        width: u16,
        height: u16,
        reply: GetImageReply,
    ) -> Result<Self, ParseError> {
        let format = find_format(setup, reply.depth)?;
        Self::new(
            width,
            height,
            format.scanline_pad.try_into()?,
            reply.depth,
            format.bits_per_pixel.try_into()?,
            setup.image_byte_order.try_into()?,
            Cow::Owned(reply.data),
        )
    }

    /// Put an image to the X11 server.
    ///
    /// This function sends a [`PutImage`](crate::protocol::xproto::PutImageRequest) request. This
    /// will upload this image to the given `drawable` to position `(dst_x, dst_y)`.
    ///
    /// The server's maximum request size is honored. This means that a too large `PutImage`
    /// request is automatically split up into smaller pieces. Thus, if this function returns an
    /// error, the image could already be partially sent.
    ///
    /// Before uploading, the image is translated into the server's native format via
    /// [`Image::native`]. This may convert the image to another format, which can be slow. If you
    /// intend to upload the same image multiple times, it is likely more efficient to call
    /// [`Image::native`] once initially so that the conversion is not repeated on each upload.
    pub fn put<'c, Conn: Connection>(
        &self,
        conn: &'c Conn,
        drawable: Drawable,
        gc: Gcontext,
        dst_x: i16,
        dst_y: i16,
    ) -> Result<Vec<VoidCookie<'c, Conn>>, ConnectionError> {
        self.native(conn.setup())?
            .put_impl(conn, drawable, gc, dst_x, dst_y)
    }

    fn put_impl<'c, Conn: Connection>(
        &self,
        conn: &'c Conn,
        drawable: Drawable,
        gc: Gcontext,
        dst_x: i16,
        dst_y: i16,
    ) -> Result<Vec<VoidCookie<'c, Conn>>, ConnectionError> {
        // Upload the image without exceeding the server's maximum request size
        let max_bytes = conn.maximum_request_bytes();
        let put_image_header = 24;
        let stride = self.stride();
        let lines_per_request = (max_bytes - put_image_header) / stride;
        let mut result = Vec::with_capacity(
            (usize::from(self.height()) + lines_per_request - 1) / lines_per_request,
        );
        let lines_per_request = lines_per_request.try_into().unwrap_or(u16::max_value());
        assert!(lines_per_request > 0);

        let (mut y_offset, mut byte_offset) = (0, 0);
        while y_offset < self.height {
            let next_lines = lines_per_request.min(self.height - y_offset);
            let next_byte_offset = byte_offset + usize::from(next_lines) * stride;
            let data = &self.data[byte_offset..next_byte_offset];
            result.push(put_image(
                conn,
                ImageFormat::Z_PIXMAP,
                drawable,
                gc,
                self.width,
                next_lines,
                dst_x,
                dst_y + i16::try_from(y_offset).unwrap(),
                0, // left_pad must always be 0 for ZPixmap
                self.depth,
                data,
            )?);

            y_offset += next_lines;
            byte_offset = next_byte_offset;
        }
        Ok(result)
    }

    /// Convert this image into the format specified by the other parameters.
    ///
    /// This function may need to copy the image, hence returns a `Cow`.
    pub fn convert(
        &self,
        scanline_pad: ScanlinePad,
        bits_per_pixel: BitsPerPixel,
        byte_order: ImageOrder,
    ) -> Cow<'_, Self> {
        let already_converted = scanline_pad == self.scanline_pad
            && bits_per_pixel == self.bits_per_pixel
            && byte_order == self.byte_order;
        if already_converted {
            Cow::Borrowed(self)
        } else {
            let mut copy = Image::allocate(
                self.width,
                self.height,
                scanline_pad,
                self.depth,
                bits_per_pixel,
                byte_order,
            );
            // This is the slowest possible way to do this. But also the easiest one to implement.
            for y in 0..self.height {
                for x in 0..self.width {
                    copy.put_pixel(x, y, self.get_pixel(x, y))
                }
            }
            Cow::Owned(copy)
        }
    }

    /// Convert this image into the native format of the X11 server.
    ///
    /// This function may need to copy the image, hence returns a `Cow`.
    pub fn native(&self, setup: &Setup) -> Result<Cow<'_, Self>, ParseError> {
        let format = find_format(setup, self.depth)?;
        Ok(self.convert(
            format.scanline_pad.try_into()?,
            format.bits_per_pixel.try_into()?,
            setup.image_byte_order.try_into()?,
        ))
    }

    /// Reencode this image to a different pixel layout / depth.
    ///
    /// Each pixel of this image is interpreted according to `own` and written to the resulting
    /// image in the format described by `output`.
    ///
    /// The resulting image is always in the native format as described by `setup`.
    pub fn reencode<'b>(
        &'b self,
        own: PixelLayout,
        output: PixelLayout,
        setup: &Setup,
    ) -> Result<Cow<'b, Self>, ParseError> {
        if own == output {
            self.native(setup)
        } else {
            // Yay, we get to convert the image :-(
            let (width, height) = (self.width(), self.height());
            let mut result = Image::allocate_native(width, height, output.depth(), setup)?;
            for y in 0..height {
                for x in 0..width {
                    let pixel = self.get_pixel(x, y);
                    let pixel = output.encode(own.decode(pixel));
                    result.put_pixel(x, y, pixel);
                }
            }
            Ok(Cow::Owned(result))
        }
    }

    /// Set a single pixel in this image.
    ///
    /// The pixel at position `(x, y)` will be set to the value `pixel`. `pixel` is truncated to
    /// this image's [`Self::bits_per_pixel`].
    ///
    /// If the image was constructed from a `Cow::Borrowed` access to its pixel data, this causes
    /// the whole pixel data to be copied. See [`Image::new`] and [`Image::data_mut`].
    pub fn put_pixel(&mut self, x: u16, y: u16, pixel: u32) {
        assert!(x < self.width);
        assert!(y < self.height);

        let row_start = usize::from(y) * self.stride();
        let x = usize::from(x);
        let data = self.data.to_mut();
        match self.bits_per_pixel {
            BitsPerPixel::B1 => {
                let (byte, bit) = compute_depth_1_address(x, self.byte_order);
                let pixel = ((pixel & 0x01) << bit) as u8;
                let old = data[row_start + byte];
                let bit_cleared = old & !(1 << bit);
                data[row_start + byte] = bit_cleared | pixel;
            }
            BitsPerPixel::B4 => {
                let mut pixel = pixel & 0x0f;
                let odd_x = x % 2 == 1;
                let mask = if odd_x == (self.byte_order == ImageOrder::MsbFirst) {
                    pixel <<= 4;
                    0xf0
                } else {
                    0x0f
                };
                data[row_start + x / 2] = (data[row_start + x / 2] & !mask) | (pixel as u8);
            }
            BitsPerPixel::B8 => data[row_start + x] = pixel as u8,
            BitsPerPixel::B16 => {
                let (p0, p1) = match self.byte_order {
                    ImageOrder::LsbFirst => (pixel, pixel >> 8),
                    ImageOrder::MsbFirst => (pixel >> 8, pixel),
                };
                data[row_start + 2 * x + 1] = p1 as u8;
                data[row_start + 2 * x] = p0 as u8;
            }
            BitsPerPixel::B24 => {
                let (p0, p1, p2) = match self.byte_order {
                    ImageOrder::LsbFirst => (pixel, pixel >> 8, pixel >> 16),
                    ImageOrder::MsbFirst => (pixel >> 16, pixel >> 8, pixel),
                };
                data[row_start + 3 * x + 2] = p2 as u8;
                data[row_start + 3 * x + 1] = p1 as u8;
                data[row_start + 3 * x] = p0 as u8;
            }
            BitsPerPixel::B32 => {
                let (p0, p1, p2, p3) = match self.byte_order {
                    ImageOrder::LsbFirst => (pixel, pixel >> 8, pixel >> 16, pixel >> 24),
                    ImageOrder::MsbFirst => (pixel >> 24, pixel >> 16, pixel >> 8, pixel),
                };
                data[row_start + 4 * x + 3] = p3 as u8;
                data[row_start + 4 * x + 2] = p2 as u8;
                data[row_start + 4 * x + 1] = p1 as u8;
                data[row_start + 4 * x] = p0 as u8;
            }
        }
    }

    /// Get the value of a single pixel.
    ///
    /// This function gets the value of the pixel at `(x, y)`.
    pub fn get_pixel(&self, x: u16, y: u16) -> u32 {
        assert!(x < self.width);
        assert!(y < self.height);

        let row_start = usize::from(y) * self.stride();
        let x = usize::from(x);
        // TODO Can this code (and the one in put_pixel) be simplified? E.g. handle B4 as a special
        // case and copy bits_per_pixel.into() / 8 bytes in other cases?
        match self.bits_per_pixel {
            BitsPerPixel::B1 => {
                let (byte, bit) = compute_depth_1_address(x, self.byte_order);
                ((self.data[row_start + byte] >> bit) & 1).into()
            }
            BitsPerPixel::B4 => {
                let byte = u32::from(self.data[row_start + x / 2]);
                let odd_x = x % 2 == 1;
                if odd_x == (self.byte_order == ImageOrder::MsbFirst) {
                    byte >> 4
                } else {
                    byte & 0x0f
                }
            }
            BitsPerPixel::B8 => self.data[row_start + x].into(),
            BitsPerPixel::B16 => {
                let p1 = u32::from(self.data[row_start + 2 * x + 1]);
                let p0 = u32::from(self.data[row_start + 2 * x]);
                match self.byte_order {
                    ImageOrder::LsbFirst => p0 | (p1 << 8),
                    ImageOrder::MsbFirst => p1 | (p0 << 8),
                }
            }
            BitsPerPixel::B24 => {
                let p2 = u32::from(self.data[row_start + 3 * x + 2]);
                let p1 = u32::from(self.data[row_start + 3 * x + 1]);
                let p0 = u32::from(self.data[row_start + 3 * x]);
                match self.byte_order {
                    ImageOrder::LsbFirst => p0 | (p1 << 8) | (p2 << 16),
                    ImageOrder::MsbFirst => p2 | (p1 << 8) | (p0 << 16),
                }
            }
            BitsPerPixel::B32 => {
                let p3 = u32::from(self.data[row_start + 4 * x + 3]);
                let p2 = u32::from(self.data[row_start + 4 * x + 2]);
                let p1 = u32::from(self.data[row_start + 4 * x + 1]);
                let p0 = u32::from(self.data[row_start + 4 * x]);
                match self.byte_order {
                    ImageOrder::LsbFirst => p0 | (p1 << 8) | (p2 << 16) | (p3 << 24),
                    ImageOrder::MsbFirst => p3 | (p2 << 8) | (p1 << 16) | (p0 << 24),
                }
            }
        }
    }

    /// Get a version of this image with `'static` lifetime.
    ///
    /// If the image was constructed from a `Cow::Borrowed`, this clones the contained data.
    /// Otherwise, this simply returns `self`.
    pub fn into_owned(self) -> Image<'static> {
        // It would be great if we could just implement ToOwned, but that requires implementing
        // Borrow, which we cannot do. Thus, this function exists as a work-around.
        Image {
            data: self.data.into_owned().into(),
            ..self
        }
    }
}

fn compute_depth_1_address(x: usize, order: ImageOrder) -> (usize, usize) {
    let bit = match order {
        ImageOrder::MsbFirst => 7 - x % 8,
        ImageOrder::LsbFirst => x % 8,
    };
    (x / 8, bit)
}

#[cfg(test)]
mod test_image {
    use super::{BitsPerPixel, Image, ImageOrder, ParseError, ScanlinePad};
    use std::borrow::Cow;

    #[test]
    fn test_new_too_short() {
        let depth = 16;
        // Due to Pad16, this image needs two bytes
        let result = Image::new(
            1,
            1,
            ScanlinePad::Pad16,
            depth,
            BitsPerPixel::B8,
            ImageOrder::MsbFirst,
            Cow::Owned(vec![0]),
        );
        assert_eq!(result.unwrap_err(), ParseError::InsufficientData);
    }

    #[test]
    fn test_new() {
        let depth = 16;
        let image = Image::new(
            2,
            1,
            ScanlinePad::Pad16,
            depth,
            BitsPerPixel::B8,
            ImageOrder::MsbFirst,
            Cow::Owned(vec![42, 125]),
        )
        .unwrap();
        assert_eq!(image.width(), 2);
        assert_eq!(image.height(), 1);
        assert_eq!(image.scanline_pad(), ScanlinePad::Pad16);
        assert_eq!(image.depth(), depth);
        assert_eq!(image.bits_per_pixel(), BitsPerPixel::B8);
        assert_eq!(image.byte_order(), ImageOrder::MsbFirst);
        assert_eq!(image.data(), [42, 125]);
    }

    #[test]
    fn test_into_owned_keeps_owned_data() {
        fn with_data(data: Cow<'_, [u8]>) -> *const u8 {
            let orig_ptr = data.as_ptr();
            let image = Image::new(
                1,
                1,
                ScanlinePad::Pad8,
                1,
                BitsPerPixel::B1,
                ImageOrder::MsbFirst,
                data,
            )
            .unwrap();
            assert_eq!(image.data().as_ptr(), orig_ptr);
            image.into_owned().data().as_ptr()
        }

        // Cow::Borrowed is copied
        let data = vec![0];
        let orig_ptr = data.as_ptr();
        assert_ne!(with_data(Cow::Borrowed(&data)), orig_ptr);

        // Cow::Owned is kept
        assert_eq!(with_data(Cow::Owned(data)), orig_ptr);
    }

    #[test]
    fn put_pixel_depth1() {
        let mut image = Image::allocate(
            16,
            2,
            ScanlinePad::Pad32,
            1,
            BitsPerPixel::B1,
            ImageOrder::MsbFirst,
        );
        for x in 0..8 {
            image.put_pixel(x, 0, 1);
        }
        assert_eq!(0b_1111_1111, image.data()[0]);

        image.put_pixel(0, 0, 0);
        assert_eq!(0b_0111_1111, image.data()[0]);

        image.put_pixel(2, 0, 0);
        assert_eq!(0b_0101_1111, image.data()[0]);

        image.put_pixel(4, 0, 0);
        assert_eq!(0b_0101_0111, image.data()[0]);

        image.put_pixel(6, 0, 0);
        assert_eq!(0b_0101_0101, image.data()[0]);

        image.data_mut()[1] = 0;

        image.put_pixel(8, 0, 1);
        assert_eq!(0b_1000_0000, image.data()[1]);

        image.put_pixel(15, 0, 1);
        assert_eq!(0b_1000_0001, image.data()[1]);

        assert_eq!(0b_0000_0000, image.data()[5]);
        image.put_pixel(15, 1, 1);
        assert_eq!(0b_0000_0001, image.data()[5]);
    }

    #[test]
    fn put_pixel_depth4() {
        let mut image = Image::allocate(
            8,
            2,
            ScanlinePad::Pad16,
            1,
            BitsPerPixel::B4,
            ImageOrder::MsbFirst,
        );
        for pos in 0..=0xf {
            image.put_pixel(pos % 8, pos / 8, pos.into());
        }
        assert_eq!(
            image.data(),
            [0x10, 0x32, 0x54, 0x76, 0x98, 0xBA, 0xDC, 0xFE]
        );
    }

    #[test]
    fn put_pixel_depth8() {
        let mut image = Image::allocate(
            256,
            2,
            ScanlinePad::Pad8,
            1,
            BitsPerPixel::B8,
            ImageOrder::MsbFirst,
        );
        for x in 0..=0xff {
            image.put_pixel(x, 0, x.into());
        }
        image.put_pixel(255, 1, 0x1245_89AB);
        let expected = (0..=0xff)
            .chain((0..0xff).map(|_| 0))
            .chain(std::iter::once(0xAB))
            .collect::<Vec<_>>();
        assert_eq!(image.data(), &expected[..]);
    }

    #[test]
    fn put_pixel_depth16() {
        let mut image = Image::allocate(
            5,
            2,
            ScanlinePad::Pad32,
            1,
            BitsPerPixel::B16,
            ImageOrder::MsbFirst,
        );
        image.put_pixel(0, 0, 0xAB_36_18_F8);
        image.put_pixel(4, 0, 0x12_34_56_78);
        image.put_pixel(4, 1, 0xFE_DC_BA_98);
        #[rustfmt::skip]
        let expected = [
            // First row
            0x18, 0xF8, 0, 0, 0, 0, 0, 0, 0x56, 0x78,
            // Padding Pad32
            0, 0,
            // Second row
            0, 0, 0, 0, 0, 0, 0, 0, 0xBA, 0x98,
            // Padding Pad32
            0, 0,
        ];
        assert_eq!(image.data(), expected);
    }

    #[test]
    fn put_pixel_depth32() {
        let mut image = Image::allocate(
            2,
            2,
            ScanlinePad::Pad32,
            1,
            BitsPerPixel::B32,
            ImageOrder::MsbFirst,
        );
        image.put_pixel(0, 0, 0xAB_36_18_F8);
        image.put_pixel(1, 0, 0x12_34_56_78);
        image.put_pixel(1, 1, 0xFE_DC_BA_98);
        #[rustfmt::skip]
        let expected = [
            // First row
            0xAB, 0x36, 0x18, 0xF8, 0x12, 0x34, 0x56, 0x78,
            // Second row
            0x00, 0x00, 0x00, 0x00, 0xFE, 0xDC, 0xBA, 0x98,
        ];
        assert_eq!(image.data(), expected);
    }

    #[test]
    fn get_pixel_depth1() {
        let image = Image::new(
            16,
            2,
            ScanlinePad::Pad32,
            1,
            BitsPerPixel::B1,
            ImageOrder::MsbFirst,
            Cow::Borrowed(&DATA),
        )
        .unwrap();
        assert_eq!(1, image.get_pixel(0, 0));
        assert_eq!(1, image.get_pixel(10, 0));
        assert_eq!(0, image.get_pixel(15, 0));
        assert_eq!(0, image.get_pixel(0, 1));
        assert_eq!(1, image.get_pixel(10, 1));
        assert_eq!(0, image.get_pixel(15, 1));
    }

    #[test]
    fn get_pixel_depth4() {
        let image = Image::new(
            16,
            2,
            ScanlinePad::Pad32,
            1,
            BitsPerPixel::B4,
            ImageOrder::MsbFirst,
            Cow::Borrowed(&DATA),
        )
        .unwrap();
        assert_eq!(0xB, image.get_pixel(0, 0));
        assert_eq!(0x4, image.get_pixel(10, 0));
        assert_eq!(0x7, image.get_pixel(15, 0));
        assert_eq!(0x0, image.get_pixel(0, 1));
        assert_eq!(0xC, image.get_pixel(10, 1));
        assert_eq!(0x9, image.get_pixel(15, 1));
    }

    #[test]
    fn get_pixel_depth8() {
        let image = Image::new(
            3,
            2,
            ScanlinePad::Pad32,
            1,
            BitsPerPixel::B8,
            ImageOrder::MsbFirst,
            Cow::Borrowed(&DATA),
        )
        .unwrap();
        assert_eq!(0xAB, image.get_pixel(0, 0));
        assert_eq!(0x36, image.get_pixel(1, 0));
        assert_eq!(0x18, image.get_pixel(2, 0));
        assert_eq!(0x12, image.get_pixel(0, 1));
        assert_eq!(0x34, image.get_pixel(1, 1));
        assert_eq!(0x56, image.get_pixel(2, 1));
    }

    #[test]
    fn get_pixel_depth16() {
        let image = Image::new(
            3,
            2,
            ScanlinePad::Pad32,
            1,
            BitsPerPixel::B16,
            ImageOrder::MsbFirst,
            Cow::Borrowed(&DATA),
        )
        .unwrap();
        assert_eq!(0xAB36, image.get_pixel(0, 0));
        assert_eq!(0x18F8, image.get_pixel(1, 0));
        assert_eq!(0x1234, image.get_pixel(2, 0));
        assert_eq!(0x0000, image.get_pixel(0, 1));
        assert_eq!(0x0000, image.get_pixel(1, 1));
        assert_eq!(0xFEDC, image.get_pixel(2, 1));
    }

    #[test]
    fn get_pixel_depth32() {
        let image = Image::new(
            2,
            2,
            ScanlinePad::Pad32,
            1,
            BitsPerPixel::B32,
            ImageOrder::MsbFirst,
            Cow::Borrowed(&DATA),
        )
        .unwrap();
        assert_eq!(0xAB36_18F8, image.get_pixel(0, 0));
        assert_eq!(0x1234_5678, image.get_pixel(1, 0));
        assert_eq!(0x0000_0000, image.get_pixel(0, 1));
        assert_eq!(0xFEDC_BA98, image.get_pixel(1, 1));
    }

    static DATA: [u8; 16] = [
        0xAB, 0x36, 0x18, 0xF8, 0x12, 0x34, 0x56, 0x78, 0x00, 0x00, 0x00, 0x00, 0xFE, 0xDC, 0xBA,
        0x98,
    ];
}
