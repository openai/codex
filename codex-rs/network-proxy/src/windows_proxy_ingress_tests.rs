use super::*;
use rama_core::service::service_fn;

#[test]
fn selects_exactly_one_registered_route() {
    let route = route_services();
    let routes = HashMap::from([("registered".to_string(), Arc::clone(&route))]);

    let selected = registered_route_for_sids(
        &routes,
        &["unrelated".to_string(), "registered".to_string()],
    )
    .expect("one registered route should be selected");

    assert!(Arc::ptr_eq(&selected, &route));
}

#[test]
fn rejects_missing_or_ambiguous_registered_routes() {
    let first = route_services();
    let second = route_services();
    let routes = HashMap::from([("first".to_string(), first), ("second".to_string(), second)]);

    let Err(missing) = registered_route_for_sids(&routes, &["missing".to_string()]) else {
        panic!("an unknown SID should fail closed");
    };
    let Err(ambiguous) =
        registered_route_for_sids(&routes, &["first".to_string(), "second".to_string()])
    else {
        panic!("multiple registered SIDs should fail closed");
    };

    assert_eq!(missing.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(ambiguous.kind(), io::ErrorKind::PermissionDenied);
}

fn route_services() -> Arc<RouteServices> {
    let service = service_fn(|_stream: TcpStream| async { Ok::<(), BoxError>(()) }).boxed();
    Arc::new(RouteServices {
        http: service,
        socks: None,
    })
}
