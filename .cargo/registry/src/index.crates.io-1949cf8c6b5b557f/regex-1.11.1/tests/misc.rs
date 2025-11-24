use regex::Regex;

macro_rules! regex {
    ($pattern:expr) => {
        regex::Regex::new($pattern).unwrap()
    };
}

#[test]
fn unclosed_group_error() {
    let err = Regex::new(r"(").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("unclosed group"), "error message: {:?}", msg);
}

#[test]
fn regex_string() {
    assert_eq!(r"[a-zA-Z0-9]+", regex!(r"[a-zA-Z0-9]+").as_str());
    assert_eq!(r"[a-zA-Z0-9]+", &format!("{}", regex!(r"[a-zA-Z0-9]+")));
    assert_eq!(
        r#"Regex("[a-zA-Z0-9]+")"#,
        &format!("{:?}", regex!(r"[a-zA-Z0-9]+"))
    );
}

#[test]
fn capture_names() {
    let re = regex!(r"(.)(?P<a>.)");
    assert_eq!(3, re.captures_len());
    assert_eq!((3, Some(3)), re.capture_names().size_hint());
    assert_eq!(
        vec![None, None, Some("a")],
        re.capture_names().collect::<Vec<_>>()
    );
}

#[test]
fn capture_index() {
    let re = regex!(r"^(?P<name>.+)$");
    let cap = re.captures("abc").unwrap();
    assert_eq!(&cap[0], "abc");
    assert_eq!(&cap[1], "abc");
    assert_eq!(&cap["name"], "abc");
}

#[test]
#[should_panic]
fn capture_index_panic_usize() {
    let re = regex!(r"^(?P<name>.+)$");
    let cap = re.captures("abc").unwrap();
    let _ = cap[2];
}

#[test]
#[should_panic]
fn capture_index_panic_name() {
    let re = regex!(r"^(?P<name>.+)$");
    let cap = re.captures("abc").unwrap();
    let _ = cap["bad name"];
}

#[test]
fn capture_index_lifetime() {
    // This is a test of whether the types on `caps["..."]` are general
    // enough. If not, this will fail to typecheck.
    fn inner(s: &str) -> usize {
        let re = regex!(r"(?P<number>[0-9]+)");
        let caps = re.captures(s).unwrap();
        caps["number"].len()
    }
    assert_eq!(3, inner("123"));
}

#[test]
fn capture_misc() {
    let re = regex!(r"(.)(?P<a>a)?(.)(?P<b>.)");
    let cap = re.captures("abc").unwrap();

    assert_eq!(5, cap.len());

    assert_eq!((0, 3), {
        let m = cap.get(0).unwrap();
        (m.start(), m.end())
    });
    assert_eq!(None, cap.get(2));
    assert_eq!((2, 3), {
        let m = cap.get(4).unwrap();
        (m.start(), m.end())
    });

    assert_eq!("abc", cap.get(0).unwrap().as_str());
    assert_eq!(None, cap.get(2));
    assert_eq!("c", cap.get(4).unwrap().as_str());

    assert_eq!(None, cap.name("a"));
    assert_eq!("c", cap.name("b").unwrap().as_str());
}

#[test]
fn sub_capture_matches() {
    let re = regex!(r"([a-z])(([a-z])|([0-9]))");
    let cap = re.captures("a5").unwrap();
    let subs: Vec<_> = cap.iter().collect();

    assert_eq!(5, subs.len());
    assert!(subs[0].is_some());
    assert!(subs[1].is_some());
    assert!(subs[2].is_some());
    assert!(subs[3].is_none());
    assert!(subs[4].is_some());

    assert_eq!("a5", subs[0].unwrap().as_str());
    assert_eq!("a", subs[1].unwrap().as_str());
    assert_eq!("5", subs[2].unwrap().as_str());
    assert_eq!("5", subs[4].unwrap().as_str());
}

// Test that the DFA can handle pathological cases. (This should result in the
// DFA's cache being flushed too frequently, which should cause it to quit and
// fall back to the NFA algorithm.)
#[test]
fn dfa_handles_pathological_case() {
    fn ones_and_zeroes(count: usize) -> String {
        let mut s = String::new();
        for i in 0..count {
            if i % 3 == 0 {
                s.push('1');
            } else {
                s.push('0');
            }
        }
        s
    }

    let re = regex!(r"[01]*1[01]{20}$");
    let text = {
        let mut pieces = ones_and_zeroes(100_000);
        pieces.push('1');
        pieces.push_str(&ones_and_zeroes(20));
        pieces
    };
    assert!(re.is_match(&text));
}
