// Copyright 2013-2014 The rust-url developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Data-driven tests imported from web-platform-tests

use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Write;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use std::sync::Mutex;
use url::Url;

// https://rustwasm.github.io/wasm-bindgen/wasm-bindgen-test/usage.html
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::{console_log, wasm_bindgen_test, wasm_bindgen_test_configure};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
wasm_bindgen_test_configure!(run_in_browser);

// wpt has its own test driver, but we shoe-horn this into wasm_bindgen_test
// which will discard stdout and stderr. So, we make println! go to
// console.log(), so we see failures that do not result in panics.

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
static PRINT_BUF: Mutex<Option<String>> = Mutex::new(None);

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
macro_rules! print {
    ($($arg:tt)*) => {
        let v = format!($($arg)*);
        {
            let mut buf = PRINT_BUF.lock().unwrap();
            if let Some(buf) = buf.as_mut() {
                buf.push_str(&v);
            } else {
                *buf = Some(v);
            }
        }
    };
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
macro_rules! println {
    () => {
        let buf = PRINT_BUF.lock().unwrap().take();
        match buf {
            Some(buf) => console_log!("{buf}"),
            None => console_log!(""),
        }
    };
    ($($arg:tt)*) => {
        let buf = PRINT_BUF.lock().unwrap().take();
        match buf {
            Some(buf) => {
                let v = format!($($arg)*);
                console_log!("{buf}{v}");
            },
            None => console_log!($($arg)*),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct UrlTest {
    input: String,
    base: Option<String>,
    #[serde(flatten)]
    result: UrlTestResult,
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
enum UrlTestResult {
    Ok(UrlTestOk),
    Fail(UrlTestFail),
}

#[derive(Debug, serde::Deserialize)]
struct UrlTestOk {
    href: String,
    protocol: String,
    username: String,
    password: String,
    host: String,
    hostname: String,
    port: String,
    pathname: String,
    search: String,
    hash: String,
}

#[derive(Debug, serde::Deserialize)]
struct UrlTestFail {
    failure: bool,
}

#[derive(Debug, serde::Deserialize)]
struct SetterTest {
    href: String,
    new_value: String,
    expected: SetterTestExpected,
}

#[derive(Debug, serde::Deserialize)]
struct SetterTestExpected {
    href: Option<String>,
    protocol: Option<String>,
    username: Option<String>,
    password: Option<String>,
    host: Option<String>,
    hostname: Option<String>,
    port: Option<String>,
    pathname: Option<String>,
    search: Option<String>,
    hash: Option<String>,
}

#[cfg_attr(all(target_arch = "wasm32", target_os = "unknown"), wasm_bindgen_test)]
fn main() {
    let mut filter = None;
    let mut args = std::env::args().skip(1);
    while filter.is_none() {
        if let Some(arg) = args.next() {
            if arg == "--test-threads" {
                args.next();
                continue;
            }
            filter = Some(arg);
        } else {
            break;
        }
    }

    let mut expected_failures = include_str!("expected_failures.txt")
        .lines()
        .collect::<Vec<_>>();

    let mut errors = vec![];

    // Copied from https://github.com/web-platform-tests/wpt/blob/master/url/
    let url_json: Vec<Value> = serde_json::from_str(include_str!("urltestdata.json"))
        .expect("JSON parse error in urltestdata.json");
    let url_tests = url_json
        .into_iter()
        .filter(|val| val.is_object())
        .map(|val| serde_json::from_value::<UrlTest>(val).expect("parsing failed"))
        .collect::<Vec<_>>();

    let setter_json: HashMap<String, Value> =
        serde_json::from_str(include_str!("setters_tests.json"))
            .expect("JSON parse error in setters_tests.json");
    let setter_tests = setter_json
        .into_iter()
        .filter(|(k, _)| k != "comment")
        .map(|(k, v)| {
            let test = serde_json::from_value::<Vec<SetterTest>>(v).expect("parsing failed");
            (k, test)
        })
        .collect::<HashMap<_, _>>();

    for url_test in url_tests {
        let mut name = format!("<{}>", url_test.input.escape_default());
        if let Some(base) = &url_test.base {
            write!(&mut name, " against <{}>", base.escape_default()).unwrap();
        }
        if should_skip(&name, filter.as_deref()) {
            continue;
        }
        print!("{} ... ", name);

        let res = run_url_test(url_test);
        report(name, res, &mut errors, &mut expected_failures);
    }

    for (kind, tests) in setter_tests {
        for test in tests {
            let name = format!(
                "<{}> set {} to <{}>",
                test.href.escape_default(),
                kind,
                test.new_value.escape_default()
            );
            if should_skip(&name, filter.as_deref()) {
                continue;
            }

            print!("{} ... ", name);

            let res = run_setter_test(&kind, test);
            report(name, res, &mut errors, &mut expected_failures);
        }
    }

    println!();
    println!("====================");
    println!();

    if !errors.is_empty() {
        println!("errors:");
        println!();

        for (name, err) in errors {
            println!("  name: {}", name);
            println!("  err:  {}", err);
            println!();
        }

        std::process::exit(1);
    } else {
        println!("all tests passed");
    }

    if !expected_failures.is_empty() && filter.is_none() {
        println!();
        println!("====================");
        println!();
        println!("tests were expected to fail but did not run:");
        println!();

        for name in expected_failures {
            println!("  {}", name);
        }

        println!();
        println!("if these tests were removed, update expected_failures.txt");
        println!();

        std::process::exit(1);
    }
}

fn should_skip(name: &str, filter: Option<&str>) -> bool {
    match filter {
        Some(filter) => !name.contains(filter),
        None => false,
    }
}

fn report(
    name: String,
    res: Result<(), String>,
    errors: &mut Vec<(String, String)>,
    expected_failures: &mut Vec<&str>,
) {
    let expected_failure = expected_failures.contains(&&*name);
    expected_failures.retain(|&s| s != &*name);
    match res {
        Ok(()) => {
            if expected_failure {
                println!("🟠 (unexpected success)");
                errors.push((name, "unexpected success".to_string()));
            } else {
                println!("✅");
            }
        }
        Err(err) => {
            if expected_failure {
                println!("✅ (expected fail)");
            } else {
                println!("❌");
                errors.push((name, err));
            }
        }
    }
}

fn run_url_test(
    UrlTest {
        base,
        input,
        result,
    }: UrlTest,
) -> Result<(), String> {
    let base = match base {
        Some(base) => {
            let base =
                Url::parse(&base).map_err(|e| format!("errored while parsing base: {}", e))?;
            Some(base)
        }
        None => None,
    };

    let res = Url::options()
        .base_url(base.as_ref())
        .parse(&input)
        .map_err(|e| format!("errored while parsing input: {}", e));

    match result {
        UrlTestResult::Ok(ok) => check_url_ok(res, ok),
        UrlTestResult::Fail(fail) => {
            assert!(fail.failure);
            if res.is_ok() {
                return Err("expected failure, but parsed successfully".to_string());
            }

            Ok(())
        }
    }
}

fn check_url_ok(res: Result<Url, String>, ok: UrlTestOk) -> Result<(), String> {
    let url = match res {
        Ok(url) => url,
        Err(err) => {
            return Err(format!("expected success, but errored: {:?}", err));
        }
    };

    let href = url::quirks::href(&url);
    if href != ok.href {
        return Err(format!("expected href {:?}, but got {:?}", ok.href, href));
    }

    let protocol = url::quirks::protocol(&url);
    if protocol != ok.protocol {
        return Err(format!(
            "expected protocol {:?}, but got {:?}",
            ok.protocol, protocol
        ));
    }

    let username = url::quirks::username(&url);
    if username != ok.username {
        return Err(format!(
            "expected username {:?}, but got {:?}",
            ok.username, username
        ));
    }

    let password = url::quirks::password(&url);
    if password != ok.password {
        return Err(format!(
            "expected password {:?}, but got {:?}",
            ok.password, password
        ));
    }

    let host = url::quirks::host(&url);
    if host != ok.host {
        return Err(format!("expected host {:?}, but got {:?}", ok.host, host));
    }

    let hostname = url::quirks::hostname(&url);
    if hostname != ok.hostname {
        return Err(format!(
            "expected hostname {:?}, but got {:?}",
            ok.hostname, hostname
        ));
    }

    let port = url::quirks::port(&url);
    if port != ok.port {
        return Err(format!("expected port {:?}, but got {:?}", ok.port, port));
    }

    let pathname = url::quirks::pathname(&url);
    if pathname != ok.pathname {
        return Err(format!(
            "expected pathname {:?}, but got {:?}",
            ok.pathname, pathname
        ));
    }

    let search = url::quirks::search(&url);
    if search != ok.search {
        return Err(format!(
            "expected search {:?}, but got {:?}",
            ok.search, search
        ));
    }

    let hash = url::quirks::hash(&url);
    if hash != ok.hash {
        return Err(format!("expected hash {:?}, but got {:?}", ok.hash, hash));
    }

    Ok(())
}

fn run_setter_test(
    kind: &str,
    SetterTest {
        href,
        new_value,
        expected,
    }: SetterTest,
) -> Result<(), String> {
    let mut url = Url::parse(&href).map_err(|e| format!("errored while parsing href: {}", e))?;

    match kind {
        "protocol" => {
            url::quirks::set_protocol(&mut url, &new_value).ok();
        }
        "username" => {
            url::quirks::set_username(&mut url, &new_value).ok();
        }
        "password" => {
            url::quirks::set_password(&mut url, &new_value).ok();
        }
        "host" => {
            url::quirks::set_host(&mut url, &new_value).ok();
        }
        "hostname" => {
            url::quirks::set_hostname(&mut url, &new_value).ok();
        }
        "port" => {
            url::quirks::set_port(&mut url, &new_value).ok();
        }
        "pathname" => url::quirks::set_pathname(&mut url, &new_value),
        "search" => url::quirks::set_search(&mut url, &new_value),
        "hash" => url::quirks::set_hash(&mut url, &new_value),
        _ => {
            return Err(format!("unknown setter kind: {:?}", kind));
        }
    }

    if let Some(expected_href) = expected.href {
        let href = url::quirks::href(&url);
        if href != expected_href {
            return Err(format!(
                "expected href {:?}, but got {:?}",
                expected_href, href
            ));
        }
    }

    if let Some(expected_protocol) = expected.protocol {
        let protocol = url::quirks::protocol(&url);
        if protocol != expected_protocol {
            return Err(format!(
                "expected protocol {:?}, but got {:?}",
                expected_protocol, protocol
            ));
        }
    }

    if let Some(expected_username) = expected.username {
        let username = url::quirks::username(&url);
        if username != expected_username {
            return Err(format!(
                "expected username {:?}, but got {:?}",
                expected_username, username
            ));
        }
    }

    if let Some(expected_password) = expected.password {
        let password = url::quirks::password(&url);
        if password != expected_password {
            return Err(format!(
                "expected password {:?}, but got {:?}",
                expected_password, password
            ));
        }
    }

    if let Some(expected_host) = expected.host {
        let host = url::quirks::host(&url);
        if host != expected_host {
            return Err(format!(
                "expected host {:?}, but got {:?}",
                expected_host, host
            ));
        }
    }

    if let Some(expected_hostname) = expected.hostname {
        let hostname = url::quirks::hostname(&url);
        if hostname != expected_hostname {
            return Err(format!(
                "expected hostname {:?}, but got {:?}",
                expected_hostname, hostname
            ));
        }
    }

    if let Some(expected_port) = expected.port {
        let port = url::quirks::port(&url);
        if port != expected_port {
            return Err(format!(
                "expected port {:?}, but got {:?}",
                expected_port, port
            ));
        }
    }

    if let Some(expected_pathname) = expected.pathname {
        let pathname = url::quirks::pathname(&url);
        if pathname != expected_pathname {
            return Err(format!(
                "expected pathname {:?}, but got {:?}",
                expected_pathname, pathname
            ));
        }
    }

    if let Some(expected_search) = expected.search {
        let search = url::quirks::search(&url);
        if search != expected_search {
            return Err(format!(
                "expected search {:?}, but got {:?}",
                expected_search, search
            ));
        }
    }

    if let Some(expected_hash) = expected.hash {
        let hash = url::quirks::hash(&url);
        if hash != expected_hash {
            return Err(format!(
                "expected hash {:?}, but got {:?}",
                expected_hash, hash
            ));
        }
    }

    Ok(())
}
