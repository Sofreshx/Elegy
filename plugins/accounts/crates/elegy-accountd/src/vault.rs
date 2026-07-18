use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use chrono::Utc;
use rand::Rng;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AccountMetadata {
    pub id: String,
    pub provider: String,
    pub verified_identity: String,
    pub auth_method: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AuthorizationSession {
    pub id: String,
    pub provider: String,
    pub status: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_at: String,
    pub interval_seconds: u64,
    pub next_poll_at: String,
    pub attempts: u64,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("vault database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("operating-system key protection failed: {0}")]
    Protection(String),
    #[error("credential was not found")]
    NotFound,
    #[error("credential authentication failed")]
    Authentication,
    #[error("vault is busy")]
    Busy,
}

pub trait KeyProtector: Send + Sync {
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, VaultError>;
    fn unprotect(&self, protected: &[u8]) -> Result<Vec<u8>, VaultError>;
}

#[derive(Clone, Copy, Debug)]
pub struct DpapiProtector;

#[cfg(windows)]
impl KeyProtector for DpapiProtector {
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, VaultError> {
        use std::ptr::{null, null_mut};
        use windows_sys::Win32::Foundation::LocalFree;
        use windows_sys::Win32::Security::Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData,
        };

        let mut input_bytes = plaintext.to_vec();
        let input = CRYPT_INTEGER_BLOB {
            cbData: input_bytes.len() as u32,
            pbData: input_bytes.as_mut_ptr(),
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: null_mut(),
        };
        let ok = unsafe {
            CryptProtectData(
                &input,
                null(),
                null(),
                null(),
                null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        input_bytes.zeroize();
        if ok == 0 {
            return Err(VaultError::Protection(
                std::io::Error::last_os_error().to_string(),
            ));
        }
        let result =
            unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
        unsafe {
            LocalFree(output.pbData.cast());
        }
        Ok(result)
    }

    fn unprotect(&self, protected: &[u8]) -> Result<Vec<u8>, VaultError> {
        use std::ptr::{null, null_mut};
        use windows_sys::Win32::Foundation::LocalFree;
        use windows_sys::Win32::Security::Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptUnprotectData,
        };

        let mut input_bytes = protected.to_vec();
        let input = CRYPT_INTEGER_BLOB {
            cbData: input_bytes.len() as u32,
            pbData: input_bytes.as_mut_ptr(),
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: null_mut(),
        };
        let ok = unsafe {
            CryptUnprotectData(
                &input,
                null_mut(),
                null(),
                null(),
                null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        input_bytes.zeroize();
        if ok == 0 {
            return Err(VaultError::Protection(
                std::io::Error::last_os_error().to_string(),
            ));
        }
        let result =
            unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
        unsafe {
            LocalFree(output.pbData.cast());
        }
        Ok(result)
    }
}

#[cfg(not(windows))]
impl KeyProtector for DpapiProtector {
    fn protect(&self, _plaintext: &[u8]) -> Result<Vec<u8>, VaultError> {
        Err(VaultError::Protection(
            "DPAPI is available only on Windows".into(),
        ))
    }

    fn unprotect(&self, _protected: &[u8]) -> Result<Vec<u8>, VaultError> {
        Err(VaultError::Protection(
            "DPAPI is available only on Windows".into(),
        ))
    }
}

pub struct Vault {
    pub(crate) connection: Mutex<Connection>,
    protector: Arc<dyn KeyProtector>,
}

impl Vault {
    pub fn open(
        path: impl AsRef<Path>,
        protector: Arc<dyn KeyProtector>,
    ) -> Result<Self, VaultError> {
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS accounts (
                id TEXT PRIMARY KEY,
                provider TEXT NOT NULL,
                verified_identity TEXT NOT NULL,
                auth_method TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS credentials (
                account_id TEXT PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
                algorithm TEXT NOT NULL CHECK (algorithm = 'AES-256-GCM+DPAPI-v1'),
                nonce BLOB NOT NULL,
                ciphertext BLOB NOT NULL,
                protected_key BLOB NOT NULL
            );
            CREATE TABLE IF NOT EXISTS broker_requests (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                status TEXT NOT NULL,
                account_id TEXT,
                provider TEXT,
                client_id TEXT,
                purpose TEXT NOT NULL,
                operations_json TEXT NOT NULL,
                duration_minutes INTEGER NOT NULL,
                idempotency_key TEXT UNIQUE,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS grants (
                id TEXT PRIMARY KEY,
                request_id TEXT NOT NULL UNIQUE REFERENCES broker_requests(id),
                account_id TEXT NOT NULL,
                provider TEXT NOT NULL,
                client_id TEXT NOT NULL,
                purpose TEXT NOT NULL,
                operations_json TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                generation INTEGER NOT NULL DEFAULT 0,
                revoked_at TEXT
            );
            CREATE TABLE IF NOT EXISTS leases (
                token_hash TEXT PRIMARY KEY,
                grant_id TEXT NOT NULL REFERENCES grants(id),
                generation INTEGER NOT NULL,
                expires_at TEXT NOT NULL,
                remaining_uses INTEGER NOT NULL DEFAULT -1
            );
            CREATE TABLE IF NOT EXISTS audit_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                time TEXT NOT NULL,
                event TEXT NOT NULL,
                account_id TEXT,
                detail_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS authorization_sessions (
                id TEXT PRIMARY KEY,
                provider TEXT NOT NULL,
                status TEXT NOT NULL,
                user_code TEXT NOT NULL,
                verification_uri TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                interval_seconds INTEGER NOT NULL,
                next_poll_at TEXT NOT NULL,
                attempts INTEGER NOT NULL DEFAULT 0,
                last_error TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS authorization_secrets (
                session_id TEXT PRIMARY KEY REFERENCES authorization_sessions(id) ON DELETE CASCADE,
                algorithm TEXT NOT NULL CHECK (algorithm = 'AES-256-GCM+DPAPI-v1'),
                nonce BLOB NOT NULL,
                ciphertext BLOB NOT NULL,
                protected_key BLOB NOT NULL
            );",
        )?;
        let _ = connection.execute(
            "ALTER TABLE leases ADD COLUMN remaining_uses INTEGER NOT NULL DEFAULT -1",
            [],
        );
        let _ = connection.execute(
            "ALTER TABLE authorization_sessions ADD COLUMN attempts INTEGER NOT NULL DEFAULT 0",
            [],
        );
        Ok(Self {
            connection: Mutex::new(connection),
            protector,
        })
    }

    pub fn store_account(
        &self,
        provider: &str,
        verified_identity: &str,
        auth_method: &str,
        secret: &[u8],
    ) -> Result<AccountMetadata, VaultError> {
        let account = AccountMetadata {
            id: format!("acct_{}", Uuid::new_v4().simple()),
            provider: provider.into(),
            verified_identity: verified_identity.into(),
            auth_method: auth_method.into(),
            created_at: Utc::now().to_rfc3339(),
        };
        let mut key = [0_u8; 32];
        let mut nonce_bytes = [0_u8; 12];
        rand::rng().fill(&mut key);
        rand::rng().fill(&mut nonce_bytes);
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| VaultError::Authentication)?;
        let aad = associated_data(&account.id, &account.provider);
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce_bytes),
                Payload {
                    msg: secret,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| VaultError::Authentication)?;
        let protected_key = self.protector.protect(&key)?;
        key.zeroize();

        let mut connection = self.connection.lock().map_err(|_| VaultError::Busy)?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO accounts (id, provider, verified_identity, auth_method, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![account.id, account.provider, account.verified_identity, account.auth_method, account.created_at],
        )?;
        transaction.execute(
            "INSERT INTO credentials (account_id, algorithm, nonce, ciphertext, protected_key) VALUES (?1, 'AES-256-GCM+DPAPI-v1', ?2, ?3, ?4)",
            params![account.id, nonce_bytes.as_slice(), ciphertext, protected_key],
        )?;
        transaction.commit()?;
        Ok(account)
    }

    pub fn list_accounts(&self) -> Result<Vec<AccountMetadata>, VaultError> {
        let connection = self.connection.lock().map_err(|_| VaultError::Busy)?;
        let mut statement = connection.prepare(
            "SELECT id, provider, verified_identity, auth_method, created_at FROM accounts ORDER BY created_at, id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(AccountMetadata {
                id: row.get(0)?,
                provider: row.get(1)?,
                verified_identity: row.get(2)?,
                auth_method: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn load_secret(&self, account_id: &str) -> Result<Zeroizing<Vec<u8>>, VaultError> {
        let connection = self.connection.lock().map_err(|_| VaultError::Busy)?;
        let record = connection
            .query_row(
                "SELECT a.provider, c.nonce, c.ciphertext, c.protected_key
             FROM accounts a JOIN credentials c ON c.account_id = a.id WHERE a.id = ?1",
                [account_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or(VaultError::NotFound)?;
        drop(connection);

        if record.1.len() != 12 {
            return Err(VaultError::Authentication);
        }
        let mut key = self.protector.unprotect(&record.3)?;
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| VaultError::Authentication)?;
        let aad = associated_data(account_id, &record.0);
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(&record.1),
                Payload {
                    msg: &record.2,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| VaultError::Authentication);
        key.zeroize();
        Ok(Zeroizing::new(plaintext?))
    }

    pub fn export_backup(&self, destination: impl AsRef<Path>) -> Result<(), VaultError> {
        let connection = self.connection.lock().map_err(|_| VaultError::Busy)?;
        connection.query_row("PRAGMA wal_checkpoint(FULL)", [], |_| Ok(()))?;
        connection.execute(
            "VACUUM INTO ?1",
            [destination.as_ref().to_string_lossy().as_ref()],
        )?;
        Ok(())
    }

    pub fn delete_account(&self, account_id: &str) -> Result<bool, VaultError> {
        let connection = self.connection.lock().map_err(|_| VaultError::Busy)?;
        Ok(connection.execute("DELETE FROM accounts WHERE id=?1", [account_id])? > 0)
    }

    pub fn store_authorization_session(
        &self,
        session: &AuthorizationSession,
        secret: &[u8],
    ) -> Result<(), VaultError> {
        let mut key = [0_u8; 32];
        let mut nonce_bytes = [0_u8; 12];
        rand::rng().fill(&mut key);
        rand::rng().fill(&mut nonce_bytes);
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| VaultError::Authentication)?;
        let aad = authorization_associated_data(&session.id, &session.provider);
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce_bytes),
                Payload {
                    msg: secret,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| VaultError::Authentication)?;
        let protected_key = self.protector.protect(&key)?;
        key.zeroize();
        let mut connection = self.connection.lock().map_err(|_| VaultError::Busy)?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO authorization_sessions (id, provider, status, user_code, verification_uri, expires_at, interval_seconds, next_poll_at, attempts, last_error, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![session.id, session.provider, session.status, session.user_code, session.verification_uri, session.expires_at, session.interval_seconds, session.next_poll_at, session.attempts, session.last_error, session.created_at, session.updated_at],
        )?;
        transaction.execute(
            "INSERT INTO authorization_secrets (session_id, algorithm, nonce, ciphertext, protected_key) VALUES (?1, 'AES-256-GCM+DPAPI-v1', ?2, ?3, ?4)",
            params![session.id, nonce_bytes.as_slice(), ciphertext, protected_key],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn list_authorization_sessions(&self) -> Result<Vec<AuthorizationSession>, VaultError> {
        let connection = self.connection.lock().map_err(|_| VaultError::Busy)?;
        let mut statement = connection.prepare(
            "SELECT id, provider, status, user_code, verification_uri, expires_at, interval_seconds, next_poll_at, attempts, last_error, created_at, updated_at FROM authorization_sessions ORDER BY created_at, id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(AuthorizationSession {
                id: row.get(0)?,
                provider: row.get(1)?,
                status: row.get(2)?,
                user_code: row.get(3)?,
                verification_uri: row.get(4)?,
                expires_at: row.get(5)?,
                interval_seconds: row.get(6)?,
                next_poll_at: row.get(7)?,
                attempts: row.get(8)?,
                last_error: row.get(9)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn load_authorization_secret(
        &self,
        session_id: &str,
    ) -> Result<Zeroizing<Vec<u8>>, VaultError> {
        let connection = self.connection.lock().map_err(|_| VaultError::Busy)?;
        let record = connection.query_row(
            "SELECT s.provider, c.nonce, c.ciphertext, c.protected_key FROM authorization_sessions s JOIN authorization_secrets c ON c.session_id=s.id WHERE s.id=?1",
            [session_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?, row.get::<_, Vec<u8>>(2)?, row.get::<_, Vec<u8>>(3)?)),
        ).optional()?.ok_or(VaultError::NotFound)?;
        drop(connection);
        if record.1.len() != 12 {
            return Err(VaultError::Authentication);
        }
        let mut key = self.protector.unprotect(&record.3)?;
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| VaultError::Authentication)?;
        let aad = authorization_associated_data(session_id, &record.0);
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(&record.1),
                Payload {
                    msg: &record.2,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| VaultError::Authentication);
        key.zeroize();
        Ok(Zeroizing::new(plaintext?))
    }

    pub fn update_authorization_session(
        &self,
        session: &AuthorizationSession,
    ) -> Result<(), VaultError> {
        let connection = self.connection.lock().map_err(|_| VaultError::Busy)?;
        connection.execute(
            "UPDATE authorization_sessions SET status=?2, user_code=?3, expires_at=?4, interval_seconds=?5, next_poll_at=?6, attempts=?7, last_error=?8, updated_at=?9 WHERE id=?1",
            params![session.id, session.status, session.user_code, session.expires_at, session.interval_seconds, session.next_poll_at, session.attempts, session.last_error, session.updated_at],
        )?;
        Ok(())
    }

    pub fn delete_authorization_secret(&self, session_id: &str) -> Result<(), VaultError> {
        let connection = self.connection.lock().map_err(|_| VaultError::Busy)?;
        connection.execute(
            "DELETE FROM authorization_secrets WHERE session_id=?1",
            [session_id],
        )?;
        Ok(())
    }
}

fn associated_data(account_id: &str, provider: &str) -> String {
    format!("elegy-accounts:v1:{provider}:{account_id}")
}

fn authorization_associated_data(session_id: &str, provider: &str) -> String {
    format!("elegy-accounts:authorization:v1:{provider}:{session_id}")
}
