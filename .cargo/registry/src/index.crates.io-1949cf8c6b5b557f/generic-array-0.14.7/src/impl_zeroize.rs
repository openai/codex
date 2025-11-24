use crate::{ArrayLength, GenericArray};

use zeroize::Zeroize;

impl<T: Zeroize, N: ArrayLength<T>> Zeroize for GenericArray<T, N> {
    fn zeroize(&mut self) {
        self.as_mut_slice().iter_mut().zeroize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zeroize() {
        let mut array = GenericArray::<u8, typenum::U2>::default();
        array[0] = 4;
        array[1] = 9;
        array.zeroize();
        assert_eq!(array[0], 0);
        assert_eq!(array[1], 0);
    }
}
