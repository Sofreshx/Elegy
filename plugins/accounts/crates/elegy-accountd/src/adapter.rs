use crate::{AuthMethod, AuthProfile, IdentitySpec};
use reqwest::{
    Client,
    header::{HeaderName, HeaderValue},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;
use zeroize::Zeroizing;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OAuthAdapterConfig {
    pub provider: String,
    pub client_id: String,
    pub token_url: String,
    pub identity: IdentitySpec,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TokenAdapterConfig {
    pub provider: String,
    pub identity: IdentitySpec,
    pub header: String,
    pub prefix: String,
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
    #[error("required credential fields are missing")]
    MissingFields,
}

pub async fn verify_credentials(
    client: &Client,
    provider: &str,
    profile: &AuthProfile,
    fields: BTreeMap<String, String>,
) -> Result<VerifiedCredential, AdapterError> {
    let identity_response = match profile.method {
        AuthMethod::HttpBasic => {
            let username = fields
                .get("username")
                .filter(|value| !value.is_empty())
                .ok_or(AdapterError::MissingFields)?;
            let password = fields
                .get("password")
                .filter(|value| !value.is_empty())
                .ok_or(AdapterError::MissingFields)?;
            client
                .get(&profile.identity.url)
                .basic_auth(username, Some(password))
                .send()
                .await
        }
        AuthMethod::ClientCredentials => {
            let client_id = fields
                .get("client_id")
                .filter(|value| !value.is_empty())
                .ok_or(AdapterError::MissingFields)?;
            let client_secret = fields
                .get("client_secret")
                .filter(|value| !value.is_empty())
                .ok_or(AdapterError::MissingFields)?;
            let token_url = profile
                .token_url
                .as_deref()
                .ok_or(AdapterError::MissingFields)?;
            let token_response = client
                .post(token_url)
                .form(&[
                    ("grant_type", "client_credentials"),
                    ("client_id", client_id.as_str()),
                    ("client_secret", client_secret.as_str()),
                    ("scope", profile.scopes.join(" ").as_str()),
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
            let access_token = token_json
                .get("access_token")
                .and_then(Value::as_str)
                .ok_or(AdapterError::InvalidToken)?;
            client
                .get(&profile.identity.url)
                .bearer_auth(access_token)
                .send()
                .await
        }
        _ => return Err(AdapterError::MissingFields),
    }
    .map_err(|_| AdapterError::Network)?;
    if !identity_response.status().is_success() {
        return Err(AdapterError::IdentityRejected);
    }
    let identity_json: Value = identity_response
        .json()
        .await
        .map_err(|_| AdapterError::UnverifiedIdentity)?;
    let identity = verified_identity(&identity_json, &profile.identity)?;
    let secret = Zeroizing::new(json_envelope(profile.method.clone(), fields)?);
    Ok(VerifiedCredential {
        provider: provider.into(),
        identity,
        secret,
    })
}

fn json_envelope(
    method: AuthMethod,
    fields: BTreeMap<String, String>,
) -> Result<String, AdapterError> {
    serde_json::to_string(&serde_json::json!({
        "version": "elegy-credential/v1",
        "kind": method,
        "fields": fields,
    }))
    .map_err(|_| AdapterError::InvalidToken)
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
        .get(&config.identity.url)
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
    let identity = verified_identity(&identity_json, &config.identity)?;
    Ok(VerifiedCredential {
        provider: config.provider.clone(),
        identity,
        secret,
    })
}

pub async fn verify_token(
    client: &Client,
    config: &TokenAdapterConfig,
    token: &str,
) -> Result<VerifiedCredential, AdapterError> {
    if token.trim().is_empty() || token.len() > 4096 {
        return Err(AdapterError::InvalidToken);
    }
    let secret = Zeroizing::new(token.to_owned());
    let header =
        HeaderName::from_bytes(config.header.as_bytes()).map_err(|_| AdapterError::InvalidToken)?;
    let value = HeaderValue::from_str(&format!("{}{}", config.prefix, secret.as_str()))
        .map_err(|_| AdapterError::InvalidToken)?;
    let response = client
        .get(&config.identity.url)
        .header(header, value)
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
    let identity = verified_identity(&body, &config.identity)?;
    Ok(VerifiedCredential {
        provider: config.provider.clone(),
        identity,
        secret,
    })
}

fn verified_identity(body: &Value, spec: &IdentitySpec) -> Result<String, AdapterError> {
    for (pointer, expected) in &spec.required {
        if body.pointer(pointer) != Some(expected) {
            return Err(AdapterError::IdentityRejected);
        }
    }
    spec.selectors
        .iter()
        .find_map(|pointer| body.pointer(pointer))
        .and_then(|value| match value {
            Value::String(value) if !value.is_empty() => Some(value.clone()),
            Value::Number(value) => Some(value.to_string()),
            _ => None,
        })
        .ok_or(AdapterError::UnverifiedIdentity)
}
