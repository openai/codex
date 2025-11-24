use matchit::{InsertError, Router};

struct InsertTest(Vec<(&'static str, Result<(), InsertError>)>);

impl InsertTest {
    fn run(self) {
        let mut router = Router::new();
        for (route, expected) in self.0 {
            let got = router.insert(route, route.to_owned());
            assert_eq!(got, expected, "{route}");
        }
    }
}

fn conflict(with: &'static str) -> InsertError {
    InsertError::Conflict { with: with.into() }
}

#[test]
fn wildcard_conflict() {
    InsertTest(vec![
        ("/cmd/{tool}/{sub}", Ok(())),
        ("/cmd/vet", Ok(())),
        ("/foo/bar", Ok(())),
        ("/foo/{name}", Ok(())),
        ("/foo/{names}", Err(conflict("/foo/{name}"))),
        ("/cmd/{*path}", Err(conflict("/cmd/{tool}/{sub}"))),
        ("/cmd/{xxx}/names", Ok(())),
        ("/cmd/{tool}/{xxx}/foo", Ok(())),
        ("/src/{*filepath}", Ok(())),
        ("/src/{file}", Err(conflict("/src/{*filepath}"))),
        ("/src/static.json", Ok(())),
        ("/src/$filepathx", Ok(())),
        ("/src/", Ok(())),
        ("/src/foo/bar", Ok(())),
        ("/src1/", Ok(())),
        ("/src1/{*filepath}", Ok(())),
        ("/src2{*filepath}", Ok(())),
        ("/src2/{*filepath}", Ok(())),
        ("/src2/", Ok(())),
        ("/src2", Ok(())),
        ("/src3", Ok(())),
        ("/src3/{*filepath}", Ok(())),
        ("/search/{query}", Ok(())),
        ("/search/valid", Ok(())),
        ("/user_{name}", Ok(())),
        ("/user_x", Ok(())),
        ("/user_{bar}", Err(conflict("/user_{name}"))),
        ("/id{id}", Ok(())),
        ("/id/{id}", Ok(())),
    ])
    .run()
}

#[test]
fn invalid_catchall() {
    InsertTest(vec![
        ("/non-leading-{*catchall}", Ok(())),
        ("/foo/bar{*catchall}", Ok(())),
        ("/src/{*filepath}/x", Err(InsertError::InvalidCatchAll)),
        ("/src2/", Ok(())),
        ("/src2/{*filepath}/x", Err(InsertError::InvalidCatchAll)),
    ])
    .run()
}

#[test]
fn catchall_root_conflict() {
    InsertTest(vec![("/", Ok(())), ("/{*filepath}", Ok(()))]).run()
}

#[test]
fn child_conflict() {
    InsertTest(vec![
        ("/cmd/vet", Ok(())),
        ("/cmd/{tool}", Ok(())),
        ("/cmd/{tool}/{sub}", Ok(())),
        ("/cmd/{tool}/misc", Ok(())),
        ("/cmd/{tool}/{bad}", Err(conflict("/cmd/{tool}/{sub}"))),
        ("/src/AUTHORS", Ok(())),
        ("/src/{*filepath}", Ok(())),
        ("/user_x", Ok(())),
        ("/user_{name}", Ok(())),
        ("/id/{id}", Ok(())),
        ("/id{id}", Ok(())),
        ("/{id}", Ok(())),
        ("/{*filepath}", Err(conflict("/{id}"))),
    ])
    .run()
}

#[test]
fn duplicates() {
    InsertTest(vec![
        ("/", Ok(())),
        ("/", Err(conflict("/"))),
        ("/doc/", Ok(())),
        ("/doc/", Err(conflict("/doc/"))),
        ("/src/{*filepath}", Ok(())),
        ("/src/{*filepath}", Err(conflict("/src/{*filepath}"))),
        ("/search/{query}", Ok(())),
        ("/search/{query}", Err(conflict("/search/{query}"))),
        ("/user_{name}", Ok(())),
        ("/user_{name}", Err(conflict("/user_{name}"))),
    ])
    .run()
}

#[test]
fn unnamed_param() {
    InsertTest(vec![
        ("/{}", Err(InsertError::InvalidParam)),
        ("/user{}/", Err(InsertError::InvalidParam)),
        ("/cmd/{}/", Err(InsertError::InvalidParam)),
        ("/src/{*}", Err(InsertError::InvalidParam)),
    ])
    .run()
}

#[test]
fn double_params() {
    InsertTest(vec![
        ("/{foo}{bar}", Err(InsertError::InvalidParamSegment)),
        ("/{foo}{bar}/", Err(InsertError::InvalidParamSegment)),
        ("/{foo}{{*bar}/", Err(InsertError::InvalidParamSegment)),
    ])
    .run()
}

#[test]
fn normalized_conflict() {
    InsertTest(vec![
        ("/x/{foo}/bar", Ok(())),
        ("/x/{bar}/bar", Err(conflict("/x/{foo}/bar"))),
        ("/{y}/bar/baz", Ok(())),
        ("/{y}/baz/baz", Ok(())),
        ("/{z}/bar/bat", Ok(())),
        ("/{z}/bar/baz", Err(conflict("/{y}/bar/baz"))),
    ])
    .run()
}

#[test]
fn more_conflicts() {
    InsertTest(vec![
        ("/con{tact}", Ok(())),
        ("/who/are/{*you}", Ok(())),
        ("/who/foo/hello", Ok(())),
        ("/whose/{users}/{name}", Ok(())),
        ("/who/are/foo", Ok(())),
        ("/who/are/foo/bar", Ok(())),
        ("/con{nection}", Err(conflict("/con{tact}"))),
        (
            "/whose/{users}/{user}",
            Err(conflict("/whose/{users}/{name}")),
        ),
    ])
    .run()
}

#[test]
fn catchall_static_overlap() {
    InsertTest(vec![
        ("/bar", Ok(())),
        ("/bar/", Ok(())),
        ("/bar/{*foo}", Ok(())),
    ])
    .run();

    InsertTest(vec![
        ("/foo", Ok(())),
        ("/{*bar}", Ok(())),
        ("/bar", Ok(())),
        ("/baz", Ok(())),
        ("/baz/{split}", Ok(())),
        ("/", Ok(())),
        ("/{*bar}", Err(conflict("/{*bar}"))),
        ("/{*zzz}", Err(conflict("/{*bar}"))),
        ("/{xxx}", Err(conflict("/{*bar}"))),
    ])
    .run();

    InsertTest(vec![
        ("/{*bar}", Ok(())),
        ("/bar", Ok(())),
        ("/bar/x", Ok(())),
        ("/bar_{x}", Ok(())),
        ("/bar_{x}", Err(conflict("/bar_{x}"))),
        ("/bar_{x}/y", Ok(())),
        ("/bar/{x}", Ok(())),
    ])
    .run();
}

#[test]
fn duplicate_conflict() {
    InsertTest(vec![
        ("/hey", Ok(())),
        ("/hey/users", Ok(())),
        ("/hey/user", Ok(())),
        ("/hey/user", Err(conflict("/hey/user"))),
    ])
    .run()
}

#[test]
fn invalid_param() {
    InsertTest(vec![
        ("{", Err(InsertError::InvalidParam)),
        ("}", Err(InsertError::InvalidParam)),
        ("x{y", Err(InsertError::InvalidParam)),
        ("x}", Err(InsertError::InvalidParam)),
        ("/{foo}s", Err(InsertError::InvalidParamSegment)),
    ])
    .run();
}

#[test]
fn escaped_param() {
    InsertTest(vec![
        ("{{", Ok(())),
        ("}}", Ok(())),
        ("xx}}", Ok(())),
        ("}}yy", Ok(())),
        ("}}yy{{}}", Ok(())),
        ("}}yy{{}}{{}}y{{", Ok(())),
        ("}}yy{{}}{{}}y{{", Err(conflict("}yy{}{}y{"))),
        ("/{{yy", Ok(())),
        ("/{yy}", Ok(())),
        ("/foo", Ok(())),
        ("/foo/{{", Ok(())),
        ("/foo/{{/{x}", Ok(())),
        ("/foo/{ba{{r}", Ok(())),
        ("/bar/{ba}}r}", Ok(())),
        ("/xxx/{x{{}}y}", Ok(())),
    ])
    .run()
}

#[test]
fn bare_catchall() {
    InsertTest(vec![("{*foo}", Ok(())), ("foo/{*bar}", Ok(()))]).run()
}
