use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn call() -> impl IntoIterator<Item = &'static str> {
    [
        "/user/repos",
        "/repos/rust-lang/rust/stargazers",
        "/orgs/rust-lang/public_members/nikomatsakis",
        "/repos/rust-lang/rust/releases/1.51.0",
    ]
}

fn compare_routers(c: &mut Criterion) {
    let mut group = c.benchmark_group("Compare Routers");

    let mut matchit = matchit::Router::new();
    for route in register!(brackets) {
        matchit.insert(route, true).unwrap();
    }
    group.bench_function("matchit", |b| {
        b.iter(|| {
            for route in black_box(call()) {
                black_box(matchit.at(route).unwrap());
            }
        });
    });

    let mut path_tree = path_tree::PathTree::new();
    for route in register!(colon) {
        path_tree.insert(route, true);
    }
    group.bench_function("path-tree", |b| {
        b.iter(|| {
            for route in black_box(call()) {
                black_box(path_tree.find(route).unwrap());
            }
        });
    });

    let gonzales = gonzales::RouterBuilder::new().build(register!(brackets));
    group.bench_function("gonzales", |b| {
        b.iter(|| {
            for route in black_box(call()) {
                black_box(gonzales.route(route).unwrap());
            }
        });
    });

    let mut actix = actix_router::Router::<bool>::build();
    for route in register!(brackets) {
        actix.path(route, true);
    }
    let actix = actix.finish();
    group.bench_function("actix", |b| {
        b.iter(|| {
            for route in black_box(call()) {
                let mut path = actix_router::Path::new(route);
                black_box(actix.recognize(&mut path).unwrap());
            }
        });
    });

    let regex_set = regex::RegexSet::new(register!(regex)).unwrap();
    group.bench_function("regex", |b| {
        b.iter(|| {
            for route in black_box(call()) {
                black_box(regex_set.matches(route));
            }
        });
    });

    let mut route_recognizer = route_recognizer::Router::new();
    for route in register!(colon) {
        route_recognizer.add(route, true);
    }
    group.bench_function("route-recognizer", |b| {
        b.iter(|| {
            for route in black_box(call()) {
                black_box(route_recognizer.recognize(route).unwrap());
            }
        });
    });

    let mut routefinder = routefinder::Router::new();
    for route in register!(colon) {
        routefinder.add(route, true).unwrap();
    }
    group.bench_function("routefinder", |b| {
        b.iter(|| {
            for route in black_box(call()) {
                black_box(routefinder.best_match(route).unwrap());
            }
        });
    });

    group.finish();
}

criterion_group!(benches, compare_routers);
criterion_main!(benches);

macro_rules! register {
    (colon) => {{
        register!(finish => ":p1", ":p2", ":p3", ":p4")
    }};
    (brackets) => {{
        register!(finish => "{p1}", "{p2}", "{p3}", "{p4}")
    }};
    (regex) => {{
        register!(finish => "(.*)", "(.*)", "(.*)", "(.*)")
    }};
    (finish => $p1:literal, $p2:literal, $p3:literal, $p4:literal) => {{
        [
            concat!("/authorizations"),
            concat!("/authorizations/", $p1),
            concat!("/applications/", $p1, "/tokens/", $p2),
            concat!("/events"),
            concat!("/repos/", $p1, "/", $p2, "/events"),
            concat!("/networks/", $p1, "/", $p2, "/events"),
            concat!("/orgs/", $p1, "/events"),
            concat!("/users/", $p1, "/received_events"),
            concat!("/users/", $p1, "/received_events/public"),
            concat!("/users/", $p1, "/events"),
            concat!("/users/", $p1, "/events/public"),
            concat!("/users/", $p1, "/events/orgs/", $p2),
            concat!("/feeds"),
            concat!("/notifications"),
            concat!("/repos/", $p1, "/", $p2, "/notifications"),
            concat!("/notifications/threads/", $p1),
            concat!("/notifications/threads/", $p1, "/subscription"),
            concat!("/repos/", $p1, "/", $p2, "/stargazers"),
            concat!("/users/", $p1, "/starred"),
            concat!("/user/starred"),
            concat!("/user/starred/", $p1, "/", $p2),
            concat!("/repos/", $p1, "/", $p2, "/subscribers"),
            concat!("/users/", $p1, "/subscriptions"),
            concat!("/user/subscriptions"),
            concat!("/repos/", $p1, "/", $p2, "/subscription"),
            concat!("/user/subscriptions/", $p1, "/", $p2),
            concat!("/users/", $p1, "/gists"),
            concat!("/gists"),
            concat!("/gists/", $p1),
            concat!("/gists/", $p1, "/star"),
            concat!("/repos/", $p1, "/", $p2, "/git/blobs/", $p3),
            concat!("/repos/", $p1, "/", $p2, "/git/commits/", $p3),
            concat!("/repos/", $p1, "/", $p2, "/git/refs"),
            concat!("/repos/", $p1, "/", $p2, "/git/tags/", $p3),
            concat!("/repos/", $p1, "/", $p2, "/git/trees/", $p3),
            concat!("/issues"),
            concat!("/user/issues"),
            concat!("/orgs/", $p1, "/issues"),
            concat!("/repos/", $p1, "/", $p2, "/issues"),
            concat!("/repos/", $p1, "/", $p2, "/issues/", $p3),
            concat!("/repos/", $p1, "/", $p2, "/assignees"),
            concat!("/repos/", $p1, "/", $p2, "/assignees/", $p3),
            concat!("/repos/", $p1, "/", $p2, "/issues/", $p3, "/comments"),
            concat!("/repos/", $p1, "/", $p2, "/issues/", $p3, "/events"),
            concat!("/repos/", $p1, "/", $p2, "/labels"),
            concat!("/repos/", $p1, "/", $p2, "/labels/", $p3),
            concat!("/repos/", $p1, "/", $p2, "/issues/", $p3, "/labels"),
            concat!("/repos/", $p1, "/", $p2, "/milestones/", $p3, "/labels"),
            concat!("/repos/", $p1, "/", $p2, "/milestones/"),
            concat!("/repos/", $p1, "/", $p2, "/milestones/", $p3),
            concat!("/emojis"),
            concat!("/gitignore/templates"),
            concat!("/gitignore/templates/", $p1),
            concat!("/meta"),
            concat!("/rate_limit"),
            concat!("/users/", $p1, "/orgs"),
            concat!("/user/orgs"),
            concat!("/orgs/", $p1),
            concat!("/orgs/", $p1, "/members"),
            concat!("/orgs/", $p1, "/members", $p2),
            concat!("/orgs/", $p1, "/public_members"),
            concat!("/orgs/", $p1, "/public_members/", $p2),
            concat!("/orgs/", $p1, "/teams"),
            concat!("/teams/", $p1),
            concat!("/teams/", $p1, "/members"),
            concat!("/teams/", $p1, "/members", $p2),
            concat!("/teams/", $p1, "/repos"),
            concat!("/teams/", $p1, "/repos/", $p2, "/", $p3),
            concat!("/user/teams"),
            concat!("/repos/", $p1, "/", $p2, "/pulls"),
            concat!("/repos/", $p1, "/", $p2, "/pulls/", $p3),
            concat!("/repos/", $p1, "/", $p2, "/pulls/", $p3, "/commits"),
            concat!("/repos/", $p1, "/", $p2, "/pulls/", $p3, "/files"),
            concat!("/repos/", $p1, "/", $p2, "/pulls/", $p3, "/merge"),
            concat!("/repos/", $p1, "/", $p2, "/pulls/", $p3, "/comments"),
            concat!("/user/repos"),
            concat!("/users/", $p1, "/repos"),
            concat!("/orgs/", $p1, "/repos"),
            concat!("/repositories"),
            concat!("/repos/", $p1, "/", $p2),
            concat!("/repos/", $p1, "/", $p2, "/contributors"),
            concat!("/repos/", $p1, "/", $p2, "/languages"),
            concat!("/repos/", $p1, "/", $p2, "/teams"),
            concat!("/repos/", $p1, "/", $p2, "/tags"),
            concat!("/repos/", $p1, "/", $p2, "/branches"),
            concat!("/repos/", $p1, "/", $p2, "/branches/", $p3),
            concat!("/repos/", $p1, "/", $p2, "/collaborators"),
            concat!("/repos/", $p1, "/", $p2, "/collaborators/", $p3),
            concat!("/repos/", $p1, "/", $p2, "/comments"),
            concat!("/repos/", $p1, "/", $p2, "/commits/", $p3, "/comments"),
            concat!("/repos/", $p1, "/", $p2, "/commits"),
            concat!("/repos/", $p1, "/", $p2, "/commits/", $p3),
            concat!("/repos/", $p1, "/", $p2, "/readme"),
            concat!("/repos/", $p1, "/", $p2, "/keys"),
            concat!("/repos/", $p1, "/", $p2, "/keys", $p3),
            concat!("/repos/", $p1, "/", $p2, "/downloads"),
            concat!("/repos/", $p1, "/", $p2, "/downloads", $p3),
            concat!("/repos/", $p1, "/", $p2, "/forks"),
            concat!("/repos/", $p1, "/", $p2, "/hooks"),
            concat!("/repos/", $p1, "/", $p2, "/hooks", $p3),
            concat!("/repos/", $p1, "/", $p2, "/releases"),
            concat!("/repos/", $p1, "/", $p2, "/releases/", $p3),
            concat!("/repos/", $p1, "/", $p2, "/releases/", $p3, "/assets"),
            concat!("/repos/", $p1, "/", $p2, "/stats/contributors"),
            concat!("/repos/", $p1, "/", $p2, "/stats/commit_activity"),
            concat!("/repos/", $p1, "/", $p2, "/stats/code_frequency"),
            concat!("/repos/", $p1, "/", $p2, "/stats/participation"),
            concat!("/repos/", $p1, "/", $p2, "/stats/punch_card"),
            concat!("/repos/", $p1, "/", $p2, "/statuses/", $p3),
            concat!("/search/repositories"),
            concat!("/search/code"),
            concat!("/search/issues"),
            concat!("/search/users"),
            concat!("/legacy/issues/search/", $p1, "/", $p2, "/", $p3, "/", $p4),
            concat!("/legacy/repos/search/", $p1),
            concat!("/legacy/user/search/", $p1),
            concat!("/legacy/user/email/", $p1),
            concat!("/users/", $p1),
            concat!("/user"),
            concat!("/users"),
            concat!("/user/emails"),
            concat!("/users/", $p1, "/followers"),
            concat!("/user/followers"),
            concat!("/users/", $p1, "/following"),
            concat!("/user/following"),
            concat!("/user/following/", $p1),
            concat!("/users/", $p1, "/following", $p2),
            concat!("/users/", $p1, "/keys"),
            concat!("/user/keys"),
            concat!("/user/keys/", $p1),
        ]
    }};
}

use register;
