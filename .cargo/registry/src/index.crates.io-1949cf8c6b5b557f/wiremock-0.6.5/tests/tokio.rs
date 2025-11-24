use reqwest::Client;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// regression tests for https://github.com/LukeMathWalker/wiremock-rs/issues/7
// running both tests will _sometimes_ trigger a hang if the runtimes aren't separated correctly

#[tokio::test]
async fn hello_reqwest() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let resp = Client::new().get(&mock_server.uri()).send().await.unwrap();

    assert_eq!(resp.status(), 200);
}

#[actix_rt::test]
async fn hello_reqwest_actix() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let resp = Client::new().get(&mock_server.uri()).send().await.unwrap();

    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn hello_reqwest_http2() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let resp = Client::builder()
        .http2_prior_knowledge()
        .build()
        .expect("http client")
        .get(&mock_server.uri())
        .send()
        .await
        .expect("response");

    assert_eq!(resp.status(), 200);
    assert_eq!(resp.version(), reqwest::Version::HTTP_2);
}
