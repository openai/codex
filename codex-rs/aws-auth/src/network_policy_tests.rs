//! Exercises policy on AWS SDK HTTP and the SDK's credential selection.

use super::AwsAuthError::Policy;
use super::*;
use aws_config::provider_config::ProviderConfig;
use codex_http_client::DestinationPolicy;
use codex_http_client::NetworkPolicyDenied::Revoked;
use pretty_assertions::assert_eq;
use std::error::Error;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;

#[tokio::test]
async fn real_imds_credentials_stop_after_policy_revocation() -> Result<(), Box<dyn Error>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let controller = codex_http_client::NetworkPolicyController::default();
    let policy = controller.policy();
    let factory = codex_http_client::HttpClientFactory::new(
        codex_http_client::OutboundProxyPolicy::ReqwestDefault,
    )
    .with_network_policy(policy.clone().for_current_account());
    let http_client = crate::transport::http_client(factory);
    let imds_client = aws_config::imds::Client::builder()
        .configure(&ProviderConfig::default().with_http_client(http_client))
        .endpoint(format!("http://{}", listener.local_addr()?))
        .unwrap()
        .build();
    let credentials_provider = aws_config::imds::credentials::ImdsCredentialsProvider::builder()
        .profile("test-role")
        .imds_client(imds_client.clone())
        .build();
    let context = AwsAuthContext {
        network_policy: policy.clone().for_current_account(),
        credentials_provider: SharedCredentialsProvider::new(credentials_provider),
        region: "us-east-1".into(),
        service: "bedrock".into(),
    };
    let request = super::tests::test_request();
    let server = async {
        let mut credentials_sent = false;
        loop {
            let (mut socket, _) = listener.accept().await?;
            let mut headers = Vec::new();
            while !headers.ends_with(b"\r\n\r\n") {
                headers.push(socket.read_u8().await?);
            }
            let body = if headers.starts_with(b"PUT /latest/api/token ") {
                "test-token"
            } else if credentials_sent {
                return Ok::<_, std::io::Error>(socket);
            } else {
                credentials_sent = true;
                r#"{"Code":"Success","AccessKeyId":"test-key","SecretAccessKey":"test-secret","Token":"test-session","Expiration":"2099-01-01T00:00:00Z"}"#
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nx-aws-ec2-metadata-token-ttl-seconds: 21600\r\nConnection: close\r\n\r\n{body}"
            );
            socket.write_all(response.as_bytes()).await?;
        }
    };
    controller.publish(policy.revision(), DestinationPolicy::Unrestricted);
    let signing = async {
        context.sign(request.clone()).await.unwrap();
        context.sign(request.clone()).await
    };
    tokio::pin!(signing);
    let timeout_duration = std::time::Duration::from_secs(/*secs*/ 5);
    let mut socket = tokio::select! {
        result = &mut signing => panic!("signing finished before revocation: {result:?}"),
        socket = timeout(timeout_duration, server) => socket??,
    };
    let restricted = DestinationPolicy::Restricted {
        allowed_hosts: Default::default(),
    };
    controller.publish(policy.revision(), restricted);
    let revoked = timeout(timeout_duration, signing).await?;
    assert!(matches!(revoked, Err(Policy(Revoked))));
    assert_eq!(timeout(timeout_duration, socket.read(&mut [0])).await??, 0);
    // The model remains allowed while metadata traffic is denied before connecting.
    controller.publish(
        policy.revision(),
        DestinationPolicy::Restricted {
            allowed_hosts: ["bedrock-runtime.us-east-1.amazonaws.com".to_string()].into(),
        },
    );
    context.sign(request.clone()).await?;
    let fresh = AwsAuthContext {
        credentials_provider: SharedCredentialsProvider::new(
            aws_config::imds::credentials::ImdsCredentialsProvider::builder()
                .profile("test-role")
                .imds_client(imds_client)
                .build(),
        ),
        ..context.clone()
    };
    let denied = fresh.sign(request.clone()).await;
    assert!(
        matches!(
            &denied,
            Err(Policy(codex_http_client::NetworkPolicyDenied::Destination))
        ),
        "{denied:?}"
    );
    assert!(
        timeout(
            std::time::Duration::from_millis(/*millis*/ 100),
            listener.accept()
        )
        .await
        .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn allowlisted_static_keys_send_sigv4_and_denied_destination_sends_nothing()
-> Result<(), Box<dyn Error>> {
    codex_utils_rustls_provider::ensure_rustls_crypto_provider();
    let certificate = rcgen::generate_simple_self_signed(vec!["127.0.0.1".to_string()])?;
    let trust = codex_http_client::HttpClientTlsConfig::default()
        .with_root_certificate_pem(certificate.cert.pem().as_bytes())?;
    let tls = tokio_rustls::TlsAcceptor::from(Arc::new(
        rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![certificate.cert.der().clone()],
                certificate.signing_key.into(),
            )?,
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("https://{}/v1/responses", listener.local_addr()?);
    let controller = codex_http_client::NetworkPolicyController::default();
    let policy = controller.policy();
    controller.publish(
        policy.revision(),
        DestinationPolicy::Restricted {
            allowed_hosts: ["127.0.0.1".to_string()].into(),
        },
    );
    let factory = codex_http_client::HttpClientFactory::new(
        codex_http_client::OutboundProxyPolicy::ReqwestDefault,
    )
    .with_network_policy(policy.for_current_account());
    let context = AwsAuthContext::load_with_access_keys(
        AwsAuthConfig {
            profile: Some("unused-profile".into()),
            region: Some("us-east-1".into()),
            service: "bedrock".into(),
        },
        AwsAccessKeys {
            access_key_id: "static-key".into(),
            secret_access_key: "static-secret".into(),
            session_token: None,
        },
        factory.clone(),
    )
    .await?;
    let request = AwsRequestToSign {
        url: url.clone(),
        ..super::tests::test_request()
    };
    let signed = context.sign(request.clone()).await?;
    let client = codex_http_client::HttpClientBuilder::new().build_with_tls(
        &factory,
        codex_http_client::ClientRouteClass::Api,
        trust,
    );
    let server = async {
        let (socket, _) = listener.accept().await?;
        let mut socket = tls.accept(socket).await?;
        let mut headers = Vec::new();
        while !headers.ends_with(b"\r\n\r\n") {
            headers.push(socket.read_u8().await?);
        }
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
            .await?;
        Ok::<_, Box<dyn Error>>(String::from_utf8(headers)?)
    };
    let (response, headers) = tokio::join!(
        client
            .post(&signed.url)
            .headers(signed.headers)
            .body(request.body.clone())
            .send(),
        server
    );
    assert!(response?.status().is_success());
    assert!(headers?.contains("Credential=static-key/"));
    controller.publish(
        controller.policy().revision(),
        DestinationPolicy::Restricted {
            allowed_hosts: Default::default(),
        },
    );
    assert!(matches!(
        context.sign(request).await,
        Err(Policy(codex_http_client::NetworkPolicyDenied::Destination))
    ));
    assert!(
        timeout(
            std::time::Duration::from_millis(/*millis*/ 100),
            listener.accept()
        )
        .await
        .is_err()
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sdk_credentials_preserve_profile_processes_and_environment_precedence()
-> Result<(), Box<dyn Error>> {
    for case in [
        "restricted-static",
        "restricted-process",
        "unrestricted-process",
        "unrestricted-offset-process",
        "restricted-role-process",
        "unrestricted-role-process",
        "env-precedence",
        "profile-precedence",
    ] {
        let home = tempfile::tempdir()?;
        let marker = home.path().join("process-started");
        let expiration = if case == "unrestricted-offset-process" {
            "2099-01-01T01:30:00+01:30"
        } else {
            "2099-01-01T00:00:00Z"
        };
        let json = format!(
            r#"{{"Version":1,"AccessKeyId":"process-key","SecretAccessKey":"process-secret","Expiration":"{expiration}"}}"#
        );
        std::fs::write(home.path().join("credentials-output.json"), &json)?;
        let command = if cfg!(windows) {
            "echo started > process-started & type credentials-output.json".to_string()
        } else {
            format!(
                "printf started > '{}' && printf '%s' '{json}'",
                marker.to_string_lossy().replace('\'', "'\\''")
            )
        };
        let contents = if matches!(case, "restricted-static" | "profile-precedence") {
            "[managed]\naws_access_key_id = profile-key\naws_secret_access_key = profile-secret\n"
                .to_string()
        } else {
            format!(
                "[managed]\ncredential_process = {command}\naws_access_key_id = fallback-key\naws_secret_access_key = fallback-secret\n"
            )
        };
        let credentials = home.path().join("credentials");
        let config = home.path().join("config");
        std::fs::write(&credentials, contents)?;
        std::fs::write(
            &config,
            if case.contains("role-process") {
                "[profile managed] # SDK accepts section comments\nrole_arn = arn:aws:iam::123456789012:role/test\nsource_profile = managed # self source\n"
            } else {
                ""
            },
        )?;
        let role_request = if case == "unrestricted-role-process" {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            let endpoint = format!("http://{}", listener.local_addr()?);
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await?;
                let mut headers = Vec::new();
                while !headers.ends_with(b"\r\n\r\n") {
                    headers.push(socket.read_u8().await?);
                }
                let headers = String::from_utf8(headers).map_err(std::io::Error::other)?;
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>())
                    })
                    .transpose()
                    .map_err(std::io::Error::other)?
                    .unwrap_or_default();
                // Drain the POST body before closing so Windows does not reset the response.
                socket.read_exact(&mut vec![0; content_length]).await?;
                let body = "<AssumeRoleResponse><AssumeRoleResult><Credentials><AccessKeyId>assumed-key</AccessKeyId><SecretAccessKey>assumed-secret</SecretAccessKey><SessionToken>assumed-token</SessionToken><Expiration>2099-01-01T00:00:00Z</Expiration></Credentials></AssumeRoleResult></AssumeRoleResponse>";
                socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await?;
                Ok::<_, std::io::Error>(headers)
            });
            Some((endpoint, server))
        } else {
            None
        };
        let mut child = std::process::Command::new(std::env::current_exe()?);
        child
            .current_dir(home.path())
            .args([
                "--ignored",
                "--exact",
                "network_policy_tests::sdk_profile_probe",
            ])
            .env("CODEX_AWS_PROFILE_PROBE_CASE", case)
            .env("AWS_CONFIG_FILE", config)
            .env("AWS_SHARED_CREDENTIALS_FILE", credentials)
            .env("AWS_EC2_METADATA_DISABLED", "true")
            .env_remove("AWS_ACCESS_KEY_ID")
            .env_remove("AWS_SECRET_ACCESS_KEY")
            .env_remove("SECRET_ACCESS_KEY")
            .env_remove("AWS_SESSION_TOKEN");
        if matches!(case, "env-precedence" | "profile-precedence") {
            child
                .env("AWS_ACCESS_KEY_ID", "env-key")
                .env("AWS_SECRET_ACCESS_KEY", "env-secret");
        }
        if let Some((endpoint, _)) = &role_request {
            child.env("AWS_ENDPOINT_URL_STS", endpoint);
        }
        let output = tokio::task::spawn_blocking(move || child.output()).await??;
        assert!(
            output.status.success(),
            "{case}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(marker.exists(), case.contains("process"), "{case}");
        if let Some((_, server)) = role_request {
            assert!(server.await??.contains("Credential=process-key/"));
        }
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sdk_default_credentials_keep_endpoint_modes_from_environment_and_profile()
-> Result<(), Box<dyn Error>> {
    for case in ["endpoint-modes-env", "endpoint-modes-profile"] {
        let home = tempfile::tempdir()?;
        let credentials = home.path().join("credentials");
        let config = home.path().join("config");
        std::fs::write(
            &credentials,
            "[source]\naws_access_key_id = source-key\naws_secret_access_key = source-secret\n",
        )?;
        let mut profile = "[profile managed]\nrole_arn = arn:aws:iam::123456789012:role/test\nsource_profile = source\n".to_string();
        if case == "endpoint-modes-profile" {
            profile.push_str("use_fips_endpoint = true\nuse_dualstack_endpoint = true\n");
        }
        std::fs::write(&config, profile)?;
        let proxy = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let proxy_url = format!("http://{}", proxy.local_addr()?);
        let server = tokio::spawn(async move {
            let (mut socket, _) = proxy.accept().await?;
            let mut headers = Vec::new();
            while !headers.ends_with(b"\r\n\r\n") {
                headers.push(socket.read_u8().await?);
            }
            socket
                .write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n")
                .await?;
            String::from_utf8(headers).map_err(std::io::Error::other)
        });
        let mut child = std::process::Command::new(std::env::current_exe()?);
        child
            .args([
                "--ignored",
                "--exact",
                "network_policy_tests::sdk_profile_probe",
            ])
            .env("CODEX_AWS_PROFILE_PROBE_CASE", case)
            .env("AWS_CONFIG_FILE", config)
            .env("AWS_SHARED_CREDENTIALS_FILE", credentials)
            .env("AWS_EC2_METADATA_DISABLED", "true")
            .env("AWS_MAX_ATTEMPTS", "1")
            .env("HTTPS_PROXY", &proxy_url)
            .env("https_proxy", proxy_url)
            .env("NO_PROXY", "")
            .env("no_proxy", "");
        for variable in [
            "AWS_ACCESS_KEY_ID",
            "AWS_SECRET_ACCESS_KEY",
            "SECRET_ACCESS_KEY",
            "AWS_SESSION_TOKEN",
            "AWS_ENDPOINT_URL",
            "AWS_ENDPOINT_URL_STS",
            "AWS_USE_FIPS_ENDPOINT",
            "AWS_USE_DUALSTACK_ENDPOINT",
        ] {
            child.env_remove(variable);
        }
        if case == "endpoint-modes-env" {
            child
                .env("AWS_USE_FIPS_ENDPOINT", "true")
                .env("AWS_USE_DUALSTACK_ENDPOINT", "true");
        }
        let output = tokio::task::spawn_blocking(move || child.output()).await??;
        assert!(
            output.status.success(),
            "{case}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let request = timeout(std::time::Duration::from_secs(/*secs*/ 5), server).await???;
        assert!(request.starts_with("CONNECT sts-fips.us-east-1.api.aws:443 "));
    }
    Ok(())
}

#[test]
#[ignore = "isolated AWS environment fixture"]
fn sdk_profile_probe() -> Result<(), Box<dyn Error>> {
    let case = std::env::var("CODEX_AWS_PROFILE_PROBE_CASE")?;
    tokio::runtime::Runtime::new()?.block_on(async {
        let controller = codex_http_client::NetworkPolicyController::default();
        let policy = controller.policy();
        let destination = if case.starts_with("unrestricted") {
            DestinationPolicy::Unrestricted
        } else {
            let mut allowed_hosts = ["bedrock-runtime.us-east-1.amazonaws.com".to_string()]
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>();
            if case.starts_with("endpoint-modes-") {
                allowed_hosts.insert("sts-fips.us-east-1.api.aws".to_string());
            }
            DestinationPolicy::Restricted { allowed_hosts }
        };
        controller.publish(policy.revision(), destination);
        let factory = codex_http_client::HttpClientFactory::new(
            codex_http_client::OutboundProxyPolicy::ReqwestDefault,
        )
        .with_network_policy(policy.for_current_account());
        let config = AwsAuthConfig {
            profile: Some("managed".into()),
            region: Some("us-east-1".into()),
            service: "bedrock".into(),
        };
        let context = if case == "env-precedence" || case.starts_with("endpoint-modes-") {
            AwsAuthContext::load(config, factory).await?
        } else {
            AwsAuthContext::load_profile(config, factory).await?
        };
        let signed = context.sign(super::tests::test_request()).await;
        if case.starts_with("endpoint-modes-") {
            assert!(
                matches!(signed, Err(AwsAuthError::Credentials(_))),
                "{signed:?}"
            );
        } else if case == "restricted-role-process" {
            assert!(
                matches!(
                    signed,
                    Err(Policy(codex_http_client::NetworkPolicyDenied::Destination))
                ),
                "{signed:?}"
            );
        } else {
            let key = match case.as_str() {
                "restricted-static" | "profile-precedence" => "profile-key",
                "env-precedence" => "env-key",
                "unrestricted-role-process" => "assumed-key",
                _ => "process-key",
            };
            assert!(
                signed?.headers[http::header::AUTHORIZATION]
                    .to_str()?
                    .contains(&format!("Credential={key}/"))
            );
        }
        Ok::<_, Box<dyn Error>>(())
    })
}
