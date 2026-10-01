//! Account-scoped Daybreak eligibility and per-turn program selection.
//! Pending or failed discovery never implies access.

use codex_protocol::openai_models::ModelAccessPrograms;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::turn_input::CyberAccessProgram;

pub(crate) fn program_for_turn(
    models: &[ModelPreset],
    model: &str,
    eligible_account: bool,
    enabled: bool,
) -> Result<Option<CyberAccessProgram>, String> {
    if !eligible_account {
        return if enabled {
            Err("Daybreak requires a signed-in ChatGPT account and the OpenAI provider. Turn it off to continue.".into())
        } else {
            Ok(None)
        };
    }
    let programs = models
        .iter()
        .find(|entry| entry.model == model)
        .and_then(|entry| entry.available_access_programs.as_ref());
    if enabled {
        programs
            .and_then(codex_protocol::openai_models::ModelAccessPrograms::daybreak)
            .map(Some)
            .ok_or_else(|| format!("Daybreak support for model {model} could not be confirmed by the connected server. Use /daybreak to turn it off, or choose a compatible model and server."))
    } else {
        Ok(programs.and_then(codex_protocol::openai_models::ModelAccessPrograms::standard))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Notice {
    Apply,
    Astra,
    Disabled,
    Enabled,
}

pub(crate) fn notice_for_setting(
    models: &[ModelPreset],
    model: &str,
    enabled: bool,
    can_enable_daybreak: bool,
) -> Notice {
    if enabled {
        return Notice::Enabled;
    }
    if can_enable_daybreak {
        let programs = models
            .iter()
            .find(|entry| entry.model == model)
            .and_then(|entry| entry.available_access_programs.as_ref());
        if matches!(model, "gpt-6-astra" | "gpt-6-astra-wm")
            && programs.is_some_and(|programs| {
                programs.cyber.contains(&CyberAccessProgram::Standard)
                    && programs.daybreak().is_none()
            })
        {
            return Notice::Astra;
        }
        if programs.and_then(ModelAccessPrograms::daybreak).is_some() {
            return Notice::Disabled;
        }
    }
    Notice::Apply
}

pub(crate) fn available(models: &[ModelPreset]) -> bool {
    models.iter().any(|model| {
        model
            .available_access_programs
            .as_ref()
            .and_then(codex_protocol::openai_models::ModelAccessPrograms::daybreak)
            .is_some()
    })
}

/// Missing or empty program lists do not establish that the account lacks access.
pub(crate) fn availability(models: &[ModelPreset]) -> Option<bool> {
    if available(models) {
        return Some(true);
    }
    if models.is_empty() {
        return None;
    }
    models
        .iter()
        .all(|model| {
            model
                .available_access_programs
                .as_ref()
                .is_some_and(|programs| !programs.cyber.is_empty())
        })
        .then_some(false)
}

#[cfg(test)]
#[path = "daybreak_tests.rs"]
mod tests;
