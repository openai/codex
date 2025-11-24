use std::{str::FromStr, time::Instant};

use regex::Regex;

macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: once_cell::sync::OnceCell<regex::Regex> = once_cell::sync::OnceCell::new();
        RE.get_or_init(|| regex::Regex::new($re).unwrap())
    }};
}

fn slow() {
    let s = r##"13.28.24.13 - - [10/Mar/2016:19:29:25 +0100] "GET /etc/lib/pChart2/examples/index.php?Action=View&Script=../../../../cnf/db.php HTTP/1.1" 404 151 "-" "HTTP_Request2/2.2.1 (http://pear.php.net/package/http_request2) PHP/5.3.16""##;

    let mut total = 0;
    for _ in 0..1000 {
        let re = Regex::new(
            r##"^(\S+) (\S+) (\S+) \[([^]]+)\] "([^"]*)" (\d+) (\d+) "([^"]*)" "([^"]*)"$"##,
        )
        .unwrap();
        let size = usize::from_str(re.captures(s).unwrap().get(7).unwrap().as_str()).unwrap();
        total += size;
    }
    println!("{}", total);
}

fn fast() {
    let s = r##"13.28.24.13 - - [10/Mar/2016:19:29:25 +0100] "GET /etc/lib/pChart2/examples/index.php?Action=View&Script=../../../../cnf/db.php HTTP/1.1" 404 151 "-" "HTTP_Request2/2.2.1 (http://pear.php.net/package/http_request2) PHP/5.3.16""##;

    let mut total = 0;
    for _ in 0..1000 {
        let re: &Regex = regex!(
            r##"^(\S+) (\S+) (\S+) \[([^]]+)\] "([^"]*)" (\d+) (\d+) "([^"]*)" "([^"]*)"$"##,
        );
        let size = usize::from_str(re.captures(s).unwrap().get(7).unwrap().as_str()).unwrap();
        total += size;
    }
    println!("{}", total);
}

fn main() {
    let t = Instant::now();
    slow();
    println!("slow: {:?}", t.elapsed());

    let t = Instant::now();
    fast();
    println!("fast: {:?}", t.elapsed());
}
