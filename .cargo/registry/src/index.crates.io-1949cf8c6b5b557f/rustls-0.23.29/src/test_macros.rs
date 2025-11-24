//! Macros used for unit testing.

/// Instantiate the given test functions once for each built-in provider.
///
/// The selected provider module is bound as `provider`; you can rely on this
/// having the union of the items common to the `crypto::ring` and
/// `crypto::aws_lc_rs` modules.
#[cfg(test)]
macro_rules! test_for_each_provider {
    ($($tt:tt)+) => {
        #[cfg(feature = "ring")]
        mod test_with_ring {
            use crate::crypto::ring as provider;
            #[allow(unused_imports)]
            use super::*;
            $($tt)+
        }

        #[cfg(feature = "aws_lc_rs")]
        mod test_with_aws_lc_rs {
            use crate::crypto::aws_lc_rs as provider;
            #[allow(unused_imports)]
            use super::*;
            $($tt)+
        }
    };
}

/// Instantiate the given benchmark functions once for each built-in provider.
///
/// The selected provider module is bound as `provider`; you can rely on this
/// having the union of the items common to the `crypto::ring` and
/// `crypto::aws_lc_rs` modules.
#[cfg(bench)]
macro_rules! bench_for_each_provider {
    ($($tt:tt)+) => {
        #[cfg(feature = "ring")]
        mod bench_with_ring {
            use crate::crypto::ring as provider;
            #[allow(unused_imports)]
            use super::*;
            $($tt)+
        }

        #[cfg(feature = "aws_lc_rs")]
        mod bench_with_aws_lc_rs {
            use crate::crypto::aws_lc_rs as provider;
            #[allow(unused_imports)]
            use super::*;
            $($tt)+
        }
    };
}

#[cfg(test)]
#[macro_rules_attribute::apply(test_for_each_provider)]
mod tests {
    #[test]
    fn test_each_provider() {
        std::println!("provider is {:?}", super::provider::default_provider());
    }
}

#[cfg(all(test, bench))]
#[macro_rules_attribute::apply(bench_for_each_provider)]
mod benchmarks {
    #[bench]
    fn bench_each_provider(b: &mut test::Bencher) {
        b.iter(|| super::provider::default_provider());
    }
}
