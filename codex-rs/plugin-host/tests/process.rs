use codex_plugin_host::PluginProcess;
use codex_plugin_host::PluginProcessError;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
#[cfg(unix)]
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

// Sibling forks can retain a copied executable's writable descriptor and cause ETXTBSY.
static PROCESS_FIXTURES: tokio::sync::Semaphore =
    tokio::sync::Semaphore::const_new(/*permits*/ 1);

#[tokio::test]
async fn owns_a_separate_process_group_and_reaps_the_helper()
-> Result<(), Box<dyn std::error::Error>> {
    let _fixture = PROCESS_FIXTURES.acquire().await?;
    let program = codex_utils_cargo_bin::cargo_bin("codex-plugin-test-helper")?;
    let mut child = PluginProcess::spawn(&program, &CancellationToken::new()).await?;
    let (input, output) = child.pipes();
    #[cfg(unix)]
    {
        let bytes = output.next().await.expect("expected frame")?;
        let (pid, group): (u32, i32) = serde_json::from_slice(&bytes)?;
        assert_eq!(group, pid as i32);
        // SAFETY: getpgrp reads the test process's group identifier.
        assert_ne!(group, unsafe { libc::getpgrp() });
    }
    input.feed(b"hello".to_vec().into()).await?;
    input.flush().await?;
    assert_eq!(
        output.next().await.expect("expected frame")?.as_ref(),
        b"hello"
    );
    input.feed(b"spawn-descendant".to_vec().into()).await?;
    input.flush().await?;
    assert_eq!(
        output.next().await.expect("expected frame")?.as_ref(),
        b"descendant-ready"
    );
    child.close().await?;
    assert!(child.pipes().1.next().await.transpose()?.is_none());
    child.close().await?;
    Ok(())
}

#[tokio::test]
async fn already_canceled_startup_does_not_launch_the_helper()
-> Result<(), Box<dyn std::error::Error>> {
    let _fixture = PROCESS_FIXTURES.acquire().await?;
    let program = codex_utils_cargo_bin::cargo_bin("codex-plugin-test-helper")?;
    let stop = CancellationToken::new();
    stop.cancel();
    let error = PluginProcess::spawn(&program, &stop)
        .await
        .err()
        .expect("canceled startup succeeded");
    let PluginProcessError::Launch(source) = error else {
        panic!("expected a launch error, got {error:?}");
    };
    assert_eq!(source.kind(), std::io::ErrorKind::Interrupted);
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn dropping_the_helper_also_closes_descendant_pipes() -> Result<(), Box<dyn std::error::Error>>
{
    let _fixture = PROCESS_FIXTURES.acquire().await?;
    let directory = tempfile::tempdir()?;
    let socket = directory.path().join("descendant.sock");
    let listener = tokio::net::UnixListener::bind(&socket)?;

    let program = codex_utils_cargo_bin::cargo_bin("codex-plugin-test-helper")?;
    let mut child = PluginProcess::spawn(&program, &CancellationToken::new()).await?;
    let (input, output) = child.pipes();
    let _group = output.next().await.expect("expected frame")?;
    input
        .send(socket.as_os_str().as_encoded_bytes().to_vec().into())
        .await?;
    let (mut descendant, _) = listener.accept().await?;
    drop(child);
    assert_eq!(descendant.read(&mut [0]).await?, 0);
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn caller_can_cancel_a_stalled_handshake_and_reap_the_helper()
-> Result<(), Box<dyn std::error::Error>> {
    let _fixture = PROCESS_FIXTURES.acquire().await?;
    let directory = tempfile::tempdir()?;
    let program = directory.path().join("unready-plugin-probe");
    std::fs::copy(
        codex_utils_cargo_bin::cargo_bin("codex-plugin-test-helper")?,
        &program,
    )?;
    let listener = tokio::net::UnixListener::bind(program.with_extension("sock"))?;
    let stop = CancellationToken::new();
    let spawn = PluginProcess::spawn(&program, &stop);
    tokio::pin!(spawn);
    let pid = tokio::select! {
        _ = &mut spawn => panic!("unready helper completed startup"),
        pid = async {
            let (mut signal, _) = listener.accept().await?;
            signal.read_i32_le().await
        } => pid?,
    };
    stop.cancel();
    let error = match spawn.await {
        Ok(_) => panic!("canceled startup succeeded"),
        Err(error) => error,
    };
    let PluginProcessError::Handshake {
        source,
        close_result,
    } = error
    else {
        panic!("expected a handshake error, got {error:?}");
    };
    assert_eq!(source.kind(), std::io::ErrorKind::Interrupted);
    close_result?;
    // SAFETY: getpgid only inspects the child recorded by the test probe.
    assert_eq!(unsafe { libc::getpgid(pid) }, -1);
    Ok(())
}

#[tokio::test]
async fn preserves_exit_status_and_bounded_stderr_from_startup_and_runtime_failures()
-> Result<(), Box<dyn std::error::Error>> {
    let _fixture = PROCESS_FIXTURES.acquire().await?;
    let directory = tempfile::tempdir()?;
    let failing = directory.path().join(format!(
        "failing-plugin-probe{}",
        std::env::consts::EXE_SUFFIX
    ));
    let program = codex_utils_cargo_bin::cargo_bin("codex-plugin-test-helper")?;
    std::fs::copy(&program, &failing)?;
    for executable in [&failing, &program] {
        let exit = if executable == &failing {
            let error = match PluginProcess::spawn(executable, &CancellationToken::new()).await {
                Ok(_) => panic!("failing helper started"),
                Err(error) => error,
            };
            let PluginProcessError::Handshake {
                source,
                close_result,
            } = error
            else {
                panic!("expected a handshake error, got {error:?}");
            };
            assert_eq!(source.kind(), std::io::ErrorKind::UnexpectedEof);
            close_result?
        } else {
            let mut child = PluginProcess::spawn(executable, &CancellationToken::new()).await?;
            let (input, output) = child.pipes();
            #[cfg(unix)]
            let _group = output.next().await.expect("expected frame")?;
            input.feed(b"crash".to_vec().into()).await?;
            input.flush().await?;
            assert!(output.next().await.transpose()?.is_none());
            let exit = child.close().await?;
            assert!(std::sync::Arc::ptr_eq(&exit, &child.close().await?));
            exit
        };
        let mut expected = vec![b'x'; 16 * 1024 - b"probe failed\n".len()];
        expected.extend_from_slice(b"probe failed\n");
        assert_eq!(
            (
                exit.status.code(),
                &exit.stderr_tail,
                exit.group_termination_error
                    .as_ref()
                    .map(std::io::Error::kind),
                exit.stderr_read_error
            ),
            (Some(42), &expected, None, None)
        );
        assert!(!format!("{exit:?}").contains("probe failed"));
    }
    Ok(())
}
