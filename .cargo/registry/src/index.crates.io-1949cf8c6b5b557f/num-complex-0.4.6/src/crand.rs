//! Rand implementations for complex numbers

use crate::Complex;
use num_traits::Num;
use rand::distributions::Standard;
use rand::prelude::*;

impl<T> Distribution<Complex<T>> for Standard
where
    T: Num + Clone,
    Standard: Distribution<T>,
{
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Complex<T> {
        Complex::new(self.sample(rng), self.sample(rng))
    }
}

/// A generic random value distribution for complex numbers.
#[derive(Clone, Copy, Debug)]
pub struct ComplexDistribution<Re, Im = Re> {
    re: Re,
    im: Im,
}

impl<Re, Im> ComplexDistribution<Re, Im> {
    /// Creates a complex distribution from independent
    /// distributions of the real and imaginary parts.
    pub fn new(re: Re, im: Im) -> Self {
        ComplexDistribution { re, im }
    }
}

impl<T, Re, Im> Distribution<Complex<T>> for ComplexDistribution<Re, Im>
where
    T: Num + Clone,
    Re: Distribution<T>,
    Im: Distribution<T>,
{
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Complex<T> {
        Complex::new(self.re.sample(rng), self.im.sample(rng))
    }
}

#[cfg(test)]
fn test_rng() -> impl RngCore {
    /// Simple `Rng` for testing without additional dependencies
    struct XorShiftStar {
        a: u64,
    }

    impl RngCore for XorShiftStar {
        fn next_u32(&mut self) -> u32 {
            self.next_u64() as u32
        }

        fn next_u64(&mut self) -> u64 {
            // https://en.wikipedia.org/wiki/Xorshift#xorshift*
            self.a ^= self.a >> 12;
            self.a ^= self.a << 25;
            self.a ^= self.a >> 27;
            self.a.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }

        fn fill_bytes(&mut self, dest: &mut [u8]) {
            for chunk in dest.chunks_mut(8) {
                let bytes = self.next_u64().to_le_bytes();
                let slice = &bytes[..chunk.len()];
                chunk.copy_from_slice(slice)
            }
        }

        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
            self.fill_bytes(dest);
            Ok(())
        }
    }

    XorShiftStar {
        a: 0x0123_4567_89AB_CDEF,
    }
}

#[test]
fn standard_f64() {
    let mut rng = test_rng();
    for _ in 0..100 {
        let c: Complex<f64> = rng.gen();
        assert!(c.re >= 0.0 && c.re < 1.0);
        assert!(c.im >= 0.0 && c.im < 1.0);
    }
}

#[test]
fn generic_standard_f64() {
    let mut rng = test_rng();
    let dist = ComplexDistribution::new(Standard, Standard);
    for _ in 0..100 {
        let c: Complex<f64> = rng.sample(dist);
        assert!(c.re >= 0.0 && c.re < 1.0);
        assert!(c.im >= 0.0 && c.im < 1.0);
    }
}

#[test]
fn generic_uniform_f64() {
    use rand::distributions::Uniform;

    let mut rng = test_rng();
    let re = Uniform::new(-100.0, 0.0);
    let im = Uniform::new(0.0, 100.0);
    let dist = ComplexDistribution::new(re, im);
    for _ in 0..100 {
        // no type annotation required, since `Uniform` only produces one type.
        let c = rng.sample(dist);
        assert!(c.re >= -100.0 && c.re < 0.0);
        assert!(c.im >= 0.0 && c.im < 100.0);
    }
}

#[test]
fn generic_mixed_f64() {
    use rand::distributions::Uniform;

    let mut rng = test_rng();
    let re = Uniform::new(-100.0, 0.0);
    let dist = ComplexDistribution::new(re, Standard);
    for _ in 0..100 {
        // no type annotation required, since `Uniform` only produces one type.
        let c = rng.sample(dist);
        assert!(c.re >= -100.0 && c.re < 0.0);
        assert!(c.im >= 0.0 && c.im < 1.0);
    }
}

#[test]
fn generic_uniform_i32() {
    use rand::distributions::Uniform;

    let mut rng = test_rng();
    let re = Uniform::new(-100, 0);
    let im = Uniform::new(0, 100);
    let dist = ComplexDistribution::new(re, im);
    for _ in 0..100 {
        // no type annotation required, since `Uniform` only produces one type.
        let c = rng.sample(dist);
        assert!(c.re >= -100 && c.re < 0);
        assert!(c.im >= 0 && c.im < 100);
    }
}
