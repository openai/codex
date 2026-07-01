//! Client-owned referral requests used by short-lived Codex experiments.

use anyhow::Context;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

const REFERRAL_KEY: &str = "codex_referral_persistent_invite";
const ELIGIBILITY_TIMEOUT: Duration = Duration::from_secs(15);
const INVITE_TIMEOUT: Duration = Duration::from_secs(45);

/// A rewarded referral offer and the account that received it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferralOffer {
    pub description: String,
    pub rules: Vec<String>,
    pub grant_action: Option<Value>,
    pub grant_amount: Option<Value>,
    pub requires_explicit_confirmation: bool,
    pub identity: ReferralIdentity,
}

/// The ChatGPT user and workspace that own a referral offer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferralIdentity {
    pub user_id: String,
    pub account_id: String,
}

/// Reward status returned after the backend accepted an invite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferralRewardStatus {
    Included,
    NotIncluded,
    Unknown,
}

#[derive(Debug)]
struct DefiniteReferralInviteRejection {
    status: u16,
}

#[derive(Debug)]
struct ReferralInvitePreflightFailure {
    message: String,
}

impl fmt::Display for DefiniteReferralInviteRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "referral invite was rejected ({})", self.status)
    }
}

impl Error for DefiniteReferralInviteRejection {}

impl fmt::Display for ReferralInvitePreflightFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "referral invite preflight failed: {}", self.message)
    }
}

impl Error for ReferralInvitePreflightFailure {}

pub fn is_definite_referral_invite_rejection(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<DefiniteReferralInviteRejection>()
        .is_some()
}

pub fn is_referral_invite_preflight_failure(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<ReferralInvitePreflightFailure>()
        .is_some()
}

#[derive(Deserialize)]
struct EligibilityResponse {
    should_show: bool,
    #[serde(default)]
    has_rewards: bool,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    grant_action: Option<Value>,
    #[serde(default)]
    grant_amount: Option<Value>,
}

#[derive(Deserialize)]
struct RulesResponse {
    #[serde(default)]
    rules: Vec<String>,
    requires_explicit_confirmation: bool,
}

#[derive(Serialize)]
struct InviteRequest<'a> {
    referral_key: &'static str,
    emails: [&'a str; 1],
}

#[derive(Deserialize)]
struct InviteResponse {
    #[serde(default)]
    has_rewards: Option<bool>,
}

/// HTTP client for the temporary client-owned referral flow.
pub struct ReferralClient {
    auth_manager: Arc<OnceLock<Arc<AuthManager>>>,
    base_url: String,
}

impl ReferralClient {
    pub fn new(auth_manager: Arc<OnceLock<Arc<AuthManager>>>, base_url: String) -> Self {
        Self {
            auth_manager,
            base_url,
        }
    }

    pub async fn load_offer(&self) -> anyhow::Result<Option<ReferralOffer>> {
        let manager = self.auth_manager()?;
        let (auth, identity) = self.load_auth(&manager).await?;
        let client = codex_login::default_client::create_client();
        let base_url = self.base_url()?;

        let response = client
            .get(format!(
                "{base_url}/referrals/invite/eligibility?referral_key={REFERRAL_KEY}&requested_referrals=1&supports_rewardless_invites=false"
            ))
            .headers(codex_model_provider::auth_provider_from_auth(&auth).to_auth_headers())
            .timeout(ELIGIBILITY_TIMEOUT)
            .send()
            .await
            .context("referral eligibility request failed")?;
        if response.status().as_u16() == 403 {
            return Ok(None);
        }
        let status = response.status();
        anyhow::ensure!(
            status.is_success(),
            "referral eligibility failed ({status})"
        );
        let eligibility: EligibilityResponse = response
            .json()
            .await
            .context("referral eligibility response was invalid")?;
        if !eligibility.should_show || !eligibility.has_rewards {
            return Ok(None);
        }

        let response = client
            .get(format!(
                "{base_url}/wham/referrals/eligibility_rules?referral_key={REFERRAL_KEY}"
            ))
            .headers(codex_model_provider::auth_provider_from_auth(&auth).to_auth_headers())
            .timeout(ELIGIBILITY_TIMEOUT)
            .send()
            .await
            .context("referral rules request failed")?;
        if response.status().as_u16() == 403 {
            return Ok(None);
        }
        let status = response.status();
        anyhow::ensure!(status.is_success(), "referral rules failed ({status})");
        let rules: RulesResponse = response
            .json()
            .await
            .context("referral rules response was invalid")?;

        let description = eligibility
            .description
            .clone()
            .unwrap_or_else(|| fallback_description(&eligibility));

        Ok(Some(ReferralOffer {
            description,
            rules: rules.rules,
            grant_action: eligibility.grant_action,
            grant_amount: eligibility.grant_amount,
            requires_explicit_confirmation: rules.requires_explicit_confirmation,
            identity,
        }))
    }

    pub async fn send_invite(
        &self,
        offer: &ReferralOffer,
        email: &str,
    ) -> anyhow::Result<ReferralRewardStatus> {
        let current_offer = match self.load_offer().await {
            Ok(Some(offer)) => offer,
            Ok(None) => {
                return Err(preflight_failure("referral offer is no longer available"));
            }
            Err(err) => {
                return Err(preflight_failure(format!(
                    "referral offer could not be revalidated: {err}"
                )));
            }
        };
        if &current_offer != offer {
            return Err(preflight_failure(
                "referral offer changed before invite send",
            ));
        }

        let manager = self.auth_manager()?;
        let (auth, identity) = self
            .load_auth(&manager)
            .await
            .map_err(|err| preflight_failure(format!("ChatGPT authentication changed: {err}")))?;
        if identity != offer.identity {
            return Err(preflight_failure("referral account changed"));
        }

        let client = codex_login::default_client::create_single_attempt_client();
        let base_url = self.base_url()?;
        let response = client
            .post(format!("{base_url}/wham/referrals/invite"))
            .headers(codex_model_provider::auth_provider_from_auth(&auth).to_auth_headers())
            .json(&InviteRequest {
                referral_key: REFERRAL_KEY,
                emails: [email],
            })
            .timeout(INVITE_TIMEOUT)
            .send()
            .await
            .context("referral invite request failed")?;
        let status = response.status();
        if matches!(status.as_u16(), 400 | 403 | 409 | 422) {
            return Err(DefiniteReferralInviteRejection {
                status: status.as_u16(),
            }
            .into());
        }
        anyhow::ensure!(status.is_success(), "referral invite failed ({status})");
        let response: InviteResponse = response
            .json()
            .await
            .context("referral invite response was invalid")?;
        Ok(match response.has_rewards {
            Some(true) => ReferralRewardStatus::Included,
            Some(false) => ReferralRewardStatus::NotIncluded,
            None => ReferralRewardStatus::Unknown,
        })
    }

    fn auth_manager(&self) -> anyhow::Result<Arc<AuthManager>> {
        self.auth_manager
            .get()
            .cloned()
            .context("ChatGPT authentication is not ready")
    }

    async fn load_auth(
        &self,
        auth_manager: &Arc<AuthManager>,
    ) -> anyhow::Result<(CodexAuth, ReferralIdentity)> {
        let auth = self
            .auth_from_manager(auth_manager)
            .await
            .context("ChatGPT authentication is unavailable")?;
        let identity = Self::identity_from_auth(&auth)?;
        Ok((auth, identity))
    }

    async fn auth_from_manager(
        &self,
        auth_manager: &Arc<AuthManager>,
    ) -> anyhow::Result<CodexAuth> {
        let auth = auth_manager
            .auth()
            .await
            .context("ChatGPT authentication is unavailable")?;
        anyhow::ensure!(
            auth.uses_codex_backend(),
            "referrals require ChatGPT authentication"
        );
        Ok(auth)
    }

    fn identity_from_auth(auth: &CodexAuth) -> anyhow::Result<ReferralIdentity> {
        let user_id = auth
            .get_chatgpt_user_id()
            .context("ChatGPT user ID is unavailable")?;
        let account_id = auth
            .get_account_id()
            .context("ChatGPT account ID is unavailable")?;
        Ok(ReferralIdentity {
            user_id,
            account_id,
        })
    }

    fn base_url(&self) -> anyhow::Result<&str> {
        let base_url = self.base_url.trim_end_matches('/');
        anyhow::ensure!(
            base_url.contains("/backend-api"),
            "referrals require a ChatGPT backend URL"
        );
        Ok(base_url)
    }
}

fn preflight_failure(message: impl Into<String>) -> anyhow::Error {
    ReferralInvitePreflightFailure {
        message: message.into(),
    }
    .into()
}

fn fallback_description(eligibility: &EligibilityResponse) -> String {
    match (&eligibility.grant_amount, &eligibility.grant_action) {
        (Some(amount), Some(action)) => format!(
            "Invite someone and earn {} {}.",
            reward_value_to_string(amount),
            reward_value_to_string(action)
        ),
        (Some(amount), None) => format!(
            "Invite someone and earn a Codex reward worth {}.",
            reward_value_to_string(amount)
        ),
        (None, Some(action)) => format!(
            "Invite someone and earn a Codex {} reward.",
            reward_value_to_string(action)
        ),
        (None, None) => "Invite someone and earn a Codex reward.".to_string(),
    }
}

fn reward_value_to_string(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}
