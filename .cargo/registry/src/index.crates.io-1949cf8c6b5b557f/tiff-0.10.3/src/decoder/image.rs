use super::ifd::Value;
use super::stream::PackBitsReader;
use super::tag_reader::TagReader;
use super::ChunkType;
use super::{predict_f16, predict_f32, predict_f64, ValueReader};
use crate::tags::{
    CompressionMethod, PhotometricInterpretation, PlanarConfiguration, Predictor, SampleFormat, Tag,
};
use crate::{
    ColorType, Directory, TiffError, TiffFormatError, TiffResult, TiffUnsupportedError, UsageError,
};

use std::io::{self, Cursor, Read, Seek};
use std::num::NonZeroUsize;
use std::sync::Arc;

#[derive(Debug)]
pub(crate) struct StripDecodeState {
    pub rows_per_strip: u32,
}

#[derive(Debug)]
/// Computed values useful for tile decoding
pub(crate) struct TileAttributes {
    pub image_width: usize,
    pub image_height: usize,

    pub tile_width: usize,
    pub tile_length: usize,
}

impl TileAttributes {
    pub fn tiles_across(&self) -> usize {
        self.image_width.div_ceil(self.tile_width)
    }
    pub fn tiles_down(&self) -> usize {
        self.image_height.div_ceil(self.tile_length)
    }
    fn padding_right(&self) -> usize {
        (self.tile_width - self.image_width % self.tile_width) % self.tile_width
    }
    fn padding_down(&self) -> usize {
        (self.tile_length - self.image_height % self.tile_length) % self.tile_length
    }
    pub fn get_padding(&self, tile: usize) -> (usize, usize) {
        let row = tile / self.tiles_across();
        let column = tile % self.tiles_across();

        let padding_right = if column == self.tiles_across() - 1 {
            self.padding_right()
        } else {
            0
        };

        let padding_down = if row == self.tiles_down() - 1 {
            self.padding_down()
        } else {
            0
        };

        (padding_right, padding_down)
    }
}

#[derive(Debug)]
pub(crate) struct Image {
    pub ifd: Option<Directory>,
    pub width: u32,
    pub height: u32,
    pub bits_per_sample: u8,
    pub samples: u16,
    pub sample_format: SampleFormat,
    pub photometric_interpretation: PhotometricInterpretation,
    pub compression_method: CompressionMethod,
    pub predictor: Predictor,
    pub jpeg_tables: Option<Arc<Vec<u8>>>,
    pub chunk_type: ChunkType,
    pub planar_config: PlanarConfiguration,
    pub strip_decoder: Option<StripDecodeState>,
    pub tile_attributes: Option<TileAttributes>,
    pub chunk_offsets: Vec<u64>,
    pub chunk_bytes: Vec<u64>,
}

impl Image {
    pub fn from_reader<R: Read + Seek>(
        decoder: &mut ValueReader<R>,
        ifd: Directory,
    ) -> TiffResult<Image> {
        let mut tag_reader = TagReader { decoder, ifd: &ifd };

        let width = tag_reader.require_tag(Tag::ImageWidth)?.into_u32()?;
        let height = tag_reader.require_tag(Tag::ImageLength)?.into_u32()?;
        if width == 0 || height == 0 {
            return Err(TiffError::FormatError(TiffFormatError::InvalidDimensions(
                width, height,
            )));
        }

        let photometric_interpretation = tag_reader
            .find_tag(Tag::PhotometricInterpretation)?
            .map(Value::into_u16)
            .transpose()?
            .and_then(PhotometricInterpretation::from_u16)
            .ok_or(TiffUnsupportedError::UnknownInterpretation)?;

        // Try to parse both the compression method and the number, format, and bits of the included samples.
        // If they are not explicitly specified, those tags are reset to their default values and not carried from previous images.
        let compression_method = match tag_reader.find_tag(Tag::Compression)? {
            Some(val) => CompressionMethod::from_u16_exhaustive(val.into_u16()?),
            None => CompressionMethod::None,
        };

        let jpeg_tables = if compression_method == CompressionMethod::ModernJPEG
            && ifd.contains(Tag::JPEGTables)
        {
            let vec = tag_reader
                .find_tag(Tag::JPEGTables)?
                .unwrap()
                .into_u8_vec()?;
            if vec.len() < 2 {
                return Err(TiffError::FormatError(
                    TiffFormatError::InvalidTagValueType(Tag::JPEGTables),
                ));
            }

            Some(Arc::new(vec))
        } else {
            None
        };

        let samples: u16 = tag_reader
            .find_tag(Tag::SamplesPerPixel)?
            .map(Value::into_u16)
            .transpose()?
            .unwrap_or(1);
        if samples == 0 {
            return Err(TiffFormatError::SamplesPerPixelIsZero.into());
        }

        let sample_format = match tag_reader.find_tag_uint_vec(Tag::SampleFormat)? {
            Some(vals) => {
                let sample_format: Vec<_> = vals
                    .into_iter()
                    .map(SampleFormat::from_u16_exhaustive)
                    .collect();

                // TODO: for now, only homogenous formats across samples are supported.
                if !sample_format.windows(2).all(|s| s[0] == s[1]) {
                    return Err(TiffUnsupportedError::UnsupportedSampleFormat(sample_format).into());
                }

                sample_format[0]
            }
            None => SampleFormat::Uint,
        };

        let bits_per_sample: Vec<u8> = tag_reader
            .find_tag_uint_vec(Tag::BitsPerSample)?
            .unwrap_or_else(|| vec![1]);

        // Technically bits_per_sample.len() should be *equal* to samples, but libtiff also allows
        // it to be a single value that applies to all samples.
        if bits_per_sample.len() != samples.into() && bits_per_sample.len() != 1 {
            return Err(TiffError::FormatError(
                TiffFormatError::InconsistentSizesEncountered,
            ));
        }

        // This library (and libtiff) do not support mixed sample formats and zero bits per sample
        // doesn't make sense.
        if bits_per_sample.iter().any(|&b| b != bits_per_sample[0]) || bits_per_sample[0] == 0 {
            return Err(TiffUnsupportedError::InconsistentBitsPerSample(bits_per_sample).into());
        }

        let predictor = tag_reader
            .find_tag(Tag::Predictor)?
            .map(Value::into_u16)
            .transpose()?
            .map(|p| {
                Predictor::from_u16(p)
                    .ok_or(TiffError::FormatError(TiffFormatError::UnknownPredictor(p)))
            })
            .transpose()?
            .unwrap_or(Predictor::None);

        let planar_config = tag_reader
            .find_tag(Tag::PlanarConfiguration)?
            .map(Value::into_u16)
            .transpose()?
            .map(|p| {
                PlanarConfiguration::from_u16(p).ok_or(TiffError::FormatError(
                    TiffFormatError::UnknownPlanarConfiguration(p),
                ))
            })
            .transpose()?
            .unwrap_or(PlanarConfiguration::Chunky);

        let planes = match planar_config {
            PlanarConfiguration::Chunky => 1,
            PlanarConfiguration::Planar => samples,
        };

        let chunk_type;
        let chunk_offsets;
        let chunk_bytes;
        let strip_decoder;
        let tile_attributes;
        match (
            ifd.contains(Tag::StripByteCounts),
            ifd.contains(Tag::StripOffsets),
            ifd.contains(Tag::TileByteCounts),
            ifd.contains(Tag::TileOffsets),
        ) {
            (true, true, false, false) => {
                chunk_type = ChunkType::Strip;

                chunk_offsets = tag_reader
                    .find_tag(Tag::StripOffsets)?
                    .unwrap()
                    .into_u64_vec()?;
                chunk_bytes = tag_reader
                    .find_tag(Tag::StripByteCounts)?
                    .unwrap()
                    .into_u64_vec()?;
                let rows_per_strip = tag_reader
                    .find_tag(Tag::RowsPerStrip)?
                    .map(Value::into_u32)
                    .transpose()?
                    .unwrap_or(height);
                strip_decoder = Some(StripDecodeState { rows_per_strip });
                tile_attributes = None;

                if chunk_offsets.len() != chunk_bytes.len()
                    || rows_per_strip == 0
                    || u32::try_from(chunk_offsets.len())?
                        != (height.saturating_sub(1) / rows_per_strip + 1) * planes as u32
                {
                    return Err(TiffError::FormatError(
                        TiffFormatError::InconsistentSizesEncountered,
                    ));
                }
            }
            (false, false, true, true) => {
                chunk_type = ChunkType::Tile;

                let tile_width =
                    usize::try_from(tag_reader.require_tag(Tag::TileWidth)?.into_u32()?)?;
                let tile_length =
                    usize::try_from(tag_reader.require_tag(Tag::TileLength)?.into_u32()?)?;

                if tile_width == 0 {
                    return Err(TiffFormatError::InvalidTagValueType(Tag::TileWidth).into());
                } else if tile_length == 0 {
                    return Err(TiffFormatError::InvalidTagValueType(Tag::TileLength).into());
                }

                strip_decoder = None;
                tile_attributes = Some(TileAttributes {
                    image_width: usize::try_from(width)?,
                    image_height: usize::try_from(height)?,
                    tile_width,
                    tile_length,
                });
                chunk_offsets = tag_reader
                    .find_tag(Tag::TileOffsets)?
                    .unwrap()
                    .into_u64_vec()?;
                chunk_bytes = tag_reader
                    .find_tag(Tag::TileByteCounts)?
                    .unwrap()
                    .into_u64_vec()?;

                let tile = tile_attributes.as_ref().unwrap();
                if chunk_offsets.len() != chunk_bytes.len()
                    || chunk_offsets.len()
                        != tile.tiles_down() * tile.tiles_across() * planes as usize
                {
                    return Err(TiffError::FormatError(
                        TiffFormatError::InconsistentSizesEncountered,
                    ));
                }
            }
            (_, _, _, _) => {
                return Err(TiffError::FormatError(
                    TiffFormatError::StripTileTagConflict,
                ))
            }
        };

        Ok(Image {
            ifd: Some(ifd),
            width,
            height,
            bits_per_sample: bits_per_sample[0],
            samples,
            sample_format,
            photometric_interpretation,
            compression_method,
            jpeg_tables,
            predictor,
            chunk_type,
            planar_config,
            strip_decoder,
            tile_attributes,
            chunk_offsets,
            chunk_bytes,
        })
    }

    pub(crate) fn colortype(&self) -> TiffResult<ColorType> {
        match self.photometric_interpretation {
            PhotometricInterpretation::RGB => match self.samples {
                3 => Ok(ColorType::RGB(self.bits_per_sample)),
                4 => Ok(ColorType::RGBA(self.bits_per_sample)),
                // FIXME: We should _ignore_ other components. In particular:
                // > Beware of extra components. Some TIFF files may have more components per pixel
                // than you think. A Baseline TIFF reader must skip over them gracefully,using the
                // values of the SamplesPerPixel and BitsPerSample fields.
                // > -- TIFF 6.0 Specification, Section 7, Additional Baseline requirements.
                _ => Err(TiffError::UnsupportedError(
                    TiffUnsupportedError::InterpretationWithBits(
                        self.photometric_interpretation,
                        vec![self.bits_per_sample; self.samples as usize],
                    ),
                )),
            },
            PhotometricInterpretation::CMYK => match self.samples {
                4 => Ok(ColorType::CMYK(self.bits_per_sample)),
                5 => Ok(ColorType::CMYKA(self.bits_per_sample)),
                _ => Err(TiffError::UnsupportedError(
                    TiffUnsupportedError::InterpretationWithBits(
                        self.photometric_interpretation,
                        vec![self.bits_per_sample; self.samples as usize],
                    ),
                )),
            },
            PhotometricInterpretation::YCbCr => match self.samples {
                3 => Ok(ColorType::YCbCr(self.bits_per_sample)),
                _ => Err(TiffError::UnsupportedError(
                    TiffUnsupportedError::InterpretationWithBits(
                        self.photometric_interpretation,
                        vec![self.bits_per_sample; self.samples as usize],
                    ),
                )),
            },
            // TODO: treatment of WhiteIsZero is not quite consistent with `invert_colors` that is
            // later called when that interpretation is read. That function does not support
            // Multiband as a color type and will error. It's unclear how to resolve that exactly.
            PhotometricInterpretation::BlackIsZero | PhotometricInterpretation::WhiteIsZero => {
                match self.samples {
                    1 => Ok(ColorType::Gray(self.bits_per_sample)),
                    _ => Ok(ColorType::Multiband {
                        bit_depth: self.bits_per_sample,
                        num_samples: self.samples,
                    }),
                }
            }
            // TODO: this is bad we should not fail at this point
            PhotometricInterpretation::RGBPalette
            | PhotometricInterpretation::TransparencyMask
            | PhotometricInterpretation::CIELab => Err(TiffError::UnsupportedError(
                TiffUnsupportedError::InterpretationWithBits(
                    self.photometric_interpretation,
                    vec![self.bits_per_sample; self.samples as usize],
                ),
            )),
        }
    }

    pub(crate) fn minimum_row_stride(&self, dims: (u32, u32)) -> Option<NonZeroUsize> {
        let (width, height) = dims;

        let row_stride = u64::from(width)
            .saturating_mul(self.samples_per_pixel() as u64)
            .saturating_mul(self.bits_per_sample as u64)
            .div_ceil(8);

        // Note: row stride should be smaller than the len if we have an actual buffer. If there
        // are no pixels in the buffer (height _or_ width is 0) then the stride is not well defined
        // and we return `None`.
        (height > 0)
            .then_some(row_stride as usize)
            .and_then(NonZeroUsize::new)
    }

    fn create_reader<'r, R: 'r + Read>(
        reader: R,
        compression_method: CompressionMethod,
        compressed_length: u64,
        // FIXME: these should be `expect` attributes or we choose another way of passing them.
        #[cfg_attr(not(feature = "jpeg"), allow(unused_variables))] jpeg_tables: Option<&[u8]>,
        #[cfg_attr(not(feature = "fax"), allow(unused_variables))] dimensions: (u32, u32),
    ) -> TiffResult<Box<dyn Read + 'r>> {
        Ok(match compression_method {
            CompressionMethod::None => Box::new(reader),
            #[cfg(feature = "lzw")]
            CompressionMethod::LZW => Box::new(super::stream::LZWReader::new(
                reader,
                usize::try_from(compressed_length)?,
            )),
            #[cfg(feature = "zstd")]
            CompressionMethod::ZSTD => Box::new(zstd::Decoder::new(reader)?),
            CompressionMethod::PackBits => Box::new(PackBitsReader::new(reader, compressed_length)),
            #[cfg(feature = "deflate")]
            CompressionMethod::Deflate | CompressionMethod::OldDeflate => {
                Box::new(super::stream::DeflateReader::new(reader))
            }
            #[cfg(feature = "jpeg")]
            CompressionMethod::ModernJPEG => {
                use zune_jpeg::zune_core;

                if jpeg_tables.is_some() && compressed_length < 2 {
                    return Err(TiffError::FormatError(
                        TiffFormatError::InvalidTagValueType(Tag::JPEGTables),
                    ));
                }

                // Construct new jpeg_reader wrapping a SmartReader.
                //
                // JPEG compression in TIFF allows saving quantization and/or huffman tables in one
                // central location. These `jpeg_tables` are simply prepended to the remaining jpeg image data.
                // Because these `jpeg_tables` start with a `SOI` (HEX: `0xFFD8`) or __start of image__ marker
                // which is also at the beginning of the remaining JPEG image data and would
                // confuse the JPEG renderer, one of these has to be taken off. In this case the first two
                // bytes of the remaining JPEG data is removed because it follows `jpeg_tables`.
                // Similary, `jpeg_tables` ends with a `EOI` (HEX: `0xFFD9`) or __end of image__ marker,
                // this has to be removed as well (last two bytes of `jpeg_tables`).
                let mut jpeg_reader = match jpeg_tables {
                    Some(jpeg_tables) => {
                        let mut reader = reader.take(compressed_length);
                        reader.read_exact(&mut [0; 2])?;

                        Box::new(
                            Cursor::new(&jpeg_tables[..jpeg_tables.len() - 2])
                                .chain(reader.take(compressed_length)),
                        ) as Box<dyn Read>
                    }
                    None => Box::new(reader.take(compressed_length)),
                };

                let mut jpeg_data = Vec::new();
                jpeg_reader.read_to_end(&mut jpeg_data)?;

                let mut decoder = zune_jpeg::JpegDecoder::new(jpeg_data);
                let mut options: zune_core::options::DecoderOptions = Default::default();

                // Disable color conversion by setting the output colorspace to the input
                // colorspace.
                decoder.decode_headers()?;
                if let Some(colorspace) = decoder.get_input_colorspace() {
                    options = options.jpeg_set_out_colorspace(colorspace);
                }

                decoder.set_options(options);

                let data = decoder.decode()?;

                Box::new(Cursor::new(data))
            }
            #[cfg(feature = "fax")]
            CompressionMethod::Fax4 => Box::new(super::stream::Group4Reader::new(
                dimensions,
                reader,
                compressed_length,
            )?),
            method => {
                return Err(TiffError::UnsupportedError(
                    TiffUnsupportedError::UnsupportedCompressionMethod(method),
                ))
            }
        })
    }

    /// Samples per pixel within chunk.
    ///
    /// In planar config, samples are stored in separate strips/chunks, also called bands.
    ///
    /// Example with `bits_per_sample = [8, 8, 8]` and `PhotometricInterpretation::RGB`:
    /// * `PlanarConfiguration::Chunky` -> 3 (RGBRGBRGB...)
    /// * `PlanarConfiguration::Planar` -> 1 (RRR...) (GGG...) (BBB...)
    pub(crate) fn samples_per_pixel(&self) -> usize {
        match self.planar_config {
            PlanarConfiguration::Chunky => self.samples.into(),
            PlanarConfiguration::Planar => 1,
        }
    }

    /// Number of strips per pixel.
    pub(crate) fn strips_per_pixel(&self) -> usize {
        match self.planar_config {
            PlanarConfiguration::Chunky => 1,
            PlanarConfiguration::Planar => self.samples.into(),
        }
    }

    pub(crate) fn chunk_file_range(&self, chunk: u32) -> TiffResult<(u64, u64)> {
        let file_offset = self
            .chunk_offsets
            .get(chunk as usize)
            .ok_or(TiffError::FormatError(
                TiffFormatError::InconsistentSizesEncountered,
            ))?;

        let compressed_bytes =
            self.chunk_bytes
                .get(chunk as usize)
                .ok_or(TiffError::FormatError(
                    TiffFormatError::InconsistentSizesEncountered,
                ))?;

        Ok((*file_offset, *compressed_bytes))
    }

    pub(crate) fn chunk_dimensions(&self) -> TiffResult<(u32, u32)> {
        match self.chunk_type {
            ChunkType::Strip => {
                let strip_attrs = self.strip_decoder.as_ref().unwrap();
                Ok((self.width, strip_attrs.rows_per_strip))
            }
            ChunkType::Tile => {
                let tile_attrs = self.tile_attributes.as_ref().unwrap();
                Ok((
                    u32::try_from(tile_attrs.tile_width)?,
                    u32::try_from(tile_attrs.tile_length)?,
                ))
            }
        }
    }

    pub(crate) fn chunk_data_dimensions(&self, chunk_index: u32) -> TiffResult<(u32, u32)> {
        let dims = self.chunk_dimensions()?;

        match self.chunk_type {
            ChunkType::Strip => {
                let strip_attrs = self.strip_decoder.as_ref().unwrap();
                let strips_per_band =
                    self.height.saturating_sub(1) / strip_attrs.rows_per_strip + 1;
                let strip_height_without_padding = (chunk_index % strips_per_band)
                    .checked_mul(dims.1)
                    .and_then(|x| self.height.checked_sub(x))
                    .ok_or(TiffError::UsageError(UsageError::InvalidChunkIndex(
                        chunk_index,
                    )))?;

                // Ignore potential vertical padding on the bottommost strip
                let strip_height = dims.1.min(strip_height_without_padding);

                Ok((dims.0, strip_height))
            }
            ChunkType::Tile => {
                let tile_attrs = self.tile_attributes.as_ref().unwrap();
                let (padding_right, padding_down) = tile_attrs.get_padding(chunk_index as usize);

                let tile_width = tile_attrs.tile_width - padding_right;
                let tile_length = tile_attrs.tile_length - padding_down;

                Ok((u32::try_from(tile_width)?, u32::try_from(tile_length)?))
            }
        }
    }

    pub(crate) fn expand_chunk(
        &self,
        reader: &mut ValueReader<impl Read>,
        buf: &mut [u8],
        output_row_stride: usize,
        chunk_index: u32,
    ) -> TiffResult<()> {
        let ValueReader {
            reader,
            bigtiff: _,
            limits,
        } = reader;

        let byte_order = reader.byte_order;
        let reader = reader.inner();

        // Validate that the color type is supported.
        let color_type = self.colortype()?;
        match color_type {
            ColorType::RGB(n)
            | ColorType::RGBA(n)
            | ColorType::CMYK(n)
            | ColorType::CMYKA(n)
            | ColorType::YCbCr(n)
            | ColorType::Gray(n)
            | ColorType::Multiband {
                bit_depth: n,
                num_samples: _,
            } if n == 8 || n == 16 || n == 32 || n == 64 => {}
            ColorType::Gray(n)
            | ColorType::Multiband {
                bit_depth: n,
                num_samples: _,
            } if n < 8 => match self.predictor {
                Predictor::None => {}
                Predictor::Horizontal => {
                    return Err(TiffError::UnsupportedError(
                        TiffUnsupportedError::HorizontalPredictor(color_type),
                    ));
                }
                Predictor::FloatingPoint => {
                    return Err(TiffError::UnsupportedError(
                        TiffUnsupportedError::FloatingPointPredictor(color_type),
                    ));
                }
            },
            type_ => {
                return Err(TiffError::UnsupportedError(
                    TiffUnsupportedError::UnsupportedColorType(type_),
                ));
            }
        }

        // Validate that the predictor is supported for the sample type.
        match (self.predictor, self.sample_format) {
            (
                Predictor::Horizontal,
                SampleFormat::Int | SampleFormat::Uint | SampleFormat::IEEEFP,
            ) => {}
            (Predictor::Horizontal, _) => {
                return Err(TiffError::UnsupportedError(
                    TiffUnsupportedError::HorizontalPredictor(color_type),
                ));
            }
            (Predictor::FloatingPoint, SampleFormat::IEEEFP) => {}
            (Predictor::FloatingPoint, _) => {
                return Err(TiffError::UnsupportedError(
                    TiffUnsupportedError::FloatingPointPredictor(color_type),
                ));
            }
            _ => {}
        }

        let compressed_bytes =
            self.chunk_bytes
                .get(chunk_index as usize)
                .ok_or(TiffError::FormatError(
                    TiffFormatError::InconsistentSizesEncountered,
                ))?;
        if *compressed_bytes > limits.intermediate_buffer_size as u64 {
            return Err(TiffError::LimitsExceeded);
        }

        let compression_method = self.compression_method;
        let photometric_interpretation = self.photometric_interpretation;
        let predictor = self.predictor;
        let samples = self.samples_per_pixel();

        let chunk_dims = self.chunk_dimensions()?;
        let data_dims = self.chunk_data_dimensions(chunk_index)?;

        let chunk_row_bits = (u64::from(chunk_dims.0) * u64::from(self.bits_per_sample))
            .checked_mul(samples as u64)
            .ok_or(TiffError::LimitsExceeded)?;
        let chunk_row_bytes: usize = chunk_row_bits.div_ceil(8).try_into()?;

        let data_row_bits = (u64::from(data_dims.0) * u64::from(self.bits_per_sample))
            .checked_mul(samples as u64)
            .ok_or(TiffError::LimitsExceeded)?;
        let data_row_bytes: usize = data_row_bits.div_ceil(8).try_into()?;

        // TODO: Should these return errors instead?
        assert!(output_row_stride >= data_row_bytes);
        assert!(buf.len() >= output_row_stride * (data_dims.1 as usize - 1) + data_row_bytes);

        let mut reader = Self::create_reader(
            reader,
            compression_method,
            *compressed_bytes,
            self.jpeg_tables.as_deref().map(|a| &**a),
            chunk_dims,
        )?;

        if output_row_stride == chunk_row_bytes {
            let tile = &mut buf[..chunk_row_bytes * data_dims.1 as usize];
            reader.read_exact(tile)?;

            for row in tile.chunks_mut(chunk_row_bytes) {
                super::fix_endianness_and_predict(
                    row,
                    color_type.bit_depth(),
                    samples,
                    byte_order,
                    predictor,
                );
            }
            if photometric_interpretation == PhotometricInterpretation::WhiteIsZero {
                super::invert_colors(tile, color_type, self.sample_format)?;
            }
        } else if chunk_row_bytes > data_row_bytes && self.predictor == Predictor::FloatingPoint {
            // The floating point predictor shuffles the padding bytes into the encoded output, so
            // this case is handled specially when needed.
            let mut encoded = vec![0u8; chunk_row_bytes];
            for row in buf.chunks_mut(output_row_stride).take(data_dims.1 as usize) {
                reader.read_exact(&mut encoded)?;

                let row = &mut row[..data_row_bytes];
                match color_type.bit_depth() {
                    16 => predict_f16(&mut encoded, row, samples),
                    32 => predict_f32(&mut encoded, row, samples),
                    64 => predict_f64(&mut encoded, row, samples),
                    _ => unreachable!(),
                }
                if photometric_interpretation == PhotometricInterpretation::WhiteIsZero {
                    super::invert_colors(row, color_type, self.sample_format)?;
                }
            }
        } else {
            for row in buf.chunks_mut(output_row_stride).take(data_dims.1 as usize) {
                let row = &mut row[..data_row_bytes];
                reader.read_exact(row)?;

                // Skip horizontal padding
                if chunk_row_bytes > data_row_bytes {
                    let len = u64::try_from(chunk_row_bytes - data_row_bytes)?;
                    io::copy(&mut reader.by_ref().take(len), &mut io::sink())?;
                }

                super::fix_endianness_and_predict(
                    row,
                    color_type.bit_depth(),
                    samples,
                    byte_order,
                    predictor,
                );
                if photometric_interpretation == PhotometricInterpretation::WhiteIsZero {
                    super::invert_colors(row, color_type, self.sample_format)?;
                }
            }
        }

        Ok(())
    }
}
