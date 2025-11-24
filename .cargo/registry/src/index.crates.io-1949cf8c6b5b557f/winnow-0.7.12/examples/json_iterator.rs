use std::collections::HashMap;

use winnow::prelude::*;
use winnow::{
    ascii::{alphanumeric1 as alphanumeric, float, take_escaped},
    combinator::alt,
    combinator::cut_err,
    combinator::separated,
    combinator::{preceded, separated_pair, terminated},
    error::ParserError,
    error::StrContext,
    stream::Offset,
    token::one_of,
    token::{literal, take_while},
};

use std::cell::Cell;
use std::str;

#[derive(Clone, Debug)]
pub struct JsonValue<'a, 'b> {
    input: &'a str,
    pub offset: &'b Cell<usize>,
}

impl<'a, 'b: 'a> JsonValue<'a, 'b> {
    pub fn new(input: &'a str, offset: &'b Cell<usize>) -> JsonValue<'a, 'b> {
        JsonValue { input, offset }
    }

    pub fn offset(&self, input: &'a str) {
        let offset = input.offset_from(&self.input);
        self.offset.set(offset);
    }

    pub fn data(&self) -> &'a str {
        &self.input[self.offset.get()..]
    }

    pub fn string(&self) -> Option<&'a str> {
        println!("string()");
        let mut data = self.data();
        match string(&mut data) {
            Ok(s) => {
                self.offset(data);
                println!("-> {s}");
                Some(s)
            }
            _ => None,
        }
    }

    pub fn boolean(&self) -> Option<bool> {
        println!("boolean()");
        let mut data = self.data();
        match boolean(&mut data) {
            Ok(o) => {
                self.offset(data);
                println!("-> {o}");
                Some(o)
            }
            _ => None,
        }
    }

    pub fn number(&self) -> Option<f64> {
        println!("number()");
        let mut data = self.data();
        match float::<_, _, ()>.parse_next(&mut data) {
            Ok(o) => {
                self.offset(data);
                println!("-> {o}");
                Some(o)
            }
            _ => None,
        }
    }

    pub fn array(&self) -> Option<impl Iterator<Item = JsonValue<'a, 'b>>> {
        println!("array()");

        let mut data = self.data();
        match literal::<_, _, ()>("[").parse_next(&mut data) {
            Err(_) => None,
            Ok(_) => {
                println!("[");
                self.offset(data);
                let mut first = true;
                let mut done = false;
                let mut previous = usize::MAX;

                let v = self.clone();

                Some(std::iter::from_fn(move || {
                    if done {
                        return None;
                    }

                    // if we ignored one of the items, skip over the value
                    if v.offset.get() == previous {
                        println!("skipping value");
                        if value(&mut data).is_ok() {
                            v.offset(data);
                        }
                    }

                    if literal::<_, _, ()>("]").parse_next(&mut data).is_ok() {
                        println!("]");
                        v.offset(data);
                        done = true;
                        return None;
                    }

                    if first {
                        first = false;
                    } else {
                        match literal::<_, _, ()>(",").parse_next(&mut data) {
                            Ok(_) => {
                                println!(",");
                                v.offset(data);
                            }
                            Err(_) => {
                                done = true;
                                return None;
                            }
                        }
                    }

                    println!("-> {}", v.data());
                    previous = v.offset.get();
                    Some(v.clone())
                }))
            }
        }
    }

    pub fn object(&self) -> Option<impl Iterator<Item = (&'a str, JsonValue<'a, 'b>)>> {
        println!("object()");
        let mut data = self.data();
        match literal::<_, _, ()>("{").parse_next(&mut data) {
            Err(_) => None,
            Ok(_) => {
                self.offset(data);

                println!("{{");

                let mut first = true;
                let mut done = false;
                let mut previous = usize::MAX;

                let v = self.clone();

                Some(std::iter::from_fn(move || {
                    if done {
                        return None;
                    }

                    // if we ignored one of the items, skip over the value
                    if v.offset.get() == previous {
                        println!("skipping value");
                        if value(&mut data).is_ok() {
                            v.offset(data);
                        }
                    }

                    if literal::<_, _, ()>("}").parse_next(&mut data).is_ok() {
                        println!("}}");
                        v.offset(data);
                        done = true;
                        return None;
                    }

                    if first {
                        first = false;
                    } else {
                        match literal::<_, _, ()>(",").parse_next(&mut data) {
                            Ok(_) => {
                                println!(",");
                                v.offset(data);
                            }
                            Err(_) => {
                                done = true;
                                return None;
                            }
                        }
                    }

                    match string(&mut data) {
                        Ok(key) => {
                            v.offset(data);

                            match literal::<_, _, ()>(":").parse_next(&mut data) {
                                Err(_) => None,
                                Ok(_) => {
                                    v.offset(data);

                                    previous = v.offset.get();

                                    println!("-> {} => {}", key, v.data());
                                    Some((key, v.clone()))
                                }
                            }
                        }
                        _ => None,
                    }
                }))
            }
        }
    }
}

fn sp<'a, E: ParserError<&'a str>>(i: &mut &'a str) -> ModalResult<&'a str, E> {
    let chars = " \t\r\n";

    take_while(0.., move |c| chars.contains(c)).parse_next(i)
}

fn parse_str<'a, E: ParserError<&'a str>>(i: &mut &'a str) -> ModalResult<&'a str, E> {
    take_escaped(alphanumeric, '\\', one_of(['"', 'n', '\\'])).parse_next(i)
}

fn string<'s>(i: &mut &'s str) -> ModalResult<&'s str> {
    preceded('\"', cut_err(terminated(parse_str, '\"')))
        .context(StrContext::Label("string"))
        .parse_next(i)
}

fn boolean(input: &mut &str) -> ModalResult<bool> {
    alt(("false".map(|_| false), "true".map(|_| true))).parse_next(input)
}

fn array(i: &mut &str) -> ModalResult<()> {
    preceded(
        '[',
        cut_err(terminated(
            separated(0.., value, preceded(sp, ',')),
            preceded(sp, ']'),
        )),
    )
    .context(StrContext::Label("array"))
    .parse_next(i)
}

fn key_value<'s>(i: &mut &'s str) -> ModalResult<(&'s str, ())> {
    separated_pair(preceded(sp, string), cut_err(preceded(sp, ':')), value).parse_next(i)
}

fn hash(i: &mut &str) -> ModalResult<()> {
    preceded(
        '{',
        cut_err(terminated(
            separated(0.., key_value, preceded(sp, ',')),
            preceded(sp, '}'),
        )),
    )
    .context(StrContext::Label("map"))
    .parse_next(i)
}

fn value(i: &mut &str) -> ModalResult<()> {
    preceded(
        sp,
        alt((
            hash,
            array,
            string.map(|_| ()),
            float::<_, f64, _>.map(|_| ()),
            boolean.map(|_| ()),
        )),
    )
    .parse_next(i)
}

/// object(input) -> iterator over (key, `JsonValue`)
/// array(input) -> iterator over `JsonValue`
///
/// JsonValue.string -> iterator over String (returns None after first successful call)
///
/// object(input).filter(|(k, _)| k == "users").flatten(|(_, v)| v.object()).filter(|(k, _)| k == "city").flatten(|(_,v)| `v.string()`)
fn main() {
    /*let data = "{
    \"users\": {
      \"user1\" : { \"city\": \"Nantes\", \"country\": \"France\" },
      \"user2\" : { \"city\": \"Bruxelles\", \"country\": \"Belgium\" },
      \"user3\": { \"city\": \"Paris\", \"country\": \"France\", \"age\": 30 }
    },
    \"countries\": [\"France\", \"Belgium\"]
    }";
    */
    let data = "{\"users\":{\"user1\":{\"city\":\"Nantes\",\"country\":\"France\"},\"user2\":{\"city\":\"Bruxelles\",\"country\":\"Belgium\"},\"user3\":{\"city\":\"Paris\",\"country\":\"France\",\"age\":30}},\"countries\":[\"France\",\"Belgium\"]}";

    let offset = Cell::new(0);
    {
        let parser = JsonValue::new(data, &offset);

        if let Some(o) = parser.object() {
            let s: HashMap<&str, &str> = o
                .filter(|(k, _)| *k == "users")
                .filter_map(|(_, v)| v.object())
                .flatten()
                .filter_map(|(user, v)| v.object().map(|o| (user, o)))
                .flat_map(|(user, o)| {
                    o.filter(|(k, _)| *k == "city")
                        .filter_map(move |(_, v)| v.string().map(|s| (user, s)))
                })
                .collect();

            println!("res = {s:?}");
        }
    };
}
