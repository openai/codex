use super::*;

const RATE_LIMIT_RESET_REQUEST_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 10);

impl AccountRequestProcessor {
    pub(crate) async fn consume_account_rate_limit_reset_credit(
        &self,
        params: ConsumeAccountRateLimitResetCreditParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        if params.redeem_request_id.is_empty() {
            return Err(invalid_request("redeemRequestId must not be empty"));
        }

        let client = self.rate_limit_reset_backend_client().await?;
        let response = tokio::time::timeout(
            RATE_LIMIT_RESET_REQUEST_TIMEOUT,
            client.consume_rate_limit_reset_credit(&params.redeem_request_id),
        )
        .await
        .map_err(|_| internal_error("rate limit reset consume timed out"))?
        .map_err(|err| internal_error(format!("failed to consume rate limit reset: {err}")))?;
        let code = match response.code {
            BackendConsumeRateLimitResetCreditCode::Reset => {
                ConsumeAccountRateLimitResetCreditCode::Reset
            }
            BackendConsumeRateLimitResetCreditCode::NothingToReset => {
                ConsumeAccountRateLimitResetCreditCode::NothingToReset
            }
            BackendConsumeRateLimitResetCreditCode::NoCredit => {
                ConsumeAccountRateLimitResetCreditCode::NoCredit
            }
            BackendConsumeRateLimitResetCreditCode::AlreadyRedeemed => {
                ConsumeAccountRateLimitResetCreditCode::AlreadyRedeemed
            }
        };
        Ok(Some(
            ConsumeAccountRateLimitResetCreditResponse {
                code,
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
