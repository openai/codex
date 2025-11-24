use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use sentry_types::{protocol, Auth, Dsn};

#[test]
fn test_auth_parsing() {
    let auth: Auth = "Sentry sentry_timestamp=1328055286.5, \
                      sentry_client=raven-python/42, \
                      sentry_version=6, \
                      sentry_key=public, \
                      sentry_secret=secret"
        .parse()
        .unwrap();
    assert_eq!(
        auth.timestamp(),
        SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs_f64(1328055286.5))
    );
    assert_eq!(auth.client_agent(), Some("raven-python/42"));
    assert_eq!(auth.version(), 6);
    assert_eq!(auth.public_key(), "public");
    assert_eq!(auth.secret_key(), Some("secret"));

    assert_eq!(
        auth.to_string(),
        "Sentry sentry_key=public, \
         sentry_version=6, \
         sentry_timestamp=1328055286.5, \
         sentry_client=raven-python/42, \
         sentry_secret=secret"
    );
}

#[test]
fn test_auth_float_parsing() {
    let auth: Auth = "Sentry sentry_version=2.0, \
                      sentry_key=public"
        .parse()
        .unwrap();
    assert_eq!(auth.version(), 2);
    assert_eq!(auth.public_key(), "public");

    assert_eq!(
        auth.to_string(),
        "Sentry sentry_key=public, \
         sentry_version=2"
    );
}
#[test]
fn test_auth_from_iterator() {
    let mut cont = HashMap::new();
    cont.insert("sentry_version", "7");
    cont.insert("sentry_client", "raven-js/3.23.3");
    cont.insert("sentry_key", "4bb5d94de752a36b8b87851a3f82726a");

    let auth = Auth::from_pairs(cont).unwrap();
    assert_eq!(auth.timestamp(), None);
    assert_eq!(auth.client_agent(), Some("raven-js/3.23.3"));
    assert_eq!(auth.version(), 7);
    assert_eq!(auth.public_key(), "4bb5d94de752a36b8b87851a3f82726a");
    assert_eq!(auth.secret_key(), None);
}

#[test]
fn test_auth_from_querystring() {
    let auth = Auth::from_querystring(b"sentry_version=7&sentry_client=raven-js/3.23.3&sentry_key=4bb5d94de752a36b8b87851a3f82726a").unwrap();

    assert_eq!(auth.timestamp(), None);
    assert_eq!(auth.client_agent(), Some("raven-js/3.23.3"));
    assert_eq!(auth.version(), 7);
    assert_eq!(auth.public_key(), "4bb5d94de752a36b8b87851a3f82726a");
    assert_eq!(auth.secret_key(), None);
}

#[test]
fn test_auth_to_dsn() {
    let url = "https://username:password@domain:8888/23";
    let dsn = url.parse::<Dsn>().unwrap();
    let auth = dsn.to_auth(Some("sentry-rust/1.0"));
    assert_eq!(auth.client_agent(), Some("sentry-rust/1.0"));
    assert_eq!(auth.version(), protocol::LATEST);
    assert_eq!(auth.public_key(), "username");
    assert_eq!(auth.secret_key(), Some("password"));
}

#[test]
fn test_auth_to_json() {
    let mut cont = HashMap::new();
    cont.insert("sentry_version", "7");
    cont.insert("sentry_client", "raven-js/3.23.3");
    cont.insert("sentry_key", "4bb5d94de752a36b8b87851a3f82726a");

    let auth = Auth::from_pairs(cont).unwrap();
    assert_eq!(
        serde_json::to_string(&auth).expect("could not serialize").as_str(),
        "{\"sentry_client\":\"raven-js/3.23.3\",\"sentry_version\":7,\"sentry_key\":\"4bb5d94de752a36b8b87851a3f82726a\",\"sentry_secret\":null}"
    );
}

#[test]
fn test_auth_from_json() {
    let json = "{\"sentry_client\":\"raven-js/3.23.3\",\"sentry_version\":7,\"sentry_key\":\"4bb5d94de752a36b8b87851a3f82726a\"}";
    let auth: Auth = serde_json::from_str(json).expect("could not deserialize");

    assert_eq!(auth.timestamp(), None);
    assert_eq!(auth.client_agent(), Some("raven-js/3.23.3"));
    assert_eq!(auth.version(), 7);
    assert_eq!(auth.public_key(), "4bb5d94de752a36b8b87851a3f82726a");
    assert_eq!(auth.secret_key(), None);
}

#[test]
fn test_auth_string_timestamp() {
    let auth = Auth::from_querystring(b"sentry_version=7&sentry_client=raven-clj&sentry_key=4bb5d94de752a36b8b87851a3f82726a&sentry_timestamp=2019-12-13 12:02:58.94").unwrap();
    assert_eq!(auth.timestamp(), None);
}
