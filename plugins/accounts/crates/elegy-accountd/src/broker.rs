use crate::{Redactor, Vault, VaultError};
use chrono::{Duration, Utc};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

type LeaseAuthorizationRecord = (String, String, String, String, String, i64, i64, i64);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NewAccessRequest {
    pub account_id: String,
    pub client_id: String,
    pub purpose: String,
    pub operations: Vec<String>,
    pub duration_minutes: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BrokerRequest {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub account_id: Option<String>,
    pub provider: Option<String>,
    pub client_id: Option<String>,
    pub purpose: String,
    pub operations: Vec<String>,
    pub duration_minutes: u32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StoredGrant {
    pub id: String,
    pub request_id: String,
    pub account_id: String,
    pub provider: String,
    pub client_id: String,
    pub purpose: String,
    pub operations: Vec<String>,
    pub expires_at: String,
    pub revoked: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OpaqueLease {
    pub token: String,
    pub expires_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AuditEvent {
    pub time: String,
    pub event: String,
    pub account_id: Option<String>,
    pub detail: Value,
}

#[derive(Debug, Error)]
pub enum BrokerError {
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error("broker database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("record was not found")]
    NotFound,
    #[error("request is not awaiting approval")]
    InvalidState,
    #[error("grant or lease has expired or was revoked")]
    Inactive,
    #[error("lease scope does not allow this action")]
    ScopeDenied,
    #[error("stored broker data is invalid")]
    InvalidData,
}

pub struct BrokerStore {
    vault: Vault,
}

impl BrokerStore {
    pub fn new(vault: Vault) -> Self {
        Self { vault }
    }
    pub fn vault(&self) -> &Vault {
        &self.vault
    }

    pub fn request_access(&self, request: NewAccessRequest) -> Result<BrokerRequest, BrokerError> {
        let account = self
            .vault
            .list_accounts()?
            .into_iter()
            .find(|a| a.id == request.account_id)
            .ok_or(BrokerError::NotFound)?;
        let now = Utc::now().to_rfc3339();
        let id = format!("access_{}", Uuid::new_v4().simple());
        let operations = normalize_operations(request.operations);
        let connection = self.vault.connection.lock().map_err(|_| VaultError::Busy)?;
        connection.execute(
            "INSERT INTO broker_requests (id,kind,status,account_id,provider,client_id,purpose,operations_json,duration_minutes,created_at,updated_at) VALUES (?1,'access','awaiting_user',?2,?3,?4,?5,?6,?7,?8,?8)",
            params![id, request.account_id, account.provider, request.client_id, request.purpose, serde_json::to_string(&operations).unwrap(), request.duration_minutes, now],
        )?;
        drop(connection);
        self.audit(
            "access.requested",
            Some(&request.account_id),
            json!({"request_id": id, "client_id": request.client_id, "operations": operations}),
        )?;
        self.get_request(&id)
    }

    pub fn request_creation(
        &self,
        key: &str,
        provider: &str,
        purpose: &str,
        constraints: Vec<String>,
    ) -> Result<BrokerRequest, BrokerError> {
        let digest = Sha256::digest(format!("{key}\0{provider}\0{purpose}").as_bytes());
        let idempotency_key = format!("{:x}", digest);
        let id = format!("create_{}", &idempotency_key[..24]);
        let now = Utc::now().to_rfc3339();
        let operations = normalize_operations(constraints);
        let connection = self.vault.connection.lock().map_err(|_| VaultError::Busy)?;
        connection.execute(
            "INSERT OR IGNORE INTO broker_requests (id,kind,status,provider,purpose,operations_json,duration_minutes,idempotency_key,created_at,updated_at) VALUES (?1,'creation','waiting_human',?2,?3,?4,0,?5,?6,?6)",
            params![id, provider, purpose, serde_json::to_string(&operations).unwrap(), idempotency_key, now],
        )?;
        drop(connection);
        self.audit(
            "creation.requested",
            None,
            json!({"request_id": id, "provider": provider}),
        )?;
        self.get_request(&id)
    }

    pub fn get_request(&self, id: &str) -> Result<BrokerRequest, BrokerError> {
        let connection = self.vault.connection.lock().map_err(|_| VaultError::Busy)?;
        connection.query_row(
            "SELECT id,kind,status,account_id,provider,client_id,purpose,operations_json,duration_minutes,created_at,updated_at FROM broker_requests WHERE id=?1", [id], request_from_row,
        ).optional()?.ok_or(BrokerError::NotFound)
    }

    pub fn cancel_request(&self, id: &str) -> Result<(), BrokerError> {
        let request = self.get_request(id)?;
        if matches!(request.status.as_str(), "approved" | "cancelled") {
            return Err(BrokerError::InvalidState);
        }
        let connection = self.vault.connection.lock().map_err(|_| VaultError::Busy)?;
        connection.execute(
            "UPDATE broker_requests SET status='cancelled',updated_at=?2 WHERE id=?1",
            params![id, Utc::now().to_rfc3339()],
        )?;
        drop(connection);
        self.audit(
            "request.cancelled",
            request.account_id.as_deref(),
            json!({"request_id":id,"partial_credential_stored":false}),
        )
    }

    pub fn list_requests(&self) -> Result<Vec<BrokerRequest>, BrokerError> {
        let connection = self.vault.connection.lock().map_err(|_| VaultError::Busy)?;
        let mut statement = connection.prepare("SELECT id,kind,status,account_id,provider,client_id,purpose,operations_json,duration_minutes,created_at,updated_at FROM broker_requests ORDER BY created_at DESC")?;
        Ok(statement
            .query_map([], request_from_row)?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn approve_access(&self, request_id: &str) -> Result<StoredGrant, BrokerError> {
        let request = self.get_request(request_id)?;
        if request.kind != "access" || request.status != "awaiting_user" {
            return Err(BrokerError::InvalidState);
        }
        let account_id = request.account_id.clone().ok_or(BrokerError::InvalidData)?;
        let provider = request.provider.clone().ok_or(BrokerError::InvalidData)?;
        let client_id = request.client_id.clone().ok_or(BrokerError::InvalidData)?;
        let id = format!("grant_{}", Uuid::new_v4().simple());
        let expires_at =
            (Utc::now() + Duration::minutes(request.duration_minutes as i64)).to_rfc3339();
        let now = Utc::now().to_rfc3339();
        let mut connection = self.vault.connection.lock().map_err(|_| VaultError::Busy)?;
        let transaction = connection.transaction()?;
        transaction.execute("UPDATE broker_requests SET status='approved',updated_at=?2 WHERE id=?1 AND status='awaiting_user'", params![request_id, now])?;
        transaction.execute("INSERT INTO grants (id,request_id,account_id,provider,client_id,purpose,operations_json,expires_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)", params![id, request_id, account_id, provider, client_id, request.purpose, serde_json::to_string(&request.operations).unwrap(), expires_at])?;
        transaction.commit()?;
        drop(connection);
        self.audit(
            "grant.approved",
            Some(&account_id),
            json!({"grant_id": id, "request_id": request_id}),
        )?;
        self.get_grant(&id)
    }

    pub fn get_grant(&self, id: &str) -> Result<StoredGrant, BrokerError> {
        let connection = self.vault.connection.lock().map_err(|_| VaultError::Busy)?;
        connection.query_row("SELECT id,request_id,account_id,provider,client_id,purpose,operations_json,expires_at,revoked_at IS NOT NULL FROM grants WHERE id=?1", [id], |row| Ok(StoredGrant { id: row.get(0)?, request_id: row.get(1)?, account_id: row.get(2)?, provider: row.get(3)?, client_id: row.get(4)?, purpose: row.get(5)?, operations: parse_vec(row.get(6)?)?, expires_at: row.get(7)?, revoked: row.get(8)? })).optional()?.ok_or(BrokerError::NotFound)
    }

    pub fn grant_for_request(&self, request_id: &str) -> Result<StoredGrant, BrokerError> {
        let connection = self.vault.connection.lock().map_err(|_| VaultError::Busy)?;
        let id = connection
            .query_row(
                "SELECT id FROM grants WHERE request_id=?1",
                [request_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(BrokerError::NotFound)?;
        drop(connection);
        self.get_grant(&id)
    }

    pub fn list_grants(&self) -> Result<Vec<StoredGrant>, BrokerError> {
        let connection = self.vault.connection.lock().map_err(|_| VaultError::Busy)?;
        let mut statement = connection.prepare("SELECT id,request_id,account_id,provider,client_id,purpose,operations_json,expires_at,revoked_at IS NOT NULL FROM grants ORDER BY expires_at DESC")?;
        Ok(statement
            .query_map([], |row| {
                Ok(StoredGrant {
                    id: row.get(0)?,
                    request_id: row.get(1)?,
                    account_id: row.get(2)?,
                    provider: row.get(3)?,
                    client_id: row.get(4)?,
                    purpose: row.get(5)?,
                    operations: parse_vec(row.get(6)?)?,
                    expires_at: row.get(7)?,
                    revoked: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn issue_lease(
        &self,
        grant_id: &str,
        ttl_minutes: u32,
    ) -> Result<OpaqueLease, BrokerError> {
        self.issue_lease_with_uses(grant_id, ttl_minutes, -1)
    }

    pub fn issue_single_use_lease(
        &self,
        grant_id: &str,
        ttl_minutes: u32,
    ) -> Result<OpaqueLease, BrokerError> {
        self.issue_lease_with_uses(grant_id, ttl_minutes, 1)
    }

    fn issue_lease_with_uses(
        &self,
        grant_id: &str,
        ttl_minutes: u32,
        remaining_uses: i64,
    ) -> Result<OpaqueLease, BrokerError> {
        let grant = self.get_grant(grant_id)?;
        let grant_expiry = chrono::DateTime::parse_from_rfc3339(&grant.expires_at)
            .map_err(|_| BrokerError::InvalidData)?
            .with_timezone(&Utc);
        if grant.revoked || grant_expiry <= Utc::now() {
            return Err(BrokerError::Inactive);
        }
        let raw = format!("ela_{}", Uuid::new_v4().simple());
        let hash = token_hash(&raw);
        let expires_at = std::cmp::min(
            Utc::now() + Duration::minutes(ttl_minutes.min(15) as i64),
            grant_expiry,
        )
        .to_rfc3339();
        let connection = self.vault.connection.lock().map_err(|_| VaultError::Busy)?;
        connection.execute("INSERT INTO leases (token_hash,grant_id,generation,expires_at,remaining_uses) SELECT ?1,id,generation,?3,?4 FROM grants WHERE id=?2", params![hash, grant_id, expires_at, remaining_uses])?;
        Ok(OpaqueLease {
            token: raw,
            expires_at,
        })
    }

    pub fn authorize(
        &self,
        token: &str,
        client_id: &str,
        purpose: &str,
        audience: &str,
        operation: &str,
    ) -> Result<(), BrokerError> {
        let connection = self.vault.connection.lock().map_err(|_| VaultError::Busy)?;
        let hash = token_hash(token);
        let record: Option<LeaseAuthorizationRecord> = connection.query_row(
            "SELECT l.expires_at,g.expires_at,g.client_id,g.purpose,g.operations_json,l.generation,g.generation,l.remaining_uses FROM leases l JOIN grants g ON g.id=l.grant_id WHERE l.token_hash=?1 AND g.provider=?2 AND g.revoked_at IS NULL",
            params![hash, audience], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?)),
        ).optional()?;
        let Some((
            lease_expiry,
            grant_expiry,
            stored_client,
            stored_purpose,
            operations,
            lease_generation,
            grant_generation,
            remaining_uses,
        )) = record
        else {
            return Err(BrokerError::Inactive);
        };
        if remaining_uses == 0
            || lease_generation != grant_generation
            || chrono::DateTime::parse_from_rfc3339(&lease_expiry)
                .map_err(|_| BrokerError::InvalidData)?
                .with_timezone(&Utc)
                <= Utc::now()
            || chrono::DateTime::parse_from_rfc3339(&grant_expiry)
                .map_err(|_| BrokerError::InvalidData)?
                .with_timezone(&Utc)
                <= Utc::now()
        {
            return Err(BrokerError::Inactive);
        }
        if stored_client != client_id
            || stored_purpose != purpose
            || !parse_vec(operations)?
                .iter()
                .any(|allowed| allowed == operation)
        {
            drop(connection);
            self.audit(
                "operation.denied",
                None,
                json!({"client_id": client_id, "audience": audience, "operation": operation}),
            )?;
            return Err(BrokerError::ScopeDenied);
        }
        if remaining_uses > 0 {
            let changed = connection.execute("UPDATE leases SET remaining_uses=remaining_uses-1 WHERE token_hash=?1 AND remaining_uses>0", [hash])?;
            if changed == 0 {
                return Err(BrokerError::Inactive);
            }
        }
        Ok(())
    }

    pub(crate) fn account_id_for_lease(&self, token: &str) -> Result<String, BrokerError> {
        let connection = self.vault.connection.lock().map_err(|_| VaultError::Busy)?;
        connection
            .query_row(
                "SELECT g.account_id FROM leases l JOIN grants g ON g.id=l.grant_id WHERE l.token_hash=?1",
                [token_hash(token)],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(BrokerError::NotFound)
    }

    pub fn revoke_grant(&self, grant_id: &str, reason: &str) -> Result<(), BrokerError> {
        let grant = self.get_grant(grant_id)?;
        let connection = self.vault.connection.lock().map_err(|_| VaultError::Busy)?;
        let changed = connection.execute("UPDATE grants SET revoked_at=?2,generation=generation+1 WHERE id=?1 AND revoked_at IS NULL", params![grant_id, Utc::now().to_rfc3339()])?;
        if changed == 0 {
            return Err(BrokerError::Inactive);
        }
        drop(connection);
        self.audit(
            "grant.revoked",
            Some(&grant.account_id),
            json!({"grant_id": grant_id, "reason": reason}),
        )
    }

    pub fn disconnect_account(&self, account_id: &str) -> Result<(), BrokerError> {
        let mut connection = self.vault.connection.lock().map_err(|_| VaultError::Busy)?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM leases WHERE grant_id IN (SELECT id FROM grants WHERE account_id=?1)",
            [account_id],
        )?;
        transaction.execute("DELETE FROM grants WHERE account_id=?1", [account_id])?;
        transaction.execute("UPDATE broker_requests SET status='cancelled',updated_at=?2 WHERE account_id=?1 AND status='awaiting_user'", params![account_id, Utc::now().to_rfc3339()])?;
        let changed = transaction.execute("DELETE FROM accounts WHERE id=?1", [account_id])?;
        transaction.commit()?;
        drop(connection);
        if changed == 0 {
            return Err(BrokerError::NotFound);
        }
        self.audit(
            "account.disconnected",
            Some(account_id),
            json!({"credential_deleted": true, "leases_invalidated": true}),
        )
    }

    pub fn list_audit(&self, limit: u32) -> Result<Vec<AuditEvent>, BrokerError> {
        let connection = self.vault.connection.lock().map_err(|_| VaultError::Busy)?;
        let mut statement = connection.prepare(
            "SELECT time,event,account_id,detail_json FROM audit_events ORDER BY id DESC LIMIT ?1",
        )?;
        Ok(statement
            .query_map([limit.min(500)], |row| {
                Ok(AuditEvent {
                    time: row.get(0)?,
                    event: row.get(1)?,
                    account_id: row.get(2)?,
                    detail: serde_json::from_str::<Value>(&row.get::<_, String>(3)?)
                        .unwrap_or(json!({"redacted": true})),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub(crate) fn audit(
        &self,
        event: &str,
        account_id: Option<&str>,
        detail: Value,
    ) -> Result<(), BrokerError> {
        let sanitized = Redactor::new(Vec::<String>::new()).sanitize(detail);
        let connection = self.vault.connection.lock().map_err(|_| VaultError::Busy)?;
        connection.execute(
            "INSERT INTO audit_events (time,event,account_id,detail_json) VALUES (?1,?2,?3,?4)",
            params![
                Utc::now().to_rfc3339(),
                event,
                account_id,
                sanitized.to_string()
            ],
        )?;
        Ok(())
    }
}

fn request_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<BrokerRequest> {
    Ok(BrokerRequest {
        id: row.get(0)?,
        kind: row.get(1)?,
        status: row.get(2)?,
        account_id: row.get(3)?,
        provider: row.get(4)?,
        client_id: row.get(5)?,
        purpose: row.get(6)?,
        operations: parse_vec(row.get(7)?)?,
        duration_minutes: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn parse_vec(value: String) -> rusqlite::Result<Vec<String>> {
    serde_json::from_str(&value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}
fn normalize_operations(mut operations: Vec<String>) -> Vec<String> {
    operations.sort();
    operations.dedup();
    operations
}
fn token_hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}
