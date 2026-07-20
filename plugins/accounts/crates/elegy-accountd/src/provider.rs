use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OperationRisk {
    Read,
    Write,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OperationExecutor {
    Http {
        profile: String,
        method: String,
        path: String,
    },
    Adapter {
        adapter: String,
        version: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProviderOperation {
    pub description: String,
    pub risk: OperationRisk,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub input_schema: serde_json::Value,
    pub result_schema: serde_json::Value,
    pub executor: OperationExecutor,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum OperationSpec {
    LegacyScopes(Vec<String>),
    Executable(ProviderOperation),
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
    pub operations: BTreeMap<String, OperationSpec>,
}

impl ProviderSpec {
    pub fn operation_scopes(&self, operation: &str) -> Option<&[String]> {
        match self.operations.get(operation)? {
            OperationSpec::LegacyScopes(scopes) => Some(scopes),
            OperationSpec::Executable(spec) => Some(&spec.scopes),
        }
    }

    pub fn executable_operation(&self, operation: &str) -> Option<&ProviderOperation> {
        match self.operations.get(operation)? {
            OperationSpec::Executable(spec) => Some(spec),
            OperationSpec::LegacyScopes(_) => None,
        }
    }
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("provider manifest is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("provider directory could not be read: {0}")]
    Io(#[from] std::io::Error),
    #[error("provider manifest must use schema elegy-account-provider/v1 or v2")]
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
    #[error("provider v2 operations must use typed executable definitions")]
    IncompleteOperation,
    #[error("provider read operation must use GET")]
    MutatingReadOperation,
    #[error("provider HTTP operation must use an audience-relative path")]
    UnsafeOperationPath,
    #[error("provider operation references an unknown auth profile")]
    UnknownOperationProfile,
}

#[derive(Clone, Debug)]
pub struct ProviderCatalog {
    providers: HashMap<String, ProviderSpec>,
    executable_providers: HashSet<String>,
}

impl ProviderCatalog {
    pub fn from_json_documents<'a>(
        documents: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, ProviderError> {
        Self::from_documents_with_trust(documents, true)
    }

    pub fn from_untrusted_json_documents<'a>(
        documents: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, ProviderError> {
        Self::from_documents_with_trust(documents, false)
    }

    fn from_documents_with_trust<'a>(
        documents: impl IntoIterator<Item = &'a str>,
        trusted: bool,
    ) -> Result<Self, ProviderError> {
        let mut providers = HashMap::new();
        let mut executable_providers = HashSet::new();
        for document in documents {
            let provider: ProviderSpec = serde_json::from_str(document)?;
            validate_provider(&provider)?;
            let id = provider.id.clone();
            if trusted {
                executable_providers.insert(id.clone());
            }
            if providers.insert(id.clone(), provider).is_some() {
                return Err(ProviderError::Duplicate(id));
            }
        }
        Ok(Self {
            providers,
            executable_providers,
        })
    }

    pub fn load_directory(path: impl AsRef<Path>) -> Result<Self, ProviderError> {
        Self::load_directory_with_trust(path, true)
    }

    pub fn load_untrusted_directory(path: impl AsRef<Path>) -> Result<Self, ProviderError> {
        Self::load_directory_with_trust(path, false)
    }

    fn load_directory_with_trust(
        path: impl AsRef<Path>,
        trusted: bool,
    ) -> Result<Self, ProviderError> {
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
        Self::from_documents_with_trust(documents.iter().map(String::as_str), trusted)
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

    pub fn executable_operation(
        &self,
        provider: &str,
        operation: &str,
    ) -> Option<&ProviderOperation> {
        if !self.executable_providers.contains(provider) {
            return None;
        }
        self.get(provider)?.executable_operation(operation)
    }
}

fn validate_provider(provider: &ProviderSpec) -> Result<(), ProviderError> {
    if !matches!(
        provider.schema_version.as_str(),
        "elegy-account-provider/v1" | "elegy-account-provider/v2"
    ) {
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
    for operation in provider.operations.values() {
        match operation {
            OperationSpec::LegacyScopes(_) if provider.schema_version.ends_with("/v2") => {
                return Err(ProviderError::IncompleteOperation);
            }
            OperationSpec::LegacyScopes(_) => {}
            OperationSpec::Executable(spec) => {
                if !provider.schema_version.ends_with("/v2") || spec.description.trim().is_empty() {
                    return Err(ProviderError::IncompleteOperation);
                }
                if let OperationExecutor::Http {
                    profile,
                    method,
                    path,
                } = &spec.executor
                {
                    if !provider
                        .auth_profiles
                        .iter()
                        .any(|candidate| candidate.id == *profile)
                    {
                        return Err(ProviderError::UnknownOperationProfile);
                    }
                    if !path.starts_with('/')
                        || path.starts_with("//")
                        || path.split('/').any(|segment| segment == "..")
                        || Url::parse(path).is_ok()
                    {
                        return Err(ProviderError::UnsafeOperationPath);
                    }
                    if spec.risk == OperationRisk::Read && !method.eq_ignore_ascii_case("GET") {
                        return Err(ProviderError::MutatingReadOperation);
                    }
                }
            }
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
