#![forbid(unsafe_code)]
use crate::error::{LimitError, LimitErrorKind};
use crate::math::multiply_accumulate;
use crate::ImageError;

use num_traits::{AsPrimitive, MulAdd};
use std::mem::size_of;
use std::ops::{Add, Mul};

pub(crate) trait SafeMul<S> {
    fn safe_mul(&self, rhs: S) -> Result<S, ImageError>;
}

pub(crate) trait SafeAdd<S> {
    fn safe_add(&self, rhs: S) -> Result<S, ImageError>;
}

impl SafeMul<usize> for usize {
    #[inline]
    fn safe_mul(&self, rhs: usize) -> Result<usize, ImageError> {
        if let Some(product) = self.checked_mul(rhs) {
            Ok(product)
        } else {
            Err(ImageError::Limits(LimitError::from_kind(
                LimitErrorKind::DimensionError,
            )))
        }
    }
}

impl SafeAdd<usize> for usize {
    #[inline]
    fn safe_add(&self, rhs: usize) -> Result<usize, ImageError> {
        if let Some(product) = self.checked_add(rhs) {
            Ok(product)
        } else {
            Err(ImageError::Limits(LimitError::from_kind(
                LimitErrorKind::DimensionError,
            )))
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct KernelShape {
    width: usize,
    height: usize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FilterImageSize {
    pub(crate) width: usize,
    pub(crate) height: usize,
}

/// Pads an image row with *clamp* strategy
///
/// This method copies real content into center of new buffer
/// and filling leading and trailing physical padded parts with *clamp* strategy
fn make_arena_row<T, const N: usize>(
    image: &[T],
    row_buffer: &mut [T],
    source_y: usize,
    image_size: FilterImageSize,
    kernel_size: KernelShape,
) -> Result<(), ImageError>
where
    T: Default + Copy + Send + Sync + 'static,
    f64: AsPrimitive<T>,
{
    assert_eq!(image.len(), N * image_size.width * image_size.height);

    let pad_w = (kernel_size.width / 2).max(1);

    let arena_width = image_size
        .width
        .safe_mul(N)?
        .safe_add(pad_w.safe_mul(2 * N)?)?;

    let source_offset = source_y * image_size.width * N;
    assert_eq!(row_buffer.len(), arena_width);

    let row_dst = &mut row_buffer[pad_w * N..(pad_w * N + image_size.width * N)];

    let source_row = &image[source_offset..(source_offset + image_size.width * N)];

    for (dst, src) in row_dst.iter_mut().zip(source_row.iter()) {
        *dst = *src;
    }

    for (x, dst) in (0..pad_w).zip(row_buffer.chunks_exact_mut(N)) {
        let old_x = x.saturating_sub(pad_w).min(image_size.width - 1);
        let old_px = old_x * N;
        let src_iter = &source_row[old_px..(old_px + N)];
        for (dst, src) in dst.iter_mut().zip(src_iter.iter()) {
            *dst = *src;
        }
    }

    for (x, dst) in
        (image_size.width..(image_size.width + pad_w)).zip(row_buffer.chunks_exact_mut(N).rev())
    {
        let old_x = x.max(0).min(image_size.width - 1);
        let old_px = old_x * N;
        let src_iter = &source_row[old_px..(old_px + N)];
        for (dst, src) in dst.iter_mut().zip(src_iter.iter()) {
            *dst = *src;
        }
    }
    Ok(())
}

#[derive(Clone)]
struct ArenaColumns<T>
where
    T: Copy,
{
    top_pad: Vec<T>,
    bottom_pad: Vec<T>,
}

/// Pads a column image with *clamp* strategy
///
/// This method is used for *virtual* column padding.
/// It produces two arrays that represents virtual image top part and bottom
fn make_columns_arenas<T, const N: usize>(
    image: &[T],
    image_size: FilterImageSize,
    kernel_size: KernelShape,
) -> ArenaColumns<T>
where
    T: Default + Copy + Send + Sync + 'static,
    f64: AsPrimitive<T>,
{
    assert_eq!(image.len(), N * image_size.width * image_size.height);
    let pad_h = kernel_size.height / 2;

    let mut top_pad = vec![T::default(); pad_h * image_size.width * N];
    let mut bottom_pad = vec![T::default(); pad_h * image_size.width * N];

    let top_pad_stride = image_size.width * N;

    for (ky, dst) in (0..pad_h).zip(top_pad.chunks_exact_mut(top_pad_stride)) {
        for (kx, dst) in (0..image_size.width).zip(dst.chunks_exact_mut(N)) {
            let y = ky.saturating_sub(pad_h).min(image_size.height - 1);
            let v_src = y * top_pad_stride + kx * N;

            let src_iter = &image[v_src..(v_src + N)];
            for (dst, src) in dst.iter_mut().zip(src_iter.iter()) {
                *dst = *src;
            }
        }
    }

    let bottom_iter_dst = bottom_pad.chunks_exact_mut(top_pad_stride);

    for (ky, dst) in (0..pad_h).zip(bottom_iter_dst) {
        for (kx, dst) in (0..image_size.width).zip(dst.chunks_exact_mut(N)) {
            let y = (ky + image_size.height).min(image_size.height - 1);
            let v_src = y * top_pad_stride + kx * N;
            let src_iter = &image[v_src..(v_src + N)];
            for (dst, src) in dst.iter_mut().zip(src_iter.iter()) {
                *dst = *src;
            }
        }
    }

    ArenaColumns {
        top_pad,
        bottom_pad,
    }
}

trait ToStorage<S> {
    fn to_(self) -> S;
}

const Q0_15: i32 = 15;

impl ToStorage<u8> for u32 {
    #[inline(always)]
    fn to_(self) -> u8 {
        ((self + (1 << (Q0_15 - 1))) >> Q0_15).min(255) as u8
    }
}

impl ToStorage<u16> for u32 {
    #[inline(always)]
    fn to_(self) -> u16 {
        ((self + (1 << (Q0_15 - 1))) >> Q0_15).min(u16::MAX as u32) as u16
    }
}

impl ToStorage<u16> for f32 {
    #[inline(always)]
    fn to_(self) -> u16 {
        self.round().min(u16::MAX as f32) as u16
    }
}

impl ToStorage<f32> for f32 {
    #[inline(always)]
    fn to_(self) -> f32 {
        self
    }
}

/// Performs column convolution pass for symmetrical filter
///
/// Common convolution formula O(x,y)=∑K(k)⋅I(x,y+k); where sums goes from 0...R
/// when filter is symmetric that we can half kernel reads by using formula
/// O(x,y)=(∑K(k)⋅(I(x,y+k) + I(x,y+(R-k)))) + K(R/2)⋅I(x,y+R/2); where sums goes from 0...R/2
fn filter_symmetric_column<T, F, const N: usize>(
    arena_src: &[&[T]],
    dst_row: &mut [T],
    image_size: FilterImageSize,
    kernel: &[F],
) where
    T: Copy + AsPrimitive<F>,
    F: ToStorage<T>
        + Mul<F, Output = F>
        + MulAdd<F, Output = F>
        + Add<F, Output = F>
        + Default
        + Copy
        + 'static,
{
    let dst_stride = image_size.width * N;

    let length = kernel.len();
    let half_len = length / 2;

    let mut cx = 0usize;

    let coeff = kernel[half_len];

    let mut dst_rem = dst_row;

    if size_of::<T>() == 1 {
        for chunk in dst_rem.chunks_exact_mut(32) {
            let mut store0: [F; 16] = [F::default(); 16];
            let mut store1: [F; 16] = [F::default(); 16];

            let v_src0 = &arena_src[half_len][cx..(cx + 16)];
            let v_src1 = &arena_src[half_len][(cx + 16)..(cx + 32)];

            for (dst, src) in store0.iter_mut().zip(v_src0) {
                *dst = src.as_().mul(coeff);
            }
            for (dst, src) in store1.iter_mut().zip(v_src1) {
                *dst = src.as_().mul(coeff);
            }

            for (i, &coeff) in kernel.iter().take(half_len).enumerate() {
                let other_side = length - i - 1;
                let fw_src = arena_src[i];
                let rw_src = arena_src[other_side];
                let fw0 = &fw_src[cx..(cx + 16)];
                let bw0 = &rw_src[cx..(cx + 16)];
                let fw1 = &fw_src[(cx + 16)..(cx + 32)];
                let bw1 = &rw_src[(cx + 16)..(cx + 32)];

                for ((dst, fw), bw) in store0.iter_mut().zip(fw0).zip(bw0) {
                    *dst = multiply_accumulate(*dst, fw.as_().add(bw.as_()), coeff);
                }

                for ((dst, fw), bw) in store1.iter_mut().zip(fw1).zip(bw1) {
                    *dst = multiply_accumulate(*dst, fw.as_().add(bw.as_()), coeff);
                }
            }

            let shaped_dst0 = &mut chunk[..16];

            for (src, dst) in store0.iter().zip(shaped_dst0.iter_mut()) {
                *dst = src.to_();
            }

            let shaped_dst1 = &mut chunk[16..32];

            for (src, dst) in store1.iter().zip(shaped_dst1.iter_mut()) {
                *dst = src.to_();
            }

            cx += 32;
        }

        dst_rem = dst_rem.chunks_exact_mut(32).into_remainder();
    }

    for chunk in dst_rem.chunks_exact_mut(16) {
        let mut store: [F; 16] = [F::default(); 16];

        let v_src = &arena_src[half_len][cx..(cx + 16)];

        for (dst, src) in store.iter_mut().zip(v_src) {
            *dst = src.as_().mul(coeff);
        }

        for (i, &coeff) in kernel.iter().take(half_len).enumerate() {
            let other_side = length - i - 1;
            let fw = &arena_src[i][cx..(cx + 16)];
            let bw = &arena_src[other_side][cx..(cx + 16)];

            for ((dst, fw), bw) in store.iter_mut().zip(fw).zip(bw) {
                *dst = multiply_accumulate(*dst, fw.as_().add(bw.as_()), coeff);
            }
        }

        for (src, dst) in store.iter().zip(chunk.iter_mut()) {
            *dst = src.to_();
        }

        cx += 16;
    }

    dst_rem = dst_rem.chunks_exact_mut(16).into_remainder();

    for chunk in dst_rem.chunks_exact_mut(4) {
        let v_src = &arena_src[half_len][cx..(cx + 4)];

        let mut k0 = v_src[0].as_().mul(coeff);
        let mut k1 = v_src[1].as_().mul(coeff);
        let mut k2 = v_src[2].as_().mul(coeff);
        let mut k3 = v_src[3].as_().mul(coeff);

        for (i, &coeff) in kernel.iter().take(half_len).enumerate() {
            let other_side = length - i - 1;
            let fw = &arena_src[i][cx..(cx + 4)];
            let bw = &arena_src[other_side][cx..(cx + 4)];
            k0 = multiply_accumulate(k0, fw[0].as_().add(bw[0].as_()), coeff);
            k1 = multiply_accumulate(k1, fw[1].as_().add(bw[1].as_()), coeff);
            k2 = multiply_accumulate(k2, fw[2].as_().add(bw[2].as_()), coeff);
            k3 = multiply_accumulate(k3, fw[3].as_().add(bw[3].as_()), coeff);
        }

        chunk[0] = k0.to_();
        chunk[1] = k1.to_();
        chunk[2] = k2.to_();
        chunk[3] = k3.to_();
        cx += 4;
    }

    dst_rem = dst_rem.chunks_exact_mut(4).into_remainder();

    for (chunk, x) in dst_rem.iter_mut().zip(cx..dst_stride) {
        let v_src = &arena_src[half_len][x..(x + 1)];

        let mut k0 = v_src[0].as_().mul(coeff);

        for (i, &coeff) in kernel.iter().take(half_len).enumerate() {
            let other_side = length - i - 1;
            let fw = &arena_src[i][x..(x + 1)];
            let bw = &arena_src[other_side][x..(x + 1)];
            k0 = multiply_accumulate(k0, fw[0].as_().add(bw[0].as_()), coeff);
        }

        *chunk = k0.to_();
    }
}

/// Performs horizontal convolution for row
///
/// Common convolution formula O(x,y)=∑K(k)⋅I(x+k,y); where sums goes from 0...R
/// when filter is symmetric that we can half kernel reads by using formula
/// O(x,y)=(∑K(k)⋅(I(x+k,y) + I(x+(R-k),y))) + K(R/2)⋅I(x+R/2,y); where sums goes from 0...R/2
fn filter_symmetric_row<T, F, const N: usize>(arena: &[T], dst_row: &mut [T], scanned_kernel: &[F])
where
    T: Copy + AsPrimitive<F> + Default,
    F: ToStorage<T>
        + Mul<Output = F>
        + MulAdd<F, Output = F>
        + Default
        + Add<F, Output = F>
        + Copy
        + 'static,
    i32: AsPrimitive<F>,
{
    let src = arena;

    let length = scanned_kernel.len();
    let half_len = length / 2;

    let hc = scanned_kernel[half_len];

    for (x, dst) in dst_row.chunks_exact_mut(4).enumerate() {
        let v_cx = x * 4;
        let src = &src[v_cx..];

        let chunk = &src[half_len * N..half_len * N + 4];

        let mut k0 = chunk[0].as_() * hc;
        let mut k1 = chunk[1].as_() * hc;
        let mut k2 = chunk[2].as_() * hc;
        let mut k3 = chunk[3].as_() * hc;

        // Note why here is no window operators:
        // https://github.com/image-rs/image/pull/2496#discussion_r2155171034
        for (i, &coeff) in scanned_kernel.iter().take(half_len).enumerate() {
            let other_side = length - i - 1;
            let fw = &src[(i * N)..(i * N) + 4];
            let bw = &src[(other_side * N)..(other_side * N) + 4];
            k0 = multiply_accumulate(k0, fw[0].as_() + bw[0].as_(), coeff);
            k1 = multiply_accumulate(k1, fw[1].as_() + bw[1].as_(), coeff);
            k2 = multiply_accumulate(k2, fw[2].as_() + bw[2].as_(), coeff);
            k3 = multiply_accumulate(k3, fw[3].as_() + bw[3].as_(), coeff);
        }

        dst[0] = k0.to_();
        dst[1] = k1.to_();
        dst[2] = k2.to_();
        dst[3] = k3.to_();
    }

    let dzx = dst_row.chunks_exact_mut(4).len() * 4;
    let remainder = dst_row.chunks_exact_mut(4).into_remainder();

    for (x, dst) in remainder.iter_mut().enumerate() {
        let v_cx = x + dzx;
        let src = &src[v_cx..];

        let mut k0 = src[half_len * N].as_() * hc;

        for (i, &coeff) in scanned_kernel.iter().take(half_len).enumerate() {
            let other_side = length - i - 1;
            let fw = &src[(i * N)..(i * N) + 1];
            let bw = &src[(other_side * N)..(other_side * N) + 1];
            k0 = multiply_accumulate(k0, fw[0].as_() + bw[0].as_(), coeff);
        }

        *dst = k0.to_();
    }
}

trait KernelTransformer<F, I> {
    fn transform(input: F) -> I;
}

impl KernelTransformer<f32, u32> for u8 {
    fn transform(input: f32) -> u32 {
        const SCALE: f32 = (1 << Q0_15) as f32;
        (input * SCALE).min(((1u32 << Q0_15) - 1) as f32).max(0.) as u32
    }
}

impl KernelTransformer<f32, u32> for u16 {
    fn transform(input: f32) -> u32 {
        const SCALE: f32 = (1 << Q0_15) as f32;
        (input * SCALE).min(((1u32 << Q0_15) - 1) as f32).max(0.) as u32
    }
}

impl KernelTransformer<f32, f32> for f32 {
    fn transform(input: f32) -> f32 {
        input
    }
}

impl KernelTransformer<f32, f32> for u16 {
    fn transform(input: f32) -> f32 {
        input
    }
}

/// Removes meaningless values from kernel preserving symmetry
fn prepare_symmetric_kernel<I: Copy + PartialEq + 'static>(kernel: &[I]) -> Vec<I>
where
    i32: AsPrimitive<I>,
{
    let zeros: I = 0i32.as_();
    let mut new_kernel = kernel.to_vec();
    while new_kernel.len() > 2
        && (new_kernel.last().unwrap().eq(&zeros) && new_kernel.first().unwrap().eq(&zeros))
    {
        new_kernel.remove(0);
        new_kernel.remove(new_kernel.len() - 1);
    }

    new_kernel
}

const RING_QUEUE_CIRCULAR_CUTOFF: usize = 55;

/// This is typical *Divide & Conquer* method with not typical *Sliding Window*.
/// Thus, instead of transient image here sliding window is used in a ring queue form.
///
/// Let's define R(n) as row with number: R0,R1,R2,R3 and so forth.
/// So to convolve an image the buffer that represents a ring is created with size:
/// `image_width * channels_count * vertical_kernel_length` as our ring is representing
/// columnar queue.
///
/// So at the very first time it's fully filled if we convolve image with vertical kernel size = 5,
/// then it holds: R0,R1,R2,R3,R4, each is already blurred horizontally.
/// Instead of removing/adding items from ring iterator position is hold
/// and after iterator is adjusted the new row just replaces the old ones.
/// Therefore, at the very moment of the first fulfilling we actually should blur now *R2*
/// that is central item, but our iterator already at *R4*, thus our *ring queue* is always
/// *ahead* of current row by `kernel_size/2`. So to send rows correctly to columnar pass,
/// it's required to reshuffle as `(current_iterator_position+1)%vertical_kernel_size`
/// our ring first ,since the iterator is always at the tail, advanced by
/// `kernel_size/2` from current row, and ahead from start by `kernel_size`.
///
/// After columnar pass iterator is adjusted one position forward in a ring style:
/// to fill the ring, the next row blurred, then it will replace leading row with a new one.
/// The flow after the first fill looks like:
/// - R0,R1,R2,R3,R4(iterator here, denoted as `I`)
/// - R5(I),R1,R2,R3,R4
/// - R5,R6(I),R2,R3,R4
/// - R5,R6,R7(I),R3,R4
/// - R5,R6,R7,R8(I),R4
/// - R5,R6,R7,R8,R9(I)
/// - R10(I),R6,R7,R8,R9
///
/// This approach is significantly more efficient for small kernels,
/// and several times faster in multithreaded environments
/// despite the presence of overlapping regions.
///
/// The algorithm consists of 3 stages that should be different for multithreaded and single-threaded
/// modes:
///
/// Single threaded:
/// - Blurring horizontal R0 and copy it into a ring buffer
///   up to `kernel_size/2 * channels_count * vertical_kernel_length` because we always clamp here
///   and first row the same
/// - Blur next row and adjust iterator until ring the first full ring fill.
/// - Blur vertically ring buffer, store row into destination,
///   adjust iterator, blur horizontally next row and repeat it until end
///
/// Multithreaded:
/// - Fill the ring buffer from tile's Y starting before the first full ring fill.
/// - Blur vertically ring buffer, store row into destination,
///   adjust iterator, blur horizontally next row and repeat it until end.
fn filter_2d_separable_ring_queue<T, I, const N: usize>(
    image: &[T],
    destination: &mut [T],
    image_size: FilterImageSize,
    row_kernel: &[I],
    column_kernel: &[I],
) -> Result<(), ImageError>
where
    T: Copy + Default + Send + Sync + AsPrimitive<I>,
    I: ToStorage<T>
        + Mul<I, Output = I>
        + Add<I, Output = I>
        + MulAdd<I, Output = I>
        + Send
        + Sync
        + PartialEq
        + Default
        + 'static
        + Copy,
    i32: AsPrimitive<I>,
    f64: AsPrimitive<T>,
{
    let pad_w = (row_kernel.len() / 2).max(1);

    let arena_width = image_size
        .width
        .safe_mul(N)?
        .safe_add(pad_w.safe_mul(2 * N)?)?;
    let mut row_buffer = vec![T::default(); arena_width];

    let full_width = image_size.width * N;

    // This is flat ring queue
    let mut buffer = vec![T::default(); (image_size.width * N).safe_mul(column_kernel.len())?];

    let column_kernel_len = column_kernel.len();

    let half_kernel = column_kernel_len / 2;

    make_arena_row::<T, N>(
        image,
        &mut row_buffer,
        0,
        image_size,
        KernelShape {
            width: row_kernel.len(),
            height: 0,
        },
    )?;

    // Blurring horizontally R0
    filter_symmetric_row::<T, I, N>(&row_buffer, &mut buffer[..full_width], row_kernel);

    // Distribute R0 up to half of kernel into a ring
    let (src_row, rest) = buffer.split_at_mut(full_width);
    for dst in rest.chunks_exact_mut(full_width).take(half_kernel) {
        for (dst, src) in dst.iter_mut().zip(src_row.iter()) {
            *dst = *src;
        }
    }

    let mut start_ky = column_kernel_len / 2 + 1;
    // This needs in case of anisotropy
    start_ky %= column_kernel_len;

    // image_size.height + half_kernel is here because as in description
    // we're always in advance on half of vertical kernel
    for y in 1..image_size.height + half_kernel {
        let new_y = if y < image_size.height {
            y
        } else {
            y.min(image_size.height - 1)
        };

        make_arena_row::<T, N>(
            image,
            &mut row_buffer,
            new_y,
            image_size,
            KernelShape {
                width: row_kernel.len(),
                height: 0,
            },
        )?;

        filter_symmetric_row::<T, I, N>(
            &row_buffer,
            &mut buffer[start_ky * full_width..(start_ky + 1) * full_width],
            row_kernel,
        );

        // As we always in advance on half of vertical kernel so half_kernel = R0
        // this is real image start
        if y >= half_kernel {
            let mut brows = vec![image; column_kernel_len];

            for (i, brow) in brows.iter_mut().enumerate() {
                let ky = (i + start_ky + 1) % column_kernel_len;
                *brow = &buffer[ky * full_width..(ky + 1) * full_width];
            }

            let dy = y - half_kernel;

            let dst = &mut destination[dy * full_width..(dy + 1) * full_width];

            filter_symmetric_column::<T, I, N>(&brows, dst, image_size, column_kernel);
        }

        start_ky += 1;
        start_ky %= column_kernel_len;
    }

    Ok(())
}

/// Performs 2D separable convolution on the image
///
/// Currently implemented only for symmetrical filters
///
/// # Arguments
///
/// * `image`: Single plane image
/// * `destination`: Destination image
/// * `image_size`: Image size see [FilterImageSize]
/// * `row_kernel`: Row kernel, *size must be odd*!
/// * `column_kernel`: Column kernel, *size must be odd*!
///
/// If both kernels after kernel scan appears to be an identity then just copy is performed.
fn filter_2d_separable<T, F, I, const N: usize>(
    image: &[T],
    destination: &mut [T],
    image_size: FilterImageSize,
    row_kernel: &[F],
    column_kernel: &[F],
) -> Result<(), ImageError>
where
    T: Copy + AsPrimitive<F> + Default + Send + Sync + KernelTransformer<F, I> + AsPrimitive<I>,
    F: Default + 'static + Copy,
    I: ToStorage<T>
        + Mul<I, Output = I>
        + Add<I, Output = I>
        + MulAdd<I, Output = I>
        + Send
        + Sync
        + PartialEq
        + Default
        + 'static
        + Copy,
    i32: AsPrimitive<F> + AsPrimitive<I>,
    f64: AsPrimitive<T>,
{
    if image.len() != image_size.width.safe_mul(image_size.height)?.safe_mul(N)? {
        return Err(ImageError::Limits(LimitError::from_kind(
            LimitErrorKind::DimensionError,
        )));
    }
    if destination.len() != image.len() {
        return Err(ImageError::Limits(LimitError::from_kind(
            LimitErrorKind::DimensionError,
        )));
    }

    assert_ne!(row_kernel.len() & 1, 0, "Row kernel length must be odd");
    assert_ne!(
        column_kernel.len() & 1,
        0,
        "Column kernel length must be odd"
    );

    let mut scanned_row_kernel = row_kernel
        .iter()
        .map(|&x| T::transform(x))
        .collect::<Vec<I>>();
    let mut scanned_column_kernel = column_kernel
        .iter()
        .map(|&x| T::transform(x))
        .collect::<Vec<I>>();

    scanned_row_kernel = prepare_symmetric_kernel(&scanned_row_kernel);
    scanned_column_kernel = prepare_symmetric_kernel(&scanned_column_kernel);

    if scanned_row_kernel.is_empty() && scanned_column_kernel.is_empty() {
        destination.copy_from_slice(image);
        return Ok(());
    }

    if column_kernel.len() < RING_QUEUE_CIRCULAR_CUTOFF {
        return filter_2d_separable_ring_queue::<T, I, N>(
            image,
            destination,
            image_size,
            &scanned_row_kernel,
            &scanned_column_kernel,
        );
    }

    let pad_w = (scanned_row_kernel.len() / 2).max(1);

    let arena_width = image_size.width * N + pad_w * 2 * N;
    let mut row_buffer = vec![T::default(); arena_width];

    let mut transient_image = vec![T::default(); image_size.width * image_size.height * N];

    for (y, dst) in transient_image
        .chunks_exact_mut(image_size.width * N)
        .enumerate()
    {
        // This is important to perform padding before convolution.
        // That approach allows to iterate without unnecessary branching
        // and highly effective from low sized kernels to reasonable huge kernels.
        // It is especially more effective for low sized kernels than copying
        // specifically for big sized images.
        // If image row is `asdfgh` then this method with clamp will produce row
        // `aaa asdfgh hhh` padded exactly on half kernel size
        make_arena_row::<T, N>(
            image,
            &mut row_buffer,
            y,
            image_size,
            KernelShape {
                width: scanned_row_kernel.len(),
                height: 0,
            },
        )?;

        filter_symmetric_row::<T, I, N>(&row_buffer, dst, &scanned_row_kernel);
    }

    let column_kernel_shape = KernelShape {
        width: 0,
        height: scanned_column_kernel.len(),
    };

    // This is important to perform padding before convolution.
    // That approach allows to iterate without unnecessary branching
    // and highly effective from low sized kernels to reasonable huge kernels.
    // It is especially more effective for low sized kernels than copying
    // specifically for big sized images.
    // `This is virtual padding!` that means it produces two non-contiguous arrays.
    // They will virtually replace image row, when convolution kernel goes out of bounds on Y coordinate.
    let column_arena_k =
        make_columns_arenas::<T, N>(transient_image.as_slice(), image_size, column_kernel_shape);

    let top_pad = column_arena_k.top_pad.as_slice();
    let bottom_pad = column_arena_k.bottom_pad.as_slice();

    let pad_h = column_kernel_shape.height / 2;

    let transient_image_slice = transient_image.as_slice();

    let src_stride = image_size.width * N;

    for (y, dst) in destination
        .chunks_exact_mut(image_size.width * N)
        .enumerate()
    {
        let mut brows: Vec<&[T]> = vec![&transient_image_slice[0..]; column_kernel_shape.height];

        for (k, row) in (0..column_kernel_shape.height).zip(brows.iter_mut()) {
            if (y as i64 - pad_h as i64 + k as i64) < 0 {
                *row = &top_pad[(pad_h - k - 1) * src_stride..];
            } else if (y as i64 - pad_h as i64 + k as i64) as usize >= image_size.height {
                *row = &bottom_pad[(k - pad_h - 1) * src_stride..];
            } else {
                let fy = (y as i64 + k as i64 - pad_h as i64) as usize;
                let start_offset = src_stride * fy;
                *row = &transient_image_slice[start_offset..(start_offset + src_stride)];
            }
        }

        let brows_slice = brows.as_slice();

        filter_symmetric_column::<T, I, N>(brows_slice, dst, image_size, &scanned_column_kernel);
    }

    Ok(())
}

pub(crate) fn filter_2d_sep_plane(
    image: &[u8],
    destination: &mut [u8],
    image_size: FilterImageSize,
    row_kernel: &[f32],
    column_kernel: &[f32],
) -> Result<(), ImageError> {
    filter_2d_separable::<u8, f32, u32, 1>(
        image,
        destination,
        image_size,
        row_kernel,
        column_kernel,
    )
}

pub(crate) fn filter_2d_sep_la(
    image: &[u8],
    destination: &mut [u8],
    image_size: FilterImageSize,
    row_kernel: &[f32],
    column_kernel: &[f32],
) -> Result<(), ImageError> {
    filter_2d_separable::<u8, f32, u32, 2>(
        image,
        destination,
        image_size,
        row_kernel,
        column_kernel,
    )
}

pub(crate) fn filter_2d_sep_rgb(
    image: &[u8],
    destination: &mut [u8],
    image_size: FilterImageSize,
    row_kernel: &[f32],
    column_kernel: &[f32],
) -> Result<(), ImageError> {
    filter_2d_separable::<u8, f32, u32, 3>(
        image,
        destination,
        image_size,
        row_kernel,
        column_kernel,
    )
}

pub(crate) fn filter_2d_sep_rgba(
    image: &[u8],
    destination: &mut [u8],
    image_size: FilterImageSize,
    row_kernel: &[f32],
    column_kernel: &[f32],
) -> Result<(), ImageError> {
    filter_2d_separable::<u8, f32, u32, 4>(
        image,
        destination,
        image_size,
        row_kernel,
        column_kernel,
    )
}

pub(crate) fn filter_2d_sep_la_f32(
    image: &[f32],
    destination: &mut [f32],
    image_size: FilterImageSize,
    row_kernel: &[f32],
    column_kernel: &[f32],
) -> Result<(), ImageError> {
    filter_2d_separable::<f32, f32, f32, 2>(
        image,
        destination,
        image_size,
        row_kernel,
        column_kernel,
    )
}

pub(crate) fn filter_2d_sep_plane_f32(
    image: &[f32],
    destination: &mut [f32],
    image_size: FilterImageSize,
    row_kernel: &[f32],
    column_kernel: &[f32],
) -> Result<(), ImageError> {
    filter_2d_separable::<f32, f32, f32, 1>(
        image,
        destination,
        image_size,
        row_kernel,
        column_kernel,
    )
}

pub(crate) fn filter_2d_sep_rgb_f32(
    image: &[f32],
    destination: &mut [f32],
    image_size: FilterImageSize,
    row_kernel: &[f32],
    column_kernel: &[f32],
) -> Result<(), ImageError> {
    filter_2d_separable::<f32, f32, f32, 3>(
        image,
        destination,
        image_size,
        row_kernel,
        column_kernel,
    )
}

pub(crate) fn filter_2d_sep_rgba_f32(
    image: &[f32],
    destination: &mut [f32],
    image_size: FilterImageSize,
    row_kernel: &[f32],
    column_kernel: &[f32],
) -> Result<(), ImageError> {
    filter_2d_separable::<f32, f32, f32, 4>(
        image,
        destination,
        image_size,
        row_kernel,
        column_kernel,
    )
}

pub(crate) fn filter_2d_sep_rgb_u16(
    image: &[u16],
    destination: &mut [u16],
    image_size: FilterImageSize,
    row_kernel: &[f32],
    column_kernel: &[f32],
) -> Result<(), ImageError> {
    filter_2d_separable::<u16, f32, u32, 3>(
        image,
        destination,
        image_size,
        row_kernel,
        column_kernel,
    )
}

pub(crate) fn filter_2d_sep_rgba_u16(
    image: &[u16],
    destination: &mut [u16],
    image_size: FilterImageSize,
    row_kernel: &[f32],
    column_kernel: &[f32],
) -> Result<(), ImageError> {
    filter_2d_separable::<u16, f32, u32, 4>(
        image,
        destination,
        image_size,
        row_kernel,
        column_kernel,
    )
}

pub(crate) fn filter_2d_sep_la_u16(
    image: &[u16],
    destination: &mut [u16],
    image_size: FilterImageSize,
    row_kernel: &[f32],
    column_kernel: &[f32],
) -> Result<(), ImageError> {
    filter_2d_separable::<u16, f32, u32, 2>(
        image,
        destination,
        image_size,
        row_kernel,
        column_kernel,
    )
}

pub(crate) fn filter_2d_sep_plane_u16(
    image: &[u16],
    destination: &mut [u16],
    image_size: FilterImageSize,
    row_kernel: &[f32],
    column_kernel: &[f32],
) -> Result<(), ImageError> {
    filter_2d_separable::<u16, f32, u32, 1>(
        image,
        destination,
        image_size,
        row_kernel,
        column_kernel,
    )
}
