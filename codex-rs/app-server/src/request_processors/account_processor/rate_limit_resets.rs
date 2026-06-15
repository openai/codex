use super::*;

const RATE_LIMIT_RESET_REQUEST_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 10);
#[cfg(debug_assertions)]
const RATE_LIMIT_RESET_REQUEST_TIMEOUT_ENV_VAR: &str =
    "CODEX_TEST_RATE_LIMIT_RESET_REQUEST_TIMEOUT_MS";

impl AccountRequestProcessor {
    pub(crate) async fn consume_account_rate_limit_reset_credit(
        &self,
        params: ConsumeAccountRateLimitResetCreditParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        if params.idempotency_key.is_empty() {
            return Err(invalid_request("idempotencyKey must not be empty"));
        }

        let client = self.rate_limit_reset_backend_client().await?;
        let request_timeout = RATE_LIMIT_RESET_REQUEST_TIMEOUT;
        #[cfg(debug_assertions)]
        let request_timeout = std::env::var(RATE_LIMIT_RESET_REQUEST_TIMEOUT_ENV_VAR)
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_millis)
            .unwrap_or(request_timeout);
        let response = tokio::time::timeout(
            request_timeout,
            client.consume_rate_limit_reset_credit(&params.idempotency_key),
        )
        .await
        .map_err(|_| internal_error("rate limit reset consume timed out"))?
        .map_err(|err| internal_error(format!("failed to consume rate limit reset: {err}")))?;
        let outcome = match response.code {
            BackendConsumeRateLimitResetCreditCode::Reset => {
                ConsumeAccountRateLimitResetCreditOutcome::Reset
            }
            BackendConsumeRateLimitResetCreditCode::NothingToReset => {
                ConsumeAccountRateLimitResetCreditOutcome::NothingToReset
            }
            BackendConsumeRateLimitResetCreditCode::NoCredit => {
                ConsumeAccountRateLimitResetCreditOutcome::NoCredit
            }
            BackendConsumeRateLimitResetCreditCode::AlreadyRedeemed => {
                ConsumeAccountRateLimitResetCreditOutcome::AlreadyRedeemed
            }
        };
        Ok(Some(
            ConsumeAccountRateLimitResetCreditResponse {
                outcome,
                windows_reset: response.windows_reset,
            }
            .into(),
        ))
    }

    async fn rate_limit_reset_backend_client(&self) -> Result<BackendClient, JSONRPCErrorError> {
        let Some(auth) = self.auth_manager.auth().await else {
            return Err(invalid_request(
                "codex account authentication required for rate limit reset credits",
            ));
        };
        if !auth.uses_codex_backend() {
            return Err(invalid_request(
                "chatgpt authentication required for rate limit reset credits",
            ));
        }

        BackendClient::from_auth(self.config.chatgpt_base_url.clone(), &auth)
            .map_err(|err| internal_error(format!("failed to construct backend client: {err}")))
    }
}
