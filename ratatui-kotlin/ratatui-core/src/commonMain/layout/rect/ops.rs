use core::ops::{Add, AddAssign, Neg, Sub, SubAssign};

use super::{Offset, Rect};

impl Neg for Offset {
    type Output = Self;

    /// Negates the offset.
    ///
    /// # Panics
    ///
    /// Panics if the negated value overflows (i.e. `x` or `y` is `i32::MIN`).
    fn neg(self) -> Self {
        Self {
            x: self.x.neg(),
            y: self.y.neg(),
        }
    }
}

impl Add<Offset> for Rect {
    type Output = Self;

    /// Moves the rect by an offset without changing its size.
    ///
    /// If the offset would move the any of the rect's edges outside the bounds of `u16`, the
    /// rect's position is clamped to the nearest edge.
    fn add(self, offset: Offset) -> Self {
        let max_x = i32::from(u16::MAX - self.width);
        let max_y = i32::from(u16::MAX - self.height);
        let x = i32::from(self.x).saturating_add(offset.x).clamp(0, max_x) as u16;
        let y = i32::from(self.y).saturating_add(offset.y).clamp(0, max_y) as u16;
        Self { x, y, ..self }
    }
}

impl Add<Rect> for Offset {
    type Output = Rect;

    /// Moves the rect by an offset without changing its size.
    ///
    /// If the offset would move the any of the rect's edges outside the bounds of `u16`, the
    /// rect's position is clamped to the nearest edge.
    fn add(self, rect: Rect) -> Rect {
        rect + self
    }
}

impl Sub<Offset> for Rect {
    type Output = Self;

    /// Subtracts an offset from the rect without changing its size.
    ///
    /// If the offset would move the any of the rect's edges outside the bounds of `u16`, the
    /// rect's position is clamped to the nearest
    fn sub(self, offset: Offset) -> Self {
        // Note this cannot be simplified to `self + -offset` because `Offset::MIN` would overflow
        let max_x = i32::from(u16::MAX - self.width);
        let max_y = i32::from(u16::MAX - self.height);
        let x = i32::from(self.x).saturating_sub(offset.x).clamp(0, max_x) as u16;
        let y = i32::from(self.y).saturating_sub(offset.y).clamp(0, max_y) as u16;
        Self { x, y, ..self }
    }
}

impl AddAssign<Offset> for Rect {
    /// Moves the rect by an offset in place without changing its size.
    ///
    /// If the offset would move the any of the rect's edges outside the bounds of `u16`, the
    /// rect's position is clamped to the nearest edge.
    fn add_assign(&mut self, offset: Offset) {
        *self = *self + offset;
    }
}

impl SubAssign<Offset> for Rect {
    /// Moves the rect by an offset in place without changing its size.
    ///
    /// If the offset would move the any of the rect's edges outside the bounds of `u16`, the
    /// rect's position is clamped to the nearest edge.
    fn sub_assign(&mut self, offset: Offset) {
        *self = *self - offset;
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::zero(Rect::new(3, 4, 5, 6), Offset::ZERO, Rect::new(3, 4, 5, 6))]
    #[case::positive(Rect::new(3, 4, 5, 6), Offset::new(1, 2), Rect::new(4, 6, 5, 6))]
    #[case::negative(Rect::new(3, 4, 5, 6), Offset::new(-1, -2), Rect::new(2, 2, 5, 6))]
    #[case::saturate_negative(Rect::new(3, 4, 5, 6), Offset::MIN, Rect::new(0, 0, 5, 6))]
    #[case::saturate_positive(Rect::new(3, 4, 5, 6), Offset::MAX, Rect::new(u16::MAX- 5, u16::MAX - 6, 5, 6))]
    fn add_offset(#[case] rect: Rect, #[case] offset: Offset, #[case] expected: Rect) {
        assert_eq!(rect + offset, expected);
        assert_eq!(offset + rect, expected);
    }

    #[rstest]
    #[case::zero(Rect::new(3, 4, 5, 6), Offset::ZERO, Rect::new(3, 4, 5, 6))]
    #[case::positive(Rect::new(3, 4, 5, 6), Offset::new(1, 2), Rect::new(2, 2, 5, 6))]
    #[case::negative(Rect::new(3, 4, 5, 6), Offset::new(-1, -2), Rect::new(4, 6, 5, 6))]
    #[case::saturate_negative(Rect::new(3, 4, 5, 6), Offset::MAX, Rect::new(0, 0, 5, 6))]
    #[case::saturate_positive(Rect::new(3, 4, 5, 6), -Offset::MAX, Rect::new(u16::MAX - 5, u16::MAX - 6, 5, 6))]
    fn sub_offset(#[case] rect: Rect, #[case] offset: Offset, #[case] expected: Rect) {
        assert_eq!(rect - offset, expected);
    }

    #[rstest]
    #[case::zero(Rect::new(3, 4, 5, 6), Offset::ZERO, Rect::new(3, 4, 5, 6))]
    #[case::positive(Rect::new(3, 4, 5, 6), Offset::new(1, 2), Rect::new(4, 6, 5, 6))]
    #[case::negative(Rect::new(3, 4, 5, 6), Offset::new(-1, -2), Rect::new(2, 2, 5, 6))]
    #[case::saturate_negative(Rect::new(3, 4, 5, 6), Offset::MIN, Rect::new(0, 0, 5, 6))]
    #[case::saturate_positive(Rect::new(3, 4, 5, 6), Offset::MAX, Rect::new(u16::MAX - 5, u16::MAX - 6, 5, 6))]
    fn add_assign_offset(#[case] rect: Rect, #[case] offset: Offset, #[case] expected: Rect) {
        let mut rect = rect;
        rect += offset;
        assert_eq!(rect, expected);
    }

    #[rstest]
    #[case::zero(Rect::new(3, 4, 5, 6), Offset::ZERO, Rect::new(3, 4, 5, 6))]
    #[case::positive(Rect::new(3, 4, 5, 6), Offset::new(1, 2), Rect::new(2, 2, 5, 6))]
    #[case::negative(Rect::new(3, 4, 5, 6), Offset::new(-1, -2), Rect::new(4, 6, 5, 6))]
    #[case::saturate_negative(Rect::new(3, 4, 5, 6), Offset::MAX, Rect::new(0, 0, 5, 6))]
    #[case::saturate_positive(Rect::new(3, 4, 5, 6), -Offset::MAX, Rect::new(u16::MAX - 5, u16::MAX - 6, 5, 6))]
    fn sub_assign_offset(#[case] rect: Rect, #[case] offset: Offset, #[case] expected: Rect) {
        let mut rect = rect;
        rect -= offset;
        assert_eq!(rect, expected);
    }
}
