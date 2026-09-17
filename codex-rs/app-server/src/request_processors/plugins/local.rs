use super::*;
use codex_core::config::ConfigOverrides;
use codex_core_plugins::PluginMarketplaceContext;
use codex_core_plugins::PluginMarketplaceScope;
use codex_core_plugins::marketplace::MarketplaceListError;
use futures::StreamExt;

const CONFIG_LOAD_CONCURRENCY: usize = 5;

impl PluginRequestProcessor {
    pub(super) async fn load_catalog_config(
        &self,
        cwds: &[AbsolutePathBuf],
    ) -> Result<Config, JSONRPCErrorError> {
        if cwds.is_empty() {
            self.config_manager
                .load_non_project_config()
                .await
                .map_err(|err| internal_error(format!("failed to reload config: {err}")))
        } else {
            self.load_latest_config(/*fallback_cwd*/ None).await
        }
    }

    pub(super) async fn load_marketplace_context(
        &self,
        cwds: Vec<AbsolutePathBuf>,
        catalog_config: &Config,
    ) -> PluginMarketplaceContext {
        let global_config = catalog_config.plugins_config_input();
        if cwds.is_empty() {
            return PluginMarketplaceContext {
                global_config: global_config.clone(),
                scopes: vec![PluginMarketplaceScope {
                    cwd: None,
                    config: global_config,
                }],
                load_errors: Vec::new(),
            };
        }
        let outcomes = futures::stream::iter(cwds)
            .map(|cwd| async move {
                match self
                    .config_manager
                    .load_with_overrides(
                        /*request_overrides*/ None,
                        ConfigOverrides {
                            cwd: Some(cwd.to_path_buf()),
                            ..Default::default()
                        },
                    )
                    .await
                {
                    Ok(config) => Ok(PluginMarketplaceScope {
                        cwd: Some(cwd),
                        config: config.plugins_config_input(),
                    }),
                    Err(err) => Err(MarketplaceListError {
                        path: cwd,
                        message: format!("failed to reload config: {err}"),
                    }),
                }
            })
            .buffered(CONFIG_LOAD_CONCURRENCY)
            .collect::<Vec<_>>()
            .await;
        let mut scopes = Vec::new();
        let mut load_errors = Vec::new();
        for outcome in outcomes {
            match outcome {
                Ok(scope) => scopes.push(scope),
                Err(error) => load_errors.push(error),
            }
        }

        PluginMarketplaceContext {
            global_config,
            scopes,
            load_errors,
        }
    }
}
