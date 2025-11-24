// Copyright 2016 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
// OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

pub(crate) use self::features::Features;
use core::mem::size_of;

macro_rules! impl_get_feature {
    {
      features: [
          $( { ( $( $arch:expr ),+ ) => $Name:ident }, )+
      ],
    } => {
        $(
            #[cfg(any( $( target_arch = $arch ),+ ))]
            #[derive(Clone, Copy)]
            pub(crate) struct $Name(crate::cpu::Features);

            #[cfg(any( $( target_arch = $arch ),+ ))]
            impl $Name {
                const fn mask() -> u32 {
                    1 << (Shift::$Name as u32)
                }
            }

            #[cfg(any( $( target_arch = $arch ),+ ))]
            impl crate::cpu::GetFeature<$Name> for super::features::Values {
                #[inline(always)]
                fn get_feature(&self) -> Option<$Name> {
                    const MASK: u32 = $Name::mask();
                    const STATICALLY_DETECTED: bool = (crate::cpu::CAPS_STATIC & MASK) == MASK;
                    if STATICALLY_DETECTED { // TODO: `const`
                        return Some($Name(self.cpu()));
                    }

                    if (self.values() & MASK) == MASK {
                        Some($Name(self.cpu()))
                    } else {
                        None
                    }
                }
            }
        )+

        #[repr(u32)]
        enum Shift {
            $(
                #[cfg(any( $( target_arch = $arch ),+ ))]
                $Name,
            )+

            #[cfg(target_arch = "x86_64")]
            IntelCpu,

            #[cfg(any(all(target_arch = "aarch64", target_endian = "little"),
                     all(target_arch = "arm", target_endian = "little"),
                     target_arch = "x86", target_arch = "x86_64"))]
            // Synthesized to ensure the dynamic flag set is always non-zero.
            //
            // Keep this at the end as it is never checked except during init.
            Initialized,
        }
    }
}

pub(crate) trait GetFeature<T> {
    fn get_feature(&self) -> Option<T>;
}

impl GetFeature<()> for features::Values {
    #[inline(always)]
    fn get_feature(&self) -> Option<()> {
        Some(())
    }
}

impl<A, B> GetFeature<(A, B)> for features::Values
where
    features::Values: GetFeature<A>,
    features::Values: GetFeature<B>,
{
    #[inline(always)]
    fn get_feature(&self) -> Option<(A, B)> {
        match (self.get_feature(), self.get_feature()) {
            (Some(a), Some(b)) => Some((a, b)),
            _ => None,
        }
    }
}

impl<A, B, C> GetFeature<(A, B, C)> for features::Values
where
    features::Values: GetFeature<A>,
    features::Values: GetFeature<B>,
    features::Values: GetFeature<C>,
{
    #[inline(always)]
    fn get_feature(&self) -> Option<(A, B, C)> {
        match (self.get_feature(), self.get_feature(), self.get_feature()) {
            (Some(a), Some(b), Some(c)) => Some((a, b, c)),
            _ => None,
        }
    }
}

impl<F> GetFeature<F> for Features
where
    features::Values: GetFeature<F>,
{
    #[inline(always)]
    fn get_feature(&self) -> Option<F> {
        self.values().get_feature()
    }
}

#[inline(always)]
pub(crate) fn features() -> Features {
    featureflags::get_or_init()
}

mod features {
    use crate::polyfill::NotSend;

    /// A witness indicating that CPU features have been detected and cached.
    ///
    /// This is a zero-sized type so that it can be "stored" wherever convenient.
    #[derive(Copy, Clone)]
    pub(crate) struct Features(NotSend);

    impl Features {
        pub fn values(self) -> Values {
            Values {
                values: super::featureflags::get(self),
                cpu: self,
            }
        }
    }

    cfg_if::cfg_if! {
        if #[cfg(any(all(target_arch = "aarch64", target_endian = "little"), all(target_arch = "arm", target_endian = "little"),
                     target_arch = "x86", target_arch = "x86_64"))] {
            impl Features {
                // SAFETY: This must only be called after CPU features have been written
                // and synchronized.
                pub(super) unsafe fn new_after_feature_flags_written_and_synced_unchecked() -> Self {
                    Self(NotSend::VALUE)
                }
            }
        } else {
            impl Features {
                pub(super) fn new_no_features_to_detect() -> Self {
                    Self(NotSend::VALUE)
                }
            }
        }
    }

    pub struct Values {
        values: u32,
        cpu: Features,
    }

    impl Values {
        #[inline(always)]
        pub(super) fn values(&self) -> u32 {
            self.values
        }

        #[inline(always)]
        pub(super) fn cpu(&self) -> Features {
            self.cpu
        }
    }
}

const _: () = assert!(size_of::<Features>() == 0);

cfg_if::cfg_if! {
    if #[cfg(any(all(target_arch = "aarch64", target_endian = "little"), all(target_arch = "arm", target_endian = "little")))] {
        pub mod arm;
        use arm::featureflags;
    } else if #[cfg(any(target_arch = "x86", target_arch = "x86_64"))] {
        pub mod intel;
        use intel::featureflags;
    } else {
        mod featureflags {
            use super::Features;

            #[inline(always)]
            pub(super) fn get_or_init() -> Features {
                Features::new_no_features_to_detect()
            }

            #[inline(always)]
            pub(super) fn get(_cpu_features: Features) -> u32 {
                STATIC_DETECTED
            }

            pub(super) const STATIC_DETECTED: u32 = 0;
            pub(super) const FORCE_DYNAMIC_DETECTION: u32 = 0;
        }
    }
}

const CAPS_STATIC: u32 = featureflags::STATIC_DETECTED & !featureflags::FORCE_DYNAMIC_DETECTION;

#[allow(clippy::assertions_on_constants, clippy::bad_bit_mask)]
const _FORCE_DYNAMIC_DETECTION_HONORED: () =
    assert!((CAPS_STATIC & featureflags::FORCE_DYNAMIC_DETECTION) == 0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_is_subset_of_dynamic() {
        let cpu = features();
        let dynamic = featureflags::get(cpu);
        assert_eq!(dynamic & CAPS_STATIC, CAPS_STATIC);
    }
}
