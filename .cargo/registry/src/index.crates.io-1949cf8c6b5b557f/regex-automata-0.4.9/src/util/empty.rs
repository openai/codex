/*!
This module provides helper routines for dealing with zero-width matches.

The main problem being solved here is this:

1. The caller wants to search something that they know is valid UTF-8, such
as a Rust `&str`.
2. The regex used by the caller can match the empty string. For example, `a*`.
3. The caller should never get match offsets returned that occur within the
encoding of a UTF-8 codepoint. It is logically incorrect, and also means that,
e.g., slicing the `&str` at those offsets will lead to a panic.

So the question here is, how do we prevent the caller from getting match
offsets that split a codepoint? For example, strictly speaking, the regex `a*`
matches `☃` at the positions `[0, 0]`, `[1, 1]`, `[2, 2]` and `[3, 3]` since
the UTF-8 encoding of `☃` is `\xE2\x98\x83`. In particular, the `NFA` that
underlies all of the matching engines in this crate doesn't have anything in
its state graph that prevents matching between UTF-8 code units. Indeed, any
engine derived from the `NFA` will match at those positions by virtue of the
fact that the `NFA` is byte oriented. That is, its transitions are defined over
bytes and the matching engines work by proceeding one byte at a time.

(An alternative architecture would be to define the transitions in an `NFA`
over codepoints, or `char`. And then make the matching engines proceed by
decoding one codepoint at a time. This is a viable strategy, but it doesn't
work for DFA matching engines because designing a fast and memory efficient
transition table for an alphabet as large as Unicode is quite difficult. More
to the point, the top-level `regex` crate supports matching on arbitrary bytes
when Unicode mode is disabled and one is searching a `&[u8]`. So in that case,
you can't just limit yourself to decoding codepoints and matching those. You
really do need to be able to follow byte oriented transitions on the `NFA`.)

In an older version of the regex crate, we handled this case not in the regex
engine, but in the iterators over matches. Namely, since this case only arises
when the match is empty, we "just" incremented the next starting position
of the search by `N`, where `N` is the length of the codepoint encoded at
the current position. The alternative or more "natural" solution of just
incrementing by `1` would result in executing a search of `a*` on `☃` like
this:

* Start search at `0`.
* Found match at `[0, 0]`.
* Next start position is `0`.
* To avoid an infinite loop, since it's an empty match, increment by `1`.
* Start search at `1`.
* Found match at `[1, 1]`. Oops.

But if we instead incremented by `3` (the length in bytes of `☃`), then we get
the following:

* Start search at `0`.
* Found match at `[0, 0]`.
* Next start position is `0`.
* To avoid an infinite loop, since it's an empty match, increment by `3`.
* Start search at `3`.
* Found match at `[3, 3]`.

And we get the correct result. But does this technique work in all cases?
Crucially, it requires that a zero-width match that splits a codepoint never
occurs beyond the starting position of the search. Because if it did, merely
incrementing the start position by the number of bytes in the codepoint at
the current position wouldn't be enough. A zero-width match could just occur
anywhere. It turns out that it is _almost_ true. We can convince ourselves by
looking at all possible patterns that can match the empty string:

* Patterns like `a*`, `a{0}`, `(?:)`, `a|` and `|a` all unconditionally match
the empty string. That is, assuming there isn't an `a` at the current position,
they will all match the empty string at the start of a search. There is no way
to move past it because any other match would not be "leftmost."
* `^` only matches at the beginning of the haystack, where the start position
is `0`. Since we know we're searching valid UTF-8 (if it isn't valid UTF-8,
then this entire problem goes away because it implies your string type supports
invalid UTF-8 and thus must deal with offsets that not only split a codepoint
but occur in entirely invalid UTF-8 somehow), it follows that `^` never matches
between the code units of a codepoint because the start of a valid UTF-8 string
is never within the encoding of a codepoint.
* `$` basically the same logic as `^`, but for the end of a string. A valid
UTF-8 string can't have an incomplete codepoint at the end of it.
* `(?m:^)` follows similarly to `^`, but it can match immediately following
a `\n`. However, since a `\n` is always a codepoint itself and can never
appear within a codepoint, it follows that the position immediately following
a `\n` in a string that is valid UTF-8 is guaranteed to not be between the
code units of another codepoint. (One caveat here is that the line terminator
for multi-line anchors can now be changed to any arbitrary byte, including
things like `\x98` which might occur within a codepoint. However, this wasn't
supported by the old regex crate. If it was, it pose the same problems as
`(?-u:\B)`, as we'll discuss below.)
* `(?m:$)` a similar argument as for `(?m:^)`. The only difference is that a
`(?m:$)` matches just before a `\n`. But the same argument applies.
* `(?Rm:^)` and `(?Rm:$)` weren't supported by the old regex crate, but the
CRLF aware line anchors follow a similar argument as for `(?m:^)` and `(?m:$)`.
Namely, since they only ever match at a boundary where one side is either a
`\r` or a `\n`, neither of which can occur within a codepoint.
* `\b` only matches at positions where both sides are valid codepoints, so
this cannot split a codepoint.
* `\B`, like `\b`, also only matches at positions where both sides are valid
codepoints. So this cannot split a codepoint either.
* `(?-u:\b)` matches only at positions where at least one side of it is an ASCII
word byte. Since ASCII bytes cannot appear as code units in non-ASCII codepoints
(one of the many amazing qualities of UTF-8), it follows that this too cannot
split a codepoint.
* `(?-u:\B)` finally represents a problem. It can matches between *any* two
bytes that are either both word bytes or non-word bytes. Since code units like
`\xE2` and `\x98` (from the UTF-8 encoding of `☃`) are both non-word bytes,
`(?-u:\B)` will match at the position between them.

Thus, our approach of incrementing one codepoint at a time after seeing an
empty match is flawed because `(?-u:\B)` can result in an empty match that
splits a codepoint at a position past the starting point of a search. For
example, searching `(?-u:\B)` on `a☃` would produce the following matches: `[2,
2]`, `[3, 3]` and `[4, 4]`. The positions at `0` and `1` don't match because
they correspond to word boundaries since `a` is an ASCII word byte.

So what did the old regex crate do to avoid this? It banned `(?-u:\B)` from
regexes that could match `&str`. That might sound extreme, but a lot of other
things were banned too. For example, all of `(?-u:.)`, `(?-u:[^a])` and
`(?-u:\W)` can match invalid UTF-8 too, including individual code units with a
codepoint. The key difference is that those expressions could never produce an
empty match. That ban happens when translating an `Ast` to an `Hir`, because
that process that reason about whether an `Hir` can produce *non-empty* matches
at invalid UTF-8 boundaries. Bottom line though is that we side-stepped the
`(?-u:\B)` issue by banning it.

If banning `(?-u:\B)` were the only issue with the old regex crate's approach,
then I probably would have kept it. `\B` is rarely used, so it's not such a big
deal to have to work-around it. However, the problem with the above approach
is that it doesn't compose. The logic for avoiding splitting a codepoint only
lived in the iterator, which means if anyone wants to implement their own
iterator over regex matches, they have to deal with this extremely subtle edge
case to get full correctness.

Instead, in this crate, we take the approach of pushing this complexity down
to the lowest layers of each regex engine. The approach is pretty simple:

* If this corner case doesn't apply, don't do anything. (For example, if UTF-8
mode isn't enabled or if the regex cannot match the empty string.)
* If an empty match is reported, explicitly check if it splits a codepoint.
* If it doesn't, we're done, return the match.
* If it does, then ignore the match and re-run the search.
* Repeat the above process until the end of the haystack is reached or a match
is found that doesn't split a codepoint or isn't zero width.

And that's pretty much what this module provides. Every regex engine uses these
methods in their lowest level public APIs, but just above the layer where
their internal engine is used. That way, all regex engines can be arbitrarily
composed without worrying about handling this case, and iterators don't need to
handle it explicitly.

(It turns out that a new feature I added, support for changing the line
terminator in a regex to any arbitrary byte, also provokes the above problem.
Namely, the byte could be invalid UTF-8 or a UTF-8 continuation byte. So that
support would need to be limited or banned when UTF-8 mode is enabled, just
like we did for `(?-u:\B)`. But thankfully our more robust approach in this
crate handles that case just fine too.)
*/

use crate::util::search::{Input, MatchError};

#[cold]
#[inline(never)]
pub(crate) fn skip_splits_fwd<T, F>(
    input: &Input<'_>,
    init_value: T,
    match_offset: usize,
    find: F,
) -> Result<Option<T>, MatchError>
where
    F: FnMut(&Input<'_>) -> Result<Option<(T, usize)>, MatchError>,
{
    skip_splits(true, input, init_value, match_offset, find)
}

#[cold]
#[inline(never)]
pub(crate) fn skip_splits_rev<T, F>(
    input: &Input<'_>,
    init_value: T,
    match_offset: usize,
    find: F,
) -> Result<Option<T>, MatchError>
where
    F: FnMut(&Input<'_>) -> Result<Option<(T, usize)>, MatchError>,
{
    skip_splits(false, input, init_value, match_offset, find)
}

fn skip_splits<T, F>(
    forward: bool,
    input: &Input<'_>,
    init_value: T,
    mut match_offset: usize,
    mut find: F,
) -> Result<Option<T>, MatchError>
where
    F: FnMut(&Input<'_>) -> Result<Option<(T, usize)>, MatchError>,
{
    // If our config says to do an anchored search, then we're definitely
    // done. We just need to determine whether we have a valid match or
    // not. If we don't, then we're not allowed to continue, so we report
    // no match.
    //
    // This is actually quite a subtle correctness thing. The key here is
    // that if we got an empty match that splits a codepoint after doing an
    // anchored search in UTF-8 mode, then that implies that we must have
    // *started* the search at a location that splits a codepoint. This
    // follows from the fact that if a match is reported from an anchored
    // search, then the start offset of the match *must* match the start
    // offset of the search.
    //
    // It also follows that no other non-empty match is possible. For
    // example, you might write a regex like '(?:)|SOMETHING' and start its
    // search in the middle of a codepoint. The first branch is an empty
    // regex that will bubble up a match at the first position, and then
    // get rejected here and report no match. But what if 'SOMETHING' could
    // have matched? We reason that such a thing is impossible, because
    // if it does, it must report a match that starts in the middle of a
    // codepoint. This in turn implies that a match is reported whose span
    // does not correspond to valid UTF-8, and this breaks the promise
    // made when UTF-8 mode is enabled. (That promise *can* be broken, for
    // example, by enabling UTF-8 mode but building an by hand NFA that
    // produces non-empty matches that span invalid UTF-8. This is an unchecked
    // but documented precondition violation of UTF-8 mode, and is documented
    // to have unspecified behavior.)
    //
    // I believe this actually means that if an anchored search is run, and
    // UTF-8 mode is enabled and the start position splits a codepoint,
    // then it is correct to immediately report no match without even
    // executing the regex engine. But it doesn't really seem worth writing
    // out that case in every regex engine to save a tiny bit of work in an
    // extremely pathological case, so we just handle it here.
    if input.get_anchored().is_anchored() {
        return Ok(if input.is_char_boundary(match_offset) {
            Some(init_value)
        } else {
            None
        });
    }
    // Otherwise, we have an unanchored search, so just keep looking for
    // matches until we have one that does not split a codepoint or we hit
    // EOI.
    let mut value = init_value;
    let mut input = input.clone();
    while !input.is_char_boundary(match_offset) {
        if forward {
            // The unwrap is OK here because overflowing usize while
            // iterating over a slice is impossible, at it would require
            // a slice of length greater than isize::MAX, which is itself
            // impossible.
            input.set_start(input.start().checked_add(1).unwrap());
        } else {
            input.set_end(match input.end().checked_sub(1) {
                None => return Ok(None),
                Some(end) => end,
            });
        }
        match find(&input)? {
            None => return Ok(None),
            Some((new_value, new_match_end)) => {
                value = new_value;
                match_offset = new_match_end;
            }
        }
    }
    Ok(Some(value))
}
