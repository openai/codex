# icu_locale [![crates.io](https://img.shields.io/crates/v/icu_locale)](https://crates.io/crates/icu_locale)

<!-- cargo-rdme start -->

Canonicalization of locale identifiers based on [`CLDR`] data.

This module is published as its own crate ([`icu_locale`](https://docs.rs/icu_locale/latest/icu_locale/))
and as part of the [`icu`](https://docs.rs/icu/latest/icu/) crate. See the latter for more details on the ICU4X project.

It currently supports locale canonicalization based upon the canonicalization
algorithm from [`UTS #35: Unicode LDML 3. LocaleId Canonicalization`],
as well as the minimize and maximize likely subtags algorithms
as described in [`UTS #35: Unicode LDML 3. Likely Subtags`].

The maximize method potentially updates a passed in locale in place
depending up the results of running the 'Add Likely Subtags' algorithm
from [`UTS #35: Unicode LDML 3. Likely Subtags`].

This minimize method returns a new Locale that is the result of running the
'Remove Likely Subtags' algorithm from [`UTS #35: Unicode LDML 3. Likely Subtags`].

## Examples

```rust
use icu::locale::Locale;
use icu::locale::{LocaleCanonicalizer, TransformResult};

let lc = LocaleCanonicalizer::new_extended();

let mut locale: Locale = "ja-Latn-fonipa-hepburn-heploc"
    .parse()
    .expect("parse failed");
assert_eq!(lc.canonicalize(&mut locale), TransformResult::Modified);
assert_eq!(locale, "ja-Latn-alalc97-fonipa".parse::<Locale>().unwrap());
```

```rust
use icu::locale::{locale, LocaleExpander, TransformResult};

let lc = LocaleExpander::new_common();

let mut locale = locale!("zh-CN");
assert_eq!(lc.maximize(&mut locale.id), TransformResult::Modified);
assert_eq!(locale, locale!("zh-Hans-CN"));

let mut locale = locale!("zh-Hant-TW");
assert_eq!(lc.maximize(&mut locale.id), TransformResult::Unmodified);
assert_eq!(locale, locale!("zh-Hant-TW"));
```

```rust
use icu::locale::{locale, LocaleExpander, TransformResult};
use writeable::assert_writeable_eq;

let lc = LocaleExpander::new_common();

let mut locale = locale!("zh-Hans-CN");
assert_eq!(lc.minimize(&mut locale.id), TransformResult::Modified);
assert_eq!(locale, locale!("zh"));

let mut locale = locale!("zh");
assert_eq!(lc.minimize(&mut locale.id), TransformResult::Unmodified);
assert_eq!(locale, locale!("zh"));
```

[`ICU4X`]: ../icu/index.html
[`CLDR`]: http://cldr.unicode.org/
[`UTS #35: Unicode LDML 3. Likely Subtags`]: https://www.unicode.org/reports/tr35/#Likely_Subtags.
[`UTS #35: Unicode LDML 3. LocaleId Canonicalization`]: http://unicode.org/reports/tr35/#LocaleId_Canonicalization,

<!-- cargo-rdme end -->

## More Information

For more information on development, authorship, contributing etc. please visit [`ICU4X home page`](https://github.com/unicode-org/icu4x).
