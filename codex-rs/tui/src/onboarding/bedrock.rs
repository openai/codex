//! Amazon Bedrock credential discovery and setup within the authentication step.

use super::auth::AuthModeWidget;
use super::auth::SignInState;
use super::auth::onboarding_request_id;
use super::keys;
use crate::key_hint::KeyBindingListExt;
use crate::wrapping::word_wrap_lines;
use codex_app_server_protocol::AwsCredentialType;
use codex_app_server_protocol::BedrockAwsProfile;
use codex_app_server_protocol::BedrockDiscoverParams;
use codex_app_server_protocol::BedrockDiscoverResponse;
use codex_app_server_protocol::BedrockEnvironmentCredential;
use codex_app_server_protocol::BedrockSetupParams;
use codex_app_server_protocol::BedrockSetupResponse;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::LoginAccountParams;
use codex_app_server_protocol::LoginAccountResponse;
use codex_app_server_protocol::RequestId;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use std::sync::PoisonError;

#[derive(Clone)]
pub(super) struct BedrockState {
    view: BedrockView,
    highlighted: usize,
    profiles: Vec<BedrockAwsProfile>,
    environment_credentials: Vec<BedrockEnvironmentCredential>,
}

#[derive(Clone)]
enum BedrockView {
    Discovering(RequestId),
    Methods(BedrockMethodList),
    ProfileEntry(String),
    AccessKeyEntry {
        values: [String; 3],
        selected_field: usize,
    },
    ApiKeyEntry(String),
    RegionEntry {
        credential: BedrockCredential,
        value: String,
    },
    EnvironmentInstructions,
    Configuring(RequestId),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BedrockMethodList {
    Detected,
    All,
}

#[derive(Clone)]
enum BedrockCredential {
    Profile(String),
    Environment,
    AccessKeys {
        access_key_id: String,
        secret_access_key: String,
        session_token: Option<String>,
    },
    ApiKey(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BedrockMethod {
    Profile(usize),
    Environment,
    OtherMethods,
    ManualProfile,
    AccessKeys,
    EnvironmentInstructions,
    ApiKey,
}

enum BedrockAction {
    BackToAuth,
    Configure(BedrockCredential, String),
}

impl BedrockState {
    fn discovering(request_id: RequestId) -> Self {
        Self {
            view: BedrockView::Discovering(request_id),
            highlighted: 0,
            profiles: Vec::new(),
            environment_credentials: Vec::new(),
        }
    }

    fn discovered(response: BedrockDiscoverResponse) -> Self {
        Self {
            view: BedrockView::Methods(BedrockMethodList::Detected),
            highlighted: 0,
            profiles: response.profiles,
            environment_credentials: response.environment_credentials,
        }
    }

    fn methods(&self) -> Vec<BedrockMethod> {
        if matches!(self.view, BedrockView::EnvironmentInstructions) {
            return vec![BedrockMethod::OtherMethods];
        }
        let detected = matches!(self.view, BedrockView::Methods(BedrockMethodList::Detected));
        if detected && !self.profiles.is_empty() {
            let mut methods = (0..self.profiles.len())
                .map(BedrockMethod::Profile)
                .collect::<Vec<_>>();
            if !self.environment_credentials.is_empty() {
                methods.push(BedrockMethod::Environment);
            }
            methods.extend([BedrockMethod::OtherMethods, BedrockMethod::ApiKey]);
            return methods;
        }
        if detected && !self.environment_credentials.is_empty() {
            return vec![
                BedrockMethod::Environment,
                BedrockMethod::OtherMethods,
                BedrockMethod::ApiKey,
            ];
        }
        let mut methods = vec![
            BedrockMethod::ManualProfile,
            BedrockMethod::AccessKeys,
            BedrockMethod::EnvironmentInstructions,
        ];
        if detected {
            methods.push(BedrockMethod::ApiKey);
        }
        methods
    }

    pub(super) fn is_text_entry_active(&self) -> bool {
        matches!(
            self.view,
            BedrockView::ProfileEntry(_)
                | BedrockView::AccessKeyEntry { .. }
                | BedrockView::ApiKeyEntry(_)
                | BedrockView::RegionEntry { .. }
        )
    }

    fn active_input_mut(&mut self) -> Option<&mut String> {
        match &mut self.view {
            BedrockView::ProfileEntry(value)
            | BedrockView::ApiKeyEntry(value)
            | BedrockView::RegionEntry { value, .. } => Some(value),
            BedrockView::AccessKeyEntry {
                values,
                selected_field,
            } => Some(&mut values[*selected_field]),
            _ => None,
        }
    }

    fn enter_region(&mut self, credential: BedrockCredential, value: String) {
        self.view = BedrockView::RegionEntry { credential, value };
    }

    fn select_method(&mut self, method: BedrockMethod) -> Option<BedrockAction> {
        self.highlighted = 0;
        match method {
            BedrockMethod::Profile(index) => {
                let profile = self.profiles[index].clone();
                let credential = BedrockCredential::Profile(profile.name);
                if let Some(region) = profile.region {
                    return Some(BedrockAction::Configure(credential, region));
                }
                self.enter_region(credential, String::new());
            }
            BedrockMethod::Environment => {
                let credential = BedrockCredential::Environment;
                if let Some(region) = self
                    .environment_credentials
                    .iter()
                    .find_map(|credential| credential.region.clone())
                {
                    return Some(BedrockAction::Configure(credential, region));
                }
                self.enter_region(credential, String::new());
            }
            BedrockMethod::OtherMethods => {
                self.view = BedrockView::Methods(BedrockMethodList::All);
            }
            BedrockMethod::ManualProfile => {
                self.view = BedrockView::ProfileEntry(String::new());
            }
            BedrockMethod::AccessKeys => {
                self.view = BedrockView::AccessKeyEntry {
                    values: Default::default(),
                    selected_field: 0,
                };
            }
            BedrockMethod::EnvironmentInstructions => {
                self.view = BedrockView::EnvironmentInstructions;
            }
            BedrockMethod::ApiKey => {
                self.view = BedrockView::ApiKeyEntry(String::new());
            }
        }
        None
    }

    fn handle_key_event(&mut self, key_event: &KeyEvent) -> Option<BedrockAction> {
        if keys::CANCEL.is_pressed(*key_event) {
            let leave_wizard = matches!(
                self.view,
                BedrockView::Discovering(_) | BedrockView::Methods(BedrockMethodList::Detected)
            );
            if leave_wizard {
                return Some(BedrockAction::BackToAuth);
            }
            self.view = BedrockView::Methods(BedrockMethodList::Detected);
            self.highlighted = 0;
            return None;
        }
        if matches!(self.view, BedrockView::Configuring(_)) {
            return None;
        }

        if matches!(
            self.view,
            BedrockView::Methods(_) | BedrockView::EnvironmentInstructions
        ) {
            let methods = self.methods();
            if keys::MOVE_UP.is_pressed(*key_event) {
                self.highlighted = (self.highlighted + methods.len() - 1) % methods.len();
            } else if keys::MOVE_DOWN.is_pressed(*key_event) {
                self.highlighted = (self.highlighted + 1) % methods.len();
            } else if keys::CONFIRM.is_pressed(*key_event) {
                return self.select_method(methods[self.highlighted]);
            } else if let KeyCode::Char(digit @ '1'..='9') = key_event.code
                && let Some(method) = methods.get(digit as usize - '1' as usize).copied()
            {
                return self.select_method(method);
            }
            return None;
        }

        if let BedrockView::AccessKeyEntry {
            values,
            selected_field,
        } = &mut self.view
        {
            if key_event.code == KeyCode::Up || key_event.code == KeyCode::BackTab {
                *selected_field = (*selected_field + values.len() - 1) % values.len();
                return None;
            }
            if key_event.code == KeyCode::Down || key_event.code == KeyCode::Tab {
                *selected_field = (*selected_field + 1) % values.len();
                return None;
            }
        }

        if keys::CONFIRM.is_pressed(*key_event) {
            return self.confirm_input();
        }
        if let Some(input) = self.active_input_mut() {
            match key_event.code {
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(character)
                    if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                        && !key_event.modifiers.intersects(
                            KeyModifiers::SUPER | KeyModifiers::CONTROL | KeyModifiers::ALT,
                        ) =>
                {
                    input.push(character);
                }
                _ => {}
            }
        }
        None
    }

    fn confirm_input(&mut self) -> Option<BedrockAction> {
        let credential = match &mut self.view {
            BedrockView::ProfileEntry(value) if !value.trim().is_empty() => {
                BedrockCredential::Profile(value.trim().to_string())
            }
            BedrockView::ApiKeyEntry(value) if !value.trim().is_empty() => {
                BedrockCredential::ApiKey(value.trim().to_string())
            }
            BedrockView::RegionEntry { credential, value } if !value.trim().is_empty() => {
                return Some(BedrockAction::Configure(
                    credential.clone(),
                    value.trim().to_string(),
                ));
            }
            BedrockView::AccessKeyEntry {
                values,
                selected_field,
            } => {
                if *selected_field < 2 {
                    if !values[*selected_field].trim().is_empty() {
                        *selected_field += 1;
                    }
                    return None;
                }
                if values[0].trim().is_empty() || values[1].trim().is_empty() {
                    *selected_field = if values[0].trim().is_empty() { 0 } else { 1 };
                    return None;
                }
                BedrockCredential::AccessKeys {
                    access_key_id: values[0].trim().to_string(),
                    secret_access_key: values[1].trim().to_string(),
                    session_token: (!values[2].trim().is_empty())
                        .then(|| values[2].trim().to_string()),
                }
            }
            _ => return None,
        };
        if let BedrockCredential::Profile(profile) = &credential
            && let Some(region) = self.profiles.iter().find_map(|discovered| {
                (discovered.name == *profile)
                    .then(|| discovered.region.clone())
                    .flatten()
            })
        {
            return Some(BedrockAction::Configure(credential, region));
        }
        self.enter_region(credential, String::new());
        None
    }

    pub(super) fn render(&self, area: Rect, buf: &mut Buffer, error: Option<String>) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let mut lines: Vec<Line> = vec![
            Line::from(vec!["> ".into(), "Set up Amazon Bedrock".bold()]),
            "".into(),
        ];
        match &self.view {
            BedrockView::Discovering(_) => {
                lines.push("  Checking for existing AWS credentials...".dim().into());
            }
            BedrockView::Configuring(_) => {
                lines.push("  Setting up Amazon Bedrock...".dim().into());
            }
            BedrockView::Methods(list) => {
                if *list == BedrockMethodList::Detected && self.profiles.len() == 1 {
                    let profile = &self.profiles[0];
                    lines.push(format!("  AWS profile detected: {}", profile.name).into());
                    if let Some(region) = &profile.region {
                        lines.push(format!("  Region: {region}").dim().into());
                    }
                } else if *list == BedrockMethodList::Detected && self.profiles.len() > 1 {
                    lines.push("  Choose an AWS profile.".into());
                } else if *list == BedrockMethodList::Detected
                    && !self.environment_credentials.is_empty()
                {
                    lines.push("  AWS credentials detected in your environment.".into());
                } else {
                    if *list == BedrockMethodList::Detected {
                        lines.push("  No AWS credentials found.".into());
                    }
                    lines.push("  Choose how you authenticate with AWS.".into());
                }
                lines.push("".into());
                self.render_methods(&mut lines);
            }
            BedrockView::ProfileEntry(value) => {
                lines.push("  Enter the name of your AWS profile.".into());
                lines.push("".into());
                lines.push(Line::from(vec![
                    "  AWS profile: ".into(),
                    value.clone().cyan(),
                ]));
            }
            BedrockView::ApiKeyEntry(value) => {
                lines.push("  Enter your Amazon Bedrock API key.".into());
                lines.push("".into());
                let mut masked_value = "•".repeat(value.chars().count().saturating_sub(1));
                if let Some(character) = value.chars().last() {
                    masked_value.push(character);
                }
                lines.push(Line::from(vec![
                    "  Bedrock API key: ".into(),
                    masked_value.cyan(),
                ]));
            }
            BedrockView::RegionEntry { value, .. } => {
                lines.push("  Enter the AWS Region to use with Amazon Bedrock.".into());
                lines.push("".into());
                lines.push(Line::from(vec![
                    "  AWS Region: ".into(),
                    value.clone().cyan(),
                ]));
            }
            BedrockView::AccessKeyEntry {
                values,
                selected_field,
            } => {
                lines.push("  Enter your AWS access keys.".into());
                lines.push("".into());
                for (index, label) in [
                    "AWS access key ID",
                    "AWS secret access key",
                    "AWS session token (optional)",
                ]
                .into_iter()
                .enumerate()
                {
                    let marker = if index == *selected_field { ">" } else { " " };
                    let value = if index == 0 {
                        values[index].clone()
                    } else if index == *selected_field {
                        let mut masked_value =
                            "•".repeat(values[index].chars().count().saturating_sub(1));
                        if let Some(character) = values[index].chars().last() {
                            masked_value.push(character);
                        }
                        masked_value
                    } else {
                        "•".repeat(values[index].chars().count())
                    };
                    let line = format!("{marker} {label}: {value}");
                    lines.push(if index == *selected_field {
                        line.cyan().into()
                    } else {
                        line.into()
                    });
                }
            }
            BedrockView::EnvironmentInstructions => {
                lines.push(
                    "  Configure AWS credentials in your environment, then restart Codex.".into(),
                );
                lines.push("".into());
                lines.push(Line::from(vec![
                    "  Setup guide: ".into(),
                    "https://learn.chatgpt.com/docs/amazon-bedrock"
                        .cyan()
                        .underlined(),
                ]));
                lines.push("".into());
                self.render_methods(&mut lines);
            }
        }
        let mut footer = Vec::new();
        if !matches!(self.view, BedrockView::Discovering(_)) {
            footer.push("".into());
            if !matches!(self.view, BedrockView::Configuring(_)) {
                footer.push(Line::from(vec![
                    "  Press ".dim(),
                    keys::CONFIRM[0].into(),
                    " to continue".dim(),
                ]));
            }
            footer.push(Line::from(vec![
                "  Press ".dim(),
                keys::CANCEL[0].into(),
                " to go back".dim(),
            ]));
        }
        if let Some(error) = error {
            footer.push("".into());
            footer.push(error.red().into());
        }
        let mut lines = word_wrap_lines(lines, usize::from(area.width));
        let footer = word_wrap_lines(footer, usize::from(area.width));
        if lines.len() + footer.len() <= usize::from(area.height) || footer.is_empty() {
            lines.extend(footer);
            Paragraph::new(lines).render(area, buf);
            return;
        }

        let footer_height = u16::try_from(footer.len())
            .unwrap_or(u16::MAX)
            .min(area.height.saturating_sub(1));
        let content_height = area.height.saturating_sub(footer_height);
        let highlighted_row = lines
            .iter()
            .enumerate()
            .skip(1)
            .find_map(|(index, line)| {
                line.spans
                    .first()
                    .is_some_and(|span| span.content.starts_with("> "))
                    .then_some(index)
            })
            .unwrap_or_default();
        let scroll = highlighted_row
            .saturating_add(2)
            .saturating_sub(usize::from(content_height))
            .min(lines.len().saturating_sub(usize::from(content_height)));
        let content_area = Rect {
            height: content_height,
            ..area
        };
        let footer_area = Rect {
            y: area.y.saturating_add(content_height),
            height: footer_height,
            ..area
        };
        Paragraph::new(lines)
            .scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0))
            .render(content_area, buf);
        Paragraph::new(footer).render(footer_area, buf);
    }

    fn render_methods(&self, lines: &mut Vec<Line<'static>>) {
        for (index, method) in self.methods().into_iter().enumerate() {
            let (title, description) = match method {
                BedrockMethod::Profile(profile_index) => {
                    let profile = &self.profiles[profile_index];
                    let title = if self.profiles.len() == 1 {
                        format!("Continue with {}", profile.name)
                    } else {
                        profile.name.clone()
                    };
                    let description = if self.profiles.len() == 1 {
                        "Use your existing AWS credentials".to_string()
                    } else {
                        profile.region.clone().unwrap_or_default()
                    };
                    (title, description)
                }
                BedrockMethod::Environment => {
                    let description = if self.environment_credentials.iter().any(|credential| {
                        credential.credential_type == AwsCredentialType::BedrockApiKey
                    }) {
                        "Use your existing Amazon Bedrock API key"
                    } else {
                        "Use your existing AWS credentials"
                    };
                    (
                        "Continue with detected credentials".to_string(),
                        description.to_string(),
                    )
                }
                BedrockMethod::OtherMethods => (
                    if matches!(self.view, BedrockView::EnvironmentInstructions) {
                        "Choose another sign-in method"
                    } else {
                        "Other AWS sign-in methods"
                    }
                    .to_string(),
                    if matches!(self.view, BedrockView::EnvironmentInstructions) {
                        ""
                    } else {
                        "Use another profile, access keys, or environment variables"
                    }
                    .to_string(),
                ),
                BedrockMethod::ManualProfile => (
                    "AWS profile".to_string(),
                    "Use AWS SSO or a named profile".to_string(),
                ),
                BedrockMethod::AccessKeys => (
                    "AWS access keys".to_string(),
                    "Enter an access key ID and secret access key".to_string(),
                ),
                BedrockMethod::EnvironmentInstructions => (
                    "Environment variables".to_string(),
                    "Configure AWS credentials in your environment, then return here".to_string(),
                ),
                BedrockMethod::ApiKey => (
                    "Bedrock API key".to_string(),
                    "Enter a Bedrock API key".to_string(),
                ),
            };
            let selected = index == self.highlighted;
            let marker = if selected { ">" } else { " " };
            let title_line = format!("{marker} {}. {title}", index + 1);
            lines.push(if selected {
                title_line.cyan().into()
            } else {
                title_line.into()
            });
            if !description.is_empty()
                && (!matches!(self.view, BedrockView::Methods(BedrockMethodList::Detected))
                    || !self.profiles.is_empty()
                    || self.environment_credentials.is_empty())
            {
                lines.push(format!("     {description}").dim().into());
            }
            lines.push("".into());
        }
    }
}

impl AuthModeWidget {
    pub(super) fn start_bedrock_discovery(&mut self) {
        let request_id = onboarding_request_id();
        *self
            .sign_in_state
            .write()
            .unwrap_or_else(PoisonError::into_inner) =
            SignInState::Bedrock(BedrockState::discovering(request_id.clone()));
        *self.error.write().unwrap_or_else(PoisonError::into_inner) = None;
        self.request_frame.schedule_frame();

        let request_handle = self.app_server_request_handle.clone();
        let sign_in_state = self.sign_in_state.clone();
        let error = self.error.clone();
        let request_frame = self.request_frame.clone();
        tokio::spawn(async move {
            let result = request_handle
                .request_typed::<BedrockDiscoverResponse>(ClientRequest::BedrockDiscover {
                    request_id: request_id.clone(),
                    params: BedrockDiscoverParams {},
                })
                .await;
            let mut guard = sign_in_state
                .write()
                .unwrap_or_else(PoisonError::into_inner);
            if !matches!(
                &*guard,
                SignInState::Bedrock(state)
                    if matches!(&state.view, BedrockView::Discovering(active_request_id)
                        if *active_request_id == request_id)
            ) {
                return;
            }
            match result {
                Ok(response) => {
                    *guard = SignInState::Bedrock(BedrockState::discovered(response));
                }
                Err(err) => {
                    *guard =
                        SignInState::Bedrock(BedrockState::discovered(BedrockDiscoverResponse {
                            profiles: Vec::new(),
                            environment_credentials: Vec::new(),
                        }));
                    *error.write().unwrap_or_else(PoisonError::into_inner) =
                        Some(format!("Unable to check AWS credentials: {err}"));
                }
            }
            drop(guard);
            request_frame.schedule_frame();
        });
    }

    pub(super) fn handle_bedrock_key_event(&mut self, key_event: &KeyEvent) -> bool {
        let (action, fallback) = {
            let mut guard = self
                .sign_in_state
                .write()
                .unwrap_or_else(PoisonError::into_inner);
            let SignInState::Bedrock(state) = &mut *guard else {
                return false;
            };
            *self.error.write().unwrap_or_else(PoisonError::into_inner) = None;
            let action = state.handle_key_event(key_event);
            (action, state.clone())
        };

        match action {
            Some(BedrockAction::BackToAuth) => {
                *self
                    .sign_in_state
                    .write()
                    .unwrap_or_else(PoisonError::into_inner) = SignInState::PickMode;
            }
            Some(BedrockAction::Configure(credential, region)) => {
                self.start_bedrock_setup(credential, region, fallback);
            }
            None => {}
        }
        self.request_frame.schedule_frame();
        true
    }

    pub(super) fn handle_bedrock_paste(&mut self, pasted: &str) -> bool {
        let pasted = pasted.trim();
        if pasted.is_empty() {
            return false;
        }
        let mut guard = self
            .sign_in_state
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        let SignInState::Bedrock(state) = &mut *guard else {
            return false;
        };
        let Some(input) = state.active_input_mut() else {
            return false;
        };
        input.push_str(pasted);
        *self.error.write().unwrap_or_else(PoisonError::into_inner) = None;
        drop(guard);
        self.request_frame.schedule_frame();
        true
    }

    fn start_bedrock_setup(
        &mut self,
        credential: BedrockCredential,
        region: String,
        mut fallback: BedrockState,
    ) {
        fallback.enter_region(credential.clone(), region.clone());
        let mut configuring = fallback.clone();
        let request_id = onboarding_request_id();
        configuring.view = BedrockView::Configuring(request_id.clone());
        *self
            .sign_in_state
            .write()
            .unwrap_or_else(PoisonError::into_inner) = SignInState::Bedrock(configuring);

        let request_handle = self.app_server_request_handle.clone();
        let sign_in_state = self.sign_in_state.clone();
        let error = self.error.clone();
        let request_frame = self.request_frame.clone();
        tokio::spawn(async move {
            let result = match credential {
                BedrockCredential::ApiKey(api_key) => request_handle
                    .request_typed::<LoginAccountResponse>(ClientRequest::LoginAccount {
                        request_id: request_id.clone(),
                        params: LoginAccountParams::AmazonBedrock { api_key, region },
                    })
                    .await
                    .map_err(|error| error.to_string())
                    .and_then(|response| match response {
                        LoginAccountResponse::AmazonBedrock {} => Ok(()),
                        response => Err(format!(
                            "Unexpected account/login/start response: {response:?}"
                        )),
                    }),
                BedrockCredential::Profile(profile) => request_handle
                    .request_typed::<BedrockSetupResponse>(ClientRequest::BedrockSetup {
                        request_id: request_id.clone(),
                        params: BedrockSetupParams::Profile { profile, region },
                    })
                    .await
                    .map(|_| ())
                    .map_err(|error| error.to_string()),
                BedrockCredential::Environment => request_handle
                    .request_typed::<BedrockSetupResponse>(ClientRequest::BedrockSetup {
                        request_id: request_id.clone(),
                        params: BedrockSetupParams::Environment { region },
                    })
                    .await
                    .map(|_| ())
                    .map_err(|error| error.to_string()),
                BedrockCredential::AccessKeys {
                    access_key_id,
                    secret_access_key,
                    session_token,
                } => request_handle
                    .request_typed::<LoginAccountResponse>(ClientRequest::LoginAccount {
                        request_id: request_id.clone(),
                        params: LoginAccountParams::AmazonBedrockAccessKeys {
                            access_key_id,
                            secret_access_key,
                            session_token,
                            region,
                        },
                    })
                    .await
                    .map_err(|error| error.to_string())
                    .and_then(|response| match response {
                        LoginAccountResponse::AmazonBedrock {} => Ok(()),
                        response => Err(format!(
                            "Unexpected account/login/start response: {response:?}"
                        )),
                    }),
            };
            let mut guard = sign_in_state
                .write()
                .unwrap_or_else(PoisonError::into_inner);
            if !matches!(
                &*guard,
                SignInState::Bedrock(state)
                    if matches!(&state.view, BedrockView::Configuring(active_request_id)
                        if *active_request_id == request_id)
            ) {
                return;
            }
            match result {
                Ok(()) => {
                    *error.write().unwrap_or_else(PoisonError::into_inner) = None;
                    *guard = SignInState::BedrockConfigured;
                }
                Err(err) => {
                    *error.write().unwrap_or_else(PoisonError::into_inner) =
                        Some(format!("Unable to set up Amazon Bedrock: {err}"));
                    *guard = SignInState::Bedrock(fallback);
                }
            }
            drop(guard);
            request_frame.schedule_frame();
        });
    }
}

#[cfg(test)]
#[path = "bedrock_tests.rs"]
mod tests;
