use super::*;
use codex_protocol::openai_models::ModelAccessPrograms;
use pretty_assertions::assert_eq;

#[test]
fn turn_program_uses_catalog_and_preserves_legacy_models() {
    let mut model = crate::test_support::TEST_MODEL_PRESETS[0].clone();
    model.available_access_programs = Some(ModelAccessPrograms {
        cyber: vec![
            CyberAccessProgram::Standard,
            CyberAccessProgram::DaybreakBlue,
        ],
    });
    let models = vec![model.clone()];
    for (name, enabled, expected) in [
        (
            model.model.as_str(),
            true,
            Some(CyberAccessProgram::DaybreakBlue),
        ),
        (
            model.model.as_str(),
            false,
            Some(CyberAccessProgram::Standard),
        ),
        ("unlisted-model", false, None),
    ] {
        assert_eq!(
            program_for_turn(&models, name, /*eligible_account*/ true, enabled),
            Ok(expected)
        );
    }
    for (name, eligible_account) in [("unlisted-model", true), (model.model.as_str(), false)] {
        assert!(program_for_turn(&models, name, eligible_account, /*enabled*/ true).is_err());
    }
}

#[test]
fn refusal_guidance_uses_the_model_catalog() {
    let mut model = crate::test_support::TEST_MODEL_PRESETS[0].clone();
    model.available_access_programs = Some(ModelAccessPrograms {
        cyber: vec![CyberAccessProgram::Standard],
    });
    for (name, expected) in [
        ("gpt-5.6-sol", Notice::Apply),
        ("gpt-6-astra", Notice::Astra),
        ("gpt-6-astra-wm", Notice::Astra),
    ] {
        model.model = name.into();
        assert_eq!(
            notice_for_setting(
                std::slice::from_ref(&model),
                name,
                /*enabled*/ false,
                /*can_enable_daybreak*/ true,
            ),
            expected
        );
    }
    model
        .available_access_programs
        .as_mut()
        .unwrap()
        .cyber
        .push(CyberAccessProgram::DaybreakBlue);
    assert_eq!(
        notice_for_setting(
            std::slice::from_ref(&model),
            &model.model,
            /*enabled*/ false,
            /*can_enable_daybreak*/ true,
        ),
        Notice::Disabled
    );
    assert_eq!(
        notice_for_setting(
            std::slice::from_ref(&model),
            &model.model,
            /*enabled*/ true,
            /*can_enable_daybreak*/ true,
        ),
        Notice::Enabled
    );
    model.available_access_programs.as_mut().unwrap().cyber =
        vec![CyberAccessProgram::DaybreakBlue];
    assert_eq!(
        notice_for_setting(
            std::slice::from_ref(&model),
            &model.model,
            /*enabled*/ false,
            /*can_enable_daybreak*/ true
        ),
        Notice::Disabled
    );
    let other = model.clone();
    model.model = "gpt-5.6-sol".into();
    model.available_access_programs.as_mut().unwrap().cyber = vec![CyberAccessProgram::Standard];
    assert_eq!(
        notice_for_setting(
            &[model.clone(), other],
            &model.model,
            /*enabled*/ false,
            /*can_enable_daybreak*/ true
        ),
        Notice::Apply
    );
    assert_eq!(
        notice_for_setting(
            &[],
            "gpt-6-astra",
            /*enabled*/ false,
            /*can_enable_daybreak*/ true
        ),
        Notice::Apply
    );
}
