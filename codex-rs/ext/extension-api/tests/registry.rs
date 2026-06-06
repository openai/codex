use std::sync::Arc;

use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::GlobalInstructions;
use codex_extension_api::GlobalInstructionsContributor;
use codex_extension_api::GlobalInstructionsFuture;

struct StaticGlobalInstructionsContributor;

impl GlobalInstructionsContributor for StaticGlobalInstructionsContributor {
    fn contribute(&self) -> GlobalInstructionsFuture<'_> {
        Box::pin(std::future::ready(Ok(GlobalInstructions::default())))
    }
}

#[test]
fn global_instructions_contributor_is_optional_and_singular() {
    let empty = ExtensionRegistryBuilder::<()>::new().build();
    assert!(empty.global_instructions_contributor().is_none());

    let contributor: Arc<dyn GlobalInstructionsContributor> =
        Arc::new(StaticGlobalInstructionsContributor);
    let mut builder = ExtensionRegistryBuilder::<()>::new();
    builder.global_instructions_contributor(Arc::clone(&contributor));
    let registry = builder.build();

    assert!(Arc::ptr_eq(
        registry
            .global_instructions_contributor()
            .expect("registered contributor"),
        &contributor
    ));
}
