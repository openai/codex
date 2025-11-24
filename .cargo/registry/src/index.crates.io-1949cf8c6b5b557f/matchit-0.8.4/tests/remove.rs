use matchit::Router;

struct RemoveTest {
    routes: Vec<&'static str>,
    ops: Vec<(Operation, &'static str, Option<&'static str>)>,
    remaining: Vec<&'static str>,
}

enum Operation {
    Insert,
    Remove,
}

use Operation::*;

impl RemoveTest {
    fn run(self) {
        let mut router = Router::new();

        for route in self.routes.iter() {
            assert_eq!(router.insert(*route, route.to_owned()), Ok(()), "{route}");
        }

        for (op, route, expected) in self.ops.iter() {
            match op {
                Insert => {
                    assert_eq!(router.insert(*route, route), Ok(()), "{route}")
                }
                Remove => {
                    assert_eq!(router.remove(*route), *expected, "removing {route}",)
                }
            }
        }

        for route in self.remaining {
            assert!(matches!(router.at(route), Ok(_)), "remaining {route}");
        }
    }
}

#[test]
fn normalized() {
    RemoveTest {
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
        ops: vec![
            (Remove, "/x/{foo}/bar", Some("/x/{foo}/bar")),
            (Remove, "/x/{bar}/baz", Some("/x/{bar}/baz")),
            (Remove, "/{foo}/{baz}/bax", Some("/{foo}/{baz}/bax")),
            (Remove, "/{foo}/{bar}/baz", Some("/{foo}/{bar}/baz")),
            (
                Remove,
                "/{fod}/{baz}/{bax}/foo",
                Some("/{fod}/{baz}/{bax}/foo"),
            ),
            (Remove, "/{fod}/baz/bax/foo", Some("/{fod}/baz/bax/foo")),
            (Remove, "/{foo}/baz/bax", Some("/{foo}/baz/bax")),
            (Remove, "/{bar}/{bay}/bay", Some("/{bar}/{bay}/bay")),
            (Remove, "/s", Some("/s")),
            (Remove, "/s/s", Some("/s/s")),
            (Remove, "/s/s/s", Some("/s/s/s")),
            (Remove, "/s/s/s/s", Some("/s/s/s/s")),
            (Remove, "/s/s/{s}/x", Some("/s/s/{s}/x")),
            (Remove, "/s/s/{y}/d", Some("/s/s/{y}/d")),
        ],
        remaining: vec![],
    }
    .run();
}

#[test]
fn test() {
    RemoveTest {
        routes: vec!["/home", "/home/{id}"],
        ops: vec![
            (Remove, "/home", Some("/home")),
            (Remove, "/home", None),
            (Remove, "/home/{id}", Some("/home/{id}")),
            (Remove, "/home/{id}", None),
        ],
        remaining: vec![],
    }
    .run();
}

#[test]
fn blog() {
    RemoveTest {
        routes: vec![
            "/{page}",
            "/posts/{year}/{month}/{post}",
            "/posts/{year}/{month}/index",
            "/posts/{year}/top",
            "/static/{*path}",
            "/favicon.ico",
        ],
        ops: vec![
            (Remove, "/{page}", Some("/{page}")),
            (
                Remove,
                "/posts/{year}/{month}/{post}",
                Some("/posts/{year}/{month}/{post}"),
            ),
            (
                Remove,
                "/posts/{year}/{month}/index",
                Some("/posts/{year}/{month}/index"),
            ),
            (Remove, "/posts/{year}/top", Some("/posts/{year}/top")),
            (Remove, "/static/{*path}", Some("/static/{*path}")),
            (Remove, "/favicon.ico", Some("/favicon.ico")),
        ],
        remaining: vec![],
    }
    .run()
}

#[test]
fn catchall() {
    RemoveTest {
        routes: vec!["/foo/{*catchall}", "/bar", "/bar/", "/bar/{*catchall}"],
        ops: vec![
            (Remove, "/foo/{*catchall}", Some("/foo/{*catchall}")),
            (Remove, "/bar/", Some("/bar/")),
            (Insert, "/foo/*catchall", Some("/foo/*catchall")),
            (Remove, "/bar/{*catchall}", Some("/bar/{*catchall}")),
        ],
        remaining: vec!["/bar", "/foo/*catchall"],
    }
    .run();
}

#[test]
fn overlapping_routes() {
    RemoveTest {
        routes: vec![
            "/home",
            "/home/{id}",
            "/users",
            "/users/{id}",
            "/users/{id}/posts",
            "/users/{id}/posts/{post_id}",
            "/articles",
            "/articles/{category}",
            "/articles/{category}/{id}",
        ],
        ops: vec![
            (Remove, "/home", Some("/home")),
            (Insert, "/home", Some("/home")),
            (Remove, "/home/{id}", Some("/home/{id}")),
            (Insert, "/home/{id}", Some("/home/{id}")),
            (Remove, "/users", Some("/users")),
            (Insert, "/users", Some("/users")),
            (Remove, "/users/{id}", Some("/users/{id}")),
            (Insert, "/users/{id}", Some("/users/{id}")),
            (Remove, "/users/{id}/posts", Some("/users/{id}/posts")),
            (Insert, "/users/{id}/posts", Some("/users/{id}/posts")),
            (
                Remove,
                "/users/{id}/posts/{post_id}",
                Some("/users/{id}/posts/{post_id}"),
            ),
            (
                Insert,
                "/users/{id}/posts/{post_id}",
                Some("/users/{id}/posts/{post_id}"),
            ),
            (Remove, "/articles", Some("/articles")),
            (Insert, "/articles", Some("/articles")),
            (Remove, "/articles/{category}", Some("/articles/{category}")),
            (Insert, "/articles/{category}", Some("/articles/{category}")),
            (
                Remove,
                "/articles/{category}/{id}",
                Some("/articles/{category}/{id}"),
            ),
            (
                Insert,
                "/articles/{category}/{id}",
                Some("/articles/{category}/{id}"),
            ),
        ],
        remaining: vec![
            "/home",
            "/home/{id}",
            "/users",
            "/users/{id}",
            "/users/{id}/posts",
            "/users/{id}/posts/{post_id}",
            "/articles",
            "/articles/{category}",
            "/articles/{category}/{id}",
        ],
    }
    .run();
}

#[test]
fn trailing_slash() {
    RemoveTest {
        routes: vec!["/{home}/", "/foo"],
        ops: vec![
            (Remove, "/", None),
            (Remove, "/{home}", None),
            (Remove, "/foo/", None),
            (Remove, "/foo", Some("/foo")),
            (Remove, "/{home}", None),
            (Remove, "/{home}/", Some("/{home}/")),
        ],
        remaining: vec![],
    }
    .run();
}

#[test]
fn remove_root() {
    RemoveTest {
        routes: vec!["/"],
        ops: vec![(Remove, "/", Some("/"))],
        remaining: vec![],
    }
    .run();
}

#[test]
fn check_escaped_params() {
    RemoveTest {
        routes: vec![
            "/foo/{id}",
            "/foo/{id}/bar",
            "/bar/{user}/{id}",
            "/bar/{user}/{id}/baz",
            "/baz/{product}/{user}/{id}",
        ],
        ops: vec![
            (Remove, "/foo/{a}", None),
            (Remove, "/foo/{a}/bar", None),
            (Remove, "/bar/{a}/{b}", None),
            (Remove, "/bar/{a}/{b}/baz", None),
            (Remove, "/baz/{a}/{b}/{c}", None),
        ],
        remaining: vec![
            "/foo/{id}",
            "/foo/{id}/bar",
            "/bar/{user}/{id}",
            "/bar/{user}/{id}/baz",
            "/baz/{product}/{user}/{id}",
        ],
    }
    .run();
}
