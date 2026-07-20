use std::collections::BTreeMap;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use thiserror::Error;

use crate::TypedExecutionRequest;

const PROTOCOL_VERSION: &str = "elegy-account-execution/v1";
const MAX_CLOCK_SKEW: Duration = Duration::minutes(2);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExecutionEnvelope {
    pub schema_version: String,
    pub issued_at: String,
    pub nonce: String,
    pub request: TypedExecutionRequest,
    pub signature: String,
}

#[derive(Default)]
pub struct ReplayGuard {
    seen: BTreeMap<String, DateTime<Utc>>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ExecutionProtocolError {
    #[error("execution protocol version is unsupported")]
    UnsupportedVersion,
    #[error("execution request belongs to another client")]
    WrongClient,
    #[error("execution request timestamp is invalid or stale")]
    Stale,
    #[error("execution request signature is invalid")]
    InvalidSignature,
    #[error("execution request nonce has already been used")]
    Replay,
    #[error("execution request could not be serialized")]
    Serialization,
}

impl ExecutionEnvelope {
    pub fn sign(
        request: TypedExecutionRequest,
        key: &[u8],
        issued_at: DateTime<Utc>,
        nonce: impl Into<String>,
    ) -> Result<Self, ExecutionProtocolError> {
        let mut envelope = Self {
            schema_version: PROTOCOL_VERSION.into(),
            issued_at: issued_at.to_rfc3339(),
            nonce: nonce.into(),
            request,
            signature: String::new(),
        };
        envelope.signature = signature_for(&envelope, key)?;
        Ok(envelope)
    }

    pub fn verify(
        &self,
        key: &[u8],
        expected_client: &str,
        now: DateTime<Utc>,
        replay: &mut ReplayGuard,
    ) -> Result<TypedExecutionRequest, ExecutionProtocolError> {
        if self.schema_version != PROTOCOL_VERSION {
            return Err(ExecutionProtocolError::UnsupportedVersion);
        }
        if self.request.client_id != expected_client {
            return Err(ExecutionProtocolError::WrongClient);
        }
        let issued_at = DateTime::parse_from_rfc3339(&self.issued_at)
            .map_err(|_| ExecutionProtocolError::Stale)?
            .with_timezone(&Utc);
        if issued_at < now - MAX_CLOCK_SKEW || issued_at > now + MAX_CLOCK_SKEW {
            return Err(ExecutionProtocolError::Stale);
        }
        let signature = URL_SAFE_NO_PAD
            .decode(&self.signature)
            .map_err(|_| ExecutionProtocolError::InvalidSignature)?;
        let payload = signing_payload(self)?;
        let mut mac = Hmac::<Sha256>::new_from_slice(key)
            .map_err(|_| ExecutionProtocolError::InvalidSignature)?;
        mac.update(&payload);
        mac.verify_slice(&signature)
            .map_err(|_| ExecutionProtocolError::InvalidSignature)?;
        replay.check_and_record(&self.nonce, issued_at, now)?;
        Ok(self.request.clone())
    }
}

impl ReplayGuard {
    fn check_and_record(
        &mut self,
        nonce: &str,
        issued_at: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<(), ExecutionProtocolError> {
        self.seen
            .retain(|_, seen_at| *seen_at >= now - MAX_CLOCK_SKEW);
        if nonce.is_empty() || self.seen.contains_key(nonce) {
            return Err(ExecutionProtocolError::Replay);
        }
        self.seen.insert(nonce.to_owned(), issued_at);
        Ok(())
    }
}

fn signature_for(
    envelope: &ExecutionEnvelope,
    key: &[u8],
) -> Result<String, ExecutionProtocolError> {
    let payload = signing_payload(envelope)?;
    let mut mac = Hmac::<Sha256>::new_from_slice(key)
        .map_err(|_| ExecutionProtocolError::InvalidSignature)?;
    mac.update(&payload);
    Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
}

fn signing_payload(envelope: &ExecutionEnvelope) -> Result<Vec<u8>, ExecutionProtocolError> {
    #[derive(Serialize)]
    struct Payload<'a> {
        schema_version: &'a str,
        issued_at: &'a str,
        nonce: &'a str,
        request: &'a TypedExecutionRequest,
    }
    serde_json::to_vec(&Payload {
        schema_version: &envelope.schema_version,
        issued_at: &envelope.issued_at,
        nonce: &envelope.nonce,
        request: &envelope.request,
    })
    .map_err(|_| ExecutionProtocolError::Serialization)
}
