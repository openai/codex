//! Child environment admission excludes loader hooks, plugin search paths, and app credentials.

use pretty_assertions::assert_eq;

use super::child_environment;

#[test]
fn forwards_only_explicit_device_network_and_os_inputs() {
    let input = [
        ("SystemRoot", "C:\\Windows"),
        ("https_proxy", "http://proxy"),
        ("HOME", "/home/user"),
        ("LD_PRELOAD", "loader"),
        ("DYLD_INSERT_LIBRARIES", "loader"),
        ("PATH", "project"),
        ("GST_PLUGIN_PATH", "plugins"),
        ("ALSA_PLUGIN_DIR", "untrusted-alsa-plugins"),
        ("GST_REGISTRY", "untrusted-registry"),
        ("GST_REGISTRY_FORK", "yes"),
        ("OPENAI_API_KEY", "secret"),
    ];
    assert_eq!(
        child_environment(
            input
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
        ),
        input[..3]
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .chain(crate::RUNTIME_ENVIRONMENT.map(|(key, value)| (key.into(), value.into())))
            .collect()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn cancelling_initialization_terminates_the_owned_helper() -> anyhow::Result<()> {
    let spawned = codex_utils_pty::spawn_pipe_process(
        std::path::Path::new("/bin/sleep"),
        &["30".to_owned()],
        std::path::Path::new("/"),
        &child_environment(std::iter::empty()),
        /*arg0*/ &None,
        &[],
    )
    .await?;
    drop(spawned.stderr_rx);
    let (_, unused_exit) = tokio::sync::oneshot::channel();
    let host = super::VoiceHost {
        process: spawned.session,
        output: crate::message_reader::MessageReader::new(spawned.stdout_rx),
        exit: unused_exit,
        observed_exit: None,
    };
    let mut initialization = Box::pin(host.initialize_runtime());
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(initialization.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(initialization);
    tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 1), spawned.exit_rx).await??;
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn negotiation_timeout_waits_for_helper_exit_before_allowing_retry() -> anyhow::Result<()> {
    let spawned = codex_utils_pty::spawn_pipe_process(
        std::path::Path::new("/bin/sleep"),
        &["30".to_owned()],
        std::path::Path::new("/"),
        &child_environment(std::iter::empty()),
        /*arg0*/ &None,
        &[],
    )
    .await?;
    let (output, receiver) = tokio::sync::mpsc::channel(/*buffer*/ 1);
    output
        .send(crate::encode_frame(&crate::Message::TransportTimedOut {})?)
        .await?;
    let (reaped, exit) = tokio::sync::oneshot::channel();
    let host = super::VoiceHost {
        process: spawned.session,
        output: crate::message_reader::MessageReader::new(receiver),
        exit,
        observed_exit: None,
    };
    let answer = crate::SessionDescription::try_from("synthetic-answer".to_owned()).unwrap();
    let mut connection = Box::pin(host.apply_answer(answer));
    std::future::poll_fn(|cx| {
        assert!(std::future::Future::poll(connection.as_mut(), cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    let code =
        tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 1), spawned.exit_rx).await??;
    // The process has exited, but the explicit reaping acknowledgement is still required.
    std::future::poll_fn(|cx| {
        assert!(std::future::Future::poll(connection.as_mut(), cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    reaped.send(code).unwrap();
    let error = connection
        .await
        .err()
        .expect("timeout must not return a connected helper");
    assert_eq!(
        error.downcast_ref::<crate::ConnectionError>(),
        Some(&crate::ConnectionError::NegotiationTimedOut)
    );
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn close_after_output_eof_reuses_observed_exit() -> anyhow::Result<()> {
    let spawned = codex_utils_pty::spawn_pipe_process(
        std::path::Path::new("/bin/sleep"),
        &["30".to_owned()],
        std::path::Path::new("/"),
        &child_environment(std::iter::empty()),
        /*arg0*/ &None,
        &[],
    )
    .await?;
    let (output, receiver) = tokio::sync::mpsc::channel(/*buffer*/ 1);
    drop(output);
    let (reaped, exit) = tokio::sync::oneshot::channel();
    let exit_code = crate::HelperExitStage::AudioService.code();
    reaped.send(exit_code).unwrap();
    let mut host = super::VoiceHost {
        process: spawned.session,
        output: crate::message_reader::MessageReader::new(receiver),
        exit,
        observed_exit: None,
    };
    assert!(host.next_response().await.is_err());
    assert_eq!(host.observed_exit, Some(Some(exit_code)));
    assert!(host.close().await.is_err());
    Ok(())
}
