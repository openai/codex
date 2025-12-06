//! Conversions from colors in the `palette` crate to [`Color`].

use ::palette::LinSrgb;
use ::palette::bool_mask::LazySelect;
use ::palette::num::{Arithmetics, MulSub, PartialCmp, Powf, Real};
use palette::Srgb;
use palette::stimulus::IntoStimulus;

use crate::style::Color;

/// Convert an [`palette::Srgb`] color to a [`Color`].
///
/// # Examples
///
/// ```
/// use palette::Srgb;
/// use ratatui_core::style::Color;
///
/// let color = Color::from(Srgb::new(1.0f32, 0.0, 0.0));
/// assert_eq!(color, Color::Rgb(255, 0, 0));
/// ```
impl<T: IntoStimulus<u8>> From<Srgb<T>> for Color {
    fn from(color: Srgb<T>) -> Self {
        let (red, green, blue) = color.into_format().into_components();
        Self::Rgb(red, green, blue)
    }
}

/// Convert a [`palette::LinSrgb`] color to a [`Color`].
///
/// Note: this conversion only works for floating point linear sRGB colors. If you have a linear
/// sRGB color in another format, you need to convert it to floating point first.
///
/// # Examples
///
/// ```
/// use palette::LinSrgb;
/// use ratatui_core::style::Color;
///
/// let color = Color::from(LinSrgb::new(1.0f32, 0.0, 0.0));
/// assert_eq!(color, Color::Rgb(255, 0, 0));
/// ```
impl<T: IntoStimulus<u8>> From<LinSrgb<T>> for Color
where
    T: Real + Powf + MulSub + Arithmetics + PartialCmp + Clone,
    T::Mask: LazySelect<T>,
{
    fn from(color: LinSrgb<T>) -> Self {
        let srgb_color = Srgb::<T>::from_linear(color);
        Self::from(srgb_color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_srgb() {
        const RED: Color = Color::Rgb(255, 0, 0);
        assert_eq!(Color::from(Srgb::new(255u8, 0, 0)), RED);
        assert_eq!(Color::from(Srgb::new(65535u16, 0, 0)), RED);
        assert_eq!(Color::from(Srgb::new(1.0f32, 0.0, 0.0)), RED);

        assert_eq!(
            Color::from(Srgb::new(0.5f32, 0.5, 0.5)),
            Color::Rgb(128, 128, 128)
        );
    }

    #[test]
    fn from_lin_srgb() {
        const RED: Color = Color::Rgb(255, 0, 0);
        assert_eq!(Color::from(LinSrgb::new(1.0f32, 0.0, 0.0)), RED);
        assert_eq!(Color::from(LinSrgb::new(1.0f64, 0.0, 0.0)), RED);

        assert_eq!(
            Color::from(LinSrgb::new(0.5f32, 0.5, 0.5)),
            Color::Rgb(188, 188, 188)
        );
    }
}
