use elegy_accountd::{AuthorizationSession, DpapiProtector, KeyProtector, Vault, VaultError};
use rusqlite::Connection;
use std::{fs, sync::Arc};
use tempfile::tempdir;

const CANARY: &str = "ELEGY_CANARY_never-plaintext-in-vault";

#[test]
#[cfg(windows)]
fn vault_persists_identity_but_never_plaintext_secret() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("accounts.sqlite");
    let protector: Arc<dyn KeyProtector> = Arc::new(DpapiProtector);
    let vault = Vault::open(&path, protector.clone()).unwrap();
    let account = vault
        .store_account(
            "cloudflare",
            "alex@example.test",
            "browser_oauth",
            CANARY.as_bytes(),
        )
        .unwrap();
    drop(vault);

    let bytes = fs::read(&path).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains(CANARY));

    let reopened = Vault::open(&path, protector).unwrap();
    assert_eq!(reopened.list_accounts().unwrap(), vec![account.clone()]);
    assert_eq!(
        reopened.load_secret(&account.id).unwrap().as_slice(),
        CANARY.as_bytes()
    );
}

#[test]
#[cfg(windows)]
fn authenticated_encryption_fails_closed_after_ciphertext_tampering() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("accounts.sqlite");
    let vault = Vault::open(&path, Arc::new(DpapiProtector)).unwrap();
    let account = vault
        .store_account(
            "github",
            "alex@example.test",
            "oauth_pkce",
            CANARY.as_bytes(),
        )
        .unwrap();
    drop(vault);

    let connection = Connection::open(&path).unwrap();
    connection.execute(
        "UPDATE credentials SET ciphertext = zeroblob(length(ciphertext)) WHERE account_id = ?1",
        [&account.id],
    ).unwrap();
    drop(connection);

    let reopened = Vault::open(&path, Arc::new(DpapiProtector)).unwrap();
    assert!(matches!(
        reopened.load_secret(&account.id),
        Err(VaultError::Authentication)
    ));
}

#[test]
#[cfg(windows)]
fn encrypted_backup_contains_no_plaintext_and_restores_for_same_user() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("accounts.sqlite");
    let backup = dir.path().join("accounts-backup.sqlite");
    let vault = Vault::open(&source, Arc::new(DpapiProtector)).unwrap();
    let account = vault
        .store_account(
            "vercel",
            "alex@example.test",
            "api_token",
            CANARY.as_bytes(),
        )
        .unwrap();
    vault.export_backup(&backup).unwrap();

    assert!(!String::from_utf8_lossy(&fs::read(&backup).unwrap()).contains(CANARY));
    let restored = Vault::open(&backup, Arc::new(DpapiProtector)).unwrap();
    assert_eq!(
        restored.load_secret(&account.id).unwrap().as_slice(),
        CANARY.as_bytes()
    );
}

#[test]
#[cfg(windows)]
fn dpapi_protection_is_nondeterministic_and_round_trips_for_current_user() {
    let protector = DpapiProtector;
    let first = protector
        .protect(b"thirty-two-byte-vault-data-key!!!")
        .unwrap();
    let second = protector
        .protect(b"thirty-two-byte-vault-data-key!!!")
        .unwrap();
    assert_ne!(first, second);
    assert_eq!(
        protector.unprotect(&first).unwrap(),
        b"thirty-two-byte-vault-data-key!!!"
    );
}

#[test]
#[cfg(windows)]
fn pending_authorization_survives_restart_without_plaintext_device_secret() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("accounts.sqlite");
    let protector: Arc<dyn KeyProtector> = Arc::new(DpapiProtector);
    let vault = Vault::open(&path, protector.clone()).unwrap();
    let session = AuthorizationSession {
        id: "auth_github_1".into(),
        provider: "github".into(),
        status: "waiting_for_user".into(),
        user_code: "ABCD-EFGH".into(),
        verification_uri: "https://github.com/login/device".into(),
        expires_at: "2099-01-01T00:00:00Z".into(),
        interval_seconds: 5,
        next_poll_at: "2098-12-31T23:59:00Z".into(),
        last_error: None,
        created_at: "2098-12-31T23:58:00Z".into(),
        updated_at: "2098-12-31T23:58:00Z".into(),
    };
    vault
        .store_authorization_session(&session, CANARY.as_bytes())
        .unwrap();
    drop(vault);

    assert!(!String::from_utf8_lossy(&fs::read(&path).unwrap()).contains(CANARY));
    let reopened = Vault::open(&path, protector).unwrap();
    assert_eq!(
        reopened.list_authorization_sessions().unwrap(),
        vec![session.clone()]
    );
    assert_eq!(
        reopened
            .load_authorization_secret(&session.id)
            .unwrap()
            .as_slice(),
        CANARY.as_bytes()
    );
}

#[test]
#[cfg(not(windows))]
fn dpapi_fails_closed_on_unsupported_platforms() {
    let error = DpapiProtector.protect(b"secret").unwrap_err();
    assert!(matches!(error, VaultError::Protection(_)));
}
