/*!
A module dedicated to plucking inner literals out of a regex pattern, and
then constructing a prefilter for them. We also include a regex pattern
"prefix" that corresponds to the bits of the regex that need to match before
the literals do. The reverse inner optimization then proceeds by looking for
matches of the inner literal(s), and then doing a reverse search of the prefix
from the start of the literal match to find the overall start position of the
match.

The essential invariant we want to uphold here is that the literals we return
reflect a set where *at least* one of them must match in order for the overall
regex to match. We also need to maintain the invariant that the regex prefix
returned corresponds to the entirety of the regex up until the literals we
return.

This somewhat limits what we can do. That is, if we a regex like
`\w+(@!|%%)\w+`, then we can pluck the `{@!, %%}` out and build a prefilter
from it. Then we just need to compile `\w+` in reverse. No fuss no muss. But if
we have a regex like \d+@!|\w+%%`, then we get kind of stymied. Technically,
we could still extract `{@!, %%}`, and it is true that at least of them must
match. But then, what is our regex prefix? Again, in theory, that could be
`\d+|\w+`, but that's not quite right, because the `\d+` only matches when `@!`
matches, and `\w+` only matches when `%%` matches.

All of that is technically possible to do, but it seemingly requires a lot of
sophistication and machinery. Probably the way to tackle that is with some kind
of formalism and approach this problem more generally.

For now, the code below basically just looks for a top-level concatenation.
And if it can find one, it looks for literals in each of the direct child
sub-expressions of that concatenation. If some good ones are found, we return
those and a concatenation of the Hir expressions seen up to that point.
*/

use alloc::vec::Vec;

use regex_syntax::hir::{self, literal, Hir, HirKind};

use crate::{util::prefilter::Prefilter, MatchKind};

/// Attempts to extract an "inner" prefilter from the given HIR expressions. If
/// one was found, then a concatenation of the HIR expressions that precede it
/// is returned.
///
/// The idea here is that the prefilter returned can be used to find candidate
/// matches. And then the HIR returned can be used to build a reverse regex
/// matcher, which will find the start of the candidate match. Finally, the
/// match still has to be confirmed with a normal anchored forward scan to find
/// the end position of the match.
///
/// Note that this assumes leftmost-first match semantics, so callers must
/// not call this otherwise.
pub(crate) fn extract(hirs: &[&Hir]) -> Option<(Hir, Prefilter)> {
    if hirs.len() != 1 {
        debug!(
            "skipping reverse inner optimization since it only \
		 	 supports 1 pattern, {} were given",
            hirs.len(),
        );
        return None;
    }
    let mut concat = match top_concat(hirs[0]) {
        Some(concat) => concat,
        None => {
            debug!(
                "skipping reverse inner optimization because a top-level \
		 	     concatenation could not found",
            );
            return None;
        }
    };
    // We skip the first HIR because if it did have a prefix prefilter in it,
    // we probably wouldn't be here looking for an inner prefilter.
    for i in 1..concat.len() {
        let hir = &concat[i];
        let pre = match prefilter(hir) {
            None => continue,
            Some(pre) => pre,
        };
        // Even if we got a prefilter, if it isn't consider "fast," then we
        // probably don't want to bother with it. Namely, since the reverse
        // inner optimization requires some overhead, it likely only makes
        // sense if the prefilter scan itself is (believed) to be much faster
        // than the regex engine.
        if !pre.is_fast() {
            debug!(
                "skipping extracted inner prefilter because \
				 it probably isn't fast"
            );
            continue;
        }
        let concat_suffix = Hir::concat(concat.split_off(i));
        let concat_prefix = Hir::concat(concat);
        // Look for a prefilter again. Why? Because above we only looked for
        // a prefilter on the individual 'hir', but we might be able to find
        // something better and more discriminatory by looking at the entire
        // suffix. We don't do this above to avoid making this loop worst case
        // quadratic in the length of 'concat'.
        let pre2 = match prefilter(&concat_suffix) {
            None => pre,
            Some(pre2) => {
                if pre2.is_fast() {
                    pre2
                } else {
                    pre
                }
            }
        };
        return Some((concat_prefix, pre2));
    }
    debug!(
        "skipping reverse inner optimization because a top-level \
	     sub-expression with a fast prefilter could not be found"
    );
    None
}

/// Attempt to extract a prefilter from an HIR expression.
///
/// We do a little massaging here to do our best that the prefilter we get out
/// of this is *probably* fast. Basically, the false positive rate has a much
/// higher impact for things like the reverse inner optimization because more
/// work needs to potentially be done for each candidate match.
///
/// Note that this assumes leftmost-first match semantics, so callers must
/// not call this otherwise.
fn prefilter(hir: &Hir) -> Option<Prefilter> {
    let mut extractor = literal::Extractor::new();
    extractor.kind(literal::ExtractKind::Prefix);
    let mut prefixes = extractor.extract(hir);
    debug!(
        "inner prefixes (len={:?}) extracted before optimization: {:?}",
        prefixes.len(),
        prefixes
    );
    // Since these are inner literals, we know they cannot be exact. But the
    // extractor doesn't know this. We mark them as inexact because this might
    // impact literal optimization. Namely, optimization weights "all literals
    // are exact" as very high, because it presumes that any match results in
    // an overall match. But of course, that is not the case here.
    //
    // In practice, this avoids plucking out a ASCII-only \s as an alternation
    // of single-byte whitespace characters.
    prefixes.make_inexact();
    prefixes.optimize_for_prefix_by_preference();
    debug!(
        "inner prefixes (len={:?}) extracted after optimization: {:?}",
        prefixes.len(),
        prefixes
    );
    prefixes
        .literals()
        .and_then(|lits| Prefilter::new(MatchKind::LeftmostFirst, lits))
}

/// Looks for a "top level" HirKind::Concat item in the given HIR. This will
/// try to return one even if it's embedded in a capturing group, but is
/// otherwise pretty conservative in what is returned.
///
/// The HIR returned is a complete copy of the concat with all capturing
/// groups removed. In effect, the concat returned is "flattened" with respect
/// to capturing groups. This makes the detection logic above for prefixes
/// a bit simpler, and it works because 1) capturing groups never influence
/// whether a match occurs or not and 2) capturing groups are not used when
/// doing the reverse inner search to find the start of the match.
fn top_concat(mut hir: &Hir) -> Option<Vec<Hir>> {
    loop {
        hir = match hir.kind() {
            HirKind::Empty
            | HirKind::Literal(_)
            | HirKind::Class(_)
            | HirKind::Look(_)
            | HirKind::Repetition(_)
            | HirKind::Alternation(_) => return None,
            HirKind::Capture(hir::Capture { ref sub, .. }) => sub,
            HirKind::Concat(ref subs) => {
                // We are careful to only do the flattening/copy when we know
                // we have a "top level" concat we can inspect. This avoids
                // doing extra work in cases where we definitely won't use it.
                // (This might still be wasted work if we can't go on to find
                // some literals to extract.)
                let concat =
                    Hir::concat(subs.iter().map(|h| flatten(h)).collect());
                return match concat.into_kind() {
                    HirKind::Concat(xs) => Some(xs),
                    // It is actually possible for this case to occur, because
                    // 'Hir::concat' might simplify the expression to the point
                    // that concatenations are actually removed. One wonders
                    // whether this leads to other cases where we should be
                    // extracting literals, but in theory, I believe if we do
                    // get here, then it means that a "real" prefilter failed
                    // to be extracted and we should probably leave well enough
                    // alone. (A "real" prefilter is unbothered by "top-level
                    // concats" and "capturing groups.")
                    _ => return None,
                };
            }
        };
    }
}

/// Returns a copy of the given HIR but with all capturing groups removed.
fn flatten(hir: &Hir) -> Hir {
    match hir.kind() {
        HirKind::Empty => Hir::empty(),
        HirKind::Literal(hir::Literal(ref x)) => Hir::literal(x.clone()),
        HirKind::Class(ref x) => Hir::class(x.clone()),
        HirKind::Look(ref x) => Hir::look(x.clone()),
        HirKind::Repetition(ref x) => Hir::repetition(x.with(flatten(&x.sub))),
        // This is the interesting case. We just drop the group information
        // entirely and use the child HIR itself.
        HirKind::Capture(hir::Capture { ref sub, .. }) => flatten(sub),
        HirKind::Alternation(ref xs) => {
            Hir::alternation(xs.iter().map(|x| flatten(x)).collect())
        }
        HirKind::Concat(ref xs) => {
            Hir::concat(xs.iter().map(|x| flatten(x)).collect())
        }
    }
}
