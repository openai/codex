use super::*;
use pretty_assertions::assert_eq;

fn render_visible(state: &BedrockState) -> String {
    let area = Rect::new(0, 0, 72, 24);
    let mut buffer = Buffer::empty(area);
    state.render(area, &mut buffer, /*error*/ None);
    let mut rows = (area.top()..area.bottom())
        .map(|row| {
            (area.left()..area.right())
                .map(|column| buffer[(column, row)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>();
    while rows.last().is_some_and(String::is_empty) {
        rows.pop();
    }
    rows.join("\n")
}

#[test]
fn discovery_prioritizes_profiles_and_keeps_bedrock_api_key_last() {
    let discovering = BedrockState::discovering(RequestId::Integer(1));
    insta::assert_snapshot!(render_visible(&discovering), @r###"
    > Set up Amazon Bedrock

      Checking for existing AWS credentials...
    "###);

    let mut configuring = discovering;
    configuring.view = BedrockView::Configuring(RequestId::Integer(2));
    insta::assert_snapshot!(render_visible(&configuring), @r###"
    > Set up Amazon Bedrock

      Setting up Amazon Bedrock...

      Press esc to go back
    "###);
    assert!(
        configuring
            .handle_key_event(&KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .is_none()
    );
    assert!(matches!(
        configuring.view,
        BedrockView::Methods(BedrockMethodList::Detected)
    ));

    let profile = BedrockAwsProfile {
        name: "engineering".to_string(),
        region: Some("us-east-2".to_string()),
    };
    let environment = BedrockEnvironmentCredential {
        credential_type: AwsCredentialType::AccessKeys,
        region: Some("us-west-2".to_string()),
    };
    for (profiles, environment_credentials, expected) in [
        (
            vec![profile.clone()],
            vec![environment.clone()],
            vec![
                BedrockMethod::Profile(0),
                BedrockMethod::Environment,
                BedrockMethod::OtherMethods,
                BedrockMethod::ApiKey,
            ],
        ),
        (
            vec![
                profile.clone(),
                BedrockAwsProfile {
                    name: "production".to_string(),
                    region: Some("us-west-2".to_string()),
                },
            ],
            vec![environment.clone()],
            vec![
                BedrockMethod::Profile(0),
                BedrockMethod::Profile(1),
                BedrockMethod::Environment,
                BedrockMethod::OtherMethods,
                BedrockMethod::ApiKey,
            ],
        ),
        (
            Vec::new(),
            vec![environment],
            vec![
                BedrockMethod::Environment,
                BedrockMethod::OtherMethods,
                BedrockMethod::ApiKey,
            ],
        ),
        (
            Vec::new(),
            Vec::new(),
            vec![
                BedrockMethod::ManualProfile,
                BedrockMethod::AccessKeys,
                BedrockMethod::EnvironmentInstructions,
                BedrockMethod::ApiKey,
            ],
        ),
    ] {
        let state = BedrockState::discovered(BedrockDiscoverResponse {
            profiles,
            environment_credentials,
        });
        assert_eq!(state.methods(), expected);
    }

    let mut profile_and_bearer = BedrockState::discovered(BedrockDiscoverResponse {
        profiles: vec![profile.clone()],
        environment_credentials: vec![BedrockEnvironmentCredential {
            credential_type: AwsCredentialType::BedrockApiKey,
            region: Some("us-east-2".to_string()),
        }],
    });
    assert_eq!(
        profile_and_bearer.methods(),
        vec![
            BedrockMethod::Profile(0),
            BedrockMethod::Environment,
            BedrockMethod::OtherMethods,
            BedrockMethod::ApiKey,
        ]
    );
    insta::assert_snapshot!(render_visible(&profile_and_bearer), @r###"
    > Set up Amazon Bedrock

      AWS profile detected: engineering
      Region: us-east-2

    > 1. Continue with engineering
         Use your existing AWS credentials

      2. Continue with detected credentials
         Use your existing Amazon Bedrock API key

      3. Other AWS sign-in methods
         Use another profile, access keys, or environment variables

      4. Bedrock API key
         Enter a Bedrock API key


      Press enter to continue
      Press esc to go back
    "###);
    profile_and_bearer.select_method(BedrockMethod::OtherMethods);
    assert_eq!(
        profile_and_bearer.methods(),
        vec![
            BedrockMethod::ManualProfile,
            BedrockMethod::AccessKeys,
            BedrockMethod::EnvironmentInstructions,
        ]
    );
    insta::assert_snapshot!(render_visible(&profile_and_bearer), @r"
    > Set up Amazon Bedrock

      Choose how you authenticate with AWS.

    > 1. AWS profile
         Use AWS SSO or a named profile

      2. AWS access keys
         Enter an access key ID and secret access key

      3. Environment variables
         Configure AWS credentials in your environment, then return here


      Press enter to continue
      Press esc to go back
    ");

    let profile_state = BedrockState::discovered(BedrockDiscoverResponse {
        profiles: vec![profile],
        environment_credentials: Vec::new(),
    });
    insta::assert_snapshot!(render_visible(&profile_state), @r###"
    > Set up Amazon Bedrock

      AWS profile detected: engineering
      Region: us-east-2

    > 1. Continue with engineering
         Use your existing AWS credentials

      2. Other AWS sign-in methods
         Use another profile, access keys, or environment variables

      3. Bedrock API key
         Enter a Bedrock API key


      Press enter to continue
      Press esc to go back
    "###);

    let multiple_profiles = BedrockState::discovered(BedrockDiscoverResponse {
        profiles: vec![
            BedrockAwsProfile {
                name: "engineering".to_string(),
                region: Some("us-east-2".to_string()),
            },
            BedrockAwsProfile {
                name: "production".to_string(),
                region: Some("us-west-2".to_string()),
            },
        ],
        environment_credentials: Vec::new(),
    });
    insta::assert_snapshot!(render_visible(&multiple_profiles), @r###"
    > Set up Amazon Bedrock

      Choose an AWS profile.

    > 1. engineering
         us-east-2

      2. production
         us-west-2

      3. Other AWS sign-in methods
         Use another profile, access keys, or environment variables

      4. Bedrock API key
         Enter a Bedrock API key


      Press enter to continue
      Press esc to go back
    "###);

    let environment_state = BedrockState::discovered(BedrockDiscoverResponse {
        profiles: Vec::new(),
        environment_credentials: vec![BedrockEnvironmentCredential {
            credential_type: AwsCredentialType::AccessKeys,
            region: Some("us-west-2".to_string()),
        }],
    });
    insta::assert_snapshot!(render_visible(&environment_state), @r###"
    > Set up Amazon Bedrock

      AWS credentials detected in your environment.

    > 1. Continue with detected credentials

      2. Other AWS sign-in methods

      3. Bedrock API key


      Press enter to continue
      Press esc to go back
    "###);

    let empty_state = BedrockState::discovered(BedrockDiscoverResponse {
        profiles: Vec::new(),
        environment_credentials: Vec::new(),
    });
    insta::assert_snapshot!(render_visible(&empty_state), @r"
    > Set up Amazon Bedrock

      No AWS credentials found.
      Choose how you authenticate with AWS.

    > 1. AWS profile
         Use AWS SSO or a named profile

      2. AWS access keys
         Enter an access key ID and secret access key

      3. Environment variables
         Configure AWS credentials in your environment, then return here

      4. Bedrock API key
         Enter a Bedrock API key


      Press enter to continue
      Press esc to go back
    ");

    let mut many_profiles = BedrockState::discovered(BedrockDiscoverResponse {
        profiles: (0..12)
            .map(|index| BedrockAwsProfile {
                name: format!("engineering-{index}"),
                region: Some("us-east-2".to_string()),
            })
            .collect(),
        environment_credentials: Vec::new(),
    });
    many_profiles.highlighted = many_profiles.methods().len() - 1;
    let area = Rect::new(0, 0, 72, 10);
    let mut buffer = Buffer::empty(area);
    many_profiles.render(area, &mut buffer, /*error*/ None);
    let visible = (area.top()..area.bottom())
        .flat_map(|row| (area.left()..area.right()).map(move |column| (column, row)))
        .map(|position| buffer[position].symbol())
        .collect::<String>();
    assert!(visible.contains("Bedrock API key"));
    assert!(visible.contains("Press enter to continue"));
    assert!(visible.contains("Press esc to go back"));

    let narrow_area = Rect::new(0, 0, 24, 12);
    let mut narrow_buffer = Buffer::empty(narrow_area);
    empty_state.render(
        narrow_area,
        &mut narrow_buffer,
        Some("AWS credentials unavailable in this terminal".to_string()),
    );
    let narrow_visible = (narrow_area.top()..narrow_area.bottom())
        .map(|row| {
            (narrow_area.left()..narrow_area.right())
                .map(|column| narrow_buffer[(column, row)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(narrow_visible.contains("Press enter to"));
    assert!(narrow_visible.contains("continue"));
    assert!(narrow_visible.contains("Press esc to go back"));
    assert!(narrow_visible.contains("AWS credentials"));
    assert!(narrow_visible.contains("unavailable"));
}

#[test]
fn credential_entries_share_region_and_keep_aws_secrets_hidden() {
    for (profiles, environment_credentials, method) in [
        (
            vec![BedrockAwsProfile {
                name: "engineering".to_string(),
                region: None,
            }],
            Vec::new(),
            BedrockMethod::Profile(0),
        ),
        (
            Vec::new(),
            vec![BedrockEnvironmentCredential {
                credential_type: AwsCredentialType::AccessKeys,
                region: None,
            }],
            BedrockMethod::Environment,
        ),
    ] {
        let mut state = BedrockState::discovered(BedrockDiscoverResponse {
            profiles,
            environment_credentials,
        });
        state.select_method(method);
        assert!(matches!(&state.view, BedrockView::RegionEntry { .. }));
        assert!(state.is_text_entry_active());
    }

    let mut access_keys = BedrockState::discovered(BedrockDiscoverResponse {
        profiles: Vec::new(),
        environment_credentials: Vec::new(),
    });
    access_keys.view = BedrockView::AccessKeyEntry {
        values: [
            "AKIAEXAMPLE".to_string(),
            "secret-value".to_string(),
            "session-value".to_string(),
        ],
        selected_field: 1,
    };
    let visible = render_visible(&access_keys);
    insta::assert_snapshot!(visible, @r###"
    > Set up Amazon Bedrock

      Enter your AWS access keys.

      AWS access key ID: AKIAEXAMPLE
    > AWS secret access key: •••••••••••e
      AWS session token (optional): •••••••••••••

      Press enter to continue
      Press esc to go back
    "###);
    assert!(visible.contains("AKIAEXAMPLE"));
    assert!(visible.contains("AWS secret access key: •••••••••••e"));
    assert!(!visible.contains("secret-value"));
    assert!(!visible.contains("session-value"));

    access_keys.handle_key_event(&KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
    assert!(render_visible(&access_keys).contains("AWS secret access key: ••••••••••••x"));

    access_keys.handle_key_event(&KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    let visible = render_visible(&access_keys);
    insta::assert_snapshot!(visible, @r###"
    > Set up Amazon Bedrock

      Enter your AWS access keys.

      AWS access key ID: AKIAEXAMPLE
      AWS secret access key: •••••••••••••
    > AWS session token (optional): ••••••••••••e

      Press enter to continue
      Press esc to go back
    "###);
    assert!(!visible.contains("secret-value"));
    assert!(!visible.contains("session-value"));

    access_keys.view = BedrockView::ApiKeyEntry("bedrock-api-key".to_string());
    let visible = render_visible(&access_keys);
    insta::assert_snapshot!(visible, @r###"
    > Set up Amazon Bedrock

      Enter your Amazon Bedrock API key.

      Bedrock API key: ••••••••••••••y

      Press enter to continue
      Press esc to go back
    "###);
    assert!(!visible.contains("bedrock-api-key"));
    access_keys.handle_key_event(&KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE));
    assert!(render_visible(&access_keys).contains("Bedrock API key: •••••••••••••••z"));

    access_keys.view = BedrockView::ProfileEntry("engineering".to_string());
    insta::assert_snapshot!(render_visible(&access_keys), @r###"
    > Set up Amazon Bedrock

      Enter the name of your AWS profile.

      AWS profile: engineering

      Press enter to continue
      Press esc to go back
    "###);

    access_keys.enter_region(BedrockCredential::Environment, "us-west-1".to_string());
    insta::assert_snapshot!(render_visible(&access_keys), @r###"
    > Set up Amazon Bedrock

      Enter the AWS Region to use with Amazon Bedrock.

      AWS Region: us-west-1

      Press enter to continue
      Press esc to go back
    "###);

    access_keys.view = BedrockView::EnvironmentInstructions;
    insta::assert_snapshot!(render_visible(&access_keys), @r###"
    > Set up Amazon Bedrock

      Configure AWS credentials in your environment, then restart Codex.

      Setup guide: https://learn.chatgpt.com/docs/amazon-bedrock

    > 1. Choose another sign-in method


      Press enter to continue
      Press esc to go back
    "###);

    for view in [
        BedrockView::ProfileEntry(String::new()),
        BedrockView::ApiKeyEntry(String::new()),
        BedrockView::RegionEntry {
            credential: BedrockCredential::Environment,
            value: String::new(),
        },
        BedrockView::AccessKeyEntry {
            values: Default::default(),
            selected_field: 0,
        },
    ] {
        access_keys.view = view;
        assert!(access_keys.is_text_entry_active());
    }

    for credential in [
        BedrockCredential::Profile("engineering".to_string()),
        BedrockCredential::Environment,
        BedrockCredential::AccessKeys {
            access_key_id: "AKIAEXAMPLE".to_string(),
            secret_access_key: "secret-value".to_string(),
            session_token: None,
        },
        BedrockCredential::ApiKey("bedrock-api-key".to_string()),
    ] {
        access_keys.enter_region(credential, "us-west-1".to_string());
        assert!(matches!(
            &access_keys.view,
            BedrockView::RegionEntry { value, .. } if value == "us-west-1"
        ));
    }
}
