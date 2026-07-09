//! Backend client operations for reading available rate-limit reset credits and consuming one.

use super::Client;
use super::PathStyle;
use crate::types::ConsumeRateLimitResetCreditResponse;
use crate::types::RateLimitResetCreditsDetails;
use crate::types::RateLimitStatusWithResetCredits;
use crate::types::RateLimitsWithResetCredits;
use anyhow::Result;
use http::Method;
use http::header::CONTENT_TYPE;
use http::header::HeaderValue;
use serde::Serialize;

#[derive(Serialize)]
struct ConsumeRateLimitResetCreditRequest<'a> {
    redeem_request_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    credit_id: Option<&'a str>,
}

impl Client {
    pub async fn get_rate_limits_with_reset_credits(&self) -> Result<RateLimitsWithResetCredits> {
        let payload = self.get_rate_limit_status().await?;
        Ok(RateLimitsWithResetCredits {
            rate_limits: Self::rate_limit_snapshots_from_payload(payload.rate_limits),
            rate_limit_reset_credits: payload.rate_limit_reset_credits,
        })
    }

    pub(super) async fn get_rate_limit_status(&self) -> Result<RateLimitStatusWithResetCredits> {
        let url = self.rate_limit_status_url();
        let req = self.request(Method::GET, &url).headers(self.headers());
        let (body, ct) = self.exec_request(req, "GET", &url).await?;
        self.decode_json(&url, &ct, &body)
    }

    pub async fn list_rate_limit_reset_credits(&self) -> Result<RateLimitResetCreditsDetails> {
        let url = self.rate_limit_reset_credits_url();
        let req = self.request(Method::GET, &url).headers(self.headers());
        let (body, ct) = self.exec_request(req, "GET", &url).await?;
        self.decode_json(&url, &ct, &body)
    }

    pub async fn consume_rate_limit_reset_credit(
        &self,
        redeem_request_id: &str,
    ) -> Result<ConsumeRateLimitResetCreditResponse> {
        self.consume_rate_limit_reset_credit_request(redeem_request_id, /*credit_id*/ None)
            .await
    }

    pub async fn consume_rate_limit_reset_credit_by_id(
        &self,
        redeem_request_id: &str,
        credit_id: &str,
    ) -> Result<ConsumeRateLimitResetCreditResponse> {
        self.consume_rate_limit_reset_credit_request(redeem_request_id, Some(credit_id))
            .await
    }

    async fn consume_rate_limit_reset_credit_request(
        &self,
        redeem_request_id: &str,
        credit_id: Option<&str>,
    ) -> Result<ConsumeRateLimitResetCreditResponse> {
        let url = self.consume_rate_limit_reset_credit_url();
        let req = self
            .request(Method::POST, &url)
            .headers(self.headers())
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .json(&ConsumeRateLimitResetCreditRequest {
                redeem_request_id,
                credit_id,
            });
        let (body, ct) = self.exec_request(req, "POST", &url).await?;
        self.decode_json(&url, &ct, &body)
    }

    fn rate_limit_status_url(&self) -> String {
        match self.path_style {
            PathStyle::CodexApi => format!("{}/api/codex/usage", self.base_url),
            PathStyle::ChatGptApi => format!("{}/wham/usage", self.base_url),
        }
    }

    fn rate_limit_reset_credits_url(&self) -> String {
        match self.path_style {
            PathStyle::CodexApi => {
                format!("{}/api/codex/rate-limit-reset-credits", self.base_url)
            }
            PathStyle::ChatGptApi => {
                format!("{}/wham/rate-limit-reset-credits", self.base_url)
            }
        }
    }

    fn consume_rate_limit_reset_credit_url(&self) -> String {
        match self.path_style {
            PathStyle::CodexApi => {
                format!(
                    "{}/api/codex/rate-limit-reset-credits/consume",
                    self.base_url
                )
            }
            PathStyle::ChatGptApi => {
                format!("{}/wham/rate-limit-reset-credits/consume", self.base_url)
            }
        }
    }
}

#[cfg(test)]
#[path = "rate_limit_resets_tests.rs"]
mod tests;
