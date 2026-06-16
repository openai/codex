//! Backend client operations for reading available rate-limit reset credits and consuming one.

use super::Client;
use super::PathStyle;
use crate::types::ConsumeRateLimitResetCreditResponse;
use crate::types::RateLimitStatusWithResetCredits;
use crate::types::RateLimitsWithResetCredits;
use anyhow::Result;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::HeaderValue;
use serde::Serialize;

#[derive(Serialize)]
struct ConsumeRateLimitResetCreditRequest<'a> {
    redeem_request_id: &'a str,
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
        let req = self.http.get(&url).headers(self.headers());
        let (body, ct) = self.exec_request(req, "GET", &url).await?;
        self.decode_json(&url, &ct, &body)
    }

    pub async fn consume_rate_limit_reset_credit(
        &self,
        redeem_request_id: &str,
    ) -> Result<ConsumeRateLimitResetCreditResponse> {
        let url = self.consume_rate_limit_reset_credit_url();
        let req = self
            .http
            .post(&url)
            .headers(self.headers())
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .json(&ConsumeRateLimitResetCreditRequest { redeem_request_id });
        let (body, ct) = self.exec_request(req, "POST", &url).await?;
        self.decode_json(&url, &ct, &body)
    }

    fn rate_limit_status_url(&self) -> String {
        match self.path_style {
            PathStyle::CodexApi => format!("{}/api/codex/usage", self.base_url),
            PathStyle::ChatGptApi => format!("{}/wham/usage", self.base_url),
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
