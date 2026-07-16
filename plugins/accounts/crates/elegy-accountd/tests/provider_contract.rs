use elegy_accountd::{AuthMethod, OAuthCallback, OAuthError, OAuthTransaction, ProviderCatalog};

#[test]
fn mvp_provider_catalog_declares_safe_connection_and_validation_modes() {
    let catalog = ProviderCatalog::mvp();
    let cloudflare = catalog.get("cloudflare").unwrap();
    assert!(
        cloudflare
            .auth_methods
            .contains(&AuthMethod::GuidedApiToken)
    );
    assert_eq!(
        cloudflare.identity_endpoint,
        "https://api.cloudflare.com/client/v4/user/tokens/verify"
    );
    assert!(cloudflare.operations.contains_key("dns.records.read"));
    assert!(
        cloudflare
            .browser_origins
            .contains(&"https://dash.cloudflare.com".into())
    );

    let github = catalog.get("github").unwrap();
    assert_eq!(github.auth_methods, vec![AuthMethod::DeviceCode]);
    assert_eq!(catalog.list().len(), 2);
    for id in ["google", "vercel", "generic"] {
        assert!(
            catalog.get(id).is_none(),
            "{id} must not be advertised as MVP-ready"
        );
    }
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
