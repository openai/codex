use crate::{bf16, f16};

use rand::{distr::Distribution, Rng};
use rand_distr::uniform::UniformFloat;

macro_rules! impl_distribution_via_f32 {
    ($Ty:ty, $Distr:ty) => {
        impl Distribution<$Ty> for $Distr {
            fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> $Ty {
                <$Ty>::from_f32(<Self as Distribution<f32>>::sample(self, rng))
            }
        }
    };
}

impl_distribution_via_f32!(f16, rand_distr::StandardUniform);
impl_distribution_via_f32!(f16, rand_distr::StandardNormal);
impl_distribution_via_f32!(f16, rand_distr::Exp1);
impl_distribution_via_f32!(f16, rand_distr::Open01);
impl_distribution_via_f32!(f16, rand_distr::OpenClosed01);

impl_distribution_via_f32!(bf16, rand_distr::StandardUniform);
impl_distribution_via_f32!(bf16, rand_distr::StandardNormal);
impl_distribution_via_f32!(bf16, rand_distr::Exp1);
impl_distribution_via_f32!(bf16, rand_distr::Open01);
impl_distribution_via_f32!(bf16, rand_distr::OpenClosed01);

#[derive(Debug, Clone, Copy)]
pub struct Float16Sampler(UniformFloat<f32>);

impl rand_distr::uniform::SampleUniform for f16 {
    type Sampler = Float16Sampler;
}

impl rand_distr::uniform::UniformSampler for Float16Sampler {
    type X = f16;
    fn new<B1, B2>(low: B1, high: B2) -> Result<Self, rand_distr::uniform::Error>
    where
        B1: rand_distr::uniform::SampleBorrow<Self::X> + Sized,
        B2: rand_distr::uniform::SampleBorrow<Self::X> + Sized,
    {
        Ok(Self(UniformFloat::new(
            low.borrow().to_f32(),
            high.borrow().to_f32(),
        )?))
    }
    fn new_inclusive<B1, B2>(low: B1, high: B2) -> Result<Self, rand_distr::uniform::Error>
    where
        B1: rand_distr::uniform::SampleBorrow<Self::X> + Sized,
        B2: rand_distr::uniform::SampleBorrow<Self::X> + Sized,
    {
        Ok(Self(UniformFloat::new_inclusive(
            low.borrow().to_f32(),
            high.borrow().to_f32(),
        )?))
    }
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Self::X {
        f16::from_f32(self.0.sample(rng))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BFloat16Sampler(UniformFloat<f32>);

impl rand_distr::uniform::SampleUniform for bf16 {
    type Sampler = BFloat16Sampler;
}

impl rand_distr::uniform::UniformSampler for BFloat16Sampler {
    type X = bf16;
    fn new<B1, B2>(low: B1, high: B2) -> Result<Self, rand_distr::uniform::Error>
    where
        B1: rand_distr::uniform::SampleBorrow<Self::X> + Sized,
        B2: rand_distr::uniform::SampleBorrow<Self::X> + Sized,
    {
        Ok(Self(UniformFloat::new(
            low.borrow().to_f32(),
            high.borrow().to_f32(),
        )?))
    }
    fn new_inclusive<B1, B2>(low: B1, high: B2) -> Result<Self, rand_distr::uniform::Error>
    where
        B1: rand_distr::uniform::SampleBorrow<Self::X> + Sized,
        B2: rand_distr::uniform::SampleBorrow<Self::X> + Sized,
    {
        Ok(Self(UniformFloat::new_inclusive(
            low.borrow().to_f32(),
            high.borrow().to_f32(),
        )?))
    }
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Self::X {
        bf16::from_f32(self.0.sample(rng))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(unused_imports)]
    use rand::{rng, Rng};
    use rand_distr::{StandardNormal, StandardUniform, Uniform};

    #[test]
    fn test_sample_f16() {
        let mut rng = rng();
        let _: f16 = rng.sample(StandardUniform);
        let _: f16 = rng.sample(StandardNormal);
        let _: f16 = rng.sample(Uniform::new(f16::from_f32(0.0), f16::from_f32(1.0)).unwrap());
        #[cfg(feature = "num-traits")]
        let _: f16 =
            rng.sample(rand_distr::Normal::new(f16::from_f32(0.0), f16::from_f32(1.0)).unwrap());
    }

    #[test]
    fn test_sample_bf16() {
        let mut rng = rng();
        let _: bf16 = rng.sample(StandardUniform);
        let _: bf16 = rng.sample(StandardNormal);
        let _: bf16 = rng.sample(Uniform::new(bf16::from_f32(0.0), bf16::from_f32(1.0)).unwrap());
        #[cfg(feature = "num-traits")]
        let _: bf16 =
            rng.sample(rand_distr::Normal::new(bf16::from_f32(0.0), bf16::from_f32(1.0)).unwrap());
    }
}
