use codex_extension_api::ExtensionRegistryBuilder;

/// Installs the Guardian V2 extension without registering contributors yet.
pub fn install<C: Sync>(_registry: &mut ExtensionRegistryBuilder<C>) {}
