use std::ops::{BitOr, BitOrAssign};

use crate::Style;

impl<T: Into<Set<T>>> BitOr<T> for Set<T> {
    type Output = Self;

    fn bitor(self, rhs: T) -> Self::Output {
        Set(PhantomData, self.1 | rhs.into().1)
    }
}

impl<T: Into<Set<T>>> BitOr<Set<T>> for Set<T> {
    type Output = Self;

    fn bitor(self, rhs: Set<T>) -> Self::Output {
        Set(PhantomData, self.1 | rhs.1)
    }
}

impl<T: Into<Set<T>>> BitOrAssign<T> for Set<T> {
    fn bitor_assign(&mut self, rhs: T) {
        self.1 |= rhs.into().1;
    }
}

impl BitOr for Emphasis {
    type Output = Style;

    fn bitor(self, rhs: Self) -> Style {
        let attribute = Set::from(self) | Set::from(rhs);
        Style { attribute, ..Default::default() }
    }
}

impl BitOr<Style> for Emphasis {
    type Output = Style;

    fn bitor(self, mut rhs: Style) -> Style {
        rhs.attribute |= self;
        rhs
    }
}

impl BitOr<Emphasis> for Style {
    type Output = Style;

    fn bitor(self, rhs: Emphasis) -> Style {
        rhs | self
    }
}

impl BitOr<Emphasis> for Color {
    type Output = Style;

    fn bitor(self, rhs: Emphasis) -> Self::Output {
        Style::from(self) | rhs
    }
}

impl BitOr<Color> for Emphasis {
    type Output = Style;

    fn bitor(self, rhs: Color) -> Self::Output {
        Style::from(rhs) | self
    }
}
