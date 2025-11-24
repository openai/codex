use std::borrow::Cow;
use std::convert::TryFrom;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

use sentry_types::protocol::v7;

fn event_id() -> Uuid {
    "d43e86c9-6e42-4a93-a4fb-da156dd17341".parse().unwrap()
}

fn event_time() -> SystemTime {
    systime(2017, 12, 24, 8, 12, 0, 0)
}

fn systime(
    year: i32,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    millisecond: u16,
) -> SystemTime {
    let month = time::Month::try_from(month).unwrap();
    let date = time::Date::from_calendar_date(year, month, day).unwrap();
    let date = date
        .with_hms_milli(hour, minute, second, millisecond)
        .unwrap()
        .assume_utc();
    let secs = date.unix_timestamp() as u64;
    let nanos = date.nanosecond();
    let duration = Duration::new(secs, nanos);
    SystemTime::UNIX_EPOCH.checked_add(duration).unwrap()
}

fn reserialize(event: &v7::Event<'_>) -> v7::Event<'static> {
    let json = serde_json::to_string(event).unwrap();
    serde_json::from_str(&json).unwrap()
}

fn assert_roundtrip(event: &v7::Event<'_>) {
    let event_roundtripped = reserialize(event);
    assert_eq!(&event.clone().into_owned(), &event_roundtripped);
}

mod test_event {
    use super::*;

    #[test]
    fn test_event_default_vs_new() {
        let event = reserialize(&v7::Event::new());

        assert!(event.event_id != uuid::Uuid::nil());
        assert_eq!(event.fingerprint, vec!["{{ default }}".to_string()]);
        assert_eq!(event.platform, "other");
        assert_eq!(event.level, v7::Level::Error);
        assert_eq!(event.sdk, None);
    }

    #[test]
    fn test_event_to_string_timestamp() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            ..Default::default()
        };
        assert_eq!(
            event.to_string(),
            "Event(id: d43e86c9-6e42-4a93-a4fb-da156dd17341, ts: 2017-12-24T08:12:00Z)"
        );
    }

    #[test]
    fn test_event_release_and_dist() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            dist: Some("42".into()),
            release: Some("my.awesome.app-1.0".into()),
            environment: Some("prod".into()),
            ..Default::default()
        };

        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"release\":\"my.awesome.app-1.0\",\"dist\":\"42\",\"environment\":\"prod\"}"
        );
    }

    #[test]
    fn test_transaction() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            message: Some("Hello World!".to_string()),
            transaction: Some("bar::foo".to_string()),
            level: v7::Level::Info,
            ..Default::default()
        };
        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"level\":\"info\",\
             \"transaction\":\"bar::foo\",\"message\":\"Hello World!\",\"timestamp\":1514103120}"
        );
    }

    #[test]
    fn test_logger() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            level: v7::Level::Warning,
            message: Some("Hello World!".to_string()),
            logger: Some("root".to_string()),
            ..Default::default()
        };
        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"level\":\"warning\",\"message\":\
             \"Hello World!\",\"logger\":\"root\",\"timestamp\":1514103120}"
        );
    }

    #[test]
    fn test_culprit() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            message: Some("Hello World!".to_string()),
            culprit: Some("foo in bar".to_string()),
            level: v7::Level::Info,
            ..Default::default()
        };
        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"level\":\"info\",\"culprit\":\
             \"foo in bar\",\"message\":\"Hello World!\",\"timestamp\":1514103120}"
        );
    }
}

mod test_fingerprint {
    use super::*;

    #[test]
    fn test_fingerprint_simple() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            fingerprint: Cow::Owned(vec!["{{ default }}".into(), "extra".into()]),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"fingerprint\":[\"{{ default \
             }}\",\"extra\"],\"timestamp\":1514103120}"
        );
    }

    #[test]
    fn test_fingerprint_string() {
        assert_eq!(
            v7::Event {
                event_id: event_id(),
                timestamp: event_time(),
                fingerprint: Cow::Borrowed(&["fingerprint".into()]),
                ..Default::default()
            },
            serde_json::from_str(
                "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"fingerprint\":\
                 [\"fingerprint\"],\"timestamp\":1514103120}"
            )
            .unwrap()
        )
    }

    #[test]
    fn test_fingerprint_empty() {
        assert_eq!(
            v7::Event {
                event_id: event_id(),
                timestamp: event_time(),
                fingerprint: Cow::Borrowed(&[]),
                ..Default::default()
            },
            serde_json::from_str(
                "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"fingerprint\":[],\
                 \"timestamp\":1514103120}"
            )
            .unwrap()
        )
    }
}

mod test_values {
    use super::*;

    #[test]
    fn test_values_object() {
        let values = v7::Values {
            values: vec![1, 2, 3],
        };

        assert_eq!(
            values,
            serde_json::from_str("{\"values\":[1,2,3]}").unwrap()
        );

        assert_eq!(
            serde_json::to_string(&values).unwrap(),
            "{\"values\":[1,2,3]}".to_string()
        );
    }

    #[test]
    fn test_values_option() {
        assert_eq!(
            None,
            serde_json::from_str::<Option<v7::Values<u32>>>("null").unwrap()
        );
    }

    #[test]
    fn test_values_empty() {
        assert!(v7::Values::<u32>::new().is_empty());
        assert!(!v7::Values::from(vec![1, 2, 3]).is_empty())
    }
}

mod test_logentry {
    use super::*;

    #[test]
    fn test_logentry_basics() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            logentry: Some(v7::LogEntry {
                message: "Hello %s!".to_string(),
                params: vec!["World".into()],
            }),
            culprit: Some("foo in bar".to_string()),
            level: v7::Level::Debug,
            ..Default::default()
        };
        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"level\":\"debug\",\"culprit\":\
             \"foo in bar\",\"logentry\":{\"message\":\"Hello \
             %s!\",\"params\":[\"World\"]},\"timestamp\":1514103120}"
        );
    }

    #[test]
    fn test_logentry_no_params() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            logentry: Some(v7::LogEntry {
                message: "Hello World!".to_string(),
                params: vec![],
            }),
            ..Default::default()
        };
        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"logentry\":{\"message\":\"Hello \
             World!\"},\"timestamp\":1514103120}"
        );
    }
}

#[test]
fn test_modules() {
    let event = v7::Event {
        event_id: event_id(),
        timestamp: event_time(),
        modules: {
            let mut m = v7::Map::new();
            m.insert("System".into(), "1.0.0".into());
            m
        },
        ..Default::default()
    };
    assert_roundtrip(&event);
    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"modules\":{\"System\":\"1.0.0\"},\
         \"timestamp\":1514103120}"
    );
}

mod test_timestamp {
    use super::*;

    #[test]
    fn test_timestamp_utc() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120}"
        );

        let event: v7::Event<'_> =
            serde_json::from_slice(b"{\"timestamp\":\"2017-12-24T08:12:00Z\"}").unwrap();
        assert_eq!(event.timestamp, event_time());
    }

    #[test]
    fn test_timestamp_float() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: systime(2017, 12, 24, 8, 12, 0, 500),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120.5}"
        );
    }
}

mod test_user {
    use super::*;

    #[test]
    fn test_user_minimal() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            user: Some(v7::User {
                id: Some("8fd5a33b-5b0e-45b2-aff2-9e4f067756ba".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\"user\":\
             {\"id\":\"8fd5a33b-5b0e-45b2-aff2-9e4f067756ba\"}}"
        );
    }

    #[test]
    fn test_user_full() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            user: Some(v7::User {
                id: Some("8fd5a33b-5b0e-45b2-aff2-9e4f067756ba".into()),
                email: Some("foo@example.invalid".into()),
                ip_address: Some("127.0.0.1".parse().unwrap()),
                username: Some("john-doe".into()),
                other: {
                    let mut hm = v7::Map::new();
                    hm.insert("foo".into(), "bar".into());
                    hm
                },
            }),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\"user\":\
             {\"id\":\"8fd5a33b-5b0e-45b2-aff2-9e4f067756ba\",\"email\":\"foo@example.invalid\",\
             \"ip_address\":\"127.0.0.1\",\"username\":\"john-doe\",\"foo\":\"bar\"}}"
        );
    }

    #[test]
    fn test_user_ip_address_auto() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            user: Some(v7::User {
                ip_address: Some(v7::IpAddress::Auto),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\"user\":\
             {\"ip_address\":\"{{auto}}\"}}"
        );
    }
}

mod test_breadcrumbs {
    use super::*;

    #[test]
    fn test_breadcrumbs_values() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            breadcrumbs: vec![
                v7::Breadcrumb {
                    timestamp: event_time(),
                    category: Some("ui.click".into()),
                    message: Some("span.platform-card > li.platform-tile".into()),
                    ..Default::default()
                },
                v7::Breadcrumb {
                    timestamp: event_time(),
                    ty: "http".into(),
                    category: Some("xhr".into()),
                    data: {
                        let mut m = v7::Map::new();
                        m.insert("url".into(), "/api/0/organizations/foo".into());
                        m.insert("status_code".into(), 200.into());
                        m.insert("method".into(), "GET".into());
                        m
                    },
                    ..Default::default()
                },
            ]
            .into(),
            ..Default::default()
        };
        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"breadcrumbs\":{\"values\":[{\"timestamp\":1514103120,\"category\":\"ui.click\",\
             \"message\":\"span.platform-card > \
             li.platform-tile\"},{\"timestamp\":1514103120,\"type\":\"http\",\"category\":\"xhr\",\
             \"data\":{\"method\":\"GET\",\"status_code\":200,\"url\":\
             \"/api/0/organizations/foo\"}}]}}"
        );
    }
}

mod test_stacktrace {
    use super::*;

    #[test]
    fn test_stacktrace() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            stacktrace: Some(v7::Stacktrace {
                frames: vec![v7::Frame {
                    function: Some("main".into()),
                    filename: Some("hello.py".into()),
                    lineno: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"stacktrace\":{\"frames\":[{\"function\":\"main\",\"filename\":\"hello.py\",\
             \"lineno\":1}]}}"
        );
    }
}

mod test_template_info {
    use super::*;

    #[test]
    fn test_template_info() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            template: Some(v7::TemplateInfo {
                filename: Some("hello.html".into()),
                lineno: Some(1),
                pre_context: vec!["foo1".into(), "bar2".into()],
                context_line: Some("hey hey hey3".into()),
                post_context: vec!["foo4".into(), "bar5".into()],
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"template\":{\"filename\":\"hello.html\",\"lineno\":1,\"pre_context\":[\"foo1\",\
             \"bar2\"],\"context_line\":\"hey hey hey3\",\"post_context\":[\"foo4\",\"bar5\"]}}"
        );
    }
}

mod test_threads {
    use super::*;

    #[test]
    fn test_threads_values() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            threads: vec![v7::Thread {
                id: Some("#1".into()),
                name: Some("Awesome Thread".into()),
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"threads\":{\"values\":[{\"id\":\"#1\",\"name\":\"Awesome Thread\"}]}}"
        );
    }

    #[test]
    fn test_threads_flags() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            threads: vec![v7::Thread {
                id: Some(42.into()),
                name: Some("Awesome Thread".into()),
                crashed: true,
                current: true,
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"threads\":{\"values\":[{\"id\":42,\"name\":\"Awesome \
             Thread\",\"crashed\":true,\"current\":true}]}}"
        );
    }

    #[test]
    fn test_threads_stacktrace() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            threads: vec![v7::Thread {
                stacktrace: Some(v7::Stacktrace {
                    frames: vec![v7::Frame {
                        function: Some("main".into()),
                        filename: Some("hello.py".into()),
                        lineno: Some(1),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                raw_stacktrace: Some(v7::Stacktrace {
                    frames: vec![v7::Frame {
                        function: Some("main".into()),
                        filename: Some("hello.py".into()),
                        lineno: Some(1),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"threads\":{\"values\":[{\"stacktrace\":{\"frames\":[{\"function\":\"main\",\
             \"filename\":\"hello.py\",\"lineno\":1}]},\"raw_stacktrace\":{\"frames\":\
             [{\"function\":\"main\",\"filename\":\"hello.py\",\"lineno\":1}]}}]}}"
        );
    }
}

mod test_request {
    use super::*;

    #[test]
    fn test_request_full() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            request: Some(v7::Request {
                url: "https://www.example.invalid/bar".parse().ok(),
                method: Some("GET".into()),
                data: Some("{}".into()),
                query_string: Some("foo=bar&blub=blah".into()),
                cookies: Some("dummy=42".into()),
                headers: {
                    let mut hm = v7::Map::new();
                    hm.insert("Content-Type".into(), "text/plain".into());
                    hm
                },
                env: {
                    let mut env = v7::Map::new();
                    env.insert("PATH_INFO".into(), "/bar".into());
                    env
                },
            }),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"request\":{\"url\":\"https://www.example.invalid/bar\",\"method\":\"GET\",\"data\":\
             \"{}\",\"query_string\":\"foo=bar&blub=blah\",\"cookies\":\"dummy=42\",\"headers\":\
             {\"Content-Type\":\"text/plain\"},\"env\":{\"PATH_INFO\":\"/bar\"}}}"
        );
    }

    #[test]
    fn test_request_other() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            request: Some(v7::Request {
                url: "https://www.example.invalid/bar".parse().ok(),
                method: Some("GET".into()),
                data: Some("{}".into()),
                query_string: Some("foo=bar&blub=blah".into()),
                cookies: Some("dummy=42".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"request\":{\"url\":\"https://www.example.invalid/bar\",\"method\":\"GET\",\"data\":\
             \"{}\",\"query_string\":\"foo=bar&blub=blah\",\"cookies\":\"dummy=42\"}}"
        );
    }

    #[test]
    fn test_request_defaults() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            request: Some(Default::default()),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"request\":{}}"
        );
    }
}

#[test]
fn test_tags() {
    let event = v7::Event {
        event_id: event_id(),
        timestamp: event_time(),
        tags: {
            let mut m = v7::Map::new();
            m.insert("device_type".into(), "mobile".into());
            m.insert("interpreter".into(), "7".into());
            m
        },
        ..Default::default()
    };

    assert_roundtrip(&event);
    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\"tags\":\
         {\"device_type\":\"mobile\",\"interpreter\":\"7\"}}"
    );
}

#[test]
fn test_extra() {
    let event = v7::Event {
        event_id: event_id(),
        timestamp: event_time(),
        extra: {
            let mut m = v7::Map::new();
            m.insert(
                "component_state".into(),
                serde_json::json!({
                    "dirty": true,
                    "revision": 17
                }),
            );
            m
        },
        ..Default::default()
    };

    assert_roundtrip(&event);
    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\"extra\":\
         {\"component_state\":{\"dirty\":true,\"revision\":17}}}"
    );
}

mod test_debug_meta {
    use super::*;

    #[test]
    fn test_debug_meta() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            debug_meta: Cow::Owned(v7::DebugMeta {
                sdk_info: Some(v7::SystemSdkInfo {
                    sdk_name: "iOS".into(),
                    version_major: 10,
                    version_minor: 3,
                    version_patchlevel: 0,
                }),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"debug_meta\":{\"sdk_info\":{\"sdk_name\":\"iOS\",\"version_major\":10,\
             \"version_minor\":3,\"version_patchlevel\":0}}}"
        );
    }

    #[test]
    fn test_debug_meta_images() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            debug_meta: Cow::Owned(v7::DebugMeta {
                images: vec![
                    v7::AppleDebugImage {
                        name: "CoreFoundation".into(),
                        arch: Some("arm64".into()),
                        cpu_type: Some(1233),
                        cpu_subtype: Some(3),
                        image_addr: 0.into(),
                        image_size: 4096,
                        image_vmaddr: 32768.into(),
                        uuid: "494f3aea-88fa-4296-9644-fa8ef5d139b6".parse().unwrap(),
                    }
                    .into(),
                    v7::SymbolicDebugImage {
                        name: "CoreFoundation".into(),
                        arch: Some("arm64".into()),
                        image_addr: 0.into(),
                        image_size: 4096,
                        image_vmaddr: 32768.into(),
                        id: "494f3aea-88fa-4296-9644-fa8ef5d139b6-1234".parse().unwrap(),
                        code_id: None,
                        debug_file: None,
                    }
                    .into(),
                    v7::ProguardDebugImage {
                        uuid: "8c954262-f905-4992-8a61-f60825f4553b".parse().unwrap(),
                    }
                    .into(),
                ],
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"debug_meta\":{\"images\":[{\"type\":\"apple\",\"name\":\"CoreFoundation\",\"arch\":\
             \"arm64\",\"cpu_type\":1233,\"cpu_subtype\":3,\"image_addr\":\"0x0\",\"image_size\":\
             4096,\"image_vmaddr\":\"0x8000\",\"uuid\":\"494f3aea-88fa-4296-9644-fa8ef5d139b6\"},\
             {\"type\":\"symbolic\",\"name\":\"CoreFoundation\",\"arch\":\"arm64\",\"image_addr\":\
             \"0x0\",\"image_size\":4096,\"image_vmaddr\":\"0x8000\",\"id\":\
             \"494f3aea-88fa-4296-9644-fa8ef5d139b6-1234\"},{\"type\":\"proguard\",\"uuid\":\
             \"8c954262-f905-4992-8a61-f60825f4553b\"}]}}"
        );
    }
}

mod test_exception {
    use super::*;

    #[test]
    fn test_exception_values() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            exception: vec![v7::Exception {
                ty: "ZeroDivisionError".into(),
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        };
        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"exception\":{\"values\":[{\"type\":\"ZeroDivisionError\"}]}}"
        );
    }

    #[test]
    fn test_exception_stacktrace_minimal() {
        let event: v7::Event<'_> = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            exception: vec![v7::Exception {
                ty: "DivisionByZero".into(),
                value: Some("integer division or modulo by zero".into()),
                module: None,
                stacktrace: Some(v7::Stacktrace {
                    frames: vec![v7::Frame {
                        function: Some("main".into()),
                        filename: Some("hello.py".into()),
                        lineno: Some(1),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                raw_stacktrace: None,
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"exception\":{\"values\":[{\"type\":\"DivisionByZero\",\"value\":\"integer division \
             or modulo by \
             zero\",\"stacktrace\":{\"frames\":[{\"function\":\"main\",\"filename\":\"hello.py\",\
             \"lineno\":1}]}}]}}"
        );
    }

    #[test]
    fn test_exception_stacktrace_larger() {
        let event: v7::Event<'_> = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            exception: vec![v7::Exception {
                ty: "DivisionByZero".into(),
                value: Some("integer division or modulo by zero".into()),
                module: None,
                stacktrace: Some(v7::Stacktrace {
                    frames: vec![v7::Frame {
                        function: Some("main".into()),
                        filename: Some("hello.py".into()),
                        lineno: Some(7),
                        colno: Some(42),
                        pre_context: vec!["foo".into(), "bar".into()],
                        context_line: Some("hey hey hey".into()),
                        post_context: vec!["foo".into(), "bar".into()],
                        in_app: Some(true),
                        vars: {
                            let mut m = v7::Map::new();
                            m.insert("var".into(), "value".into());
                            m
                        },
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                raw_stacktrace: None,
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"exception\":{\"values\":[{\"type\":\"DivisionByZero\",\"value\":\"integer division \
             or modulo by \
             zero\",\"stacktrace\":{\"frames\":[{\"function\":\"main\",\"filename\":\"hello.py\",\
             \"lineno\":7,\"colno\":42,\"pre_context\":[\"foo\",\"bar\"],\"context_line\":\"hey \
             hey hey\",\"post_context\":[\"foo\",\"bar\"],\"in_app\":true,\"vars\":{\"var\":\
             \"value\"}}]}}]}}"
        );
    }

    #[test]
    fn test_exception_stacktrace_full() {
        let event: v7::Event<'_> = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            exception: vec![v7::Exception {
                ty: "DivisionByZero".into(),
                value: Some("integer division or modulo by zero".into()),
                module: Some("x".into()),
                stacktrace: Some(v7::Stacktrace {
                    frames: vec![v7::Frame {
                        function: Some("main".into()),
                        symbol: Some("main".into()),
                        filename: Some("hello.py".into()),
                        abs_path: Some("/app/hello.py".into()),
                        lineno: Some(7),
                        colno: Some(42),
                        pre_context: vec!["foo".into(), "bar".into()],
                        context_line: Some("hey hey hey".into()),
                        post_context: vec!["foo".into(), "bar".into()],
                        in_app: Some(true),
                        vars: {
                            let mut m = v7::Map::new();
                            m.insert("var".into(), "value".into());
                            m
                        },
                        package: Some("hello.whl".into()),
                        module: Some("hello".into()),
                        image_addr: Some(v7::Addr(0)),
                        instruction_addr: Some(v7::Addr(0)),
                        symbol_addr: Some(v7::Addr(0)),
                        addr_mode: None,
                    }],
                    frames_omitted: Some((1, 2)),
                    registers: {
                        let mut m = v7::Map::new();
                        m.insert("x8".into(), v7::RegVal(0x0));
                        m.insert("x20".into(), v7::RegVal(0x1));
                        m.insert("x21".into(), v7::RegVal(0x1));
                        m.insert("x28".into(), v7::RegVal(0x1_7025_f650));
                        m.insert("x4".into(), v7::RegVal(0x1_702e_b100));
                        m.insert("x24".into(), v7::RegVal(0x1_b139_9c20));
                        m.insert("sp".into(), v7::RegVal(0x1_6fd7_5060));
                        m.insert("x1".into(), v7::RegVal(0x1_b139_9bb1));
                        m.insert("x23".into(), v7::RegVal(0x1_afe1_0040));
                        m.insert("x14".into(), v7::RegVal(0x1));
                        m.insert("x19".into(), v7::RegVal(0x0));
                        m.insert("x18".into(), v7::RegVal(0x0));
                        m.insert("x3".into(), v7::RegVal(0x1));
                        m.insert("pc".into(), v7::RegVal(0x1_8a31_0ea4));
                        m.insert("x7".into(), v7::RegVal(0x0));
                        m.insert("x10".into(), v7::RegVal(0x57b));
                        m.insert("x6".into(), v7::RegVal(0x0));
                        m.insert("x13".into(), v7::RegVal(0x1));
                        m.insert("x2".into(), v7::RegVal(0x1));
                        m.insert("x27".into(), v7::RegVal(0x1));
                        m.insert("x26".into(), v7::RegVal(0x1_91ec_48d1));
                        m.insert("x9".into(), v7::RegVal(0x1_b139_9c20));
                        m.insert("x29".into(), v7::RegVal(0x1_6fd7_5060));
                        m.insert("x5".into(), v7::RegVal(0x1_702e_b100));
                        m.insert("fp".into(), v7::RegVal(0x1_6fd7_5060));
                        m.insert("x0".into(), v7::RegVal(0x1));
                        m.insert("lr".into(), v7::RegVal(0x1_8a31_aadc));
                        m.insert("x25".into(), v7::RegVal(0x0));
                        m.insert("x16".into(), v7::RegVal(0x1_8a31_aa34));
                        m.insert("x11".into(), v7::RegVal(0x1_b3b3_7b1d));
                        m.insert("cpsr".into(), v7::RegVal(0x2000_0000));
                        m.insert("x17".into(), v7::RegVal(0x0));
                        m.insert("x15".into(), v7::RegVal(0x881));
                        m.insert("x22".into(), v7::RegVal(0x1_b139_9bb0));
                        m.insert("x12".into(), v7::RegVal(0x1_b3b3_7b1d));
                        m
                    },
                }),
                raw_stacktrace: Some(v7::Stacktrace {
                    frames: vec![v7::Frame {
                        function: Some("main".into()),
                        image_addr: Some(v7::Addr(0)),
                        instruction_addr: Some(v7::Addr(0)),
                        symbol_addr: Some(v7::Addr(0)),
                        ..Default::default()
                    }],
                    frames_omitted: Some((1, 2)),
                    ..Default::default()
                }),
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"exception\":{\"values\":[{\"type\":\"DivisionByZero\",\"value\":\"integer division \
             or modulo by \
             zero\",\"module\":\"x\",\"stacktrace\":{\"frames\":[{\"function\":\"main\",\
             \"symbol\":\"main\",\"module\":\"hello\",\"package\":\"hello.whl\",\"filename\":\
             \"hello.py\",\"abs_path\":\"/app/hello.py\",\"lineno\":7,\"colno\":42,\
             \"pre_context\":[\"foo\",\"bar\"],\"context_line\":\"hey hey \
             hey\",\"post_context\":[\"foo\",\"bar\"],\"in_app\":true,\"vars\":{\"var\":\
             \"value\"},\"image_addr\":\"0x0\",\"instruction_addr\":\"0x0\",\"symbol_addr\":\
             \"0x0\"}],\"frames_omitted\":[1,2],\"registers\":{\"cpsr\":\"0x20000000\",\"fp\":\
             \"0x16fd75060\",\"lr\":\"0x18a31aadc\",\"pc\":\"0x18a310ea4\",\"sp\":\"0x16fd75060\",\
             \"x0\":\"0x1\",\"x1\":\"0x1b1399bb1\",\"x10\":\"0x57b\",\"x11\":\"0x1b3b37b1d\",\
             \"x12\":\"0x1b3b37b1d\",\"x13\":\"0x1\",\"x14\":\"0x1\",\"x15\":\"0x881\",\"x16\":\
             \"0x18a31aa34\",\"x17\":\"0x0\",\"x18\":\"0x0\",\"x19\":\"0x0\",\"x2\":\"0x1\",\
             \"x20\":\"0x1\",\"x21\":\"0x1\",\"x22\":\"0x1b1399bb0\",\"x23\":\"0x1afe10040\",\
             \"x24\":\"0x1b1399c20\",\"x25\":\"0x0\",\"x26\":\"0x191ec48d1\",\"x27\":\"0x1\",\
             \"x28\":\"0x17025f650\",\"x29\":\"0x16fd75060\",\"x3\":\"0x1\",\"x4\":\
             \"0x1702eb100\",\"x5\":\"0x1702eb100\",\"x6\":\"0x0\",\"x7\":\"0x0\",\"x8\":\"0x0\",\
             \"x9\":\"0x1b1399c20\"}},\"raw_stacktrace\":{\"frames\":[{\"function\":\"main\",\
             \"image_addr\":\"0x0\",\"instruction_addr\":\"0x0\",\"symbol_addr\":\"0x0\"}],\
             \"frames_omitted\":[1,2]}}]}}"
        );
    }

    #[test]
    fn test_exception_mechanism() {
        let event: v7::Event<'_> = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            exception: vec![v7::Exception {
                ty: "EXC_BAD_ACCESS".into(),
                value: Some("Attempted to dereference garbage pointer 0x1".into()),
                mechanism: Some(v7::Mechanism {
                    ty: "mach".into(),
                    description: None,
                    help_link: Some(
                        "https://developer.apple.com/library/content/qa/qa1367/_index.html"
                            .parse()
                            .unwrap(),
                    ),
                    handled: Some(false),
                    synthetic: None,
                    data: {
                        let mut map = v7::Map::new();
                        map.insert("relevant_address".into(), "0x1".into());
                        map
                    },
                    meta: v7::MechanismMeta {
                        errno: Some(v7::CError {
                            number: 2,
                            name: None,
                        }),
                        signal: Some(v7::PosixSignal {
                            number: 11,
                            code: None,
                            name: None,
                            code_name: None,
                        }),
                        mach_exception: Some(v7::MachException {
                            exception: 1,
                            code: 1,
                            subcode: 8,
                            name: None,
                        }),
                    },
                }),
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"exception\":{\"values\":[{\"type\":\"EXC_BAD_ACCESS\",\"value\":\"Attempted to \
             dereference garbage pointer \
             0x1\",\"mechanism\":{\"type\":\"mach\",\"help_link\":\"https://developer.apple.\
             com/library/content/qa/qa1367/_index.html\",\"handled\":false,\"data\":\
             {\"relevant_address\":\"0x1\"},\"meta\":{\"errno\":{\"number\":2},\"signal\":\
             {\"number\":11},\"mach_exception\":{\"exception\":1,\"code\":1,\"subcode\":8}}}}]}}"
        );
    }
}

#[test]
fn test_sdk_info() {
    let event = v7::Event {
        event_id: event_id(),
        timestamp: event_time(),
        sdk: Some(Cow::Owned(v7::ClientSdkInfo {
            name: "sentry-rust".into(),
            version: "1.0".into(),
            integrations: vec!["rocket".into()],
            packages: vec![v7::ClientSdkPackage {
                name: "cargo:sentry".into(),
                version: "1.0".into(),
            }],
        })),
        ..Default::default()
    };

    assert_roundtrip(&event);
    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\"sdk\":\
         {\"name\":\"sentry-rust\",\"version\":\"1.0\",\"integrations\":[\"rocket\"],\"packages\":\
         [{\"name\":\"cargo:sentry\",\"version\":\"1.0\"}]}}"
    );
}

mod test_contexts {
    use super::*;

    #[test]
    fn test_device_context() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            contexts: {
                let mut m = v7::Map::new();
                m.insert(
                    "device".into(),
                    v7::DeviceContext {
                        name: Some("iphone".into()),
                        family: Some("iphone".into()),
                        model: Some("iphone7,3".into()),
                        model_id: Some("AH223".into()),
                        arch: Some("arm64".into()),
                        battery_level: Some(58.5),
                        orientation: Some(v7::Orientation::Landscape),
                        simulator: Some(true),
                        memory_size: Some(3_137_978_368),
                        free_memory: Some(322_781_184),
                        usable_memory: Some(2_843_525_120),
                        storage_size: Some(63_989_469_184),
                        free_storage: Some(31_994_734_592),
                        external_storage_size: Some(2_097_152),
                        external_free_storage: Some(2_097_152),
                        boot_time: Some(systime(2018, 2, 8, 12, 52, 12, 0)),
                        timezone: Some("Europe/Vienna".into()),
                        other: Default::default(),
                    }
                    .into(),
                );
                m
            },
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"contexts\":{\"device\":{\"type\":\"device\",\"name\":\"iphone\",\"family\":\
             \"iphone\",\"model\":\"iphone7,3\",\"model_id\":\"AH223\",\"arch\":\"arm64\",\
             \"battery_level\":58.5,\"orientation\":\"landscape\",\"simulator\":true,\
             \"memory_size\":3137978368,\"free_memory\":322781184,\"usable_memory\":2843525120,\
             \"storage_size\":63989469184,\"free_storage\":31994734592,\"external_storage_size\":\
             2097152,\"external_free_storage\":2097152,\"boot_time\":\"2018-02-08T12:52:12Z\",\
             \"timezone\":\"Europe/Vienna\"}}}"
        );
    }

    #[test]
    fn test_os_context() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            contexts: {
                let mut m = v7::Map::new();
                m.insert(
                    "os".into(),
                    v7::OsContext {
                        name: Some("iOS".into()),
                        version: Some("11.4.2".into()),
                        build: Some("ADSA23".into()),
                        kernel_version: Some("17.4.0".into()),
                        rooted: Some(true),
                        other: Default::default(),
                    }
                    .into(),
                );
                m
            },
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"contexts\":{\"os\":{\"type\":\"os\",\"name\":\"iOS\",\"version\":\"11.4.2\",\
             \"build\":\"ADSA23\",\"kernel_version\":\"17.4.0\",\"rooted\":true}}}"
        );
    }

    #[test]
    fn test_app_context() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            contexts: {
                let mut m = v7::Map::new();
                m.insert(
                    "app".into(),
                    v7::AppContext {
                        app_start_time: Some(systime(2018, 2, 8, 22, 21, 57, 0)),
                        device_app_hash: Some("4c793e3776474877ae30618378e9662a".into()),
                        build_type: Some("testflight".into()),
                        app_identifier: Some("foo.bar.baz".into()),
                        app_name: Some("Baz App".into()),
                        app_version: Some("1.0".into()),
                        app_build: Some("100001".into()),
                        other: Default::default(),
                    }
                    .into(),
                );
                m
            },
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"contexts\":{\"app\":{\"type\":\"app\",\"app_start_time\":\"2018-02-08T22:21:57Z\",\
             \"device_app_hash\":\"4c793e3776474877ae30618378e9662a\",\"build_type\":\
             \"testflight\",\"app_identifier\":\"foo.bar.baz\",\"app_name\":\"Baz \
             App\",\"app_version\":\"1.0\",\"app_build\":\"100001\"}}}"
        );
    }

    #[test]
    fn test_browser_context() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            contexts: {
                let mut m = v7::Map::new();
                m.insert(
                    "browser".into(),
                    v7::BrowserContext {
                        name: Some("Chrome".into()),
                        version: Some("59.0.3071".into()),
                        other: Default::default(),
                    }
                    .into(),
                );
                m
            },
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"contexts\":{\"browser\":{\"type\":\"browser\",\"name\":\"Chrome\",\"version\":\"59.\
             0.3071\"}}}"
        );
    }

    #[test]
    fn test_runtime_context() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            contexts: {
                let mut m = v7::Map::new();
                m.insert(
                    "runtime".into(),
                    v7::RuntimeContext {
                        name: Some("magicvm".into()),
                        version: Some("5.3".into()),
                        other: Default::default(),
                    }
                    .into(),
                );
                m
            },
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"contexts\":{\"runtime\":{\"type\":\"runtime\",\"name\":\"magicvm\",\"version\":\"5.\
             3\"}}}"
        );
    }

    #[test]
    fn test_renamed_contexts() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            contexts: {
                let mut m = v7::Map::new();
                m.insert(
                    "magicvm".into(),
                    v7::RuntimeContext {
                        name: Some("magicvm".into()),
                        version: Some("5.3".into()),
                        other: Default::default(),
                    }
                    .into(),
                );
                m.insert(
                    "othervm".into(),
                    v7::RuntimeContext {
                        name: Some("magicvm".into()),
                        version: Some("5.3".into()),
                        other: Default::default(),
                    }
                    .into(),
                );
                m
            },
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"contexts\":{\"magicvm\":{\"type\":\"runtime\",\"name\":\"magicvm\",\"version\":\"5.\
             3\"},\"othervm\":{\"type\":\"runtime\",\"name\":\"magicvm\",\"version\":\"5.3\"}}}"
        );
    }

    #[test]
    fn test_other_context() {
        let event = v7::Event {
            event_id: event_id(),
            timestamp: event_time(),
            contexts: {
                let mut m = v7::Map::new();
                m.insert(
                    "other".into(),
                    v7::Context::Other({
                        let mut m = v7::Map::new();
                        m.insert("aha".into(), "oho".into());
                        m
                    }),
                );
                m
            },
            ..Default::default()
        };

        assert_roundtrip(&event);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            "{\"event_id\":\"d43e86c96e424a93a4fbda156dd17341\",\"timestamp\":1514103120,\
             \"contexts\":{\"other\":{\"type\":\"unknown\",\"aha\":\"oho\"}}}"
        );
    }

    #[test]
    fn test_typeless_context() {
        let payload = r#"
            {
                "event_id": "d43e86c96e424a93a4fbda156dd17341",
                "timestamp": 1514103120,
                "contexts": {
                    "device": {
                        "name": "iphone",
                        "family": "iphone",
                        "model": "iphone7,3",
                        "model_id": "AH223",
                        "arch": "arm64"
                    },
                    "os": { "name": "iOS", "version": "11.4.2", "build": "ADSA23" },
                    "runtime": { "name": "magicvm", "version": "5.3" },
                    "app": {
                        "app_start_time": "2018-02-08T22:21:57Z",
                        "build_type": "testflight",
                        "app_name": "Baz App",
                        "app_version": "1.0",
                        "app_build": "100001"
                    },
                    "browser": { "name": "Chrome", "version": "59.0.3071" },
                    "gpu": {
                        "name": "AMD Radeon Pro 560",
                        "vendor_name": "Apple",
                        "memory_size": 4096
                    },
                    "trace": {
                        "trace_id": "12312012123120121231201212312012",
                        "span_id": "0415201309082013",
                        "parent_span_id": null,
                        "description": "<OrganizationContext>",
                        "op": "http.server"
                    },
                    "random": { "aha": "oho" }
                }
            }
        "#;

        let event: v7::Event = serde_json::from_str(payload).unwrap();
        let ctx = event.contexts;

        assert!(ctx.contains_key("device"));
        assert_eq!(ctx.get("device").unwrap().type_name(), "device");

        assert!(ctx.contains_key("os"));
        assert_eq!(ctx.get("os").unwrap().type_name(), "os");

        assert!(ctx.contains_key("runtime"));
        assert_eq!(ctx.get("runtime").unwrap().type_name(), "runtime");

        assert!(ctx.contains_key("app"));
        assert_eq!(ctx.get("app").unwrap().type_name(), "app");

        assert!(ctx.contains_key("browser"));
        assert_eq!(ctx.get("browser").unwrap().type_name(), "browser");

        assert!(ctx.contains_key("trace"));
        assert_eq!(ctx.get("trace").unwrap().type_name(), "trace");

        assert!(ctx.contains_key("gpu"));
        assert_eq!(ctx.get("gpu").unwrap().type_name(), "gpu");

        assert!(ctx.contains_key("random"));
        assert_eq!(ctx.get("random").unwrap().type_name(), "unknown");
    }
}

#[test]
fn test_level_log() {
    assert_eq!(v7::Level::Info, serde_json::from_str("\"log\"").unwrap());
}

#[test]
fn test_addr_format() {
    assert_eq!(serde_json::to_string(&v7::Addr(0)).unwrap(), "\"0x0\"");
    assert_eq!(serde_json::to_string(&v7::Addr(42)).unwrap(), "\"0x2a\"");
    assert_eq!(serde_json::from_str::<v7::Addr>("0").unwrap(), v7::Addr(0));
    assert_eq!(
        serde_json::from_str::<v7::Addr>("\"0\"").unwrap(),
        v7::Addr(0)
    );
    assert_eq!(
        serde_json::from_str::<v7::Addr>("\"0x0\"").unwrap(),
        v7::Addr(0)
    );
    assert_eq!(
        serde_json::from_str::<v7::Addr>("42").unwrap(),
        v7::Addr(42)
    );
    assert_eq!(
        serde_json::from_str::<v7::Addr>("\"42\"").unwrap(),
        v7::Addr(42)
    );
    assert_eq!(
        serde_json::from_str::<v7::Addr>("\"0x2a\"").unwrap(),
        v7::Addr(42)
    );
    assert_eq!(
        serde_json::from_str::<v7::Addr>("\"0X2A\"").unwrap(),
        v7::Addr(42)
    );
}

#[test]
fn test_addr_api() {
    use std::ptr;
    assert_eq!(v7::Addr::from(42u64), v7::Addr(42));
    assert_eq!(v7::Addr::from(42), v7::Addr(42));
    assert_eq!(v7::Addr::from(ptr::null::<()>()), v7::Addr(0));
}

#[test]
fn test_thread_id_format() {
    assert_eq!(serde_json::to_string(&v7::ThreadId::Int(0)).unwrap(), "0");
    assert_eq!(serde_json::to_string(&v7::ThreadId::Int(42)).unwrap(), "42");
    assert_eq!(
        serde_json::to_string(&v7::ThreadId::String("x".into())).unwrap(),
        "\"x\""
    );
    assert_eq!(
        serde_json::from_str::<v7::ThreadId>("0").unwrap(),
        v7::ThreadId::Int(0)
    );
    assert_eq!(
        serde_json::from_str::<v7::ThreadId>("\"0\"").unwrap(),
        v7::ThreadId::String("0".into())
    );
}

#[test]
fn test_orientation() {
    assert_eq!(
        serde_json::to_string(&v7::Orientation::Landscape).unwrap(),
        "\"landscape\""
    );
    assert_eq!(
        serde_json::to_string(&v7::Orientation::Portrait).unwrap(),
        "\"portrait\""
    );
}
