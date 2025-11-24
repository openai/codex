use reqwest::Client;
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn test_body() {
    // Arrange
    let mock_server = MockServer::start().await;

    let response = ResponseTemplate::new(200).set_delay(std::time::Duration::from_secs(60));
    Mock::given(any())
        .respond_with(response)
        .mount(&mock_server)
        .await;

    // Act
    let outcome = Client::builder()
        .timeout(std::time::Duration::from_secs(1))
        .build()
        .unwrap()
        .get(&mock_server.uri())
        .send()
        .await;

    // Assert
    assert!(outcome.is_err());
}

#[actix_rt::test]
async fn request_times_out_if_the_server_takes_too_long_with_actix() {
    test_body().await
}

#[tokio::test]
async fn request_times_out_if_the_server_takes_too_long_with_tokio() {
    test_body().await
}
