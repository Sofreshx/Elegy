use chrono::{Duration, Utc};
use elegy_accountd::{
    CheckpointKind, GrantRequest, LeaseError, PolicyEngine, ProvisioningSaga, Redactor,
};
use serde_json::json;

#[test]
fn leases_are_scoped_to_client_purpose_audience_operations_and_expiry() {
    let mut policy = PolicyEngine::default();
    let grant = policy.approve(GrantRequest {
        client_id: "codex-local".into(),
        account_id: "cf-alex".into(),
        purpose: "research-client-infrastructure".into(),
        audience: "cloudflare-api".into(),
        operations: ["dns.records.read".into()].into(),
        expires_at: Utc::now() + Duration::hours(1),
    });
    let lease = policy.issue_lease(&grant.id, Duration::minutes(5)).unwrap();

    assert!(
        policy
            .authorize(
                &lease.token,
                "codex-local",
                "research-client-infrastructure",
                "cloudflare-api",
                "dns.records.read"
            )
            .is_ok()
    );
    assert_eq!(
        policy.authorize(
            &lease.token,
            "holon",
            "research-client-infrastructure",
            "cloudflare-api",
            "dns.records.read"
        ),
        Err(LeaseError::WrongClient)
    );
    assert_eq!(
        policy.authorize(
            &lease.token,
            "codex-local",
            "deploy-quizu",
            "cloudflare-api",
            "dns.records.read"
        ),
        Err(LeaseError::WrongPurpose)
    );
    assert_eq!(
        policy.authorize(
            &lease.token,
            "codex-local",
            "research-client-infrastructure",
            "github-api",
            "dns.records.read"
        ),
        Err(LeaseError::WrongAudience)
    );
    assert_eq!(
        policy.authorize(
            &lease.token,
            "codex-local",
            "research-client-infrastructure",
            "cloudflare-api",
            "dns.records.write"
        ),
        Err(LeaseError::OperationDenied)
    );
}

#[test]
fn revocation_invalidates_every_derived_lease_immediately() {
    let mut policy = PolicyEngine::default();
    let grant = policy.approve(GrantRequest {
        client_id: "codex-local".into(),
        account_id: "cf-alex".into(),
        purpose: "dns-audit".into(),
        audience: "cloudflare-api".into(),
        operations: ["dns.records.read".into()].into(),
        expires_at: Utc::now() + Duration::hours(1),
    });
    let lease = policy.issue_lease(&grant.id, Duration::minutes(5)).unwrap();
    policy.revoke(&grant.id).unwrap();

    assert_eq!(
        policy.authorize(
            &lease.token,
            "codex-local",
            "dns-audit",
            "cloudflare-api",
            "dns.records.read"
        ),
        Err(LeaseError::Revoked)
    );
}

#[test]
fn redactor_removes_secrets_from_nested_structured_output() {
    let secret = "ELEGY_CANARY_super-secret-refresh-token";
    let redacted = Redactor::new([secret]).sanitize(json!({
        "authorization": format!("Bearer {secret}"),
        "nested": { "refresh_token": secret, "safe": "account-123" },
        "message": format!("provider rejected {secret}")
    }));
    let serialized = serde_json::to_string(&redacted).unwrap();

    assert!(!serialized.contains(secret));
    assert!(serialized.contains("account-123"));
    assert!(serialized.contains("[REDACTED]"));
}

#[test]
fn every_sensitive_signup_boundary_requires_a_human() {
    for checkpoint in [
        CheckpointKind::Captcha,
        CheckpointKind::Mfa,
        CheckpointKind::Terms,
        CheckpointKind::Payment,
        CheckpointKind::IdentityVerification,
        CheckpointKind::AmbiguousPlan,
        CheckpointKind::UnexpectedPage,
    ] {
        let mut saga =
            ProvisioningSaga::requested("signup-1", "cloudflare", "create deployment account");
        saga.start().unwrap();
        saga.require_human(checkpoint.clone(), "Complete this step in Brave")
            .unwrap();
        assert!(saga.is_waiting_for_human());
        assert_eq!(saga.checkpoint().unwrap().kind, checkpoint);
        assert!(saga.automated_resume().is_err());
    }
}

#[test]
fn creation_idempotency_key_is_stable_for_same_request() {
    let a = ProvisioningSaga::requested("request-42", "github", "client research");
    let b = ProvisioningSaga::requested("request-42", "github", "client research");
    let other = ProvisioningSaga::requested("request-43", "github", "client research");
    assert_eq!(a.id(), b.id());
    assert_ne!(a.id(), other.id());
}
