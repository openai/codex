//! Tests for URI template.
#![cfg(feature = "alloc")]

#[macro_use]
mod utils;

use std::cell::Cell;

use iri_string::spec::UriSpec;
use iri_string::template::context::{Context, DynamicContext, Visitor};
use iri_string::template::simple_context::{SimpleContext, Value};
use iri_string::template::UriTemplateStr;

/// Returns the context used by examples in RFC 6570 section 3.2.
fn rfc6570_context() -> SimpleContext {
    let mut ctx = SimpleContext::new();
    ctx.insert(
        "count",
        Value::List(vec!["one".to_owned(), "two".to_owned(), "three".to_owned()]),
    );
    ctx.insert(
        "dom",
        Value::List(vec!["example".to_owned(), "com".to_owned()]),
    );
    ctx.insert("dub", Value::String("me/too".to_owned()));
    ctx.insert("hello", Value::String("Hello World!".to_owned()));
    ctx.insert("half", Value::String("50%".to_owned()));
    ctx.insert("var", Value::String("value".to_owned()));
    ctx.insert("who", Value::String("fred".to_owned()));
    ctx.insert("base", Value::String("http://example.com/home/".to_owned()));
    ctx.insert("path", Value::String("/foo/bar".to_owned()));
    ctx.insert(
        "list",
        Value::List(vec![
            "red".to_owned(),
            "green".to_owned(),
            "blue".to_owned(),
        ]),
    );
    ctx.insert(
        "keys",
        Value::Assoc(vec![
            ("semi".to_owned(), ";".to_owned()),
            ("dot".to_owned(), ".".to_owned()),
            ("comma".to_owned(), ",".to_owned()),
        ]),
    );
    ctx.insert("v", Value::String("6".to_owned()));
    ctx.insert("x", Value::String("1024".to_owned()));
    ctx.insert("y", Value::String("768".to_owned()));
    ctx.insert("empty", Value::String("".to_owned()));
    ctx.insert("empty_keys", Value::Assoc(vec![]));
    ctx.insert("undef", Value::Undefined);

    ctx
}

/// Expression and expected expansion.
const SUCCESS_CASES: &[(&str, &str)] = &[
    // Section 3.2.1. Variable Expansion.
    ("{count}", "one,two,three"),
    ("{count*}", "one,two,three"),
    ("{/count}", "/one,two,three"),
    ("{/count*}", "/one/two/three"),
    ("{;count}", ";count=one,two,three"),
    ("{;count*}", ";count=one;count=two;count=three"),
    ("{?count}", "?count=one,two,three"),
    ("{?count*}", "?count=one&count=two&count=three"),
    ("{&count*}", "&count=one&count=two&count=three"),
    // Section 3.2.2. Simple String Expansion.
    ("{var}", "value"),
    ("{hello}", "Hello%20World%21"),
    ("{half}", "50%25"),
    ("O{empty}X", "OX"),
    ("O{undef}X", "OX"),
    ("{x,y}", "1024,768"),
    ("{x,hello,y}", "1024,Hello%20World%21,768"),
    ("?{x,empty}", "?1024,"),
    ("?{x,undef}", "?1024"),
    ("?{undef,y}", "?768"),
    ("{var:3}", "val"),
    ("{var:30}", "value"),
    ("{list}", "red,green,blue"),
    ("{list*}", "red,green,blue"),
    ("{keys}", "semi,%3B,dot,.,comma,%2C"),
    ("{keys*}", "semi=%3B,dot=.,comma=%2C"),
    // Section 3.2.3. Reserved Expansion.
    ("{+var}", "value"),
    ("{+hello}", "Hello%20World!"),
    ("{+half}", "50%25"),
    ("{base}index", "http%3A%2F%2Fexample.com%2Fhome%2Findex"),
    ("{+base}index", "http://example.com/home/index"),
    ("O{+empty}X", "OX"),
    ("O{+undef}X", "OX"),
    ("{+path}/here", "/foo/bar/here"),
    ("here?ref={+path}", "here?ref=/foo/bar"),
    ("up{+path}{var}/here", "up/foo/barvalue/here"),
    ("{+x,hello,y}", "1024,Hello%20World!,768"),
    ("{+path,x}/here", "/foo/bar,1024/here"),
    ("{+path:6}/here", "/foo/b/here"),
    ("{+list}", "red,green,blue"),
    ("{+list*}", "red,green,blue"),
    ("{+keys}", "semi,;,dot,.,comma,,"),
    ("{+keys*}", "semi=;,dot=.,comma=,"),
    // Section 3.2.4. Fragment Expansion.
    ("{#var}", "#value"),
    ("{#hello}", "#Hello%20World!"),
    ("{#half}", "#50%25"),
    ("foo{#empty}", "foo#"),
    ("foo{#undef}", "foo"),
    ("{#x,hello,y}", "#1024,Hello%20World!,768"),
    ("{#path,x}/here", "#/foo/bar,1024/here"),
    ("{#path:6}/here", "#/foo/b/here"),
    ("{#list}", "#red,green,blue"),
    ("{#list*}", "#red,green,blue"),
    ("{#keys}", "#semi,;,dot,.,comma,,"),
    ("{#keys*}", "#semi=;,dot=.,comma=,"),
    // Section 3.2.5. Label Expansion with Dot-Prefix.
    ("{.who}", ".fred"),
    ("{.who,who}", ".fred.fred"),
    ("{.half,who}", ".50%25.fred"),
    ("www{.dom*}", "www.example.com"),
    ("X{.var}", "X.value"),
    ("X{.empty}", "X."),
    ("X{.undef}", "X"),
    ("X{.var:3}", "X.val"),
    ("X{.list}", "X.red,green,blue"),
    ("X{.list*}", "X.red.green.blue"),
    ("X{.keys}", "X.semi,%3B,dot,.,comma,%2C"),
    ("X{.keys*}", "X.semi=%3B.dot=..comma=%2C"),
    ("X{.empty_keys}", "X"),
    ("X{.empty_keys*}", "X"),
    // Section 3.2.6. Path Segment Expansion.
    ("{/who}", "/fred"),
    ("{/who,who}", "/fred/fred"),
    ("{/half,who}", "/50%25/fred"),
    ("{/who,dub}", "/fred/me%2Ftoo"),
    ("{/var}", "/value"),
    ("{/var,empty}", "/value/"),
    ("{/var,undef}", "/value"),
    ("{/var,x}/here", "/value/1024/here"),
    ("{/var:1,var}", "/v/value"),
    ("{/list}", "/red,green,blue"),
    ("{/list*}", "/red/green/blue"),
    ("{/list*,path:4}", "/red/green/blue/%2Ffoo"),
    ("{/keys}", "/semi,%3B,dot,.,comma,%2C"),
    ("{/keys*}", "/semi=%3B/dot=./comma=%2C"),
    // Section 3.2.7. Path-Style Parameter Expansion.
    ("{;who}", ";who=fred"),
    ("{;half}", ";half=50%25"),
    ("{;empty}", ";empty"),
    ("{;v,empty,who}", ";v=6;empty;who=fred"),
    ("{;v,bar,who}", ";v=6;who=fred"),
    ("{;x,y}", ";x=1024;y=768"),
    ("{;x,y,empty}", ";x=1024;y=768;empty"),
    ("{;x,y,undef}", ";x=1024;y=768"),
    ("{;hello:5}", ";hello=Hello"),
    ("{;list}", ";list=red,green,blue"),
    ("{;list*}", ";list=red;list=green;list=blue"),
    ("{;keys}", ";keys=semi,%3B,dot,.,comma,%2C"),
    ("{;keys*}", ";semi=%3B;dot=.;comma=%2C"),
    // Section 3.2.8. Form-Style Query Expansion.
    ("{?who}", "?who=fred"),
    ("{?half}", "?half=50%25"),
    ("{?x,y}", "?x=1024&y=768"),
    ("{?x,y,empty}", "?x=1024&y=768&empty="),
    ("{?x,y,undef}", "?x=1024&y=768"),
    ("{?var:3}", "?var=val"),
    ("{?list}", "?list=red,green,blue"),
    ("{?list*}", "?list=red&list=green&list=blue"),
    ("{?keys}", "?keys=semi,%3B,dot,.,comma,%2C"),
    ("{?keys*}", "?semi=%3B&dot=.&comma=%2C"),
    // Section 3.2.9. Form-Style Query Continuation.
    ("{&who}", "&who=fred"),
    ("{&half}", "&half=50%25"),
    ("?fixed=yes{&x}", "?fixed=yes&x=1024"),
    ("{&x,y,empty}", "&x=1024&y=768&empty="),
    ("{&x,y,undef}", "&x=1024&y=768"),
    ("{&var:3}", "&var=val"),
    ("{&list}", "&list=red,green,blue"),
    ("{&list*}", "&list=red&list=green&list=blue"),
    ("{&keys}", "&keys=semi,%3B,dot,.,comma,%2C"),
    ("{&keys*}", "&semi=%3B&dot=.&comma=%2C"),
];

/// Tests for examples in RFC 6570 section 3.2.
#[test]
fn rfc6570_section3_2() {
    let context = rfc6570_context();

    for (template, expected) in SUCCESS_CASES {
        let template = UriTemplateStr::new(template).expect("must be valid template");
        let expanded = template
            .expand::<UriSpec, _>(&context)
            .expect("must not have variable type error");
        assert_eq_display!(expanded, expected, "template={template:?}");
        assert_eq!(expanded.to_string(), *expected, "template={template:?}");
    }
}

#[test]
fn prefix_modifier_for_percent_encoded_content() {
    let mut context = SimpleContext::new();
    context.insert("abcdef", "%61%62%63%64%65%66");
    // `%CE`, `%CE%B1`, `%B1`, `%CE`, `%CE%B2`, `%B2`.
    context.insert("invalid1", "%CE%CE%B1%B1%CE%CE%B2%B2");
    // Each `%ff` is considered as an independent "character".
    context.insert("invalid2", "%ff%ff%ff%ff%ff%ff");

    // `&[(template, expected)]`.
    const CASES: &[(&str, &str)] = &[
        ("{abcdef:4}", "%2561%25"),
        ("{+abcdef:4}", "%61%62%63%64"),
        ("{invalid1:2}", "%25C"),
        ("{invalid1:4}", "%25CE%25"),
        ("{+invalid1:2}", "%CE%CE%B1"),
        ("{+invalid1:4}", "%CE%CE%B1%B1%CE"),
        ("{invalid2:2}", "%25f"),
        ("{invalid2:4}", "%25ff%25"),
        ("{+invalid2:2}", "%ff%ff"),
        ("{+invalid2:4}", "%ff%ff%ff%ff"),
    ];

    for (template, expected) in CASES {
        let template = UriTemplateStr::new(template).expect("must be valid template");
        let expanded = template
            .expand::<UriSpec, _>(&context)
            .expect("must not have variable type error");
        assert_eq_display!(expanded, *expected, "template={template:?}");
        assert_eq!(expanded.to_string(), *expected, "template={template:?}");

        let expanded_dynamic = template
            .expand_dynamic_to_string::<UriSpec, _>(&mut context.clone())
            .expect("must not have variable type error");
        assert_eq!(
            expanded_dynamic, *expected,
            "dynamic, template={template:?}"
        );
    }
}

#[test]
fn incomplete_percent_encode() {
    let mut context = SimpleContext::new();
    context.insert("incomplete1", "%ce%b1%");
    context.insert("incomplete2", "%ce%b1%c");
    context.insert("incomplete3", "%ce%b1%ce");

    // `&[(template, expected)]`.
    const CASES: &[(&str, &str)] = &[
        ("{incomplete1:1}", "%25"),
        ("{incomplete1:2}", "%25c"),
        ("{incomplete1:3}", "%25ce"),
        ("{incomplete1:4}", "%25ce%25"),
        ("{+incomplete1:1}", "%ce%b1"),
        ("{+incomplete1:2}", "%ce%b1%25"),
        ("{+incomplete2:1}", "%ce%b1"),
        ("{+incomplete2:2}", "%ce%b1%25"),
        ("{+incomplete2:3}", "%ce%b1%25c"),
        ("{+incomplete3:1}", "%ce%b1"),
        ("{+incomplete3:2}", "%ce%b1%ce"),
        ("{+incomplete3:3}", "%ce%b1%ce"),
    ];

    for (template, expected) in CASES {
        let template = UriTemplateStr::new(template).expect("must be valid template");
        let expanded = template
            .expand::<UriSpec, _>(&context)
            .expect("must not have variable type error");
        assert_eq_display!(expanded, *expected, "template={template:?}");
        assert_eq!(expanded.to_string(), *expected, "template={template:?}");

        let expanded_dynamic = template
            .expand_dynamic_to_string::<UriSpec, _>(&mut context.clone())
            .expect("must not have variable type error");
        assert_eq!(
            expanded_dynamic, *expected,
            "dynamic, template={template:?}"
        );
    }
}

#[test]
fn fragmented_write() {
    use core::fmt;

    #[derive(Clone)]
    enum Foo {
        Incomplete1,
        Incomplete2,
        Incomplete3,
    }
    impl fmt::Display for Foo {
        #[inline]
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            use core::fmt::Write;

            f.write_char('%')?;
            f.write_char('c')?;
            f.write_char('e')?;
            f.write_char('%')?;
            f.write_char('b')?;
            f.write_char('1')?;
            f.write_char('%')?;
            match self {
                Foo::Incomplete1 => {}
                Foo::Incomplete2 => {
                    f.write_char('c')?;
                }
                Foo::Incomplete3 => {
                    f.write_char('c')?;
                    f.write_char('e')?;
                }
            }
            Ok(())
        }
    }

    #[derive(Clone)]
    struct MyContext {
        incomplete1: Foo,
        incomplete2: Foo,
        incomplete3: Foo,
    }
    impl Context for MyContext {
        fn visit<V: Visitor>(&self, visitor: V) -> V::Result {
            let name = visitor.var_name().as_str();
            match name {
                "incomplete1" => visitor.visit_string(&self.incomplete1),
                "incomplete2" => visitor.visit_string(&self.incomplete2),
                "incomplete3" => visitor.visit_string(&self.incomplete3),
                _ => visitor.visit_undefined(),
            }
        }
    }

    let context = MyContext {
        incomplete1: Foo::Incomplete1,
        incomplete2: Foo::Incomplete2,
        incomplete3: Foo::Incomplete3,
    };

    // `&[(template, expected)]`.
    const CASES: &[(&str, &str)] = &[
        ("{incomplete1:1}", "%25"),
        ("{incomplete1:2}", "%25c"),
        ("{incomplete1:3}", "%25ce"),
        ("{incomplete1:4}", "%25ce%25"),
        ("{+incomplete1:1}", "%ce%b1"),
        ("{+incomplete1:2}", "%ce%b1%25"),
        ("{+incomplete2:1}", "%ce%b1"),
        ("{+incomplete2:2}", "%ce%b1%25"),
        ("{+incomplete2:3}", "%ce%b1%25c"),
        ("{+incomplete3:1}", "%ce%b1"),
        ("{+incomplete3:2}", "%ce%b1%ce"),
        ("{+incomplete3:3}", "%ce%b1%ce"),
    ];

    for (template, expected) in CASES {
        let template = UriTemplateStr::new(template).expect("must be valid template");
        let expanded = template
            .expand::<UriSpec, _>(&context)
            .expect("must not have variable type error");
        assert_eq_display!(expanded, *expected, "template={template:?}");
        assert_eq!(expanded.to_string(), *expected, "template={template:?}");

        let expanded_dynamic = template
            .expand_dynamic_to_string::<UriSpec, _>(&mut context.clone())
            .expect("must not have variable type error");
        assert_eq!(
            expanded_dynamic, *expected,
            "dynamic, template={template:?}"
        );
    }
}

#[test]
fn github_issue_39() {
    #[derive(Default)]
    struct MyContext {
        on_expansion_start_called: Cell<bool>,
        on_expansion_end_called: Cell<bool>,
    }
    impl DynamicContext for MyContext {
        fn visit_dynamic<V: Visitor>(&mut self, visitor: V) -> V::Result {
            visitor.visit_undefined()
        }
        fn on_expansion_start(&mut self) {
            self.on_expansion_start_called.set(true);
        }
        fn on_expansion_end(&mut self) {
            self.on_expansion_end_called.set(true);
        }
    }

    let mut dyctx = MyContext::default();
    let template = UriTemplateStr::new("hello/{world}").expect("valid template string");
    let s = template
        .expand_dynamic_to_string::<UriSpec, _>(&mut dyctx)
        .expect("must not have variable type error");
    assert_eq!(s, "hello/");
    assert!(dyctx.on_expansion_start_called.get());
    assert!(dyctx.on_expansion_end_called.get());
}
