use elegy_accountd::{BrokerStore, DpapiProtector, NewAccessRequest, Vault};
use std::sync::Arc;

#[test]
#[cfg(windows)]
fn requests_grants_audit_and_revocation_survive_restart() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("accounts.sqlite");
    let request_id;
    let grant_id;

    {
        let vault = Vault::open(&database, Arc::new(DpapiProtector)).unwrap();
        let store = BrokerStore::new(vault);
        let account = store
            .vault()
            .store_account(
                "cloudflare",
                "owner@example.test",
                "oauth_pkce",
                b"SECRET_CANARY",
            )
            .unwrap();
        let request = store
            .request_access(NewAccessRequest {
                account_id: account.id,
                client_id: "registered-test-client".into(),
                purpose: "research client DNS posture".into(),
                operations: vec!["dns.list".into()],
                duration_minutes: 60,
            })
            .unwrap();
        request_id = request.id.clone();
        let grant = store.approve_access(&request.id).unwrap();
        grant_id = grant.id.clone();
        let lease = store.issue_lease(&grant.id, 10).unwrap();
        assert!(
            store
                .authorize(
                    &lease.token,
                    "registered-test-client",
                    "research client DNS posture",
                    "cloudflare",
                    "dns.list"
                )
                .is_ok()
        );
        assert!(
            store
                .authorize(
                    &lease.token,
                    "registered-test-client",
                    "research client DNS posture",
                    "cloudflare",
                    "dns.write"
                )
                .is_err()
        );
        let once = store.issue_single_use_lease(&grant.id, 10).unwrap();
        assert!(
            store
                .authorize(
                    &once.token,
                    "registered-test-client",
                    "research client DNS posture",
                    "cloudflare",
                    "dns.list"
                )
                .is_ok()
        );
        assert!(
            store
                .authorize(
                    &once.token,
                    "registered-test-client",
                    "research client DNS posture",
                    "cloudflare",
                    "dns.list"
                )
                .is_err()
        );
    }

    {
        let vault = Vault::open(&database, Arc::new(DpapiProtector)).unwrap();
        let store = BrokerStore::new(vault);
        assert_eq!(store.get_request(&request_id).unwrap().status, "approved");
        let lease = store.issue_lease(&grant_id, 10).unwrap();
        store.revoke_grant(&grant_id, "user revoked").unwrap();
        assert!(
            store
                .authorize(
                    &lease.token,
                    "registered-test-client",
                    "research client DNS posture",
                    "cloudflare",
                    "dns.list"
                )
                .is_err()
        );
        assert!(
            store
                .list_audit(100)
                .unwrap()
                .iter()
                .any(|event| event.event == "grant.revoked")
        );
    }
}

#[test]
fn creation_request_is_idempotent_and_resumable() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("accounts.sqlite");
    let vault = Vault::open(&database, Arc::new(DpapiProtector)).unwrap();
    let store = BrokerStore::new(vault);
    let first = store
        .request_creation(
            "stable-key",
            "github",
            "publish a project",
            vec!["free plan".into()],
        )
        .unwrap();
    let second = store
        .request_creation(
            "stable-key",
            "github",
            "publish a project",
            vec!["free plan".into()],
        )
        .unwrap();
    assert_eq!(first.id, second.id);
    assert_eq!(first.status, "waiting_human");
    drop(store);

    let store = BrokerStore::new(Vault::open(&database, Arc::new(DpapiProtector)).unwrap());
    assert_eq!(
        store.get_request(&first.id).unwrap().status,
        "waiting_human"
    );
    store.cancel_request(&first.id).unwrap();
    assert_eq!(store.get_request(&first.id).unwrap().status, "cancelled");
    assert!(
        store
            .list_audit(20)
            .unwrap()
            .iter()
            .any(|event| event.event == "request.cancelled")
    );
}

#[test]
#[cfg(windows)]
fn reopening_the_broker_preserves_accounts_but_revokes_legacy_codex_grants() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("accounts.sqlite");
    let account_id;
    let grant_id;
    {
        let broker = BrokerStore::new(Vault::open(&database, Arc::new(DpapiProtector)).unwrap());
        let account = broker
            .vault()
            .store_account("github", "owner", "device_authorization", b"secret")
            .unwrap();
        account_id = account.id.clone();
        let request = broker
            .request_access(NewAccessRequest {
                account_id: account.id,
                client_id: "codex-local".into(),
                purpose: "legacy request".into(),
                operations: vec!["profile.read".into()],
                duration_minutes: 60,
            })
            .unwrap();
        grant_id = broker.approve_access(&request.id).unwrap().id;
    }

    let reopened = BrokerStore::new(Vault::open(&database, Arc::new(DpapiProtector)).unwrap());
    assert!(
        reopened
            .vault()
            .list_accounts()
            .unwrap()
            .iter()
            .any(|account| account.id == account_id)
    );
    assert!(reopened.get_grant(&grant_id).unwrap().revoked);
    assert!(
        reopened
            .list_audit(20)
            .unwrap()
            .iter()
            .any(|event| event.event == "grant.legacy_revoked")
    );
}
