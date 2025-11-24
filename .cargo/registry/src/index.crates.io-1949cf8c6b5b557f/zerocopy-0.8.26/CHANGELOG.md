<!-- Copyright 2023 The Fuchsia Authors

Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
<LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
This file may not be copied, modified, or distributed except according to
those terms. -->

# Changelog

## Releases

We track releases and release notes using [GitHub
Releases](https://github.com/google/zerocopy/releases).

## Yanks and Regressions

### 0.2.2 through 0.2.8, 0.3.0 through 0.3.1, 0.4.0, 0.5.0, 0.6.0 through 0.6.5, 0.7.0 through 0.7.30

*Security advisories for this bug have been published as
[RUSTSEC-2023-0074][rustsec-advisory] and [GHSA-3mv5-343c-w2qg][github-advisory].*

In these versions, the `Ref` methods `into_ref`, `into_mut`, `into_slice`, and
`into_mut_slice` were permitted in combination with the standard library
`cell::Ref` and `cell::RefMut` types for `Ref<B, T>`'s `B` type parameter. These
combinations are unsound, and may permit safe code to exhibit undefined
behavior. Fixes have been published to each affected minor version which do not
permit this code to compile.

See [#716][issue-716] for more details.

[rustsec-advisory]: https://rustsec.org/advisories/RUSTSEC-2023-0074.html
[github-advisory]: https://github.com/google/zerocopy/security/advisories/GHSA-3mv5-343c-w2qg
[issue-716]: https://github.com/google/zerocopy/issues/716

### 0.7.27, 0.7.28

These versions were briefly yanked due to a non-soundness regression reported in
[#672][pull-672]. After reconsidering our yanking policy in [#679][issue-679],
we un-yanked these versions.

[pull-672]: https://github.com/google/zerocopy/pull/672
[issue-679]: https://github.com/google/zerocopy/issues/679
