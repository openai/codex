//! Validate inline model choices before applying them through session-only model selection.

use super::ChatWidget;
use super::session_model_selection::AstraModelPickerAction;
use super::slash_command_args;
use crate::app_event::AppEvent;
use crate::history_cell;
use codex_protocol::config_types::SERVICE_TIER_DEFAULT_REQUEST_VALUE;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ReasoningEffort;

#[derive(Default)]
struct ModelCommandArgs {
    model: Option<String>,
    effort: Option<String>,
    speed: Option<String>,
}

struct ModelCommand {
    model: String,
    effort: ReasoningEffort,
    service_tier: Option<String>,
}

fn parse_model_command_args(args: &str) -> Result<ModelCommandArgs, String> {
    let parsed = slash_command_args::parse(args, &["--reasoning", "--speed"], &[], &[])?;
    if let Some(unexpected) = parsed.positionals.get(1) {
        return Err(format!(
            "Unexpected argument '{unexpected}'. Use '--reasoning=<level>' or '--speed=<tier>'."
        ));
    }
    Ok(ModelCommandArgs {
        model: parsed.positionals.first().cloned(),
        effort: parsed.value("--reasoning").map(str::to_owned),
        speed: parsed.value("--speed").map(str::to_owned),
    })
}

fn resolve_reasoning_effort(
    chat: &ChatWidget,
    args: &ModelCommandArgs,
    model: &ModelPreset,
) -> Result<ReasoningEffort, String> {
    let Some(effort) = args.effort.as_deref() else {
        return Ok(if args.model.is_none() {
            chat.current_reasoning_effort()
                .unwrap_or_else(|| model.default_reasoning_effort.clone())
        } else {
            model.default_reasoning_effort.clone()
        });
    };
    let supported = if model.supported_reasoning_efforts.is_empty() {
        (model.default_reasoning_effort.as_str() == effort)
            .then_some(&model.default_reasoning_effort)
    } else {
        model
            .supported_reasoning_efforts
            .iter()
            .find(|option| option.effort.as_str() == effort)
            .map(|option| &option.effort)
    };
    supported.cloned().ok_or_else(|| {
        let available = if model.supported_reasoning_efforts.is_empty() {
            model.default_reasoning_effort.to_string()
        } else {
            model
                .supported_reasoning_efforts
                .iter()
                .map(|option| option.effort.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!(
            "Reasoning level '{effort}' is unavailable for {}. Available: {available}.",
            model.model
        )
    })
}

fn resolve_service_tier(
    chat: &ChatWidget,
    speed: Option<&str>,
    model: &ModelPreset,
) -> Result<Option<String>, String> {
    let Some(speed) = speed else {
        return Ok(None);
    };
    if speed == "standard" {
        return Ok(Some(SERVICE_TIER_DEFAULT_REQUEST_VALUE.to_string()));
    }
    if speed == "flex" {
        return Ok(Some("flex".to_string()));
    }
    let tier = model
        .service_tiers
        .iter()
        .find(|tier| tier.id == speed || tier.name.eq_ignore_ascii_case(speed))
        .filter(|tier| chat.config.features.service_tier_enabled(&tier.id));
    tier.map(|tier| Some(tier.id.clone())).ok_or_else(|| {
        let available = std::iter::once("standard".to_string())
            .chain(std::iter::once("flex".to_string()))
            .chain(
                model
                    .service_tiers
                    .iter()
                    .filter(|tier| chat.config.features.service_tier_enabled(&tier.id))
                    .map(|tier| tier.id.clone()),
            )
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "Speed '{speed}' is unavailable for {}. Available: {available}.",
            model.model
        )
    })
}

fn resolve_model_command(chat: &ChatWidget, args: &str) -> Result<ModelCommand, String> {
    let args = parse_model_command_args(args)?;
    if chat.thread_id().is_none() {
        return Err("Session is still starting; try /model again in a moment.".into());
    }
    if chat.restrict_model_picker_to_luna_reserve() {
        return Err(
            "Model arguments are unavailable while using Reserve. Use /model to adjust effort."
                .into(),
        );
    }
    if let Some(message) = chat.model_picker_unavailable() {
        return Err(message);
    }
    let model_name = args
        .model
        .clone()
        .unwrap_or_else(|| chat.selected_model().to_owned());
    let Some(model) =
        chat.model_catalog.models.iter().find(|model| {
            model.model == model_name && (args.model.is_none() || model.show_in_picker)
        })
    else {
        return Err(format!(
            "Model '{model_name}' is unavailable. Use /model to see available models."
        ));
    };
    Ok(ModelCommand {
        model: model.model.clone(),
        effort: resolve_reasoning_effort(chat, &args, model)?,
        service_tier: resolve_service_tier(chat, args.speed.as_deref(), model)?,
    })
}

impl ChatWidget {
    pub(super) fn handle_model_command_args(&mut self, args: &str) {
        let command = match resolve_model_command(self, args) {
            Ok(command) => command,
            Err(error) => {
                self.add_error_message(error);
                return;
            }
        };
        let ModelCommand {
            model,
            effort,
            service_tier,
        } = command;
        let warning = self.ultra_reasoning_concurrency_warning(&effort);
        let thread_id = self.sparkle_thread_for_picker_action(&model);
        self.set_queue_autosend_suppressed(/*suppressed*/ true);
        let scoped = self.scope_model_picker_event();
        self.app_event_tx.send(scoped(
            AstraModelPickerAction::SelectSessionModel {
                effort: Some(effort),
                service_tier,
            }
            .into_picker_event(thread_id, model),
        ));
        if let Some(warning) = warning {
            self.app_event_tx
                .send(scoped(AppEvent::InsertHistoryCell(Box::new(
                    history_cell::new_warning_event(warning),
                ))));
        }
    }
}
