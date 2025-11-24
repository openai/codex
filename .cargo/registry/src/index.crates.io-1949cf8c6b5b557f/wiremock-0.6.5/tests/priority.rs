use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, path_regex},
};

#[async_std::test]
async fn should_prioritize_mock_with_highest_priority() {
    // Arrange
    let mock_server = MockServer::start().await;
    let exact = Mock::given(method("GET"))
        .and(path("abcd"))
        .respond_with(ResponseTemplate::new(200))
        .with_priority(2);
    mock_server.register(exact).await;
    let regex = Mock::given(method("GET"))
        .and(path_regex("[a-z]{4}"))
        .respond_with(ResponseTemplate::new(201))
        .with_priority(1);
    mock_server.register(regex).await;

    // Act
    let should_match = reqwest::get(format!("{}/abcd", mock_server.uri()))
        .await
        .unwrap();
    // Assert
    assert_eq!(should_match.status(), 201);
}

#[async_std::test]
async fn should_not_prioritize_mock_with_lower_priority() {
    // Arrange
    let mock_server = MockServer::start().await;
    let exact = Mock::given(method("GET"))
        .and(path("abcd"))
        .respond_with(ResponseTemplate::new(200))
        .with_priority(u8::MAX);
    mock_server.register(exact).await;
    let regex = Mock::given(method("GET"))
        .and(path_regex("[a-z]{4}"))
        .respond_with(ResponseTemplate::new(201));
    mock_server.register(regex).await;

    // Act
    let should_match = reqwest::get(format!("{}/abcd", mock_server.uri()))
        .await
        .unwrap();
    // Assert
    assert_eq!(should_match.status(), 201);
}

#[async_std::test]
async fn by_default_should_use_insertion_order() {
    // Arrange
    let mock_server = MockServer::start().await;
    let exact = Mock::given(method("GET"))
        .and(path("abcd"))
        .respond_with(ResponseTemplate::new(200));
    let regex = Mock::given(method("GET"))
        .and(path_regex("[a-z]{4}"))
        .respond_with(ResponseTemplate::new(201));
    mock_server.register(exact).await;
    mock_server.register(regex).await;

    // Act
    let should_match = reqwest::get(format!("{}/abcd", mock_server.uri()))
        .await
        .unwrap();
    // Assert
    assert_eq!(should_match.status(), 200);

    // Insert mocks in the opposite order
    // Arrange
    let mock_server = MockServer::start().await;
    let exact = Mock::given(method("GET"))
        .and(path("abcd"))
        .respond_with(ResponseTemplate::new(200));
    let regex = Mock::given(method("GET"))
        .and(path_regex("[a-z]{4}"))
        .respond_with(ResponseTemplate::new(201));
    mock_server.register(regex).await;
    mock_server.register(exact).await;

    // Act
    let should_match = reqwest::get(format!("{}/abcd", mock_server.uri()))
        .await
        .unwrap();
    // Assert
    assert_eq!(should_match.status(), 201);
}
