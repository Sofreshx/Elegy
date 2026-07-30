use crate::{AuthMethod, AuthProfile, IdentitySpec, OAuthLifecycle, Vault};
use chrono::{Duration, Utc};
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
    pub required_scopes: Vec<String>,
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

#[derive(Clone, Deserialize, Serialize)]
pub struct OAuthCredential {
    pub version: String,
    pub access_token: String,
    pub refresh_token: String,
    pub scopes: Vec<String>,
    pub expires_at: String,
}

impl std::fmt::Debug for OAuthCredential {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OAuthCredential")
            .field("version", &self.version)
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("scopes", &self.scopes)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

impl OAuthCredential {
    pub fn from_secret(secret: &str) -> Result<Self, AdapterError> {
        let credential: Self =
            serde_json::from_str(secret).map_err(|_| AdapterError::InvalidToken)?;
        if credential.version != "elegy-oauth-credential/v1"
            || credential.access_token.is_empty()
            || credential.refresh_token.is_empty()
        {
            return Err(AdapterError::InvalidToken);
        }
        Ok(credential)
    }

    pub fn to_secret(&self) -> Result<Zeroizing<String>, AdapterError> {
        serde_json::to_string(self)
            .map(Zeroizing::new)
            .map_err(|_| AdapterError::InvalidToken)
    }

    pub fn expires_within(&self, duration: Duration) -> Result<bool, AdapterError> {
        let expires_at = chrono::DateTime::parse_from_rfc3339(&self.expires_at)
            .map_err(|_| AdapterError::InvalidToken)?
            .with_timezone(&Utc);
        Ok(expires_at <= Utc::now() + duration)
    }
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
    #[error("provider granted fewer scopes than required")]
    ReducedScopes,
    #[error("provider credential revocation is pending")]
    RevocationPending,
    #[error("credential was refreshed concurrently; stale rotation was discarded")]
    RefreshConflict,
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
    let refresh_token = token_json
        .get("refresh_token")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(AdapterError::InvalidToken)?;
    let scopes = token_scopes(&token_json).ok_or(AdapterError::InvalidToken)?;
    require_scopes(&scopes, &config.required_scopes)?;
    let expires_at = expiry_from(&token_json)?;
    let access_token = Zeroizing::new(token.to_owned());
    let identity_response = client
        .get(&config.identity.url)
        .bearer_auth(access_token.as_str())
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
    let secret = OAuthCredential {
        version: "elegy-oauth-credential/v1".to_string(),
        access_token: access_token.to_string(),
        refresh_token: refresh_token.to_string(),
        scopes,
        expires_at,
    }
    .to_secret()?;
    Ok(VerifiedCredential {
        provider: config.provider.clone(),
        identity,
        secret,
    })
}

pub async fn refresh_oauth_credential(
    client: &Client,
    config: &OAuthAdapterConfig,
    lifecycle: &OAuthLifecycle,
    current: &OAuthCredential,
) -> Result<OAuthCredential, AdapterError> {
    let response = client
        .post(&lifecycle.refresh_url)
        .header(reqwest::header::ACCEPT, "application/json")
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", config.client_id.as_str()),
            ("refresh_token", current.refresh_token.as_str()),
        ])
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
    let access_token = body
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(AdapterError::InvalidToken)?;
    let scopes = token_scopes(&body).unwrap_or_else(|| current.scopes.clone());
    require_scopes(&scopes, &config.required_scopes)?;
    Ok(OAuthCredential {
        version: current.version.clone(),
        access_token: access_token.to_string(),
        refresh_token: body
            .get("refresh_token")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .unwrap_or(&current.refresh_token)
            .to_string(),
        scopes,
        expires_at: expiry_from(&body)?,
    })
}

pub async fn refresh_stored_oauth_credential(
    client: &Client,
    vault: &Vault,
    account_id: &str,
    config: &OAuthAdapterConfig,
    lifecycle: &OAuthLifecycle,
) -> Result<OAuthCredential, AdapterError> {
    let (secret, generation) = vault
        .load_secret_versioned(account_id)
        .map_err(|_| AdapterError::InvalidToken)?;
    let current = std::str::from_utf8(secret.as_slice())
        .ok()
        .and_then(|secret| OAuthCredential::from_secret(secret).ok())
        .ok_or(AdapterError::InvalidToken)?;
    let refreshed = refresh_oauth_credential(client, config, lifecycle, &current).await?;
    let replacement = refreshed.to_secret()?;
    vault
        .replace_secret_if_generation(account_id, generation, replacement.as_bytes())
        .map_err(|error| match error {
            crate::VaultError::Conflict => AdapterError::RefreshConflict,
            _ => AdapterError::InvalidToken,
        })?;
    Ok(refreshed)
}

pub async fn revoke_oauth_credential(
    client: &Client,
    lifecycle: &OAuthLifecycle,
    credential: &OAuthCredential,
) -> Result<(), AdapterError> {
    let mut form = vec![("token", credential.refresh_token.as_str())];
    if let Some(hint) = lifecycle.revocation_token_type_hint.as_deref() {
        form.push(("token_type_hint", hint));
    }
    let response = client
        .post(&lifecycle.revocation_url)
        .form(&form)
        .send()
        .await
        .map_err(|_| AdapterError::Network)?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(AdapterError::RevocationPending)
    }
}

fn token_scopes(body: &Value) -> Option<Vec<String>> {
    body.get("scope")
        .and_then(Value::as_str)
        .map(|scope| {
            scope
                .split_whitespace()
                .filter(|scope| !scope.is_empty())
                .map(str::to_string)
                .collect()
        })
        .or_else(|| {
            body.get("scopes").and_then(Value::as_array).map(|scopes| {
                scopes
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
        })
}

fn require_scopes(granted: &[String], required: &[String]) -> Result<(), AdapterError> {
    if required
        .iter()
        .all(|scope| granted.iter().any(|granted| granted == scope))
    {
        Ok(())
    } else {
        Err(AdapterError::ReducedScopes)
    }
}

fn expiry_from(body: &Value) -> Result<String, AdapterError> {
    let expires_in = body
        .get("expires_in")
        .and_then(Value::as_i64)
        .filter(|seconds| *seconds > 0)
        .ok_or(AdapterError::InvalidToken)?;
    Ok((Utc::now() + Duration::seconds(expires_in)).to_rfc3339())
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
