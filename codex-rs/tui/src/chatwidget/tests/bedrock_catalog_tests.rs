//! Renders Bedrock provider catalogs and Astra reasoning choices.

use super::*;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_model_provider::create_model_provider;
use codex_model_provider_info::ModelProviderInfo;
use codex_models_manager::manager::RefreshStrategy;

#[tokio::test]
async fn bedrock_astra_model_and_reasoning_pickers() {
    for (name, provider_info, default_model, astra_model) in [
        (
            "mantle",
            ModelProviderInfo::create_amazon_bedrock_provider(/*aws*/ None),
            "openai.gpt-6-sol",
            "openai.gpt-6-astra",
        ),
        (
            "runtime",
            ModelProviderInfo::create_amazon_bedrock_runtime_provider(/*aws*/ None),
            "global.openai.gpt-6-sol",
            "global.openai.gpt-6-astra",
        ),
    ] {
        let presets = create_model_provider(provider_info, /*auth_manager*/ None)
            .models_manager_without_cache(/*config_model_catalog*/ None)
            .list_models(
                RefreshStrategy::Offline,
                HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            )
            .await;
        let astra = presets
            .iter()
            .find(|model| model.model == astra_model)
            .expect("Astra preset")
            .clone();
        let (mut chat, _events, _ops) = make_chatwidget_manual(Some(default_model)).await;
        chat.thread_id = Some(ThreadId::new());
        chat.model_catalog = Arc::new(ModelCatalog::new(presets));
        chat.open_model_popup();
        assert_chatwidget_snapshot!(
            format!("bedrock_{name}_models"),
            render_bottom_popup(&chat, /*width*/ 100)
        );
        chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
        chat.open_reasoning_popup(astra);
        assert_chatwidget_snapshot!(
            format!("bedrock_{name}_astra_reasoning"),
            render_bottom_popup(&chat, /*width*/ 100)
        );
    }
}

#[tokio::test]
async fn bedrock_govcloud_model_pickers() {
    for region in ["us-gov-west-1", "us-gov-east-1"] {
        let mut provider_info =
            ModelProviderInfo::create_amazon_bedrock_provider(/*aws*/ None);
        provider_info.base_url = Some(format!("https://bedrock-mantle.{region}.api.aws/openai/v1"));
        let presets = create_model_provider(provider_info, /*auth_manager*/ None)
            .models_manager_without_cache(/*config_model_catalog*/ None)
            .list_models(
                RefreshStrategy::Offline,
                HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            )
            .await;
        let default_preset = presets
            .iter()
            .find(|preset| preset.is_default)
            .expect("default Bedrock model");
        let (mut chat, _events, _ops) = make_chatwidget_manual(Some(&default_preset.model)).await;
        chat.thread_id = Some(ThreadId::new());
        chat.model_catalog = Arc::new(ModelCatalog::new(presets));
        chat.open_model_popup();
        assert_chatwidget_snapshot!(
            "bedrock_govcloud_models",
            render_bottom_popup(&chat, /*width*/ 100)
        );
    }
}
