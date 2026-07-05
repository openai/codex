use crate::app_event::TerminalBrowserProfileCommand;
use crate::bottom_pane::SelectionAction;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::popup_consts::standard_popup_hint_line;

pub(crate) fn requested_profile_command(
    arguments: &serde_json::Value,
) -> Option<TerminalBrowserProfileCommand> {
    let action = arguments.get("action")?.as_str()?;
    let name = || {
        arguments
            .get("name")
            .and_then(serde_json::Value::as_str)
            .filter(|name| valid_profile_name(name))
            .map(str::to_string)
    };
    match action {
        "requestCreate" => name().map(TerminalBrowserProfileCommand::Create),
        "requestSelect" => name().map(TerminalBrowserProfileCommand::Use),
        "requestEphemeral" if arguments.get("name").is_none() => {
            Some(TerminalBrowserProfileCommand::Ephemeral)
        }
        "requestForget" => name().map(TerminalBrowserProfileCommand::Forget),
        _ => None,
    }
}

fn valid_profile_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    name.len() <= 64
        && first.is_ascii_alphanumeric()
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

pub(crate) fn profile_approval_view_params(
    command: TerminalBrowserProfileCommand,
) -> SelectionViewParams {
    let (subtitle, approve_name, approve_description) = match &command {
        TerminalBrowserProfileCommand::Create(name) => (
            format!("The model wants to create and select browser profile `{name}`."),
            "Create and select profile",
            "Store browsing data for this workspace in a named profile",
        ),
        TerminalBrowserProfileCommand::Use(name) => (
            format!("The model wants to select browser profile `{name}`."),
            "Select profile",
            "Close the current browser and use the named profile",
        ),
        TerminalBrowserProfileCommand::Ephemeral => (
            "The model wants to return to a fresh ephemeral browser profile.".to_string(),
            "Use ephemeral profile",
            "Close the current browser and discard future ephemeral data on close",
        ),
        TerminalBrowserProfileCommand::Forget(name) => (
            format!(
                "The model wants to permanently delete browser profile `{name}` and its browsing data."
            ),
            "Permanently delete profile",
            "This cannot be undone",
        ),
        TerminalBrowserProfileCommand::List => (
            "The model requested a profile listing.".to_string(),
            "List profiles",
            "Read profile names without changing them",
        ),
    };
    let approve_actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
        tx.send(crate::app_event::AppEvent::ManageTerminalBrowserProfile(
            command.clone(),
        ));
    })];
    SelectionViewParams {
        title: Some("Approve browser profile change?".to_string()),
        subtitle: Some(subtitle),
        footer_hint: Some(standard_popup_hint_line()),
        items: vec![
            SelectionItem {
                name: approve_name.to_string(),
                description: Some(approve_description.to_string()),
                actions: approve_actions,
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Cancel".to_string(),
                description: Some("Do not change browser profiles".to_string()),
                dismiss_on_select: true,
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}
