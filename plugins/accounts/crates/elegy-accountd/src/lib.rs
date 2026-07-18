use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use thiserror::Error;
use uuid::Uuid;

mod vault;
pub use vault::{
    AccountMetadata, AuthorizationSession, DpapiProtector, KeyProtector, Vault, VaultError,
};
mod provider;
pub use provider::{
    AuthMethod, AuthProfile, ClientRegistration, ClientRegistrationMode, CredentialField,
    IdentitySpec, OAuthCallback, OAuthError, OAuthTransaction, PkceVerifier, ProviderCatalog,
    ProviderError, ProviderSpec,
};
mod broker;
pub use broker::{
    AuditEvent, BrokerError, BrokerRequest, BrokerStore, NewAccessRequest, OpaqueLease, StoredGrant,
};
mod adapter;
pub use adapter::{
    AdapterError, OAuthAdapterConfig, TokenAdapterConfig, VerifiedCredential, exchange_and_verify,
    verify_credentials, verify_token,
};
mod proxy;
pub use proxy::{AuthenticatedRequest, AuthenticatedResponse, ProxyError};

#[derive(Clone, Debug)]
pub struct GrantRequest {
    pub client_id: String,
    pub account_id: String,
    pub purpose: String,
    pub audience: String,
    pub operations: BTreeSet<String>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct Grant {
    pub id: String,
    request: GrantRequest,
    generation: u64,
    revoked: bool,
}

#[derive(Clone, Debug)]
pub struct Lease {
    pub token: String,
    grant_id: String,
    grant_generation: u64,
    expires_at: DateTime<Utc>,
}

#[derive(Default)]
pub struct PolicyEngine {
    grants: HashMap<String, Grant>,
    leases: HashMap<String, Lease>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum LeaseError {
    #[error("grant or lease was not found")]
    NotFound,
    #[error("grant or lease has expired")]
    Expired,
    #[error("grant has been revoked")]
    Revoked,
    #[error("lease belongs to another client")]
    WrongClient,
    #[error("lease was issued for another purpose")]
    WrongPurpose,
    #[error("lease was issued for another audience")]
    WrongAudience,
    #[error("operation is not allowed")]
    OperationDenied,
}

impl PolicyEngine {
    pub fn approve(&mut self, request: GrantRequest) -> Grant {
        let grant = Grant {
            id: Uuid::new_v4().to_string(),
            request,
            generation: 0,
            revoked: false,
        };
        self.grants.insert(grant.id.clone(), grant.clone());
        grant
    }

    pub fn issue_lease(&mut self, grant_id: &str, ttl: Duration) -> Result<Lease, LeaseError> {
        let grant = self.grants.get(grant_id).ok_or(LeaseError::NotFound)?;
        if grant.revoked {
            return Err(LeaseError::Revoked);
        }
        let now = Utc::now();
        if grant.request.expires_at <= now {
            return Err(LeaseError::Expired);
        }
        let lease = Lease {
            token: format!("ela_{}", Uuid::new_v4().simple()),
            grant_id: grant.id.clone(),
            grant_generation: grant.generation,
            expires_at: std::cmp::min(now + ttl, grant.request.expires_at),
        };
        self.leases.insert(lease.token.clone(), lease.clone());
        Ok(lease)
    }

    pub fn authorize(
        &self,
        token: &str,
        client_id: &str,
        purpose: &str,
        audience: &str,
        operation: &str,
    ) -> Result<(), LeaseError> {
        let lease = self.leases.get(token).ok_or(LeaseError::NotFound)?;
        let grant = self
            .grants
            .get(&lease.grant_id)
            .ok_or(LeaseError::NotFound)?;
        if grant.revoked || grant.generation != lease.grant_generation {
            return Err(LeaseError::Revoked);
        }
        let now = Utc::now();
        if lease.expires_at <= now || grant.request.expires_at <= now {
            return Err(LeaseError::Expired);
        }
        if grant.request.client_id != client_id {
            return Err(LeaseError::WrongClient);
        }
        if grant.request.purpose != purpose {
            return Err(LeaseError::WrongPurpose);
        }
        if grant.request.audience != audience {
            return Err(LeaseError::WrongAudience);
        }
        if !grant.request.operations.contains(operation) {
            return Err(LeaseError::OperationDenied);
        }
        Ok(())
    }

    pub fn revoke(&mut self, grant_id: &str) -> Result<(), LeaseError> {
        let grant = self.grants.get_mut(grant_id).ok_or(LeaseError::NotFound)?;
        grant.revoked = true;
        grant.generation = grant.generation.saturating_add(1);
        Ok(())
    }
}

pub struct Redactor {
    canaries: Vec<String>,
}

impl Redactor {
    pub fn new<I, S>(canaries: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            canaries: canaries.into_iter().map(Into::into).collect(),
        }
    }

    pub fn sanitize(&self, mut value: Value) -> Value {
        self.walk(&mut value, None);
        value
    }

    fn walk(&self, value: &mut Value, key: Option<&str>) {
        const SECRET_KEYS: &[&str] = &[
            "authorization",
            "password",
            "access_token",
            "refresh_token",
            "api_key",
            "client_secret",
            "cookie",
            "set-cookie",
            "secret_value",
        ];
        if key.is_some_and(|candidate| {
            SECRET_KEYS
                .iter()
                .any(|secret| candidate.eq_ignore_ascii_case(secret))
        }) {
            *value = Value::String("[REDACTED]".into());
            return;
        }
        match value {
            Value::Object(map) => {
                for (child_key, child) in map {
                    self.walk(child, Some(child_key));
                }
            }
            Value::Array(items) => {
                for item in items {
                    self.walk(item, None);
                }
            }
            Value::String(text) => {
                for canary in &self.canaries {
                    if !canary.is_empty() && text.contains(canary) {
                        *text = text.replace(canary, "[REDACTED]");
                    }
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointKind {
    Captcha,
    Mfa,
    Terms,
    Payment,
    IdentityVerification,
    AmbiguousPlan,
    UnexpectedPage,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HumanCheckpoint {
    pub kind: CheckpointKind,
    pub instructions: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProvisioningState {
    Requested,
    Preflight,
    WaitingHuman,
    Verifying,
    Connected,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum SagaError {
    #[error("transition is not allowed from the current state")]
    InvalidTransition,
    #[error("a human must explicitly resume this checkpoint")]
    HumanRequired,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProvisioningSaga {
    id: String,
    pub provider: String,
    pub purpose: String,
    pub state: ProvisioningState,
    checkpoint: Option<HumanCheckpoint>,
}

impl ProvisioningSaga {
    pub fn requested(request_key: &str, provider: &str, purpose: &str) -> Self {
        let mut digest = Sha256::new();
        digest.update(request_key.as_bytes());
        digest.update([0]);
        digest.update(provider.as_bytes());
        digest.update([0]);
        digest.update(purpose.as_bytes());
        let id = format!("saga_{}", &format!("{:x}", digest.finalize())[..24]);
        Self {
            id,
            provider: provider.into(),
            purpose: purpose.into(),
            state: ProvisioningState::Requested,
            checkpoint: None,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn start(&mut self) -> Result<(), SagaError> {
        if self.state != ProvisioningState::Requested {
            return Err(SagaError::InvalidTransition);
        }
        self.state = ProvisioningState::Preflight;
        Ok(())
    }

    pub fn require_human(
        &mut self,
        kind: CheckpointKind,
        instructions: impl Into<String>,
    ) -> Result<(), SagaError> {
        if self.state != ProvisioningState::Preflight && self.state != ProvisioningState::Verifying
        {
            return Err(SagaError::InvalidTransition);
        }
        self.checkpoint = Some(HumanCheckpoint {
            kind,
            instructions: instructions.into(),
        });
        self.state = ProvisioningState::WaitingHuman;
        Ok(())
    }

    pub fn is_waiting_for_human(&self) -> bool {
        self.state == ProvisioningState::WaitingHuman
    }

    pub fn checkpoint(&self) -> Option<&HumanCheckpoint> {
        self.checkpoint.as_ref()
    }

    pub fn automated_resume(&mut self) -> Result<(), SagaError> {
        if self.state == ProvisioningState::WaitingHuman {
            return Err(SagaError::HumanRequired);
        }
        Err(SagaError::InvalidTransition)
    }

    pub fn human_resume(&mut self) -> Result<(), SagaError> {
        if self.state != ProvisioningState::WaitingHuman {
            return Err(SagaError::InvalidTransition);
        }
        self.checkpoint = None;
        self.state = ProvisioningState::Verifying;
        Ok(())
    }

    pub fn complete(&mut self) -> Result<(), SagaError> {
        if self.state != ProvisioningState::Verifying {
            return Err(SagaError::InvalidTransition);
        }
        self.state = ProvisioningState::Connected;
        Ok(())
    }

    pub fn cancel(&mut self) -> Result<(), SagaError> {
        if matches!(
            self.state,
            ProvisioningState::Connected | ProvisioningState::Cancelled
        ) {
            return Err(SagaError::InvalidTransition);
        }
        self.checkpoint = None;
        self.state = ProvisioningState::Cancelled;
        Ok(())
    }
}
