use matchit::{MatchError, Router};

// https://github.com/ibraheemdev/matchit/issues/22
#[test]
fn partial_overlap() {
    let mut x = Router::new();
    x.insert("/foo_bar", "Welcome!").unwrap();
    x.insert("/foo/bar", "Welcome!").unwrap();
    assert_eq!(x.at("/foo/").unwrap_err(), MatchError::NotFound);

    let mut x = Router::new();
    x.insert("/foo", "Welcome!").unwrap();
    x.insert("/foo/bar", "Welcome!").unwrap();
    assert_eq!(x.at("/foo/").unwrap_err(), MatchError::NotFound);
}

// https://github.com/ibraheemdev/matchit/issues/31
#[test]
fn wildcard_overlap() {
    let mut router = Router::new();
    router.insert("/path/foo", "foo").unwrap();
    router.insert("/path/{*rest}", "wildcard").unwrap();

    assert_eq!(router.at("/path/foo").map(|m| *m.value), Ok("foo"));
    assert_eq!(router.at("/path/bar").map(|m| *m.value), Ok("wildcard"));
    assert_eq!(router.at("/path/foo/").map(|m| *m.value), Ok("wildcard"));

    let mut router = Router::new();
    router.insert("/path/foo/{arg}", "foo").unwrap();
    router.insert("/path/{*rest}", "wildcard").unwrap();

    assert_eq!(router.at("/path/foo/myarg").map(|m| *m.value), Ok("foo"));
    assert_eq!(
        router.at("/path/foo/myarg/").map(|m| *m.value),
        Ok("wildcard")
    );
    assert_eq!(
        router.at("/path/foo/myarg/bar/baz").map(|m| *m.value),
        Ok("wildcard")
    );
}

// https://github.com/ibraheemdev/matchit/issues/12
#[test]
fn overlapping_param_backtracking() {
    let mut matcher = Router::new();

    matcher.insert("/{object}/{id}", "object with id").unwrap();
    matcher
        .insert("/secret/{id}/path", "secret with id and path")
        .unwrap();

    let matched = matcher.at("/secret/978/path").unwrap();
    assert_eq!(matched.params.get("id"), Some("978"));

    let matched = matcher.at("/something/978").unwrap();
    assert_eq!(matched.params.get("id"), Some("978"));
    assert_eq!(matched.params.get("object"), Some("something"));

    let matched = matcher.at("/secret/978").unwrap();
    assert_eq!(matched.params.get("id"), Some("978"));
}

struct MatchTest {
    routes: Vec<&'static str>,
    matches: Vec<(
        &'static str,
        &'static str,
        Result<Vec<(&'static str, &'static str)>, ()>,
    )>,
}

impl MatchTest {
    fn run(self) {
        let mut router = Router::new();

        for route in self.routes {
            assert_eq!(router.insert(route, route.to_owned()), Ok(()), "{route}");
        }

        router.check_priorities().unwrap();

        for (path, route, params) in self.matches {
            match router.at(path) {
                Ok(x) => {
                    assert_eq!(x.value, route);

                    let got = x.params.iter().collect::<Vec<_>>();
                    assert_eq!(params.unwrap(), got);

                    router.at_mut(path).unwrap().value.push_str("Z");
                    assert!(router.at(path).unwrap().value.contains("Z"));
                    router.at_mut(path).unwrap().value.pop();
                }
                Err(_) => params.unwrap_err(),
            }
        }
    }
}

macro_rules! p {
    ($($k:expr => $v:expr),* $(,)?) => {
        Ok(vec![$(($k, $v)),*])
    };
}

// https://github.com/ibraheemdev/matchit/issues/42
#[test]
fn bare_catchall() {
    MatchTest {
        routes: vec!["{*foo}", "foo/{*bar}"],
        matches: vec![
            ("x/y", "{*foo}", p! { "foo" => "x/y" }),
            ("/x/y", "{*foo}", p! { "foo" => "/x/y" }),
            ("/foo/x/y", "{*foo}", p! { "foo" => "/foo/x/y" }),
            ("foo/x/y", "foo/{*bar}", p! { "bar" => "x/y" }),
        ],
    }
    .run()
}

#[test]
fn normalized() {
    MatchTest {
        routes: vec![
            "/x/{foo}/bar",
            "/x/{bar}/baz",
            "/{foo}/{baz}/bax",
            "/{foo}/{bar}/baz",
            "/{fod}/{baz}/{bax}/foo",
            "/{fod}/baz/bax/foo",
            "/{foo}/baz/bax",
            "/{bar}/{bay}/bay",
            "/s",
            "/s/s",
            "/s/s/s",
            "/s/s/s/s",
            "/s/s/{s}/x",
            "/s/s/{y}/d",
        ],
        matches: vec![
            ("/x/foo/bar", "/x/{foo}/bar", p! { "foo" => "foo" }),
            ("/x/foo/baz", "/x/{bar}/baz", p! { "bar" => "foo" }),
            (
                "/y/foo/baz",
                "/{foo}/{bar}/baz",
                p! { "foo" => "y", "bar" => "foo" },
            ),
            (
                "/y/foo/bax",
                "/{foo}/{baz}/bax",
                p! { "foo" => "y", "baz" => "foo" },
            ),
            (
                "/y/baz/baz",
                "/{foo}/{bar}/baz",
                p! { "foo" => "y", "bar" => "baz" },
            ),
            ("/y/baz/bax/foo", "/{fod}/baz/bax/foo", p! { "fod" => "y" }),
            (
                "/y/baz/b/foo",
                "/{fod}/{baz}/{bax}/foo",
                p! { "fod" => "y", "baz" => "baz", "bax" => "b" },
            ),
            ("/y/baz/bax", "/{foo}/baz/bax", p! { "foo" => "y" }),
            (
                "/z/bar/bay",
                "/{bar}/{bay}/bay",
                p! { "bar" => "z", "bay" => "bar" },
            ),
            ("/s", "/s", p! {}),
            ("/s/s", "/s/s", p! {}),
            ("/s/s/s", "/s/s/s", p! {}),
            ("/s/s/s/s", "/s/s/s/s", p! {}),
            ("/s/s/s/x", "/s/s/{s}/x", p! { "s" => "s" }),
            ("/s/s/s/d", "/s/s/{y}/d", p! { "y" => "s" }),
        ],
    }
    .run()
}

#[test]
fn blog() {
    MatchTest {
        routes: vec![
            "/{page}",
            "/posts/{year}/{month}/{post}",
            "/posts/{year}/{month}/index",
            "/posts/{year}/top",
            "/static/{*path}",
            "/favicon.ico",
        ],
        matches: vec![
            ("/about", "/{page}", p! { "page" => "about" }),
            (
                "/posts/2021/01/rust",
                "/posts/{year}/{month}/{post}",
                p! { "year" => "2021", "month" => "01", "post" => "rust" },
            ),
            (
                "/posts/2021/01/index",
                "/posts/{year}/{month}/index",
                p! { "year" => "2021", "month" => "01" },
            ),
            (
                "/posts/2021/top",
                "/posts/{year}/top",
                p! { "year" => "2021" },
            ),
            (
                "/static/foo.png",
                "/static/{*path}",
                p! { "path" => "foo.png" },
            ),
            ("/favicon.ico", "/favicon.ico", p! {}),
        ],
    }
    .run()
}

#[test]
fn double_overlap() {
    MatchTest {
        routes: vec![
            "/{object}/{id}",
            "/secret/{id}/path",
            "/secret/978",
            "/other/{object}/{id}/",
            "/other/an_object/{id}",
            "/other/static/path",
            "/other/long/static/path/",
        ],
        matches: vec![
            (
                "/secret/978/path",
                "/secret/{id}/path",
                p! { "id" => "978" },
            ),
            (
                "/some_object/978",
                "/{object}/{id}",
                p! { "object" => "some_object", "id" => "978" },
            ),
            ("/secret/978", "/secret/978", p! {}),
            ("/super_secret/978/", "/{object}/{id}", Err(())),
            (
                "/other/object/1/",
                "/other/{object}/{id}/",
                p! { "object" => "object", "id" => "1" },
            ),
            ("/other/object/1/2", "/other/{object}/{id}", Err(())),
            (
                "/other/an_object/1",
                "/other/an_object/{id}",
                p! { "id" => "1" },
            ),
            ("/other/static/path", "/other/static/path", p! {}),
            (
                "/other/long/static/path/",
                "/other/long/static/path/",
                p! {},
            ),
        ],
    }
    .run()
}

#[test]
fn catchall_off_by_one() {
    MatchTest {
        routes: vec!["/foo/{*catchall}", "/bar", "/bar/", "/bar/{*catchall}"],
        matches: vec![
            ("/foo", "", Err(())),
            ("/foo/", "", Err(())),
            ("/foo/x", "/foo/{*catchall}", p! { "catchall" => "x" }),
            ("/bar", "/bar", p! {}),
            ("/bar/", "/bar/", p! {}),
            ("/bar/x", "/bar/{*catchall}", p! { "catchall" => "x" }),
        ],
    }
    .run()
}

#[test]
fn overlap() {
    MatchTest {
        routes: vec![
            "/foo",
            "/bar",
            "/{*bar}",
            "/baz",
            "/baz/",
            "/baz/x",
            "/baz/{xxx}",
            "/",
            "/xxx/{*x}",
            "/xxx/",
        ],
        matches: vec![
            ("/foo", "/foo", p! {}),
            ("/bar", "/bar", p! {}),
            ("/baz", "/baz", p! {}),
            ("/baz/", "/baz/", p! {}),
            ("/baz/x", "/baz/x", p! {}),
            ("/???", "/{*bar}", p! { "bar" => "???" }),
            ("/", "/", p! {}),
            ("", "", Err(())),
            ("/xxx/y", "/xxx/{*x}", p! { "x" => "y" }),
            ("/xxx/", "/xxx/", p! {}),
            ("/xxx", "/{*bar}", p! { "bar" => "xxx" }),
        ],
    }
    .run()
}

#[test]
fn missing_trailing_slash_param() {
    MatchTest {
        routes: vec!["/foo/{object}/{id}", "/foo/bar/baz", "/foo/secret/978/"],
        matches: vec![
            ("/foo/secret/978/", "/foo/secret/978/", p! {}),
            (
                "/foo/secret/978",
                "/foo/{object}/{id}",
                p! { "object" => "secret", "id" => "978" },
            ),
        ],
    }
    .run()
}

#[test]
fn extra_trailing_slash_param() {
    MatchTest {
        routes: vec!["/foo/{object}/{id}", "/foo/bar/baz", "/foo/secret/978"],
        matches: vec![
            ("/foo/secret/978/", "", Err(())),
            ("/foo/secret/978", "/foo/secret/978", p! {}),
        ],
    }
    .run()
}

#[test]
fn missing_trailing_slash_catch_all() {
    MatchTest {
        routes: vec!["/foo/{*bar}", "/foo/bar/baz", "/foo/secret/978/"],
        matches: vec![
            (
                "/foo/secret/978",
                "/foo/{*bar}",
                p! { "bar" => "secret/978" },
            ),
            ("/foo/secret/978/", "/foo/secret/978/", p! {}),
        ],
    }
    .run()
}

#[test]
fn extra_trailing_slash_catch_all() {
    MatchTest {
        routes: vec!["/foo/{*bar}", "/foo/bar/baz", "/foo/secret/978"],
        matches: vec![
            (
                "/foo/secret/978/",
                "/foo/{*bar}",
                p! { "bar" => "secret/978/" },
            ),
            ("/foo/secret/978", "/foo/secret/978", p! {}),
        ],
    }
    .run()
}

#[test]
fn double_overlap_trailing_slash() {
    MatchTest {
        routes: vec![
            "/{object}/{id}",
            "/secret/{id}/path",
            "/secret/978/",
            "/other/{object}/{id}/",
            "/other/an_object/{id}",
            "/other/static/path",
            "/other/long/static/path/",
        ],
        matches: vec![
            ("/secret/978/path/", "", Err(())),
            ("/object/id/", "", Err(())),
            ("/object/id/path", "", Err(())),
            ("/other/object/1", "", Err(())),
            ("/other/object/1/2", "", Err(())),
            (
                "/other/an_object/1/",
                "/other/{object}/{id}/",
                p! { "object" => "an_object", "id" => "1" },
            ),
            (
                "/other/static/path/",
                "/other/{object}/{id}/",
                p! { "object" => "static", "id" => "path" },
            ),
            ("/other/long/static/path", "", Err(())),
            ("/other/object/static/path", "", Err(())),
        ],
    }
    .run()
}

#[test]
fn trailing_slash_overlap() {
    MatchTest {
        routes: vec!["/foo/{x}/baz/", "/foo/{x}/baz", "/foo/bar/bar"],
        matches: vec![
            ("/foo/x/baz/", "/foo/{x}/baz/", p! { "x" => "x" }),
            ("/foo/x/baz", "/foo/{x}/baz", p! { "x" => "x" }),
            ("/foo/bar/bar", "/foo/bar/bar", p! {}),
        ],
    }
    .run()
}

#[test]
fn trailing_slash() {
    MatchTest {
        routes: vec![
            "/hi",
            "/b/",
            "/search/{query}",
            "/cmd/{tool}/",
            "/src/{*filepath}",
            "/x",
            "/x/y",
            "/y/",
            "/y/z",
            "/0/{id}",
            "/0/{id}/1",
            "/1/{id}/",
            "/1/{id}/2",
            "/aa",
            "/a/",
            "/admin",
            "/admin/static",
            "/admin/{category}",
            "/admin/{category}/{page}",
            "/doc",
            "/doc/rust_faq.html",
            "/doc/rust1.26.html",
            "/no/a",
            "/no/b",
            "/no/a/b/{*other}",
            "/api/{page}/{name}",
            "/api/hello/{name}/bar/",
            "/api/bar/{name}",
            "/api/baz/foo",
            "/api/baz/foo/bar",
            "/foo/{p}",
        ],
        matches: vec![
            ("/hi/", "", Err(())),
            ("/b", "", Err(())),
            ("/search/rustacean/", "", Err(())),
            ("/cmd/vet", "", Err(())),
            ("/src", "", Err(())),
            ("/src/", "", Err(())),
            ("/x/", "", Err(())),
            ("/y", "", Err(())),
            ("/0/rust/", "", Err(())),
            ("/1/rust", "", Err(())),
            ("/a", "", Err(())),
            ("/admin/", "", Err(())),
            ("/doc/", "", Err(())),
            ("/admin/static/", "", Err(())),
            ("/admin/cfg/", "", Err(())),
            ("/admin/cfg/users/", "", Err(())),
            ("/api/hello/x/bar", "", Err(())),
            ("/api/baz/foo/", "", Err(())),
            ("/api/baz/bax/", "", Err(())),
            ("/api/bar/huh/", "", Err(())),
            ("/api/baz/foo/bar/", "", Err(())),
            ("/api/world/abc/", "", Err(())),
            ("/foo/pp/", "", Err(())),
            ("/", "", Err(())),
            ("/no", "", Err(())),
            ("/no/", "", Err(())),
            ("/no/a/b", "", Err(())),
            ("/no/a/b/", "", Err(())),
            ("/_", "", Err(())),
            ("/_/", "", Err(())),
            ("/api", "", Err(())),
            ("/api/", "", Err(())),
            ("/api/hello/x/foo", "", Err(())),
            ("/api/baz/foo/bad", "", Err(())),
            ("/foo/p/p", "", Err(())),
        ],
    }
    .run()
}

#[test]
fn backtracking_trailing_slash() {
    MatchTest {
        routes: vec!["/a/{b}/{c}", "/a/b/{c}/d/"],
        matches: vec![("/a/b/c/d", "", Err(()))],
    }
    .run()
}

#[test]
fn root_trailing_slash() {
    MatchTest {
        routes: vec!["/foo", "/bar", "/{baz}"],
        matches: vec![("/", "", Err(()))],
    }
    .run()
}

#[test]
fn catchall_overlap() {
    MatchTest {
        routes: vec!["/yyy/{*x}", "/yyy{*x}"],
        matches: vec![
            ("/yyy/y", "/yyy/{*x}", p! { "x" => "y" }),
            ("/yyy/", "/yyy{*x}", p! { "x" => "/" }),
        ],
    }
    .run();
}

#[test]
fn escaped() {
    MatchTest {
        routes: vec![
            "/",
            "/{{",
            "/}}",
            "/{{x",
            "/}}y{{",
            "/xy{{",
            "/{{/xyz",
            "/{ba{{r}",
            "/{ba{{r}/",
            "/{ba{{r}/x",
            "/baz/{xxx}",
            "/baz/{xxx}/xy{{",
            "/baz/{xxx}/}}xy{{{{",
            "/{{/{x}",
            "/xxx/",
            "/xxx/{x}}{{}}}}{{}}{{{{}}y}",
        ],
        matches: vec![
            ("/", "/", p! {}),
            ("/{", "/{{", p! {}),
            ("/}", "/}}", p! {}),
            ("/{x", "/{{x", p! {}),
            ("/}y{", "/}}y{{", p! {}),
            ("/xy{", "/xy{{", p! {}),
            ("/{/xyz", "/{{/xyz", p! {}),
            ("/foo", "/{ba{{r}", p! { "ba{r" => "foo" }),
            ("/{{", "/{ba{{r}", p! { "ba{r" => "{{" }),
            ("/{{}}/", "/{ba{{r}/", p! { "ba{r" => "{{}}" }),
            ("/{{}}{{/x", "/{ba{{r}/x", p! { "ba{r" => "{{}}{{" }),
            ("/baz/x", "/baz/{xxx}", p! { "xxx" => "x" }),
            ("/baz/x/xy{", "/baz/{xxx}/xy{{", p! { "xxx" => "x" }),
            ("/baz/x/xy{{", "", Err(())),
            ("/baz/x/}xy{{", "/baz/{xxx}/}}xy{{{{", p! { "xxx" => "x" }),
            ("/{/{{", "/{{/{x}", p! { "x" => "{{" }),
            ("/xxx", "/{ba{{r}", p! { "ba{r" => "xxx" }),
            ("/xxx/", "/xxx/", p!()),
            (
                "/xxx/foo",
                "/xxx/{x}}{{}}}}{{}}{{{{}}y}",
                p! { "x}{}}{}{{}y" => "foo" },
            ),
        ],
    }
    .run()
}

#[test]
fn basic() {
    MatchTest {
        routes: vec![
            "/hi",
            "/contact",
            "/co",
            "/c",
            "/a",
            "/ab",
            "/doc/",
            "/doc/rust_faq.html",
            "/doc/rust1.26.html",
            "/ʯ",
            "/β",
            "/sd!here",
            "/sd$here",
            "/sd&here",
            "/sd'here",
            "/sd(here",
            "/sd)here",
            "/sd+here",
            "/sd,here",
            "/sd;here",
            "/sd=here",
        ],
        matches: vec![
            ("/a", "/a", p! {}),
            ("", "/", Err(())),
            ("/hi", "/hi", p! {}),
            ("/contact", "/contact", p! {}),
            ("/co", "/co", p! {}),
            ("", "/con", Err(())),
            ("", "/cona", Err(())),
            ("", "/no", Err(())),
            ("/ab", "/ab", p! {}),
            ("/ʯ", "/ʯ", p! {}),
            ("/β", "/β", p! {}),
            ("/sd!here", "/sd!here", p! {}),
            ("/sd$here", "/sd$here", p! {}),
            ("/sd&here", "/sd&here", p! {}),
            ("/sd'here", "/sd'here", p! {}),
            ("/sd(here", "/sd(here", p! {}),
            ("/sd)here", "/sd)here", p! {}),
            ("/sd+here", "/sd+here", p! {}),
            ("/sd,here", "/sd,here", p! {}),
            ("/sd;here", "/sd;here", p! {}),
            ("/sd=here", "/sd=here", p! {}),
        ],
    }
    .run()
}

#[test]
fn wildcard() {
    MatchTest {
        routes: vec![
            "/",
            "/cmd/{tool}/",
            "/cmd/{tool2}/{sub}",
            "/cmd/whoami",
            "/cmd/whoami/root",
            "/cmd/whoami/root/",
            "/src",
            "/src/",
            "/src/{*filepath}",
            "/search/",
            "/search/{query}",
            "/search/actix-web",
            "/search/google",
            "/user_{name}",
            "/user_{name}/about",
            "/files/{dir}/{*filepath}",
            "/doc/",
            "/doc/rust_faq.html",
            "/doc/rust1.26.html",
            "/info/{user}/public",
            "/info/{user}/project/{project}",
            "/info/{user}/project/rustlang",
            "/aa/{*xx}",
            "/ab/{*xx}",
            "/ab/hello{*xx}",
            "/{cc}",
            "/c1/{dd}/e",
            "/c1/{dd}/e1",
            "/{cc}/cc",
            "/{cc}/{dd}/ee",
            "/{cc}/{dd}/{ee}/ff",
            "/{cc}/{dd}/{ee}/{ff}/gg",
            "/{cc}/{dd}/{ee}/{ff}/{gg}/hh",
            "/get/test/abc/",
            "/get/{param}/abc/",
            "/something/{paramname}/thirdthing",
            "/something/secondthing/test",
            "/get/abc",
            "/get/{param}",
            "/get/abc/123abc",
            "/get/abc/{param}",
            "/get/abc/123abc/xxx8",
            "/get/abc/123abc/{param}",
            "/get/abc/123abc/xxx8/1234",
            "/get/abc/123abc/xxx8/{param}",
            "/get/abc/123abc/xxx8/1234/ffas",
            "/get/abc/123abc/xxx8/1234/{param}",
            "/get/abc/123abc/xxx8/1234/kkdd/12c",
            "/get/abc/123abc/xxx8/1234/kkdd/{param}",
            "/get/abc/{param}/test",
            "/get/abc/123abd/{param}",
            "/get/abc/123abddd/{param}",
            "/get/abc/123/{param}",
            "/get/abc/123abg/{param}",
            "/get/abc/123abf/{param}",
            "/get/abc/123abfff/{param}",
        ],
        matches: vec![
            ("/", "/", p! {}),
            ("/cmd/test", "/cmd/{tool}/", Err(())),
            ("/cmd/test/", "/cmd/{tool}/", p! { "tool" => "test" }),
            (
                "/cmd/test/3",
                "/cmd/{tool2}/{sub}",
                p! { "tool2" => "test", "sub" => "3" },
            ),
            ("/cmd/who", "/cmd/{tool}/", Err(())),
            ("/cmd/who/", "/cmd/{tool}/", p! { "tool" => "who" }),
            ("/cmd/whoami", "/cmd/whoami", p! {}),
            ("/cmd/whoami/", "/cmd/{tool}/", p! { "tool" => "whoami" }),
            (
                "/cmd/whoami/r",
                "/cmd/{tool2}/{sub}",
                p! { "tool2" => "whoami", "sub" => "r" },
            ),
            ("/cmd/whoami/r/", "/cmd/{tool}/{sub}", Err(())),
            ("/cmd/whoami/root", "/cmd/whoami/root", p! {}),
            ("/cmd/whoami/root/", "/cmd/whoami/root/", p! {}),
            ("/src", "/src", p! {}),
            ("/src/", "/src/", p! {}),
            (
                "/src/some/file.png",
                "/src/{*filepath}",
                p! { "filepath" => "some/file.png" },
            ),
            ("/search/", "/search/", p! {}),
            (
                "/search/actix",
                "/search/{query}",
                p! { "query" => "actix" },
            ),
            ("/search/actix-web", "/search/actix-web", p! {}),
            (
                "/search/someth!ng+in+ünìcodé",
                "/search/{query}",
                p! { "query" => "someth!ng+in+ünìcodé" },
            ),
            ("/search/someth!ng+in+ünìcodé/", "", Err(())),
            (
                "/user_rustacean",
                "/user_{name}",
                p! { "name" => "rustacean" },
            ),
            (
                "/user_rustacean/about",
                "/user_{name}/about",
                p! { "name" => "rustacean" },
            ),
            (
                "/files/js/inc/framework.js",
                "/files/{dir}/{*filepath}",
                p! { "dir" => "js", "filepath" => "inc/framework.js" },
            ),
            (
                "/info/gordon/public",
                "/info/{user}/public",
                p! { "user" => "gordon" },
            ),
            (
                "/info/gordon/project/rust",
                "/info/{user}/project/{project}",
                p! { "user" => "gordon", "project" => "rust" },
            ),
            (
                "/info/gordon/project/rustlang",
                "/info/{user}/project/rustlang",
                p! { "user" => "gordon" },
            ),
            ("/aa/", "/", Err(())),
            ("/aa/aa", "/aa/{*xx}", p! { "xx" => "aa" }),
            ("/ab/ab", "/ab/{*xx}", p! { "xx" => "ab" }),
            ("/ab/hello-world", "/ab/hello{*xx}", p! { "xx" => "-world" }),
            ("/a", "/{cc}", p! { "cc" => "a" }),
            ("/all", "/{cc}", p! { "cc" => "all" }),
            ("/d", "/{cc}", p! { "cc" => "d" }),
            ("/ad", "/{cc}", p! { "cc" => "ad" }),
            ("/dd", "/{cc}", p! { "cc" => "dd" }),
            ("/dddaa", "/{cc}", p! { "cc" => "dddaa" }),
            ("/aa", "/{cc}", p! { "cc" => "aa" }),
            ("/aaa", "/{cc}", p! { "cc" => "aaa" }),
            ("/aaa/cc", "/{cc}/cc", p! { "cc" => "aaa" }),
            ("/ab", "/{cc}", p! { "cc" => "ab" }),
            ("/abb", "/{cc}", p! { "cc" => "abb" }),
            ("/abb/cc", "/{cc}/cc", p! { "cc" => "abb" }),
            ("/allxxxx", "/{cc}", p! { "cc" => "allxxxx" }),
            ("/alldd", "/{cc}", p! { "cc" => "alldd" }),
            ("/all/cc", "/{cc}/cc", p! { "cc" => "all" }),
            ("/a/cc", "/{cc}/cc", p! { "cc" => "a" }),
            ("/c1/d/e", "/c1/{dd}/e", p! { "dd" => "d" }),
            ("/c1/d/e1", "/c1/{dd}/e1", p! { "dd" => "d" }),
            (
                "/c1/d/ee",
                "/{cc}/{dd}/ee",
                p! { "cc" => "c1", "dd" => "d" },
            ),
            ("/cc/cc", "/{cc}/cc", p! { "cc" => "cc" }),
            ("/ccc/cc", "/{cc}/cc", p! { "cc" => "ccc" }),
            ("/deedwjfs/cc", "/{cc}/cc", p! { "cc" => "deedwjfs" }),
            ("/acllcc/cc", "/{cc}/cc", p! { "cc" => "acllcc" }),
            ("/get/test/abc/", "/get/test/abc/", p! {}),
            ("/get/te/abc/", "/get/{param}/abc/", p! { "param" => "te" }),
            (
                "/get/testaa/abc/",
                "/get/{param}/abc/",
                p! { "param" => "testaa" },
            ),
            ("/get/xx/abc/", "/get/{param}/abc/", p! { "param" => "xx" }),
            ("/get/tt/abc/", "/get/{param}/abc/", p! { "param" => "tt" }),
            ("/get/a/abc/", "/get/{param}/abc/", p! { "param" => "a" }),
            ("/get/t/abc/", "/get/{param}/abc/", p! { "param" => "t" }),
            ("/get/aa/abc/", "/get/{param}/abc/", p! { "param" => "aa" }),
            (
                "/get/abas/abc/",
                "/get/{param}/abc/",
                p! { "param" => "abas" },
            ),
            (
                "/something/secondthing/test",
                "/something/secondthing/test",
                p! {},
            ),
            (
                "/something/abcdad/thirdthing",
                "/something/{paramname}/thirdthing",
                p! { "paramname" => "abcdad" },
            ),
            (
                "/something/secondthingaaaa/thirdthing",
                "/something/{paramname}/thirdthing",
                p! { "paramname" => "secondthingaaaa" },
            ),
            (
                "/something/se/thirdthing",
                "/something/{paramname}/thirdthing",
                p! { "paramname" => "se" },
            ),
            (
                "/something/s/thirdthing",
                "/something/{paramname}/thirdthing",
                p! { "paramname" => "s" },
            ),
            ("/c/d/ee", "/{cc}/{dd}/ee", p! { "cc" => "c", "dd" => "d" }),
            (
                "/c/d/e/ff",
                "/{cc}/{dd}/{ee}/ff",
                p! { "cc" => "c", "dd" => "d", "ee" => "e" },
            ),
            (
                "/c/d/e/f/gg",
                "/{cc}/{dd}/{ee}/{ff}/gg",
                p! { "cc" => "c", "dd" => "d", "ee" => "e", "ff" => "f" },
            ),
            (
                "/c/d/e/f/g/hh",
                "/{cc}/{dd}/{ee}/{ff}/{gg}/hh",
                p! { "cc" => "c", "dd" => "d", "ee" => "e", "ff" => "f", "gg" => "g" },
            ),
            (
                "/cc/dd/ee/ff/gg/hh",
                "/{cc}/{dd}/{ee}/{ff}/{gg}/hh",
                p! { "cc" => "cc", "dd" => "dd", "ee" => "ee", "ff" => "ff", "gg" => "gg" },
            ),
            ("/get/abc", "/get/abc", p! {}),
            ("/get/a", "/get/{param}", p! { "param" => "a" }),
            ("/get/abz", "/get/{param}", p! { "param" => "abz" }),
            ("/get/12a", "/get/{param}", p! { "param" => "12a" }),
            ("/get/abcd", "/get/{param}", p! { "param" => "abcd" }),
            ("/get/abc/123abc", "/get/abc/123abc", p! {}),
            ("/get/abc/12", "/get/abc/{param}", p! { "param" => "12" }),
            (
                "/get/abc/123ab",
                "/get/abc/{param}",
                p! { "param" => "123ab" },
            ),
            ("/get/abc/xyz", "/get/abc/{param}", p! { "param" => "xyz" }),
            (
                "/get/abc/123abcddxx",
                "/get/abc/{param}",
                p! { "param" => "123abcddxx" },
            ),
            ("/get/abc/123abc/xxx8", "/get/abc/123abc/xxx8", p! {}),
            (
                "/get/abc/123abc/x",
                "/get/abc/123abc/{param}",
                p! { "param" => "x" },
            ),
            (
                "/get/abc/123abc/xxx",
                "/get/abc/123abc/{param}",
                p! { "param" => "xxx" },
            ),
            (
                "/get/abc/123abc/abc",
                "/get/abc/123abc/{param}",
                p! { "param" => "abc" },
            ),
            (
                "/get/abc/123abc/xxx8xxas",
                "/get/abc/123abc/{param}",
                p! { "param" => "xxx8xxas" },
            ),
            (
                "/get/abc/123abc/xxx8/1234",
                "/get/abc/123abc/xxx8/1234",
                p! {},
            ),
            (
                "/get/abc/123abc/xxx8/1",
                "/get/abc/123abc/xxx8/{param}",
                p! { "param" => "1" },
            ),
            (
                "/get/abc/123abc/xxx8/123",
                "/get/abc/123abc/xxx8/{param}",
                p! { "param" => "123" },
            ),
            (
                "/get/abc/123abc/xxx8/78k",
                "/get/abc/123abc/xxx8/{param}",
                p! { "param" => "78k" },
            ),
            (
                "/get/abc/123abc/xxx8/1234xxxd",
                "/get/abc/123abc/xxx8/{param}",
                p! { "param" => "1234xxxd" },
            ),
            (
                "/get/abc/123abc/xxx8/1234/ffas",
                "/get/abc/123abc/xxx8/1234/ffas",
                p! {},
            ),
            (
                "/get/abc/123abc/xxx8/1234/f",
                "/get/abc/123abc/xxx8/1234/{param}",
                p! { "param" => "f" },
            ),
            (
                "/get/abc/123abc/xxx8/1234/ffa",
                "/get/abc/123abc/xxx8/1234/{param}",
                p! { "param" => "ffa" },
            ),
            (
                "/get/abc/123abc/xxx8/1234/kka",
                "/get/abc/123abc/xxx8/1234/{param}",
                p! { "param" => "kka" },
            ),
            (
                "/get/abc/123abc/xxx8/1234/ffas321",
                "/get/abc/123abc/xxx8/1234/{param}",
                p! { "param" => "ffas321" },
            ),
            (
                "/get/abc/123abc/xxx8/1234/kkdd/12c",
                "/get/abc/123abc/xxx8/1234/kkdd/12c",
                p! {},
            ),
            (
                "/get/abc/123abc/xxx8/1234/kkdd/1",
                "/get/abc/123abc/xxx8/1234/kkdd/{param}",
                p! { "param" => "1" },
            ),
            (
                "/get/abc/123abc/xxx8/1234/kkdd/12",
                "/get/abc/123abc/xxx8/1234/kkdd/{param}",
                p! { "param" => "12" },
            ),
            (
                "/get/abc/123abc/xxx8/1234/kkdd/12b",
                "/get/abc/123abc/xxx8/1234/kkdd/{param}",
                p! { "param" => "12b" },
            ),
            (
                "/get/abc/123abc/xxx8/1234/kkdd/34",
                "/get/abc/123abc/xxx8/1234/kkdd/{param}",
                p! { "param" => "34" },
            ),
            (
                "/get/abc/123abc/xxx8/1234/kkdd/12c2e3",
                "/get/abc/123abc/xxx8/1234/kkdd/{param}",
                p! { "param" => "12c2e3" },
            ),
            (
                "/get/abc/12/test",
                "/get/abc/{param}/test",
                p! { "param" => "12" },
            ),
            (
                "/get/abc/123abdd/test",
                "/get/abc/{param}/test",
                p! { "param" => "123abdd" },
            ),
            (
                "/get/abc/123abdddf/test",
                "/get/abc/{param}/test",
                p! { "param" => "123abdddf" },
            ),
            (
                "/get/abc/123ab/test",
                "/get/abc/{param}/test",
                p! { "param" => "123ab" },
            ),
            (
                "/get/abc/123abgg/test",
                "/get/abc/{param}/test",
                p! { "param" => "123abgg" },
            ),
            (
                "/get/abc/123abff/test",
                "/get/abc/{param}/test",
                p! { "param" => "123abff" },
            ),
            (
                "/get/abc/123abffff/test",
                "/get/abc/{param}/test",
                p! { "param" => "123abffff" },
            ),
            (
                "/get/abc/123abd/test",
                "/get/abc/123abd/{param}",
                p! { "param" => "test" },
            ),
            (
                "/get/abc/123abddd/test",
                "/get/abc/123abddd/{param}",
                p! { "param" => "test" },
            ),
            (
                "/get/abc/123/test22",
                "/get/abc/123/{param}",
                p! { "param" => "test22" },
            ),
            (
                "/get/abc/123abg/test",
                "/get/abc/123abg/{param}",
                p! { "param" => "test" },
            ),
            (
                "/get/abc/123abf/testss",
                "/get/abc/123abf/{param}",
                p! { "param" => "testss" },
            ),
            (
                "/get/abc/123abfff/te",
                "/get/abc/123abfff/{param}",
                p! { "param" => "te" },
            ),
        ],
    }
    .run()
}
