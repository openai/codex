use super::OwnedHandle;
use super::PipeConnection;
use super::ServiceRequest;
use super::accept_pipe_connection;
use super::home::prepare_codex_home;
use super::is_config_parse_error;
use super::pin_existing_ancestors;
use super::pipe_security_descriptor;
use super::refresh_session;
use super::request::ProvisioningRequest;
use super::validate_request;
use super::wake;
use codex_windows_sandbox::DirectoryOpenDisposition;
use codex_windows_sandbox::FramedProvisioningMessage;
use codex_windows_sandbox::PROVISIONING_PROTOCOL_VERSION;
use codex_windows_sandbox::ProvisioningMessage;
use codex_windows_sandbox::SandboxProvisioningRequest;
use codex_windows_sandbox::SandboxProvisioningResponse;
use codex_windows_sandbox::SetupRuntime;
use codex_windows_sandbox::WindowsSandboxProvisioningSettings;
use codex_windows_sandbox::WindowsSandboxProxyListeners;
use codex_windows_sandbox::read_provisioning_frame;
use codex_windows_sandbox::to_wide;
use codex_windows_sandbox::write_provisioning_frame;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::path::PathBuf;
use std::ptr;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;
use std::time::Instant;
use windows_sys::Win32::Foundation as foundation;
use windows_sys::Win32::Storage::FileSystem as filesystem;
use windows_sys::Win32::Storage::Packaging::Appx;
use windows_sys::Win32::System::Pipes as pipes;

#[test]
fn session_refresh_observes_cleanup_completion_before_dispatch() {
    let shutdown = AtomicBool::new(false);
    assert!(
        !refresh_session(&shutdown, || {
            shutdown.store(true, Ordering::Release);
            Ok(())
        })
        .unwrap()
    );
}

#[test]
fn session_refresh_preserves_authoritative_stop() {
    let shutdown = AtomicBool::new(true);
    assert!(!refresh_session(&shutdown, || panic!("stopped listener must not refresh")).unwrap());
    assert!(shutdown.load(Ordering::Acquire));
}

#[test]
fn session_refresh_keeps_admission_open_without_cleanup() {
    let shutdown = AtomicBool::new(false);
    assert!(refresh_session(&shutdown, || Ok(())).unwrap());
    assert!(!shutdown.load(Ordering::Acquire));
}

#[test]
fn session_refresh_propagates_cleanup_failure() {
    let shutdown = AtomicBool::new(false);
    let error = refresh_session(&shutdown, || anyhow::bail!("cleanup failed")).unwrap_err();
    assert!(error.to_string().contains("cleanup failed"));
}

#[test]
fn provisioning_request_rejects_malformed_registered_flag() {
    for registered_core in [
        serde_json::json!("true"),
        serde_json::json!(1),
        serde_json::json!(null),
    ] {
        let bytes = serde_json::to_vec(&serde_json::json!({
            "version": PROVISIONING_PROTOCOL_VERSION,
            "type": "provision_sandbox_request",
            "payload": {
                "codex_home": "C:\\Users\\owner\\.codex",
                "registered_core": registered_core,
                "settings": WindowsSandboxProvisioningSettings::default(),
                "listeners": WindowsSandboxProxyListeners::default(),
            },
        }))
        .unwrap();
        let mut frame = (bytes.len() as u32).to_le_bytes().to_vec();
        frame.extend(bytes);
        assert!(validate_request(&frame).is_err());
    }
}

#[test]
fn runtime_registration_is_an_explicit_wire_opt_in() {
    let request = SandboxProvisioningRequest {
        codex_home: r"C:\Users\owner\.codex".to_string(),
        registered_core: false,
        refresh_only: false,
        settings: WindowsSandboxProvisioningSettings::default(),
        listeners: WindowsSandboxProxyListeners::default(),
    };
    let serialized = serde_json::to_value(&request).unwrap();
    assert_eq!(
        serialized,
        serde_json::json!({
            "codex_home": request.codex_home,
            "settings": request.settings,
            "listeners": request.listeners,
        }),
    );
    let legacy: SandboxProvisioningRequest = serde_json::from_value(serialized).unwrap();
    assert!(matches!(
        validate_request(&framed_request(legacy)).unwrap(),
        ServiceRequest::ProvisionSandbox(ProvisioningRequest {
            registered_core: false,
            refresh_only: false,
            ..
        })
    ));
    let registered = SandboxProvisioningRequest {
        registered_core: true,
        refresh_only: false,
        ..request
    };
    assert!(matches!(
        validate_request(&framed_request(registered)).unwrap(),
        ServiceRequest::ProvisionSandbox(ProvisioningRequest {
            registered_core: true,
            refresh_only: false,
            ..
        })
    ));
}

#[test]
fn registration_refresh_is_explicit_and_requires_registered_core() {
    let mut request = SandboxProvisioningRequest {
        codex_home: r"C:\Users\owner\.codex".to_string(),
        registered_core: false,
        refresh_only: true,
        settings: WindowsSandboxProvisioningSettings::default(),
        listeners: WindowsSandboxProxyListeners::default(),
    };
    assert!(validate_request(&framed_request(request.clone())).is_err());
    request.registered_core = true;
    assert!(matches!(
        validate_request(&framed_request(request)).unwrap(),
        ServiceRequest::ProvisionSandbox(ProvisioningRequest {
            registered_core: true,
            refresh_only: true,
            ..
        })
    ));
}

#[test]
fn runtime_registration_has_no_unpackaged_or_foreground_identity_exemption() {
    let service = Some("OpenAI.Codex_testpublisher");
    assert!(crate::package_identity::require_runtime_package_family(service, service).is_ok());
    for (client, service) in [
        (None, service),
        (service, None),
        (None, None),
        (Some("OpenAI.CodexBeta_testpublisher"), service),
        (Some("OpenAI.Codex_otherpublisher"), service),
    ] {
        assert!(crate::package_identity::require_runtime_package_family(client, service).is_err());
    }
}

fn framed_request(request: SandboxProvisioningRequest) -> Vec<u8> {
    let mut frame = Vec::new();
    write_provisioning_frame(
        &mut frame,
        &FramedProvisioningMessage {
            version: PROVISIONING_PROTOCOL_VERSION,
            message: ProvisioningMessage::ProvisionSandboxRequest { payload: request },
        },
    )
    .unwrap();
    frame
}

#[test]
fn unavailable_response_round_trips_through_provisioning_frame() {
    let mut frame = Vec::new();
    write_provisioning_frame(
        &mut frame,
        &FramedProvisioningMessage {
            version: PROVISIONING_PROTOCOL_VERSION,
            message: ProvisioningMessage::ProvisionSandboxResponse {
                payload: SandboxProvisioningResponse::Unavailable,
            },
        },
    )
    .unwrap();

    assert!(matches!(
        read_provisioning_frame(frame.as_slice()).unwrap(),
        Some(FramedProvisioningMessage {
            version: PROVISIONING_PROTOCOL_VERSION,
            message: ProvisioningMessage::ProvisionSandboxResponse {
                payload: SandboxProvisioningResponse::Unavailable,
            },
        })
    ));
}

#[test]
fn config_parse_errors_are_distinguished_from_policy_and_io_failures() {
    let schema_error = codex_core::config::deserialize_config_toml_with_base(
        toml::from_str(r#"model_verbosity = "future-verbosity""#).unwrap(),
        Path::new(r"C:\CodexTest"),
    )
    .unwrap_err();
    let contents = "[";
    let parse_error = toml::from_str::<toml::Value>(contents).unwrap_err();
    let syntax_error = codex_config::io_error_from_config_error(
        std::io::ErrorKind::InvalidData,
        codex_config::config_error_from_toml("config.toml", contents, parse_error.clone()),
        Some(parse_error),
    );
    for error in [schema_error, syntax_error] {
        assert!(is_config_parse_error(
            &anyhow::Error::new(error).context("load managed configuration"),
        ));
    }

    for error in [
        anyhow::anyhow!("managed policy does not permit the elevated Windows sandbox"),
        anyhow::Error::new(std::io::Error::from_raw_os_error(
            foundation::ERROR_ACCESS_DENIED as i32,
        )),
        anyhow::Error::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid credentials",
        )),
    ] {
        assert!(!is_config_parse_error(
            &error.context("load managed configuration"),
        ));
    }
}

#[test]
fn provisioning_request_preserves_home_spaces_and_unicode() {
    let request = framed_request(SandboxProvisioningRequest {
        codex_home: "D:\\Codex Homes\\Jos\u{00e9}\\.codex".to_string(),
        registered_core: false,
        refresh_only: false,
        settings: WindowsSandboxProvisioningSettings::default(),
        listeners: WindowsSandboxProxyListeners::default(),
    });
    assert_eq!(
        validate_request(&request).unwrap(),
        ServiceRequest::ProvisionSandbox(ProvisioningRequest {
            codex_home: PathBuf::from("D:\\Codex Homes\\Jos\u{00e9}\\.codex"),
            registered_core: false,
            refresh_only: false,
            listeners: WindowsSandboxProxyListeners::default(),
            settings: WindowsSandboxProvisioningSettings::default(),
        })
    );
}

#[test]
fn structured_provisioning_request_carries_normalized_proxy_settings() {
    for (http_port, socks_port, proxy_ports) in
        [(8081, 3128, vec![3128, 8081]), (8081, 8081, vec![8081])]
    {
        let request = framed_request(SandboxProvisioningRequest {
            codex_home: "D:\\Codex Homes\\Jos\u{00e9}\\.codex".to_string(),
            registered_core: false,
            refresh_only: false,
            settings: WindowsSandboxProvisioningSettings {
                proxy_ports: vec![http_port, socks_port, http_port],
                allow_local_binding: true,
            },
            listeners: WindowsSandboxProxyListeners {
                http_ports: vec![http_port, http_port],
                socks_ports: vec![socks_port],
            },
        });
        assert_eq!(
            validate_request(&request).unwrap(),
            ServiceRequest::ProvisionSandbox(ProvisioningRequest {
                codex_home: PathBuf::from("D:\\Codex Homes\\Jos\u{00e9}\\.codex"),
                registered_core: false,
                refresh_only: false,
                listeners: WindowsSandboxProxyListeners {
                    http_ports: vec![http_port],
                    socks_ports: vec![socks_port],
                },
                settings: WindowsSandboxProvisioningSettings {
                    proxy_ports,
                    allow_local_binding: true,
                },
            }),
        );
    }
}

#[test]
fn structured_provisioning_request_accepts_independent_and_additional_proxy_ports() {
    for (http_ports, socks_ports, proxy_ports) in [
        (vec![3128], vec![], vec![3128]),
        (vec![], vec![1080], vec![1080]),
        (vec![3128, 8080], vec![1080], vec![1080, 1082, 3128, 8080]),
    ] {
        let settings = WindowsSandboxProvisioningSettings {
            proxy_ports,
            allow_local_binding: false,
        };
        let listeners = WindowsSandboxProxyListeners {
            http_ports,
            socks_ports,
        };
        let request = framed_request(SandboxProvisioningRequest {
            codex_home: r"C:\Users\alice\.codex".to_string(),
            registered_core: false,
            refresh_only: false,
            settings: settings.clone(),
            listeners: listeners.clone(),
        });
        assert_eq!(
            validate_request(&request).unwrap(),
            ServiceRequest::ProvisionSandbox(ProvisioningRequest {
                codex_home: PathBuf::from(r"C:\Users\alice\.codex"),
                registered_core: false,
                refresh_only: false,
                settings,
                listeners,
            })
        );
    }
}

#[test]
fn structured_provisioning_request_accepts_disabled_listeners() {
    let request = framed_request(SandboxProvisioningRequest {
        codex_home: r"C:\Users\alice\.codex".to_string(),
        registered_core: false,
        refresh_only: false,
        settings: WindowsSandboxProvisioningSettings::default(),
        listeners: WindowsSandboxProxyListeners::default(),
    });
    assert_eq!(
        validate_request(&request).unwrap(),
        ServiceRequest::ProvisionSandbox(ProvisioningRequest {
            codex_home: PathBuf::from(r"C:\Users\alice\.codex"),
            registered_core: false,
            refresh_only: false,
            listeners: WindowsSandboxProxyListeners::default(),
            settings: WindowsSandboxProvisioningSettings::default(),
        })
    );
}

#[test]
fn structured_provisioning_request_requires_exact_version_fields_and_framing() {
    let valid = SandboxProvisioningRequest {
        codex_home: r"C:\Users\alice\.codex".to_string(),
        registered_core: false,
        refresh_only: false,
        settings: WindowsSandboxProvisioningSettings::default(),
        listeners: WindowsSandboxProxyListeners::default(),
    };
    let mut invalid_version = Vec::new();
    write_provisioning_frame(
        &mut invalid_version,
        &FramedProvisioningMessage {
            version: PROVISIONING_PROTOCOL_VERSION + 1,
            message: ProvisioningMessage::ProvisionSandboxRequest {
                payload: valid.clone(),
            },
        },
    )
    .unwrap();
    assert!(validate_request(&invalid_version).is_err());

    let mut unexpected_message = Vec::new();
    write_provisioning_frame(
        &mut unexpected_message,
        &FramedProvisioningMessage {
            version: PROVISIONING_PROTOCOL_VERSION,
            message: ProvisioningMessage::ProvisionSandboxResponse {
                payload: SandboxProvisioningResponse::Ok,
            },
        },
    )
    .unwrap();
    assert!(validate_request(&unexpected_message).is_err());

    let valid = framed_request(valid);
    assert!(validate_request(&valid[..valid.len() - 1]).is_err());
    let mut duplicate = valid.clone();
    duplicate.extend_from_slice(&valid);
    assert!(validate_request(&duplicate).is_err());
}

#[test]
fn structured_provisioning_request_rejects_invalid_or_inconsistent_ports() {
    for (proxy_ports, http_ports, socks_ports) in [
        (vec![0], vec![], vec![]),
        (vec![3128], vec![0], vec![]),
        (vec![3128], vec![3128], vec![0]),
        (vec![3128], vec![3128], vec![8081]),
    ] {
        let request = framed_request(SandboxProvisioningRequest {
            codex_home: r"C:\Users\alice\.codex".to_string(),
            registered_core: false,
            refresh_only: false,
            settings: WindowsSandboxProvisioningSettings {
                proxy_ports,
                allow_local_binding: false,
            },
            listeners: WindowsSandboxProxyListeners {
                http_ports,
                socks_ports,
            },
        });
        assert!(validate_request(&request).is_err());
    }
}

#[test]
fn provisioning_request_rejects_empty_control_characters_and_invalid_utf8() {
    for home in ["", "C:\\safe\0evil", "C:\\safe\rmore", "C:\\safe\nmore"] {
        let request = framed_request(SandboxProvisioningRequest {
            codex_home: home.to_string(),
            registered_core: false,
            refresh_only: false,
            settings: WindowsSandboxProvisioningSettings::default(),
            listeners: WindowsSandboxProxyListeners::default(),
        });
        assert!(validate_request(&request).is_err());
    }

    let mut invalid_utf8 = framed_request(SandboxProvisioningRequest {
        codex_home: r"C:\Users\alice\.codex".to_string(),
        registered_core: false,
        refresh_only: false,
        settings: WindowsSandboxProvisioningSettings::default(),
        listeners: WindowsSandboxProxyListeners::default(),
    });
    invalid_utf8[std::mem::size_of::<u32>()] = 0xff;
    assert!(validate_request(&invalid_utf8).is_err());
}

#[test]
fn unpackaged_pipe_clients_are_rejected_before_sending_a_request() {
    let mut family_length = 0;
    let package_status =
        unsafe { Appx::GetCurrentPackageFamilyName(&mut family_length, ptr::null_mut()) };
    if package_status != foundation::APPMODEL_ERROR_NO_PACKAGE {
        assert_eq!(package_status, foundation::ERROR_INSUFFICIENT_BUFFER);
        return;
    }

    static NEXT_PIPE_INSTANCE: AtomicU64 = AtomicU64::new(0);
    let name = to_wide(format!(
        r"\\.\pipe\OpenAI.CodexSandbox.Tests.{}.{}",
        std::process::id(),
        NEXT_PIPE_INSTANCE.fetch_add(1, Ordering::Relaxed)
    ));
    let server = unsafe {
        pipes::CreateNamedPipeW(
            name.as_ptr(),
            filesystem::PIPE_ACCESS_DUPLEX | filesystem::FILE_FLAG_FIRST_PIPE_INSTANCE,
            pipes::PIPE_TYPE_BYTE
                | pipes::PIPE_READMODE_BYTE
                | pipes::PIPE_WAIT
                | pipes::PIPE_REJECT_REMOTE_CLIENTS,
            1,
            1024,
            1024,
            0,
            ptr::null(),
        )
    };
    assert_ne!(server, foundation::INVALID_HANDLE_VALUE);
    let server = OwnedHandle(server);

    let client = unsafe {
        filesystem::CreateFileW(
            name.as_ptr(),
            foundation::GENERIC_READ | foundation::GENERIC_WRITE,
            0,
            ptr::null(),
            filesystem::OPEN_EXISTING,
            0,
            0,
        )
    };
    assert_ne!(client, foundation::INVALID_HANDLE_VALUE);
    let _client = OwnedHandle(client);

    let connected = unsafe { pipes::ConnectNamedPipe(server.0, ptr::null_mut()) };
    assert!(
        connected != 0 || unsafe { foundation::GetLastError() } == foundation::ERROR_PIPE_CONNECTED
    );

    let error = match crate::package_identity::authorize_client_process(server.0) {
        Ok(_) => panic!("an unpackaged client was authorized without sending a request"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("installed Codex package identity"),
        "unexpected package authorization failure: {error:#}"
    );
}

#[test]
fn disconnected_pipe_clients_do_not_prevent_subsequent_connections() {
    let pipe_name = format!(
        r"\\.\pipe\OpenAI.CodexSandbox.DisconnectTests.{}",
        std::process::id()
    );
    let name = to_wide(&pipe_name);
    let server = unsafe {
        pipes::CreateNamedPipeW(
            name.as_ptr(),
            filesystem::PIPE_ACCESS_DUPLEX | filesystem::FILE_FLAG_FIRST_PIPE_INSTANCE,
            pipes::PIPE_TYPE_BYTE
                | pipes::PIPE_READMODE_BYTE
                | pipes::PIPE_WAIT
                | pipes::PIPE_REJECT_REMOTE_CLIENTS,
            1,
            1024,
            1024,
            0,
            ptr::null(),
        )
    };
    assert_ne!(server, foundation::INVALID_HANDLE_VALUE);
    let server = OwnedHandle(server);

    let first_client = unsafe {
        filesystem::CreateFileW(
            name.as_ptr(),
            foundation::GENERIC_READ | foundation::GENERIC_WRITE,
            0,
            ptr::null(),
            filesystem::OPEN_EXISTING,
            0,
            0,
        )
    };
    assert_ne!(first_client, foundation::INVALID_HANDLE_VALUE);
    drop(OwnedHandle(first_client));

    assert_eq!(
        accept_pipe_connection(server.0).unwrap(),
        PipeConnection::Disconnected
    );

    let server_handle = server.0;
    let listener = std::thread::spawn(move || accept_pipe_connection(server_handle));
    let available = unsafe { pipes::WaitNamedPipeW(name.as_ptr(), 5_000) };
    assert_ne!(
        available,
        0,
        "wait for replacement named-pipe listener: {}",
        std::io::Error::last_os_error()
    );
    let next_client = unsafe {
        filesystem::CreateFileW(
            name.as_ptr(),
            foundation::GENERIC_READ | foundation::GENERIC_WRITE,
            0,
            ptr::null(),
            filesystem::OPEN_EXISTING,
            0,
            0,
        )
    };
    assert_ne!(
        next_client,
        foundation::INVALID_HANDLE_VALUE,
        "connect replacement named-pipe client: {}",
        std::io::Error::last_os_error()
    );
    let next_client = OwnedHandle(next_client);
    assert_eq!(
        listener.join().expect("join named-pipe listener").unwrap(),
        PipeConnection::Connected
    );

    drop(next_client);
    assert_ne!(unsafe { pipes::DisconnectNamedPipe(server.0) }, 0);

    let timeout = Duration::from_secs(5);
    let deadline = Instant::now() + timeout;
    let (checks_tx, checks_rx) = mpsc::channel();
    let waker = std::thread::spawn(move || {
        wake(&pipe_name, || {
            let _ = checks_tx.send(());
            Instant::now() >= deadline
        });
    });
    checks_rx.recv_timeout(timeout).unwrap();
    // The second check proves a wake failed before the listener started reconnecting.
    checks_rx.recv_timeout(timeout).unwrap();

    let (accepted_tx, accepted_rx) = mpsc::channel();
    let listener = std::thread::spawn(move || {
        let _ = accepted_tx.send(accept_pipe_connection(server.0));
        drop(server);
    });
    let accepted = accepted_rx.recv_timeout(timeout);
    waker.join().expect("join shutdown waker");
    // The wake handle may close before ConnectNamedPipe observes the connection.
    assert!(
        accepted
            .expect("shutdown wake should unblock the listener")
            .is_ok()
    );
    listener.join().expect("join shutdown listener");
}

#[test]
fn unexpected_pipe_connection_errors_remain_fatal() {
    let error = accept_pipe_connection(foundation::INVALID_HANDLE_VALUE).unwrap_err();

    assert_eq!(
        error
            .downcast_ref::<std::io::Error>()
            .and_then(std::io::Error::raw_os_error),
        Some(foundation::ERROR_INVALID_HANDLE as i32)
    );
}

#[test]
fn pin_existing_ancestors_accepts_drive_and_verbatim_drive_roots() {
    let executable = std::env::current_exe().unwrap().canonicalize().unwrap();
    let verbatim_root = executable.ancestors().last().unwrap();
    let ordinary_root = PathBuf::from(
        verbatim_root
            .to_str()
            .unwrap()
            .strip_prefix(r"\\?\")
            .unwrap(),
    );

    for root in [ordinary_root.as_path(), verbatim_root] {
        let mut handles = Vec::new();
        pin_existing_ancestors(root, &mut handles).unwrap();
        assert_eq!(handles.len(), 1);
    }
}

#[test]
fn pipe_descriptor_denies_sandbox_group_before_interactive_users() {
    let descriptor = pipe_security_descriptor("S-1-5-21-11-12-13-14");
    let deny = descriptor.find("(D;;GA;;;S-1-5-21-11-12-13-14)").unwrap();
    let interactive = descriptor.find("(A;;0x0012019b;;;IU)").unwrap();
    assert!(deny < interactive);
}

#[test]
fn registered_home_preparation_does_not_touch_the_legacy_bin() -> anyhow::Result<()> {
    let root = std::env::temp_dir().join(format!(
        "codex-service-home-{:?}",
        windows::core::GUID::new()?
    ));
    std::fs::create_dir(&root)?;
    let result = (|| -> anyhow::Result<()> {
        let home = root.join(".codex");
        std::fs::create_dir(&home)?;
        let bin = home.join(".sandbox-bin");
        let marker = b"legacy bin is deliberately not a directory";
        std::fs::write(&bin, marker)?;

        assert!(
            prepare_codex_home(
                &home,
                SetupRuntime::Registered,
                DirectoryOpenDisposition::OpenExisting,
            )
            .is_err()
        );
        assert!(!home.join(".sandbox").exists());
        assert!(!home.join(".sandbox-secrets").exists());
        let (_, handles) = prepare_codex_home(
            &home,
            SetupRuntime::Registered,
            DirectoryOpenDisposition::OpenOrCreate,
        )?;
        assert!(home.join(".sandbox").is_dir());
        assert!(home.join(".sandbox-secrets").is_dir());
        assert_eq!(std::fs::read(&bin)?.as_slice(), marker.as_slice());
        drop(handles);

        // Legacy still validates and pins the copied-bin path.
        let (_, handles) = prepare_codex_home(
            &home,
            SetupRuntime::Registered,
            DirectoryOpenDisposition::OpenExisting,
        )?;
        drop(handles);
        assert!(
            prepare_codex_home(
                &home,
                SetupRuntime::Legacy,
                DirectoryOpenDisposition::OpenOrCreate,
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&bin)?.as_slice(), marker.as_slice());
        std::fs::remove_file(&bin)?;
        let (_, handles) = prepare_codex_home(
            &home,
            SetupRuntime::Legacy,
            DirectoryOpenDisposition::OpenOrCreate,
        )?;
        assert!(bin.is_dir());
        drop(handles);

        Ok(())
    })();
    std::fs::remove_dir_all(&root)?;
    result
}

#[test]
fn installation_registration_requires_no_sandbox_settings() {
    let mut frame = Vec::new();
    write_provisioning_frame(
        &mut frame,
        &FramedProvisioningMessage {
            version: PROVISIONING_PROTOCOL_VERSION,
            message: ProvisioningMessage::RegisterInstallationRequest {
                codex_home: r"C:\Users\alice\.codex".to_string(),
            },
        },
    )
    .unwrap();
    assert_eq!(
        validate_request(&frame).unwrap(),
        ServiceRequest::RegisterInstallation {
            codex_home: PathBuf::from(r"C:\Users\alice\.codex")
        },
    );
}
