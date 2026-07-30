use chrono::Utc;
use elegy_accountd::{
    ConnectionControlEnvelope, ConnectionControlOperation, ConnectionControlResult,
    ConnectionSession, ConnectionSnapshot, ConnectionState, DisconnectPreview,
    ExecutionProtocolError, ReplayGuard,
};

#[test]
fn holon_connection_control_is_client_bound_and_credential_free() {
    let now = Utc::now();
    let key = b"holon-host-control-key";
    let envelope = ConnectionControlEnvelope::sign(
        "holon",
        ConnectionControlOperation::Verify {
            connection_id: "github-account-1".into(),
        },
        key,
        now,
        "connection-control-1",
    )
    .expect("sign host control request");
    let mut replay = ReplayGuard::default();

    let operation = envelope
        .verify(key, "holon", now, &mut replay)
        .expect("verify host control request");

    assert_eq!(
        operation,
        ConnectionControlOperation::Verify {
            connection_id: "github-account-1".into()
        }
    );
    assert_eq!(
        envelope
            .verify(key, "codex", now, &mut ReplayGuard::default())
            .expect_err("another host cannot reuse the request"),
        ExecutionProtocolError::WrongClient
    );
    let serialized = serde_json::to_string(&envelope).expect("serialize envelope");
    assert!(!serialized.contains("holon-host-control-key"));
    assert!(!serialized.contains("access_token"));
    assert!(!serialized.contains("refresh_token"));
}

#[test]
fn connection_snapshot_uses_explicit_verified_lifecycle_states() {
    let snapshot = ConnectionSnapshot {
        id: "github-account-1".into(),
        service: "github".into(),
        account_summary: "octocat".into(),
        state: ConnectionState::Connected,
        verified_at: Some("2026-07-23T12:00:00Z".into()),
        valid_until: Some("2026-07-23T12:15:00Z".into()),
        adapter: "elegy-accounts".into(),
        last_error_code: None,
    };

    let serialized = serde_json::to_value(snapshot).expect("serialize connection snapshot");

    assert_eq!(serialized["state"], "connected");
    assert_eq!(serialized["service"], "github");
    assert!(serialized.get("credential").is_none());
    assert!(serialized.get("token").is_none());
}

#[test]
fn connection_control_results_cover_connect_status_and_safe_disconnect() {
    let session = ConnectionControlResult::Session {
        session: ConnectionSession {
            id: "authorization-1".into(),
            service: "github".into(),
            state: ConnectionState::AttentionRequired,
            user_action_url: Some("http://127.0.0.1:43119/?connection=authorization-1".into()),
            expires_at: Some("2026-07-23T12:15:00Z".into()),
            last_error_code: None,
        },
    };
    let preview = ConnectionControlResult::DisconnectPreview {
        preview: DisconnectPreview {
            connection_id: "github-account-1".into(),
            account_summary: "octocat".into(),
            revoked_grant_count: 2,
            confirmation_digest: "sha256:disconnect-preview".into(),
        },
    };

    let session_json = serde_json::to_value(session).expect("serialize connection session");
    let preview_json = serde_json::to_value(preview).expect("serialize disconnect preview");

    assert_eq!(session_json["result"], "session");
    assert_eq!(session_json["session"]["state"], "attention-required");
    assert_eq!(preview_json["result"], "disconnect-preview");
    assert_eq!(preview_json["preview"]["revokedGrantCount"], 2);
    for value in [session_json, preview_json] {
        let serialized = serde_json::to_string(&value).expect("serialize result");
        assert!(!serialized.contains("access_token"));
        assert!(!serialized.contains("refresh_token"));
        assert!(!serialized.contains("credential"));
    }
}
