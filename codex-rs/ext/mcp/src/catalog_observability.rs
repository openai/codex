//! Measures published cloud and V2 executor plugin catalogs, excluding cached reads.

use codex_core_plugins::PluginCatalog;
use codex_extension_api::ExtensionMetrics;
use codex_extension_api::record_catalog_entries;

pub(crate) fn record_catalog(
    metrics: Option<&dyn ExtensionMetrics>,
    stage: &'static str,
    catalog: &PluginCatalog,
) {
    record_catalog_entries(
        metrics,
        "codex.plugins.catalog.entries",
        stage,
        catalog.entries.len(),
        catalog.is_partial,
        catalog.warnings.len(),
    );
}
