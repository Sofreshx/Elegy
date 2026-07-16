use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    OAuthPkce,
    DeviceCode,
    GitHubApp,
    GuidedApiToken,
    GuidedFineGrainedPat,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProviderSpec {
    pub id: String,
    pub display_name: String,
    pub auth_methods: Vec<AuthMethod>,
    pub issuer: String,
    pub audience: String,
    pub identity_endpoint: String,
    pub browser_origins: Vec<String>,
    pub operations: BTreeMap<String, Vec<String>>,
}

pub struct ProviderCatalog {
    providers: HashMap<String, ProviderSpec>,
}

impl ProviderCatalog {
    pub fn mvp() -> Self {
        let providers = [
            ProviderSpec {
                id: "cloudflare".into(),
                display_name: "Cloudflare".into(),
                auth_methods: vec![AuthMethod::GuidedApiToken],
                issuer: "https://dash.cloudflare.com".into(),
                audience: "https://api.cloudflare.com".into(),
                identity_endpoint: "https://api.cloudflare.com/client/v4/user/tokens/verify".into(),
                browser_origins: vec!["https://dash.cloudflare.com".into()],
                operations: BTreeMap::from([
                    ("account.profile.read".into(), vec!["user:read".into()]),
                    ("zones.read".into(), vec!["zone:read".into()]),
                    ("dns.records.read".into(), vec!["dns:read".into()]),
                    ("dns.records.write".into(), vec!["dns:write".into()]),
                ]),
            },
            ProviderSpec {
                id: "github".into(),
                display_name: "GitHub".into(),
                auth_methods: vec![AuthMethod::DeviceCode],
                issuer: "https://github.com".into(),
                audience: "https://api.github.com".into(),
                identity_endpoint: "https://api.github.com/user".into(),
                browser_origins: vec!["https://github.com".into()],
                operations: BTreeMap::from([
                    ("profile.read".into(), vec!["read:user".into()]),
                    ("repositories.read".into(), vec!["contents:read".into()]),
                    (
                        "pull_requests.write".into(),
                        vec!["pull_requests:write".into()],
                    ),
                ]),
            },
        ]
        .into_iter()
        .map(|provider| (provider.id.clone(), provider))
        .collect();
        Self { providers }
    }

    pub fn get(&self, id: &str) -> Option<&ProviderSpec> {
        self.providers.get(id)
    }

    pub fn list(&self) -> Vec<&ProviderSpec> {
        let mut providers: Vec<_> = self.providers.values().collect();
        providers.sort_by_key(|provider| provider.display_name.as_str());
        providers
    }
}

#[derive(Clone, Debug, Zeroize, ZeroizeOnDrop)]
pub struct PkceVerifier(String);

impl PkceVerifier {
    pub fn expose_for_token_exchange(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct OAuthTransaction {
    pub provider_id: String,
    pub state: String,
    pub nonce: String,
    pub pkce_verifier: PkceVerifier,
    pub pkce_challenge: String,
    expected_issuer: String,
    expected_audience: String,
    expected_redirect_uri: String,
}

#[derive(Clone, Debug)]
pub struct OAuthCallback {
    pub state: String,
    pub nonce: String,
    pub issuer: String,
    pub audience: String,
    pub redirect_uri: String,
    pub code: String,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum OAuthError {
    #[error("OAuth state did not match")]
    StateMismatch,
    #[error("OIDC nonce did not match")]
    NonceMismatch,
    #[error("issuer did not match")]
    IssuerMismatch,
    #[error("audience or resource did not match")]
    AudienceMismatch,
    #[error("redirect URI did not match")]
    RedirectMismatch,
    #[error("authorization code was missing")]
    MissingCode,
}

impl OAuthTransaction {
    pub fn new(provider_id: &str, issuer: &str, audience: &str, redirect_uri: &str) -> Self {
        let state = random_urlsafe(32);
        let nonce = random_urlsafe(32);
        let verifier = random_urlsafe(64);
        let pkce_challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Self {
            provider_id: provider_id.into(),
            state,
            nonce,
            pkce_verifier: PkceVerifier(verifier),
            pkce_challenge,
            expected_issuer: issuer.into(),
            expected_audience: audience.into(),
            expected_redirect_uri: redirect_uri.into(),
        }
    }

    pub fn validate(&self, callback: &OAuthCallback) -> Result<(), OAuthError> {
        if callback.state != self.state {
            return Err(OAuthError::StateMismatch);
        }
        if callback.nonce != self.nonce {
            return Err(OAuthError::NonceMismatch);
        }
        if callback.issuer != self.expected_issuer {
            return Err(OAuthError::IssuerMismatch);
        }
        if callback.audience != self.expected_audience {
            return Err(OAuthError::AudienceMismatch);
        }
        if callback.redirect_uri != self.expected_redirect_uri {
            return Err(OAuthError::RedirectMismatch);
        }
        if callback.code.is_empty() {
            return Err(OAuthError::MissingCode);
        }
        Ok(())
    }
}

fn random_urlsafe(bytes: usize) -> String {
    let mut value = vec![0_u8; bytes];
    rand::rng().fill(value.as_mut_slice());
    URL_SAFE_NO_PAD.encode(value)
}
