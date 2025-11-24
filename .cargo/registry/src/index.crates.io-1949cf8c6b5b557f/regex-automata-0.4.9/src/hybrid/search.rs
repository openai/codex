use crate::{
    hybrid::{
        dfa::{Cache, OverlappingState, DFA},
        id::LazyStateID,
    },
    util::{
        prefilter::Prefilter,
        search::{HalfMatch, Input, MatchError, Span},
    },
};

#[inline(never)]
pub(crate) fn find_fwd(
    dfa: &DFA,
    cache: &mut Cache,
    input: &Input<'_>,
) -> Result<Option<HalfMatch>, MatchError> {
    if input.is_done() {
        return Ok(None);
    }
    let pre = if input.get_anchored().is_anchored() {
        None
    } else {
        dfa.get_config().get_prefilter()
    };
    // So what we do here is specialize four different versions of 'find_fwd':
    // one for each of the combinations for 'has prefilter' and 'is earliest
    // search'. The reason for doing this is that both of these things require
    // branches and special handling in some code that can be very hot,
    // and shaving off as much as we can when we don't need it tends to be
    // beneficial in ad hoc benchmarks. To see these differences, you often
    // need a query with a high match count. In other words, specializing these
    // four routines *tends* to help latency more than throughput.
    if pre.is_some() {
        if input.get_earliest() {
            find_fwd_imp(dfa, cache, input, pre, true)
        } else {
            find_fwd_imp(dfa, cache, input, pre, false)
        }
    } else {
        if input.get_earliest() {
            find_fwd_imp(dfa, cache, input, None, true)
        } else {
            find_fwd_imp(dfa, cache, input, None, false)
        }
    }
}

#[cfg_attr(feature = "perf-inline", inline(always))]
fn find_fwd_imp(
    dfa: &DFA,
    cache: &mut Cache,
    input: &Input<'_>,
    pre: Option<&'_ Prefilter>,
    earliest: bool,
) -> Result<Option<HalfMatch>, MatchError> {
    // See 'prefilter_restart' docs for explanation.
    let universal_start = dfa.get_nfa().look_set_prefix_any().is_empty();
    let mut mat = None;
    let mut sid = init_fwd(dfa, cache, input)?;
    let mut at = input.start();
    // This could just be a closure, but then I think it would be unsound
    // because it would need to be safe to invoke. This way, the lack of safety
    // is clearer in the code below.
    macro_rules! next_unchecked {
        ($sid:expr, $at:expr) => {{
            let byte = *input.haystack().get_unchecked($at);
            dfa.next_state_untagged_unchecked(cache, $sid, byte)
        }};
    }

    if let Some(ref pre) = pre {
        let span = Span::from(at..input.end());
        match pre.find(input.haystack(), span) {
            None => return Ok(mat),
            Some(ref span) => {
                at = span.start;
                if !universal_start {
                    sid = prefilter_restart(dfa, cache, &input, at)?;
                }
            }
        }
    }
    cache.search_start(at);
    while at < input.end() {
        if sid.is_tagged() {
            cache.search_update(at);
            sid = dfa
                .next_state(cache, sid, input.haystack()[at])
                .map_err(|_| gave_up(at))?;
        } else {
            // SAFETY: There are two safety invariants we need to uphold
            // here in the loops below: that 'sid' and 'prev_sid' are valid
            // state IDs for this DFA, and that 'at' is a valid index into
            // 'haystack'. For the former, we rely on the invariant that
            // next_state* and start_state_forward always returns a valid state
            // ID (given a valid state ID in the former case), and that we are
            // only at this place in the code if 'sid' is untagged. Moreover,
            // every call to next_state_untagged_unchecked below is guarded by
            // a check that sid is untagged. For the latter safety invariant,
            // we always guard unchecked access with a check that 'at' is less
            // than 'end', where 'end <= haystack.len()'. In the unrolled loop
            // below, we ensure that 'at' is always in bounds.
            //
            // PERF: For justification of omitting bounds checks, it gives us a
            // ~10% bump in search time. This was used for a benchmark:
            //
            //     regex-cli find half hybrid -p '(?m)^.+$' -UBb bigfile
            //
            // PERF: For justification for the loop unrolling, we use a few
            // different tests:
            //
            //     regex-cli find half hybrid -p '\w{50}' -UBb bigfile
            //     regex-cli find half hybrid -p '(?m)^.+$' -UBb bigfile
            //     regex-cli find half hybrid -p 'ZQZQZQZQ' -UBb bigfile
            //
            // And there are three different configurations:
            //
            //     nounroll: this entire 'else' block vanishes and we just
            //               always use 'dfa.next_state(..)'.
            //      unroll1: just the outer loop below
            //      unroll2: just the inner loop below
            //      unroll3: both the outer and inner loops below
            //
            // This results in a matrix of timings for each of the above
            // regexes with each of the above unrolling configurations:
            //
            //              '\w{50}'   '(?m)^.+$'   'ZQZQZQZQ'
            //   nounroll   1.51s      2.34s        1.51s
            //    unroll1   1.53s      2.32s        1.56s
            //    unroll2   2.22s      1.50s        0.61s
            //    unroll3   1.67s      1.45s        0.61s
            //
            // Ideally we'd be able to find a configuration that yields the
            // best time for all regexes, but alas we settle for unroll3 that
            // gives us *almost* the best for '\w{50}' and the best for the
            // other two regexes.
            //
            // So what exactly is going on here? The first unrolling (grouping
            // together runs of untagged transitions) specifically targets
            // our choice of representation. The second unrolling (grouping
            // together runs of self-transitions) specifically targets a common
            // DFA topology. Let's dig in a little bit by looking at our
            // regexes:
            //
            // '\w{50}': This regex spends a lot of time outside of the DFA's
            // start state matching some part of the '\w' repetition. This
            // means that it's a bit of a worst case for loop unrolling that
            // targets self-transitions since the self-transitions in '\w{50}'
            // are not particularly active for this haystack. However, the
            // first unrolling (grouping together untagged transitions)
            // does apply quite well here since very few transitions hit
            // match/dead/quit/unknown states. It is however worth mentioning
            // that if start states are configured to be tagged (which you
            // typically want to do if you have a prefilter), then this regex
            // actually slows way down because it is constantly ping-ponging
            // out of the unrolled loop and into the handling of a tagged start
            // state below. But when start states aren't tagged, the unrolled
            // loop stays hot. (This is why it's imperative that start state
            // tagging be disabled when there isn't a prefilter!)
            //
            // '(?m)^.+$': There are two important aspects of this regex: 1)
            // on this haystack, its match count is very high, much higher
            // than the other two regex and 2) it spends the vast majority
            // of its time matching '.+'. Since Unicode mode is disabled,
            // this corresponds to repeatedly following self transitions for
            // the vast majority of the input. This does benefit from the
            // untagged unrolling since most of the transitions will be to
            // untagged states, but the untagged unrolling does more work than
            // what is actually required. Namely, it has to keep track of the
            // previous and next state IDs, which I guess requires a bit more
            // shuffling. This is supported by the fact that nounroll+unroll1
            // are both slower than unroll2+unroll3, where the latter has a
            // loop unrolling that specifically targets self-transitions.
            //
            // 'ZQZQZQZQ': This one is very similar to '(?m)^.+$' because it
            // spends the vast majority of its time in self-transitions for
            // the (implicit) unanchored prefix. The main difference with
            // '(?m)^.+$' is that it has a much lower match count. So there
            // isn't much time spent in the overhead of reporting matches. This
            // is the primary explainer in the perf difference here. We include
            // this regex and the former to make sure we have comparison points
            // with high and low match counts.
            //
            // NOTE: I used 'OpenSubtitles2018.raw.sample.en' for 'bigfile'.
            //
            // NOTE: In a follow-up, it turns out that the "inner" loop
            // mentioned above was a pretty big pessimization in some other
            // cases. Namely, it resulted in too much ping-ponging into and out
            // of the loop, which resulted in nearly ~2x regressions in search
            // time when compared to the originally lazy DFA in the regex crate.
            // So I've removed the second loop unrolling that targets the
            // self-transition case.
            let mut prev_sid = sid;
            while at < input.end() {
                prev_sid = unsafe { next_unchecked!(sid, at) };
                if prev_sid.is_tagged() || at + 3 >= input.end() {
                    core::mem::swap(&mut prev_sid, &mut sid);
                    break;
                }
                at += 1;

                sid = unsafe { next_unchecked!(prev_sid, at) };
                if sid.is_tagged() {
                    break;
                }
                at += 1;

                prev_sid = unsafe { next_unchecked!(sid, at) };
                if prev_sid.is_tagged() {
                    core::mem::swap(&mut prev_sid, &mut sid);
                    break;
                }
                at += 1;

                sid = unsafe { next_unchecked!(prev_sid, at) };
                if sid.is_tagged() {
                    break;
                }
                at += 1;
            }
            // If we quit out of the code above with an unknown state ID at
            // any point, then we need to re-compute that transition using
            // 'next_state', which will do NFA powerset construction for us.
            if sid.is_unknown() {
                cache.search_update(at);
                sid = dfa
                    .next_state(cache, prev_sid, input.haystack()[at])
                    .map_err(|_| gave_up(at))?;
            }
        }
        if sid.is_tagged() {
            if sid.is_start() {
                if let Some(ref pre) = pre {
                    let span = Span::from(at..input.end());
                    match pre.find(input.haystack(), span) {
                        None => {
                            cache.search_finish(span.end);
                            return Ok(mat);
                        }
                        Some(ref span) => {
                            // We want to skip any update to 'at' below
                            // at the end of this iteration and just
                            // jump immediately back to the next state
                            // transition at the leading position of the
                            // candidate match.
                            //
                            // ... but only if we actually made progress
                            // with our prefilter, otherwise if the start
                            // state has a self-loop, we can get stuck.
                            if span.start > at {
                                at = span.start;
                                if !universal_start {
                                    sid = prefilter_restart(
                                        dfa, cache, &input, at,
                                    )?;
                                }
                                continue;
                            }
                        }
                    }
                }
            } else if sid.is_match() {
                let pattern = dfa.match_pattern(cache, sid, 0);
                // Since slice ranges are inclusive at the beginning and
                // exclusive at the end, and since forward searches report
                // the end, we can return 'at' as-is. This only works because
                // matches are delayed by 1 byte. So by the time we observe a
                // match, 'at' has already been set to 1 byte past the actual
                // match location, which is precisely the exclusive ending
                // bound of the match.
                mat = Some(HalfMatch::new(pattern, at));
                if earliest {
                    cache.search_finish(at);
                    return Ok(mat);
                }
            } else if sid.is_dead() {
                cache.search_finish(at);
                return Ok(mat);
            } else if sid.is_quit() {
                cache.search_finish(at);
                return Err(MatchError::quit(input.haystack()[at], at));
            } else {
                debug_assert!(sid.is_unknown());
                unreachable!("sid being unknown is a bug");
            }
        }
        at += 1;
    }
    eoi_fwd(dfa, cache, input, &mut sid, &mut mat)?;
    cache.search_finish(input.end());
    Ok(mat)
}

#[inline(never)]
pub(crate) fn find_rev(
    dfa: &DFA,
    cache: &mut Cache,
    input: &Input<'_>,
) -> Result<Option<HalfMatch>, MatchError> {
    if input.is_done() {
        return Ok(None);
    }
    if input.get_earliest() {
        find_rev_imp(dfa, cache, input, true)
    } else {
        find_rev_imp(dfa, cache, input, false)
    }
}

#[cfg_attr(feature = "perf-inline", inline(always))]
fn find_rev_imp(
    dfa: &DFA,
    cache: &mut Cache,
    input: &Input<'_>,
    earliest: bool,
) -> Result<Option<HalfMatch>, MatchError> {
    let mut mat = None;
    let mut sid = init_rev(dfa, cache, input)?;
    // In reverse search, the loop below can't handle the case of searching an
    // empty slice. Ideally we could write something congruent to the forward
    // search, i.e., 'while at >= start', but 'start' might be 0. Since we use
    // an unsigned offset, 'at >= 0' is trivially always true. We could avoid
    // this extra case handling by using a signed offset, but Rust makes it
    // annoying to do. So... We just handle the empty case separately.
    if input.start() == input.end() {
        eoi_rev(dfa, cache, input, &mut sid, &mut mat)?;
        return Ok(mat);
    }

    let mut at = input.end() - 1;
    macro_rules! next_unchecked {
        ($sid:expr, $at:expr) => {{
            let byte = *input.haystack().get_unchecked($at);
            dfa.next_state_untagged_unchecked(cache, $sid, byte)
        }};
    }
    cache.search_start(at);
    loop {
        if sid.is_tagged() {
            cache.search_update(at);
            sid = dfa
                .next_state(cache, sid, input.haystack()[at])
                .map_err(|_| gave_up(at))?;
        } else {
            // SAFETY: See comments in 'find_fwd' for a safety argument.
            //
            // PERF: The comments in 'find_fwd' also provide a justification
            // from a performance perspective as to 1) why we elide bounds
            // checks and 2) why we do a specialized version of unrolling
            // below. The reverse search does have a slightly different
            // consideration in that most reverse searches tend to be
            // anchored and on shorter haystacks. However, this still makes a
            // difference. Take this command for example:
            //
            //     regex-cli find match hybrid -p '(?m)^.+$' -UBb bigfile
            //
            // (Notice that we use 'find hybrid regex', not 'find hybrid dfa'
            // like in the justification for the forward direction. The 'regex'
            // sub-command will find start-of-match and thus run the reverse
            // direction.)
            //
            // Without unrolling below, the above command takes around 3.76s.
            // But with the unrolling below, we get down to 2.55s. If we keep
            // the unrolling but add in bounds checks, then we get 2.86s.
            //
            // NOTE: I used 'OpenSubtitles2018.raw.sample.en' for 'bigfile'.
            let mut prev_sid = sid;
            while at >= input.start() {
                prev_sid = unsafe { next_unchecked!(sid, at) };
                if prev_sid.is_tagged()
                    || at <= input.start().saturating_add(3)
                {
                    core::mem::swap(&mut prev_sid, &mut sid);
                    break;
                }
                at -= 1;

                sid = unsafe { next_unchecked!(prev_sid, at) };
                if sid.is_tagged() {
                    break;
                }
                at -= 1;

                prev_sid = unsafe { next_unchecked!(sid, at) };
                if prev_sid.is_tagged() {
                    core::mem::swap(&mut prev_sid, &mut sid);
                    break;
                }
                at -= 1;

                sid = unsafe { next_unchecked!(prev_sid, at) };
                if sid.is_tagged() {
                    break;
                }
                at -= 1;
            }
            // If we quit out of the code above with an unknown state ID at
            // any point, then we need to re-compute that transition using
            // 'next_state', which will do NFA powerset construction for us.
            if sid.is_unknown() {
                cache.search_update(at);
                sid = dfa
                    .next_state(cache, prev_sid, input.haystack()[at])
                    .map_err(|_| gave_up(at))?;
            }
        }
        if sid.is_tagged() {
            if sid.is_start() {
                // do nothing
            } else if sid.is_match() {
                let pattern = dfa.match_pattern(cache, sid, 0);
                // Since reverse searches report the beginning of a match
                // and the beginning is inclusive (not exclusive like the
                // end of a match), we add 1 to make it inclusive.
                mat = Some(HalfMatch::new(pattern, at + 1));
                if earliest {
                    cache.search_finish(at);
                    return Ok(mat);
                }
            } else if sid.is_dead() {
                cache.search_finish(at);
                return Ok(mat);
            } else if sid.is_quit() {
                cache.search_finish(at);
                return Err(MatchError::quit(input.haystack()[at], at));
            } else {
                debug_assert!(sid.is_unknown());
                unreachable!("sid being unknown is a bug");
            }
        }
        if at == input.start() {
            break;
        }
        at -= 1;
    }
    cache.search_finish(input.start());
    eoi_rev(dfa, cache, input, &mut sid, &mut mat)?;
    Ok(mat)
}

#[inline(never)]
pub(crate) fn find_overlapping_fwd(
    dfa: &DFA,
    cache: &mut Cache,
    input: &Input<'_>,
    state: &mut OverlappingState,
) -> Result<(), MatchError> {
    state.mat = None;
    if input.is_done() {
        return Ok(());
    }
    let pre = if input.get_anchored().is_anchored() {
        None
    } else {
        dfa.get_config().get_prefilter()
    };
    if pre.is_some() {
        find_overlapping_fwd_imp(dfa, cache, input, pre, state)
    } else {
        find_overlapping_fwd_imp(dfa, cache, input, None, state)
    }
}

#[cfg_attr(feature = "perf-inline", inline(always))]
fn find_overlapping_fwd_imp(
    dfa: &DFA,
    cache: &mut Cache,
    input: &Input<'_>,
    pre: Option<&'_ Prefilter>,
    state: &mut OverlappingState,
) -> Result<(), MatchError> {
    // See 'prefilter_restart' docs for explanation.
    let universal_start = dfa.get_nfa().look_set_prefix_any().is_empty();
    let mut sid = match state.id {
        None => {
            state.at = input.start();
            init_fwd(dfa, cache, input)?
        }
        Some(sid) => {
            if let Some(match_index) = state.next_match_index {
                let match_len = dfa.match_len(cache, sid);
                if match_index < match_len {
                    state.next_match_index = Some(match_index + 1);
                    let pattern = dfa.match_pattern(cache, sid, match_index);
                    state.mat = Some(HalfMatch::new(pattern, state.at));
                    return Ok(());
                }
            }
            // Once we've reported all matches at a given position, we need to
            // advance the search to the next position.
            state.at += 1;
            if state.at > input.end() {
                return Ok(());
            }
            sid
        }
    };

    // NOTE: We don't optimize the crap out of this routine primarily because
    // it seems like most overlapping searches will have higher match counts,
    // and thus, throughput is perhaps not as important. But if you have a use
    // case for something faster, feel free to file an issue.
    cache.search_start(state.at);
    while state.at < input.end() {
        sid = dfa
            .next_state(cache, sid, input.haystack()[state.at])
            .map_err(|_| gave_up(state.at))?;
        if sid.is_tagged() {
            state.id = Some(sid);
            if sid.is_start() {
                if let Some(ref pre) = pre {
                    let span = Span::from(state.at..input.end());
                    match pre.find(input.haystack(), span) {
                        None => return Ok(()),
                        Some(ref span) => {
                            if span.start > state.at {
                                state.at = span.start;
                                if !universal_start {
                                    sid = prefilter_restart(
                                        dfa, cache, &input, state.at,
                                    )?;
                                }
                                continue;
                            }
                        }
                    }
                }
            } else if sid.is_match() {
                state.next_match_index = Some(1);
                let pattern = dfa.match_pattern(cache, sid, 0);
                state.mat = Some(HalfMatch::new(pattern, state.at));
                cache.search_finish(state.at);
                return Ok(());
            } else if sid.is_dead() {
                cache.search_finish(state.at);
                return Ok(());
            } else if sid.is_quit() {
                cache.search_finish(state.at);
                return Err(MatchError::quit(
                    input.haystack()[state.at],
                    state.at,
                ));
            } else {
                debug_assert!(sid.is_unknown());
                unreachable!("sid being unknown is a bug");
            }
        }
        state.at += 1;
        cache.search_update(state.at);
    }

    let result = eoi_fwd(dfa, cache, input, &mut sid, &mut state.mat);
    state.id = Some(sid);
    if state.mat.is_some() {
        // '1' is always correct here since if we get to this point, this
        // always corresponds to the first (index '0') match discovered at
        // this position. So the next match to report at this position (if
        // it exists) is at index '1'.
        state.next_match_index = Some(1);
    }
    cache.search_finish(input.end());
    result
}

#[inline(never)]
pub(crate) fn find_overlapping_rev(
    dfa: &DFA,
    cache: &mut Cache,
    input: &Input<'_>,
    state: &mut OverlappingState,
) -> Result<(), MatchError> {
    state.mat = None;
    if input.is_done() {
        return Ok(());
    }
    let mut sid = match state.id {
        None => {
            let sid = init_rev(dfa, cache, input)?;
            state.id = Some(sid);
            if input.start() == input.end() {
                state.rev_eoi = true;
            } else {
                state.at = input.end() - 1;
            }
            sid
        }
        Some(sid) => {
            if let Some(match_index) = state.next_match_index {
                let match_len = dfa.match_len(cache, sid);
                if match_index < match_len {
                    state.next_match_index = Some(match_index + 1);
                    let pattern = dfa.match_pattern(cache, sid, match_index);
                    state.mat = Some(HalfMatch::new(pattern, state.at));
                    return Ok(());
                }
            }
            // Once we've reported all matches at a given position, we need
            // to advance the search to the next position. However, if we've
            // already followed the EOI transition, then we know we're done
            // with the search and there cannot be any more matches to report.
            if state.rev_eoi {
                return Ok(());
            } else if state.at == input.start() {
                // At this point, we should follow the EOI transition. This
                // will cause us the skip the main loop below and fall through
                // to the final 'eoi_rev' transition.
                state.rev_eoi = true;
            } else {
                // We haven't hit the end of the search yet, so move on.
                state.at -= 1;
            }
            sid
        }
    };
    cache.search_start(state.at);
    while !state.rev_eoi {
        sid = dfa
            .next_state(cache, sid, input.haystack()[state.at])
            .map_err(|_| gave_up(state.at))?;
        if sid.is_tagged() {
            state.id = Some(sid);
            if sid.is_start() {
                // do nothing
            } else if sid.is_match() {
                state.next_match_index = Some(1);
                let pattern = dfa.match_pattern(cache, sid, 0);
                state.mat = Some(HalfMatch::new(pattern, state.at + 1));
                cache.search_finish(state.at);
                return Ok(());
            } else if sid.is_dead() {
                cache.search_finish(state.at);
                return Ok(());
            } else if sid.is_quit() {
                cache.search_finish(state.at);
                return Err(MatchError::quit(
                    input.haystack()[state.at],
                    state.at,
                ));
            } else {
                debug_assert!(sid.is_unknown());
                unreachable!("sid being unknown is a bug");
            }
        }
        if state.at == input.start() {
            break;
        }
        state.at -= 1;
        cache.search_update(state.at);
    }

    let result = eoi_rev(dfa, cache, input, &mut sid, &mut state.mat);
    state.rev_eoi = true;
    state.id = Some(sid);
    if state.mat.is_some() {
        // '1' is always correct here since if we get to this point, this
        // always corresponds to the first (index '0') match discovered at
        // this position. So the next match to report at this position (if
        // it exists) is at index '1'.
        state.next_match_index = Some(1);
    }
    cache.search_finish(input.start());
    result
}

#[cfg_attr(feature = "perf-inline", inline(always))]
fn init_fwd(
    dfa: &DFA,
    cache: &mut Cache,
    input: &Input<'_>,
) -> Result<LazyStateID, MatchError> {
    let sid = dfa.start_state_forward(cache, input)?;
    // Start states can never be match states, since all matches are delayed
    // by 1 byte.
    debug_assert!(!sid.is_match());
    Ok(sid)
}

#[cfg_attr(feature = "perf-inline", inline(always))]
fn init_rev(
    dfa: &DFA,
    cache: &mut Cache,
    input: &Input<'_>,
) -> Result<LazyStateID, MatchError> {
    let sid = dfa.start_state_reverse(cache, input)?;
    // Start states can never be match states, since all matches are delayed
    // by 1 byte.
    debug_assert!(!sid.is_match());
    Ok(sid)
}

#[cfg_attr(feature = "perf-inline", inline(always))]
fn eoi_fwd(
    dfa: &DFA,
    cache: &mut Cache,
    input: &Input<'_>,
    sid: &mut LazyStateID,
    mat: &mut Option<HalfMatch>,
) -> Result<(), MatchError> {
    let sp = input.get_span();
    match input.haystack().get(sp.end) {
        Some(&b) => {
            *sid =
                dfa.next_state(cache, *sid, b).map_err(|_| gave_up(sp.end))?;
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
                .map_err(|_| gave_up(input.haystack().len()))?;
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

#[cfg_attr(feature = "perf-inline", inline(always))]
fn eoi_rev(
    dfa: &DFA,
    cache: &mut Cache,
    input: &Input<'_>,
    sid: &mut LazyStateID,
    mat: &mut Option<HalfMatch>,
) -> Result<(), MatchError> {
    let sp = input.get_span();
    if sp.start > 0 {
        let byte = input.haystack()[sp.start - 1];
        *sid = dfa
            .next_state(cache, *sid, byte)
            .map_err(|_| gave_up(sp.start))?;
        if sid.is_match() {
            let pattern = dfa.match_pattern(cache, *sid, 0);
            *mat = Some(HalfMatch::new(pattern, sp.start));
        } else if sid.is_quit() {
            return Err(MatchError::quit(byte, sp.start - 1));
        }
    } else {
        *sid =
            dfa.next_eoi_state(cache, *sid).map_err(|_| gave_up(sp.start))?;
        if sid.is_match() {
            let pattern = dfa.match_pattern(cache, *sid, 0);
            *mat = Some(HalfMatch::new(pattern, 0));
        }
        // N.B. We don't have to check 'is_quit' here because the EOI
        // transition can never lead to a quit state.
        debug_assert!(!sid.is_quit());
    }
    Ok(())
}

/// Re-compute the starting state that a DFA should be in after finding a
/// prefilter candidate match at the position `at`.
///
/// It is always correct to call this, but not always necessary. Namely,
/// whenever the DFA has a universal start state, the DFA can remain in the
/// start state that it was in when it ran the prefilter. Why? Because in that
/// case, there is only one start state.
///
/// When does a DFA have a universal start state? In precisely cases where
/// it has no look-around assertions in its prefix. So for example, `\bfoo`
/// does not have a universal start state because the start state depends on
/// whether the byte immediately before the start position is a word byte or
/// not. However, `foo\b` does have a universal start state because the word
/// boundary does not appear in the pattern's prefix.
///
/// So... most cases don't need this, but when a pattern doesn't have a
/// universal start state, then after a prefilter candidate has been found, the
/// current state *must* be re-litigated as if computing the start state at the
/// beginning of the search because it might change. That is, not all start
/// states are created equal.
///
/// Why avoid it? Because while it's not super expensive, it isn't a trivial
/// operation to compute the start state. It is much better to avoid it and
/// just state in the current state if you know it to be correct.
#[cfg_attr(feature = "perf-inline", inline(always))]
fn prefilter_restart(
    dfa: &DFA,
    cache: &mut Cache,
    input: &Input<'_>,
    at: usize,
) -> Result<LazyStateID, MatchError> {
    let mut input = input.clone();
    input.set_start(at);
    init_fwd(dfa, cache, &input)
}

/// A convenience routine for constructing a "gave up" match error.
#[cfg_attr(feature = "perf-inline", inline(always))]
fn gave_up(offset: usize) -> MatchError {
    MatchError::gave_up(offset)
}
