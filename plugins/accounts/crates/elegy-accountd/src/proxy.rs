use crate::{AuthMethod, BrokerError, BrokerStore, ProviderCatalog, Redactor, VaultError};
use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use url::Url;

#[derive(Deserialize)]
struct CredentialEnvelope {
    version: String,
    fields: std::collections::BTreeMap<String, String>,
}

pub struct AuthenticatedRequest<'a> {
    pub lease: &'a str,
    pub client_id: &'a str,
    pub purpose: &'a str,
    pub provider: &'a str,
    pub operation: &'a str,
    pub method: &'a str,
    pub url: &'a str,
    pub body: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AuthenticatedResponse {
    pub status: u16,
    pub body: Value,
}

#[derive(Debug, Error)]
pub enum ProxyError {
    #[error(transparent)]
    Broker(#[from] BrokerError),
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error("provider operation is not registered")]
    UnknownOperation,
    #[error("request destination is outside the provider audience")]
    DestinationDenied,
    #[error("request method is invalid")]
    InvalidMethod,
    #[error("credential kind requires a code adapter")]
    UnsupportedCredential,
    #[error("provider request failed")]
    Network,
}

impl BrokerStore {
    pub async fn execute_authenticated(
        &self,
        client: &Client,
        catalog: &ProviderCatalog,
        request: AuthenticatedRequest<'_>,
    ) -> Result<AuthenticatedResponse, ProxyError> {
        self.authorize(
            request.lease,
            request.client_id,
            request.purpose,
            request.provider,
            request.operation,
        )?;
        let provider = catalog
            .get(request.provider)
            .ok_or(ProxyError::UnknownOperation)?;
        if !provider.operations.contains_key(request.operation) {
            return Err(ProxyError::UnknownOperation);
        }
        let target = Url::parse(request.url).map_err(|_| ProxyError::DestinationDenied)?;
        let profile = provider
            .auth_profiles
            .iter()
            .find(|profile| same_origin(&target, &profile.audience))
            .ok_or(ProxyError::DestinationDenied)?;
        let method =
            Method::from_bytes(request.method.as_bytes()).map_err(|_| ProxyError::InvalidMethod)?;
        if !matches!(
            method,
            Method::GET | Method::POST | Method::PUT | Method::PATCH | Method::DELETE
        ) {
            return Err(ProxyError::InvalidMethod);
        }
        let account_id = self.account_id_for_lease(request.lease)?;
        let secret = self.vault().load_secret(&account_id)?;
        let secret_text = std::str::from_utf8(secret.as_slice())
            .map_err(|_| ProxyError::UnsupportedCredential)?;
        let builder = client.request(method, target);
        let mut redactions = vec![secret_text.to_owned()];
        let mut builder = match profile.method {
            AuthMethod::OAuthPkce | AuthMethod::DeviceAuthorization | AuthMethod::ApiToken => {
                if let Some(header) = &profile.credential_header {
                    builder.header(header, format!("Bearer {secret_text}"))
                } else {
                    builder.bearer_auth(secret_text)
                }
            }
            AuthMethod::HttpBasic => {
                let envelope = parse_envelope(secret_text)?;
                let username = envelope
                    .fields
                    .get("username")
                    .filter(|value| !value.is_empty())
                    .ok_or(ProxyError::UnsupportedCredential)?;
                let password = envelope
                    .fields
                    .get("password")
                    .filter(|value| !value.is_empty())
                    .ok_or(ProxyError::UnsupportedCredential)?;
                redactions.extend([username.clone(), password.clone()]);
                builder.basic_auth(username, Some(password))
            }
            AuthMethod::ClientCredentials => {
                let envelope = parse_envelope(secret_text)?;
                let client_id = required_field(&envelope, "client_id")?;
                let client_secret = required_field(&envelope, "client_secret")?;
                let token_url = profile
                    .token_url
                    .as_deref()
                    .ok_or(ProxyError::UnsupportedCredential)?;
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
                    .map_err(|_| ProxyError::Network)?;
                if !token_response.status().is_success() {
                    return Err(ProxyError::UnsupportedCredential);
                }
                let token_body: Value = token_response
                    .json()
                    .await
                    .map_err(|_| ProxyError::UnsupportedCredential)?;
                let access_token = token_body
                    .get("access_token")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .ok_or(ProxyError::UnsupportedCredential)?
                    .to_owned();
                redactions.extend([client_id, client_secret, access_token.clone()]);
                builder.bearer_auth(access_token)
            }
            _ => return Err(ProxyError::UnsupportedCredential),
        };
        if let Some(body) = request.body {
            builder = builder.json(&body);
        }
        let response = builder.send().await.map_err(|_| ProxyError::Network)?;
        let status = response.status().as_u16();
        let body = response.json::<Value>().await.unwrap_or(Value::Null);
        let body = Redactor::new(redactions).sanitize(body);
        Ok(AuthenticatedResponse { status, body })
    }
}

fn parse_envelope(secret: &str) -> Result<CredentialEnvelope, ProxyError> {
    let envelope: CredentialEnvelope =
        serde_json::from_str(secret).map_err(|_| ProxyError::UnsupportedCredential)?;
    if envelope.version != "elegy-credential/v1" {
        return Err(ProxyError::UnsupportedCredential);
    }
    Ok(envelope)
}

fn required_field(envelope: &CredentialEnvelope, name: &str) -> Result<String, ProxyError> {
    envelope
        .fields
        .get(name)
        .filter(|value| !value.is_empty())
        .cloned()
        .ok_or(ProxyError::UnsupportedCredential)
}

fn same_origin(target: &Url, audience: &str) -> bool {
    let Ok(audience) = Url::parse(audience) else {
        return false;
    };
    target.scheme() == audience.scheme()
        && target.host_str() == audience.host_str()
        && target.port_or_known_default() == audience.port_or_known_default()
}
