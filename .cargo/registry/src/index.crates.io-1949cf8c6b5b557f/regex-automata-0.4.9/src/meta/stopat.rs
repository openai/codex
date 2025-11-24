/*!
This module defines two bespoke forward DFA search routines. One for the lazy
DFA and one for the fully compiled DFA. These routines differ from the normal
ones by reporting the position at which the search terminates when a match
*isn't* found.

This position at which a search terminates is useful in contexts where the meta
regex engine runs optimizations that could go quadratic if we aren't careful.
Namely, a regex search *could* scan to the end of the haystack only to report a
non-match. If the caller doesn't know that the search scanned to the end of the
haystack, it might restart the search at the next literal candidate it finds
and repeat the process.

Providing the caller with the position at which the search stopped provides a
way for the caller to determine the point at which subsequent scans should not
pass. This is principally used in the "reverse inner" optimization, which works
like this:

1. Look for a match of an inner literal. Say, 'Z' in '\w+Z\d+'.
2. At the spot where 'Z' matches, do a reverse anchored search from there for
'\w+'.
3. If the reverse search matches, it corresponds to the start position of a
(possible) match. At this point, do a forward anchored search to find the end
position. If an end position is found, then we have a match and we know its
bounds.

If the forward anchored search in (3) searches the entire rest of the haystack
but reports a non-match, then a naive implementation of the above will continue
back at step 1 looking for more candidates. There might still be a match to be
found! It's possible. But we already scanned the whole haystack. So if we keep
repeating the process, then we might wind up taking quadratic time in the size
of the haystack, which is not great.

So if the forward anchored search in (3) reports the position at which it
stops, then we can detect whether quadratic behavior might be occurring in
steps (1) and (2). For (1), it occurs if the literal candidate found occurs
*before* the end of the previous search in (3), since that means we're now
going to look for another match in a place where the forward search has already
scanned. It is *correct* to do so, but our technique has become inefficient.
For (2), quadratic behavior occurs similarly when its reverse search extends
past the point where the previous forward search in (3) terminated. Indeed, to
implement (2), we use the sibling 'limited' module for ensuring our reverse
scan doesn't go further than we want.

See the 'opt/reverse-inner' benchmarks in rebar for a real demonstration of
how quadratic behavior is mitigated.
*/

use crate::{meta::error::RetryFailError, HalfMatch, Input, MatchError};

#[cfg(feature = "dfa-build")]
pub(crate) fn dfa_try_search_half_fwd(
    dfa: &crate::dfa::dense::DFA<alloc::vec::Vec<u32>>,
    input: &Input<'_>,
) -> Result<Result<HalfMatch, usize>, RetryFailError> {
    use crate::dfa::{accel, Automaton};

    let mut mat = None;
    let mut sid = dfa.start_state_forward(input)?;
    let mut at = input.start();
    while at < input.end() {
        sid = dfa.next_state(sid, input.haystack()[at]);
        if dfa.is_special_state(sid) {
            if dfa.is_match_state(sid) {
                let pattern = dfa.match_pattern(sid, 0);
                mat = Some(HalfMatch::new(pattern, at));
                if input.get_earliest() {
                    return Ok(mat.ok_or(at));
                }
                if dfa.is_accel_state(sid) {
                    let needs = dfa.accelerator(sid);
                    at = accel::find_fwd(needs, input.haystack(), at)
                        .unwrap_or(input.end());
                    continue;
                }
            } else if dfa.is_accel_state(sid) {
                let needs = dfa.accelerator(sid);
                at = accel::find_fwd(needs, input.haystack(), at)
                    .unwrap_or(input.end());
                continue;
            } else if dfa.is_dead_state(sid) {
                return Ok(mat.ok_or(at));
            } else if dfa.is_quit_state(sid) {
                return Err(MatchError::quit(input.haystack()[at], at).into());
            } else {
                // Ideally we wouldn't use a DFA that specialized start states
                // and thus 'is_start_state()' could never be true here, but in
                // practice we reuse the DFA created for the full regex which
                // will specialize start states whenever there is a prefilter.
                debug_assert!(dfa.is_start_state(sid));
            }
        }
        at += 1;
    }
    dfa_eoi_fwd(dfa, input, &mut sid, &mut mat)?;
    Ok(mat.ok_or(at))
}

#[cfg(feature = "hybrid")]
pub(crate) fn hybrid_try_search_half_fwd(
    dfa: &crate::hybrid::dfa::DFA,
    cache: &mut crate::hybrid::dfa::Cache,
    input: &Input<'_>,
) -> Result<Result<HalfMatch, usize>, RetryFailError> {
    let mut mat = None;
    let mut sid = dfa.start_state_forward(cache, input)?;
    let mut at = input.start();
    while at < input.end() {
        sid = dfa
            .next_state(cache, sid, input.haystack()[at])
            .map_err(|_| MatchError::gave_up(at))?;
        if sid.is_tagged() {
            if sid.is_match() {
                let pattern = dfa.match_pattern(cache, sid, 0);
                mat = Some(HalfMatch::new(pattern, at));
                if input.get_earliest() {
                    return Ok(mat.ok_or(at));
                }
            } else if sid.is_dead() {
                return Ok(mat.ok_or(at));
            } else if sid.is_quit() {
                return Err(MatchError::quit(input.haystack()[at], at).into());
            } else {
                // We should NEVER get an unknown state ID back from
                // dfa.next_state().
                debug_assert!(!sid.is_unknown());
                // Ideally we wouldn't use a lazy DFA that specialized start
                // states and thus 'sid.is_start()' could never be true here,
                // but in practice we reuse the lazy DFA created for the full
                // regex which will specialize start states whenever there is
                // a prefilter.
                debug_assert!(sid.is_start());
            }
        }
        at += 1;
    }
    hybrid_eoi_fwd(dfa, cache, input, &mut sid, &mut mat)?;
    Ok(mat.ok_or(at))
}

#[cfg(feature = "dfa-build")]
#[cfg_attr(feature = "perf-inline", inline(always))]
fn dfa_eoi_fwd(
    dfa: &crate::dfa::dense::DFA<alloc::vec::Vec<u32>>,
    input: &Input<'_>,
    sid: &mut crate::util::primitives::StateID,
    mat: &mut Option<HalfMatch>,
) -> Result<(), MatchError> {
    use crate::dfa::Automaton;

    let sp = input.get_span();
    match input.haystack().get(sp.end) {
        Some(&b) => {
            *sid = dfa.next_state(*sid, b);
            if dfa.is_match_state(*sid) {
                let pattern = dfa.match_pattern(*sid, 0);
                *mat = Some(HalfMatch::new(pattern, sp.end));
            } else if dfa.is_quit_state(*sid) {
                return Err(MatchError::quit(b, sp.end));
            }
        }
        None => {
            *sid = dfa.next_eoi_state(*sid);
            if dfa.is_match_state(*sid) {
                let pattern = dfa.match_pattern(*sid, 0);
                *mat = Some(HalfMatch::new(pattern, input.haystack().len()));
            }
            // N.B. We don't have to check 'is_quit' here because the EOI
            // transition can never lead to a quit state.
            debug_assert!(!dfa.is_quit_state(*sid));
        }
    }
    Ok(())
}

#[cfg(feature = "hybrid")]
#[cfg_attr(feature = "perf-inline", inline(always))]
fn hybrid_eoi_fwd(
    dfa: &crate::hybrid::dfa::DFA,
    cache: &mut crate::hybrid::dfa::Cache,
    input: &Input<'_>,
    sid: &mut crate::hybrid::LazyStateID,
    mat: &mut Option<HalfMatch>,
) -> Result<(), MatchError> {
    let sp = input.get_span();
    match input.haystack().get(sp.end) {
        Some(&b) => {
            *sid = dfa
                .next_state(cache, *sid, b)
                .map_err(|_| MatchError::gave_up(sp.end))?;
            if sid.is_match() {
                let pattern = dfa.match_pattern(cache, *sid, 0);
                *mat = Some(HalfMatch::new(pattern, sp.end));
            } else if sid.is_quit() {
                return Err(MatchError::quit(b, sp.end));
            }
        }
        None => {
            *sid = dfa
                .next_eoi_state(cache, *sid)
                .map_err(|_| MatchError::gave_up(input.haystack().len()))?;
            if sid.is_match() {
                let pattern = dfa.match_pattern(cache, *sid, 0);
                *mat = Some(HalfMatch::new(pattern, input.haystack().len()));
            }
            // N.B. We don't have to check 'is_quit' here because the EOI
            // transition can never lead to a quit state.
            debug_assert!(!sid.is_quit());
        }
    }
    Ok(())
}
