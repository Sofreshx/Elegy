use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use zeroize::Zeroizing;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OAuthAdapterConfig {
    pub provider: String,
    pub client_id: String,
    pub token_url: String,
    pub identity_url: String,
}

pub struct VerifiedCredential {
    pub provider: String,
    pub identity: String,
    pub secret: Zeroizing<String>,
}

impl std::fmt::Debug for VerifiedCredential {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VerifiedCredential")
            .field("provider", &self.provider)
            .field("identity", &self.identity)
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("provider network request failed")]
    Network,
    #[error("provider rejected the authorization code")]
    TokenRejected,
    #[error("provider returned an invalid token response")]
    InvalidToken,
    #[error("provider rejected identity verification")]
    IdentityRejected,
    #[error("provider did not return a verifiable identity")]
    UnverifiedIdentity,
}

pub async fn exchange_and_verify(
    client: &Client,
    config: &OAuthAdapterConfig,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<VerifiedCredential, AdapterError> {
    let token_response = client
        .post(&config.token_url)
        .header(reqwest::header::ACCEPT, "application/json")
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", config.client_id.as_str()),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", verifier),
        ])
        .send()
        .await
        .map_err(|_| AdapterError::Network)?;
    if !token_response.status().is_success() {
        return Err(AdapterError::TokenRejected);
    }
    let token_json: Value = token_response
        .json()
        .await
        .map_err(|_| AdapterError::InvalidToken)?;
    let token = token_json
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or(AdapterError::InvalidToken)?;
    let secret = Zeroizing::new(token.to_owned());
    let identity_response = client
        .get(&config.identity_url)
        .bearer_auth(secret.as_str())
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|_| AdapterError::Network)?;
    if !identity_response.status().is_success() {
        return Err(AdapterError::IdentityRejected);
    }
    let identity_json: Value = identity_response
        .json()
        .await
        .map_err(|_| AdapterError::UnverifiedIdentity)?;
    let identity = ["email", "login", "username", "id"]
        .iter()
        .find_map(|key| {
            identity_json
                .get(key)
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .ok_or(AdapterError::UnverifiedIdentity)?;
    Ok(VerifiedCredential {
        provider: config.provider.clone(),
        identity,
        secret,
    })
}

pub async fn verify_cloudflare_token(
    client: &Client,
    verify_url: &str,
    token: &str,
) -> Result<VerifiedCredential, AdapterError> {
    if token.trim().is_empty() || token.len() > 4096 {
        return Err(AdapterError::InvalidToken);
    }
    let secret = Zeroizing::new(token.to_owned());
    let response = client
        .get(verify_url)
        .bearer_auth(secret.as_str())
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|_| AdapterError::Network)?;
    if !response.status().is_success() {
        return Err(AdapterError::TokenRejected);
    }
    let body: Value = response
        .json()
        .await
        .map_err(|_| AdapterError::InvalidToken)?;
    let result = body.get("result").ok_or(AdapterError::InvalidToken)?;
    let active = body.get("success").and_then(Value::as_bool) == Some(true)
        && result.get("status").and_then(Value::as_str) == Some("active");
    let token_id = result.get("id").and_then(Value::as_str);
    if !active || token_id.is_none_or(str::is_empty) {
        return Err(AdapterError::TokenRejected);
    }
    Ok(VerifiedCredential {
        provider: "cloudflare".into(),
        identity: format!("token:{}", token_id.unwrap()),
        secret,
    })
}
