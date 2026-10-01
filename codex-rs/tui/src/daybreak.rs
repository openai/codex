//! Choose cyber refusal guidance from the connected host's model catalog.

use codex_protocol::openai_models::ModelAccessPrograms;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::turn_input::CyberAccessProgram;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Notice {
    Apply,
    Astra,
    #[default]
    Limited,
}

pub(crate) fn notice_for_model(models: &[ModelPreset], model: &str) -> Notice {
    let programs = models
        .iter()
        .find(|entry| entry.model == model)
        .and_then(|entry| entry.available_access_programs.as_ref());
    if matches!(model, "gpt-6-astra" | "gpt-6-astra-wm")
        && programs.is_some_and(|programs| {
            programs.cyber.contains(&CyberAccessProgram::Standard) && programs.daybreak().is_none()
        })
    {
        return Notice::Astra;
    }
    if programs.and_then(ModelAccessPrograms::daybreak).is_some() {
        Notice::Limited
    } else {
        Notice::Apply
    }
}

#[cfg(test)]
#[path = "daybreak_tests.rs"]
mod tests;
