use crate::{rgb::Rgb, Color};

/// Linear color gradient between two color stops
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gradient {
    /// Start Color of Gradient
    pub start: Rgb,

    /// End Color of Gradient
    pub end: Rgb,
}

impl Gradient {
    /// Creates a new [Gradient] with two [Rgb] colors, `start` and `end`
    #[inline]
    pub const fn new(start: Rgb, end: Rgb) -> Self {
        Self { start, end }
    }

    pub const fn from_color_rgb(start: Color, end: Color) -> Self {
        let start_grad = match start {
            Color::Rgb(r, g, b) => Rgb { r, g, b },
            _ => Rgb { r: 0, g: 0, b: 0 },
        };
        let end_grad = match end {
            Color::Rgb(r, g, b) => Rgb { r, g, b },
            _ => Rgb { r: 0, g: 0, b: 0 },
        };

        Self {
            start: start_grad,
            end: end_grad,
        }
    }

    /// Computes the [Rgb] color between `start` and `end` for `t`
    pub fn at(&self, t: f32) -> Rgb {
        self.start.lerp(self.end, t)
    }

    /// Returns the reverse of `self`
    #[inline]
    pub const fn reverse(&self) -> Self {
        Self::new(self.end, self.start)
    }

    pub fn build(&self, text: &str, target: TargetGround) -> String {
        let delta = 1.0 / text.len() as f32;
        let mut result = text.char_indices().fold(String::new(), |mut acc, (i, c)| {
            let temp = format!(
                "\x1B[{}m{}",
                self.at(i as f32 * delta).ansi_color_code(target),
                c
            );
            acc.push_str(&temp);
            acc
        });

        result.push_str("\x1B[0m");
        result
    }
}

pub fn build_all_gradient_text(text: &str, foreground: Gradient, background: Gradient) -> String {
    let delta = 1.0 / text.len() as f32;
    let mut result = text.char_indices().fold(String::new(), |mut acc, (i, c)| {
        let step = i as f32 * delta;
        let temp = format!(
            "\x1B[{};{}m{}",
            foreground
                .at(step)
                .ansi_color_code(TargetGround::Foreground),
            background
                .at(step)
                .ansi_color_code(TargetGround::Background),
            c
        );
        acc.push_str(&temp);
        acc
    });

    result.push_str("\x1B[0m");
    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetGround {
    Foreground,
    Background,
}

impl TargetGround {
    #[inline]
    pub const fn code(&self) -> u8 {
        match self {
            Self::Foreground => 30,
            Self::Background => 40,
        }
    }
}

pub trait ANSIColorCode {
    fn ansi_color_code(&self, target: TargetGround) -> String;
}
