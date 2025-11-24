# mime_guess ![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/abonander/mime_guess/rust.yml?branch=master) [![Crates.io](https://img.shields.io/crates/v/mime_guess.svg)](https://crates.io/crates/mime_guess)

MIME/MediaType guessing by file extension. 
Uses a static map of known file extension -> MIME type mappings.

**Returning Contributors: New Requirements for Submissions Below**

##### Required Rust Version: 1.33

#### [Documentation](https://docs.rs/mime_guess/)

### Versioning

Due to a mistaken premature release, `mime_guess` currently publicly depends on a pre-1.0 `mime`,
which means `mime` upgrades are breaking changes and necessitate a major version bump. 
Refer to the following table to find a version of `mime_guess` which matches your version of `mime`:

| `mime` version | `mime_guess` version |
|----------------|----------------------|
| `0.1.x, 0.2.x` | `1.x.y` |
| `0.3.x`        | `2.x.y` |

#### Note: MIME Types Returned Are Not Stable/Guaranteed
The media types returned for a given extension are not considered to be part of the crate's
 stable API and are often updated in patch (`x.y.z + 1`) releases to be as correct as possible. MIME
 changes are backported to previous major releases on a best-effort basis.
 
Note that only the extensions of paths/filenames are inspected in order to guess the MIME type. The
file that may or may not reside at that path may or may not be a valid file of the returned MIME type.
Be wary of unsafe or un-validated assumptions about file structure or length.

An extension may also have multiple applicable MIME types. When more than one is returned, the first
is considered to be the most "correct"--see below for elaboration.

Contributing
-----------

#### Adding or correcting MIME types for extensions

Is the MIME type for a file extension wrong or missing? Great! 
Well, not great for us, but great for you if you'd like to open a pull request! 

The file extension -> MIME type mappings are listed in `src/mime_types.rs`. 
**The list is sorted lexicographically by file extension, and all extensions are lowercase (where applicable).** 
The former is necessary to support fallback to binary search when the 
`phf-map` feature is turned off, and for the maintainers' sanity.
The latter is only for consistency's sake; the search is case-insensitive.

Simply add or update the appropriate string pair(s) to make the correction(s) needed. 
Run `cargo test` to make sure the library continues to work correctly.

#### Important! Citing the corrected MIME type 

When opening a pull request, please include a link to an official document or RFC noting 
the correct MIME type for the file type in question **in the commit message** so
that the commit history can be used as an audit trail.

Though we're only guessing here, we like to be as correct as we can. 
It makes it much easier to vet your contribution if we don't have to search for corroborating material.

#### Multiple MIME types per extension
As of `2.0.0`, multiple MIME types per extension are supported. The first MIME type in the list for 
a given extension should be the most "correct" so users who only care about getting a single MIME 
type can use the `first*()` methods.

The definition of "correct" is open to debate, however. In the author's opinion this should be 
whatever is defined by the latest IETF RFC for the given file format, or otherwise explicitly 
supercedes all others.

If an official IANA registration replaces an older "experimental" style media type, please
place the new type before the old type in the list, but keep the old type for reference:

```
- ("md", &["text/x-markdown"]),
+ ("md", &["text/markdown", "text/x-markdown"]),
```

#### Changes to the API or operation of the crate

We're open to changes to the crate's API or its inner workings, breaking or not, if it improves the overall operation, efficiency, or ergonomics of the crate. However, it would be a good idea to open an issue on the repository so we can discuss your proposed changes and decide how best to approach them.


License
-------

MIT (See the `LICENSE` file in this repository for more information.)
