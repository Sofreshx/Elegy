use chrono::{Duration, Utc};
use elegy_accountd::{
    ExecutionEnvelope, ExecutionProtocolError, ReplayGuard, TypedExecutionRequest,
};
use serde_json::json;

fn request() -> TypedExecutionRequest {
    TypedExecutionRequest {
        client_id: "codex-actions".into(),
        purpose_class: "github.repositories.read".into(),
        provider: "github".into(),
        operation: "repositories.read".into(),
        account_id: None,
        arguments: json!({}),
    }
}

#[test]
fn signed_execution_envelope_binds_client_request_and_timestamp() {
    let now = Utc::now();
    let key = b"fixture-client-authentication-key";
    let envelope =
        ExecutionEnvelope::sign(request(), key, now, "nonce-1").expect("signed execution request");
    let mut replay = ReplayGuard::default();

    let verified = envelope
        .verify(key, "codex-actions", now, &mut replay)
        .expect("verified execution request");
    assert_eq!(verified.provider, "github");
    assert_eq!(verified.operation, "repositories.read");
    assert!(
        !serde_json::to_string(&envelope)
            .expect("serialized envelope")
            .contains("fixture-client-authentication-key")
    );
}

#[test]
fn execution_envelope_rejects_tampering_wrong_client_staleness_and_replay() {
    let now = Utc::now();
    let key = b"fixture-client-authentication-key";

    let mut tampered =
        ExecutionEnvelope::sign(request(), key, now, "nonce-tampered").expect("envelope");
    tampered.request.operation = "profile.read".into();
    assert_eq!(
        tampered
            .verify(key, "codex-actions", now, &mut ReplayGuard::default())
            .expect_err("tampered request"),
        ExecutionProtocolError::InvalidSignature
    );

    let wrong_client =
        ExecutionEnvelope::sign(request(), key, now, "nonce-client").expect("envelope");
    assert_eq!(
        wrong_client
            .verify(key, "holon", now, &mut ReplayGuard::default())
            .expect_err("wrong client"),
        ExecutionProtocolError::WrongClient
    );

    let stale = ExecutionEnvelope::sign(request(), key, now - Duration::minutes(3), "nonce-stale")
        .expect("envelope");
    assert_eq!(
        stale
            .verify(key, "codex-actions", now, &mut ReplayGuard::default())
            .expect_err("stale request"),
        ExecutionProtocolError::Stale
    );

    let replayed = ExecutionEnvelope::sign(request(), key, now, "nonce-replay").expect("envelope");
    let mut replay = ReplayGuard::default();
    replayed
        .verify(key, "codex-actions", now, &mut replay)
        .expect("first use");
    assert_eq!(
        replayed
            .verify(key, "codex-actions", now, &mut replay)
            .expect_err("replay"),
        ExecutionProtocolError::Replay
    );
}
