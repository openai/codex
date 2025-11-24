#[cfg(all(feature = "dfa-build", feature = "dfa-search"))]
mod api;
#[cfg(feature = "dfa-onepass")]
mod onepass;
#[cfg(all(feature = "dfa-build", feature = "dfa-search"))]
mod regression;
#[cfg(all(not(miri), feature = "dfa-build", feature = "dfa-search"))]
mod suite;
