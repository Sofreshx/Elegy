use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::Path,
};
use thiserror::Error;
use url::Url;
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    #[serde(rename = "oauth_pkce")]
    OAuthPkce,
    DeviceAuthorization,
    ApiToken,
    HttpBasic,
    ClientCredentials,
    ServiceCredential,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClientRegistrationMode {
    Environment,
    Public,
    UserProvided,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ClientRegistration {
    pub mode: ClientRegistrationMode,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub client_id_env: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct IdentitySpec {
    pub url: String,
    pub selectors: Vec<String>,
    #[serde(default)]
    pub required: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CredentialField {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub secret: bool,
    #[serde(default)]
    pub autocomplete: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AuthProfile {
    pub id: String,
    pub method: AuthMethod,
    pub audience: String,
    #[serde(default)]
    pub issuer: Option<String>,
    #[serde(default)]
    pub authorization_url: Option<String>,
    #[serde(default)]
    pub token_url: Option<String>,
    #[serde(default)]
    pub device_authorization_url: Option<String>,
    pub identity: IdentitySpec,
    pub client: ClientRegistration,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub credential_header: Option<String>,
    #[serde(default)]
    pub creation_url: Option<String>,
    #[serde(default)]
    pub credential_fields: Vec<CredentialField>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProviderSpec {
    pub schema_version: String,
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub publisher: String,
    pub browser_origins: Vec<String>,
    pub auth_profiles: Vec<AuthProfile>,
    pub operations: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("provider manifest is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("provider directory could not be read: {0}")]
    Io(#[from] std::io::Error),
    #[error("provider manifest must use schema elegy-account-provider/v1")]
    UnsupportedSchema,
    #[error("provider identifier is invalid")]
    InvalidId,
    #[error("provider {0} is already registered")]
    Duplicate(String),
    #[error("provider URLs must use HTTPS, except loopback test and callback URLs")]
    InsecureUrl,
    #[error("provider URL is invalid")]
    InvalidUrl,
    #[error("provider auth profile is incomplete")]
    IncompleteProfile,
}

#[derive(Clone, Debug)]
pub struct ProviderCatalog {
    providers: HashMap<String, ProviderSpec>,
}

impl ProviderCatalog {
    pub fn from_json_documents<'a>(
        documents: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, ProviderError> {
        let mut providers = HashMap::new();
        for document in documents {
            let provider: ProviderSpec = serde_json::from_str(document)?;
            validate_provider(&provider)?;
            let id = provider.id.clone();
            if providers.insert(id.clone(), provider).is_some() {
                return Err(ProviderError::Duplicate(id));
            }
        }
        Ok(Self { providers })
    }

    pub fn load_directory(path: impl AsRef<Path>) -> Result<Self, ProviderError> {
        let mut documents = Vec::new();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry
                .path()
                .extension()
                .is_some_and(|value| value == "json")
            {
                documents.push(fs::read_to_string(entry.path())?);
            }
        }
        Self::from_json_documents(documents.iter().map(String::as_str))
    }

    pub fn get(&self, id: &str) -> Option<&ProviderSpec> {
        self.providers.get(id)
    }

    pub fn list(&self) -> Vec<&ProviderSpec> {
        let mut providers: Vec<_> = self.providers.values().collect();
        providers.sort_by_key(|provider| provider.display_name.as_str());
        providers
    }

    pub fn profile(&self, provider: &str, profile: &str) -> Option<&AuthProfile> {
        self.get(provider)?
            .auth_profiles
            .iter()
            .find(|candidate| candidate.id == profile)
    }
}

fn validate_provider(provider: &ProviderSpec) -> Result<(), ProviderError> {
    if provider.schema_version != "elegy-account-provider/v1" {
        return Err(ProviderError::UnsupportedSchema);
    }
    if provider.id.is_empty()
        || provider.id.len() > 64
        || !provider
            .id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(ProviderError::InvalidId);
    }
    for value in &provider.browser_origins {
        validate_url(value)?;
    }
    for profile in &provider.auth_profiles {
        validate_url(&profile.audience)?;
        validate_url(&profile.identity.url)?;
        for value in [
            profile.issuer.as_deref(),
            profile.authorization_url.as_deref(),
            profile.token_url.as_deref(),
            profile.device_authorization_url.as_deref(),
            profile.creation_url.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            validate_url(value)?;
        }
        let complete = match profile.method {
            AuthMethod::OAuthPkce => {
                profile.authorization_url.is_some() && profile.token_url.is_some()
            }
            AuthMethod::DeviceAuthorization => {
                profile.device_authorization_url.is_some() && profile.token_url.is_some()
            }
            AuthMethod::ClientCredentials => profile.token_url.is_some(),
            AuthMethod::ApiToken | AuthMethod::HttpBasic | AuthMethod::ServiceCredential => true,
        };
        if !complete || profile.identity.selectors.is_empty() {
            return Err(ProviderError::IncompleteProfile);
        }
    }
    Ok(())
}

fn validate_url(value: &str) -> Result<(), ProviderError> {
    let url = Url::parse(value).map_err(|_| ProviderError::InvalidUrl)?;
    let loopback = url.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("localhost")
            || host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
        return Err(ProviderError::InsecureUrl);
    }
    Ok(())
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
