use elegy_accountd::{AuthMethod, OAuthCallback, OAuthError, OAuthTransaction, ProviderCatalog};
use std::path::Path;

#[test]
fn bundled_provider_packs_are_data_driven_conformance_examples() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../providers");
    let catalog = ProviderCatalog::load_directory(directory).expect("bundled provider packs");
    assert_eq!(catalog.list().len(), 3);
    assert_eq!(
        catalog.get("github").expect("github pack").auth_profiles[0].method,
        AuthMethod::DeviceAuthorization
    );
    assert_eq!(
        catalog
            .get("cloudflare")
            .expect("cloudflare pack")
            .auth_profiles[0]
            .method,
        AuthMethod::ApiToken
    );
    assert_eq!(
        catalog.get("google").expect("google pack").auth_profiles[0].method,
        AuthMethod::OAuthPkce
    );
}

#[test]
fn provider_catalog_loads_generic_manifest_without_compiled_provider_knowledge() {
    let manifest = r#"{
      "schema_version":"elegy-account-provider/v1",
      "id":"synthetic-mail",
      "display_name":"Synthetic Mail",
      "version":"1.0.0",
      "publisher":"test-suite",
      "browser_origins":["https://accounts.example.test"],
      "auth_profiles":[{
        "id":"desktop-oauth",
        "method":"oauth_pkce",
        "issuer":"https://accounts.example.test",
        "audience":"https://api.example.test",
        "authorization_url":"https://accounts.example.test/authorize",
        "token_url":"https://accounts.example.test/token",
        "identity":{"url":"https://api.example.test/me","selectors":["/email","/id"]},
        "client":{"mode":"environment","client_id_env":"SYNTHETIC_CLIENT_ID"},
        "scopes":["profile.read"]
      }],
      "operations":{"mail.read":["profile.read"]}
    }"#;

    let catalog = ProviderCatalog::from_json_documents([manifest]).expect("valid provider pack");
    let provider = catalog.get("synthetic-mail").expect("provider loaded");
    assert_eq!(provider.auth_profiles[0].method, AuthMethod::OAuthPkce);
    assert_eq!(
        provider.auth_profiles[0].identity.selectors,
        ["/email", "/id"]
    );
    assert_eq!(catalog.list().len(), 1);
}

#[test]
fn provider_catalog_rejects_non_loopback_plain_http_endpoints() {
    let manifest = r#"{
      "schema_version":"elegy-account-provider/v1",
      "id":"unsafe",
      "display_name":"Unsafe",
      "version":"1.0.0",
      "publisher":"test-suite",
      "browser_origins":["http://accounts.example.test"],
      "auth_profiles":[],
      "operations":{}
    }"#;

    let error = ProviderCatalog::from_json_documents([manifest]).expect_err("unsafe origin");
    assert!(error.to_string().contains("HTTPS"));
}

#[test]
fn oauth_callback_requires_exact_transaction_binding() {
    let transaction = OAuthTransaction::new(
        "cloudflare",
        "https://dash.cloudflare.com",
        "https://api.cloudflare.com",
        "http://127.0.0.1:43119/oauth/callback",
    );
    let valid = OAuthCallback {
        state: transaction.state.clone(),
        nonce: transaction.nonce.clone(),
        issuer: "https://dash.cloudflare.com".into(),
        audience: "https://api.cloudflare.com".into(),
        redirect_uri: "http://127.0.0.1:43119/oauth/callback".into(),
        code: "synthetic-code".into(),
    };
    assert!(transaction.validate(&valid).is_ok());

    let cases = [
        (
            "state",
            OAuthCallback {
                state: "wrong".into(),
                ..valid.clone()
            },
            OAuthError::StateMismatch,
        ),
        (
            "nonce",
            OAuthCallback {
                nonce: "wrong".into(),
                ..valid.clone()
            },
            OAuthError::NonceMismatch,
        ),
        (
            "issuer",
            OAuthCallback {
                issuer: "https://evil.test".into(),
                ..valid.clone()
            },
            OAuthError::IssuerMismatch,
        ),
        (
            "audience",
            OAuthCallback {
                audience: "https://evil.test".into(),
                ..valid.clone()
            },
            OAuthError::AudienceMismatch,
        ),
        (
            "redirect",
            OAuthCallback {
                redirect_uri: "http://127.0.0.1:9999/callback".into(),
                ..valid.clone()
            },
            OAuthError::RedirectMismatch,
        ),
    ];
    for (label, callback, error) in cases {
        assert_eq!(transaction.validate(&callback), Err(error), "{label}");
    }
}

#[test]
fn pkce_verifier_is_secret_and_s256_challenge_is_stable() {
    let transaction = OAuthTransaction::new(
        "test-oauth",
        "https://issuer.example",
        "https://api.example",
        "http://127.0.0.1:43119/oauth/callback",
    );
    assert!(transaction.pkce_verifier.expose_for_token_exchange().len() >= 43);
    assert!(!transaction.pkce_challenge.contains('='));
    assert_ne!(
        transaction.pkce_challenge,
        transaction.pkce_verifier.expose_for_token_exchange()
    );
}
