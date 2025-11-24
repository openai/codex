use super::MagicRule;

use nom::{
    bytes::complete::{is_not, tag, take, take_while},
    character::is_digit,
    combinator::{map, map_res, opt},
    multi::many0,
    number::complete::be_u16,
    sequence::{delimited, preceded, terminated, tuple},
    IResult,
};
use petgraph::prelude::*;
use std::collections::HashMap;
use std::str;

// Singular magic ruleset
fn magic_rules(input: &[u8]) -> IResult<&[u8], MagicRule<'_>> {
    let int_or = |default| {
        map(take_while(is_digit), move |digits| {
            str::from_utf8(digits).unwrap().parse().unwrap_or(default)
        })
    };

    let (input, (indent_level, start_off, val_len)) = tuple((
        terminated(int_or(0), tag(">")),
        terminated(int_or(0), tag("=")),
        be_u16,
    ))(input)?;

    let (input, (val, mask, _word_len, region_len)) = terminated(
        tuple((
            take(val_len),
            opt(preceded(tag("&"), take(val_len))),
            opt(preceded(tag("~"), int_or(1))),
            opt(preceded(tag("+"), int_or(0))),
        )),
        tag("\n"),
    )(input)?;

    Ok((
        input,
        MagicRule {
            indent_level,
            start_off,
            val,
            mask,
            region_len: region_len.unwrap_or(0),
        },
    ))
}

/// Converts a magic file given as a &[u8] array
/// to a vector of MagicEntry structs
fn ruleset(input: &[u8]) -> IResult<&[u8], Vec<(&str, Vec<MagicRule<'_>>)>> {
    // Parse the MIME type from "[priority: mime]"
    let mime = map_res(
        terminated(
            delimited(
                delimited(tag("["), is_not(":"), tag(":")), // priority
                is_not("]"),                                // mime
                tag("]"),
            ),
            tag("\n"),
        ),
        str::from_utf8,
    );

    let magic_entry = tuple((mime, many0(magic_rules)));
    preceded(tag("MIME-Magic\0\n"), many0(magic_entry))(input)
}

fn gen_graph(magic_rules: Vec<MagicRule<'_>>) -> DiGraph<MagicRule<'_>, u32> {
    use petgraph::prelude::*;
    // Whip up a graph real quick
    let mut graph = DiGraph::<MagicRule, u32>::new();
    let mut rulestack = Vec::<(MagicRule, NodeIndex)>::new();

    for x in magic_rules {
        let xnode = graph.add_node(x.clone());

        loop {
            let y = rulestack.pop();
            match y {
                None => {
                    break;
                }
                Some(rule) => {
                    if rule.0.indent_level < x.indent_level {
                        graph.add_edge(rule.1, xnode, 1);
                        rulestack.push(rule);
                        break;
                    }
                }
            };
        }
        rulestack.push((x, xnode));
    }
    graph
}

#[cfg(feature = "with-gpl-data")]
pub fn from_u8(b: &[u8]) -> Result<HashMap<&str, DiGraph<MagicRule<'_>, u32>>, String> {
    let tuplevec = ruleset(b).map_err(|e| e.to_string())?.1;
    let res = tuplevec
        .into_iter()
        .map(|x| (x.0, gen_graph(x.1)))
        .collect();
    Ok(res)
}

#[cfg(not(feature = "with-gpl-data"))]
/// Parse multiple ruleset magic files and aggregate the tuples into a single graph
pub fn from_multiple(
    files: &[Vec<u8>],
) -> Result<HashMap<&str, DiGraph<MagicRule<'_>, u32>>, String> {
    let mut tuplevec = vec![];
    for slice in files {
        tuplevec.append(&mut ruleset(slice.as_ref()).map_err(|e| e.to_string())?.1);
    }
    let res = tuplevec
        .into_iter()
        .map(|x| (x.0, gen_graph(x.1)))
        .collect();
    Ok(res)
}
