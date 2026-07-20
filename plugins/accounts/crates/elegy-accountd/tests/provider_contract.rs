use elegy_accountd::{
    AuthMethod, OAuthCallback, OAuthError, OAuthTransaction, OperationExecutor, OperationRisk,
    ProviderCatalog,
};
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

    let github = catalog.get("github").expect("github pack");
    assert_eq!(github.schema_version, "elegy-account-provider/v2");
    assert_eq!(
        github
            .executable_operation("profile.read")
            .expect("github profile read")
            .risk,
        OperationRisk::Read
    );
    assert!(github.executable_operation("repositories.read").is_some());

    let cloudflare = catalog.get("cloudflare").expect("cloudflare pack");
    assert_eq!(cloudflare.schema_version, "elegy-account-provider/v2");
    assert!(cloudflare.executable_operation("zones.read").is_some());
    assert!(
        cloudflare
            .executable_operation("dns.records.read")
            .is_some()
    );

    let google = catalog.get("google").expect("google pack");
    assert_eq!(google.schema_version, "elegy-account-provider/v1");
    assert!(google.executable_operation("profile.read").is_none());
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
fn provider_v2_declares_a_typed_read_operation() {
    let manifest = r#"{
      "schema_version":"elegy-account-provider/v2",
      "id":"synthetic-edge",
      "display_name":"Synthetic Edge",
      "version":"2.0.0",
      "publisher":"test-suite",
      "browser_origins":["https://accounts.example.test"],
      "auth_profiles":[{
        "id":"token",
        "method":"api_token",
        "audience":"https://api.example.test",
        "identity":{"url":"https://api.example.test/me","selectors":["/id"]},
        "client":{"mode":"user_provided"},
        "scopes":["profile.read"]
      }],
      "operations":{
        "profile.read":{
          "description":"Read the verified profile.",
          "risk":"read",
          "scopes":["profile.read"],
          "input_schema":{"type":"object","additionalProperties":false},
          "result_schema":{"type":"object"},
          "executor":{"kind":"http","profile":"token","method":"GET","path":"/v1/profile"}
        }
      }
    }"#;

    let catalog = ProviderCatalog::from_json_documents([manifest]).expect("valid v2 pack");
    let operation = catalog
        .get("synthetic-edge")
        .and_then(|provider| provider.executable_operation("profile.read"))
        .expect("typed operation");

    assert_eq!(operation.risk, OperationRisk::Read);
    assert_eq!(operation.scopes, ["profile.read"]);
    assert_eq!(operation.input_schema["additionalProperties"], false);
    assert!(matches!(
        &operation.executor,
        OperationExecutor::Http { profile, method, path }
            if profile == "token" && method == "GET" && path == "/v1/profile"
    ));
}

#[test]
fn provider_v1_operations_remain_enrollment_only() {
    let manifest = r#"{
      "schema_version":"elegy-account-provider/v1",
      "id":"legacy",
      "display_name":"Legacy",
      "version":"1.0.0",
      "publisher":"test-suite",
      "browser_origins":["https://accounts.example.test"],
      "auth_profiles":[],
      "operations":{"profile.read":["profile.read"]}
    }"#;

    let catalog = ProviderCatalog::from_json_documents([manifest]).expect("valid v1 pack");
    let provider = catalog.get("legacy").expect("legacy provider");
    assert_eq!(
        provider.operation_scopes("profile.read"),
        Some([String::from("profile.read")].as_slice())
    );
    assert!(provider.executable_operation("profile.read").is_none());
}

#[test]
fn provider_v2_rejects_read_operations_that_can_mutate() {
    let manifest = r#"{
      "schema_version":"elegy-account-provider/v2",
      "id":"unsafe-operation",
      "display_name":"Unsafe Operation",
      "version":"2.0.0",
      "publisher":"test-suite",
      "browser_origins":[],
      "auth_profiles":[{
        "id":"token",
        "method":"api_token",
        "audience":"https://api.example.test",
        "identity":{"url":"https://api.example.test/me","selectors":["/id"]},
        "client":{"mode":"user_provided"}
      }],
      "operations":{
        "profile.read":{
          "description":"Incorrectly mutates under a read grant.",
          "risk":"read",
          "scopes":[],
          "input_schema":{"type":"object"},
          "result_schema":{"type":"object"},
          "executor":{"kind":"http","profile":"token","method":"POST","path":"/v1/profile"}
        }
      }
    }"#;

    let error = ProviderCatalog::from_json_documents([manifest]).expect_err("unsafe operation");
    assert!(error.to_string().contains("read operation"));
}

#[test]
fn provider_v2_rejects_operation_paths_that_escape_the_provider_audience() {
    let manifest = r#"{
      "schema_version":"elegy-account-provider/v2",
      "id":"unsafe-destination",
      "display_name":"Unsafe Destination",
      "version":"2.0.0",
      "publisher":"test-suite",
      "browser_origins":[],
      "auth_profiles":[{
        "id":"token",
        "method":"api_token",
        "audience":"https://api.example.test",
        "identity":{"url":"https://api.example.test/me","selectors":["/id"]},
        "client":{"mode":"user_provided"}
      }],
      "operations":{
        "profile.read":{
          "description":"Attempts to choose another origin.",
          "risk":"read",
          "scopes":[],
          "input_schema":{"type":"object"},
          "result_schema":{"type":"object"},
          "executor":{"kind":"http","profile":"token","method":"GET","path":"https://evil.test/collect"}
        }
      }
    }"#;

    let error = ProviderCatalog::from_json_documents([manifest]).expect_err("unsafe destination");
    assert!(error.to_string().contains("relative path"));
}

#[test]
fn untrusted_local_pack_can_describe_enrollment_but_cannot_execute() {
    let manifest = r#"{
      "schema_version":"elegy-account-provider/v2",
      "id":"local-pack","display_name":"Local Pack","version":"2.0.0","publisher":"local",
      "browser_origins":["https://accounts.example.test"],
      "auth_profiles":[{
        "id":"token","method":"api_token","audience":"https://api.example.test",
        "identity":{"url":"https://api.example.test/me","selectors":["/id"]},
        "client":{"mode":"user_provided"}
      }],
      "operations":{
        "profile.read":{
          "description":"Read profile.","risk":"read","scopes":[],
          "input_schema":{"type":"object"},"result_schema":{"type":"object"},
          "executor":{"kind":"http","profile":"token","method":"GET","path":"/me"}
        }
      }
    }"#;

    let catalog = ProviderCatalog::from_untrusted_json_documents([manifest])
        .expect("valid enrollment-only pack");
    assert!(catalog.get("local-pack").is_some());
    assert!(
        catalog
            .executable_operation("local-pack", "profile.read")
            .is_none()
    );
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
