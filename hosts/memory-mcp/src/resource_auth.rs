use jsonwebtoken::{
    decode, decode_header,
    jwk::{AlgorithmParameters, Jwk, JwkSet, KeyAlgorithm, KeyOperations, PublicKeyUse},
    Algorithm, DecodingKey, Validation,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};
use thiserror::Error;
use url::Url;

use crate::config::ExternalOAuthConfig;

#[derive(Clone, Debug)]
pub struct ExternalTokenValidator {
    config: ExternalOAuthConfig,
    jwks: Arc<RwLock<JwkSet>>,
    client: reqwest::Client,
    refresh_state: Arc<tokio::sync::Mutex<Option<Instant>>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ValidatedTokenClaims {
    pub iss: String,
    pub aud: Value,
    pub exp: usize,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub jti: Option<String>,
}

#[derive(Debug, Error)]
pub enum TokenValidationError {
    #[error("access token is invalid")]
    Invalid,
    #[error("access token lacks required scopes")]
    MissingScope,
    #[error("identity provider JWKS contains no usable signing keys")]
    EmptyJwks,
    #[error("access token references an unknown signing key")]
    UnknownKey,
    #[error("identity provider JWKS refresh failed")]
    JwksUnavailable,
}

impl ExternalTokenValidator {
    pub async fn load(config: ExternalOAuthConfig) -> anyhow::Result<Self> {
        let client = jwks_client().map_err(anyhow::Error::from)?;
        let jwks = client
            .get(config.jwks_url.clone())
            .send()
            .await?
            .error_for_status()?
            .json::<JwkSet>()
            .await?;
        Self::from_parts(config, jwks, client).map_err(Into::into)
    }

    pub fn from_jwks(
        config: ExternalOAuthConfig,
        jwks: JwkSet,
    ) -> Result<Self, TokenValidationError> {
        let client = jwks_client()?;
        Self::from_parts(config, jwks, client)
    }

    fn from_parts(
        config: ExternalOAuthConfig,
        jwks: JwkSet,
        client: reqwest::Client,
    ) -> Result<Self, TokenValidationError> {
        validate_jwks(&jwks)?;
        Ok(Self {
            config,
            jwks: Arc::new(RwLock::new(jwks)),
            client,
            refresh_state: Arc::new(tokio::sync::Mutex::new(None)),
        })
    }

    pub fn validate(&self, token: &str) -> Result<ValidatedTokenClaims, TokenValidationError> {
        let jwks = self
            .jwks
            .read()
            .map_err(|_| TokenValidationError::Invalid)?;
        self.validate_against(token, &jwks)
    }

    fn validate_against(
        &self,
        token: &str,
        jwks: &JwkSet,
    ) -> Result<ValidatedTokenClaims, TokenValidationError> {
        let header = decode_header(token).map_err(|_| TokenValidationError::Invalid)?;
        let kid = header.kid.ok_or(TokenValidationError::Invalid)?;
        let jwk = jwks.find(&kid).ok_or(TokenValidationError::UnknownKey)?;
        let algorithm = signing_algorithm(jwk).ok_or(TokenValidationError::Invalid)?;
        if header.alg != algorithm {
            return Err(TokenValidationError::Invalid);
        }
        let key = DecodingKey::from_jwk(jwk).map_err(|_| TokenValidationError::Invalid)?;
        let mut validation = Validation::new(algorithm);
        validation.set_required_spec_claims(&["exp", "iss", "aud"]);
        validation.set_issuer(&[self.config.issuer.as_str()]);
        validation.set_audience(&[self.config.audience.as_str()]);
        let decoded = decode::<ValidatedTokenClaims>(token, &key, &validation)
            .map_err(|_| TokenValidationError::Invalid)?;
        let token_scopes = decoded
            .claims
            .scope
            .split_whitespace()
            .collect::<std::collections::BTreeSet<_>>();
        if !self
            .config
            .scopes
            .iter()
            .all(|scope| token_scopes.contains(scope.as_str()))
        {
            return Err(TokenValidationError::MissingScope);
        }
        Ok(decoded.claims)
    }

    pub async fn validate_with_refresh(
        &self,
        token: &str,
    ) -> Result<ValidatedTokenClaims, TokenValidationError> {
        match self.validate(token) {
            Ok(claims) => return Ok(claims),
            Err(TokenValidationError::UnknownKey) => {}
            Err(error) => return Err(error),
        }

        let mut refresh_state = self.refresh_state.lock().await;
        if let Ok(claims) = self.validate(token) {
            return Ok(claims);
        }
        if refresh_state
            .as_ref()
            .is_some_and(|last| last.elapsed() < Duration::from_secs(30))
        {
            return Err(TokenValidationError::UnknownKey);
        }
        let jwks = self
            .client
            .get(self.config.jwks_url.clone())
            .send()
            .await
            .map_err(|_| TokenValidationError::JwksUnavailable)?
            .error_for_status()
            .map_err(|_| TokenValidationError::JwksUnavailable)?
            .json::<JwkSet>()
            .await
            .map_err(|_| TokenValidationError::JwksUnavailable)?;
        validate_jwks(&jwks)?;
        *self
            .jwks
            .write()
            .map_err(|_| TokenValidationError::JwksUnavailable)? = jwks;
        *refresh_state = Some(Instant::now());
        self.validate(token)
    }

    pub fn protected_resource_metadata(&self, public_url: &Url) -> Value {
        json!({
            "resource": public_url.join("mcp").map_or_else(
                |_| public_url.as_str().to_string(),
                |url| url.to_string()
            ),
            "authorization_servers": [self.config.issuer.as_str()],
            "scopes_supported": self.config.scopes,
            "bearer_methods_supported": ["header"]
        })
    }

    pub fn bearer_challenge(&self, public_url: &Url) -> String {
        let metadata = public_url
            .join(".well-known/oauth-protected-resource")
            .map_or_else(|_| public_url.to_string(), |url| url.to_string());
        format!("Bearer realm=\"elegy-memory-mcp\", resource_metadata=\"{metadata}\"")
    }
}

fn jwks_client() -> Result<reqwest::Client, TokenValidationError> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| TokenValidationError::JwksUnavailable)
}

fn validate_jwks(jwks: &JwkSet) -> Result<(), TokenValidationError> {
    let mut key_ids = BTreeSet::new();
    for key in &jwks.keys {
        if signing_algorithm(key).is_none()
            || key
                .common
                .public_key_use
                .as_ref()
                .is_some_and(|usage| *usage != PublicKeyUse::Signature)
            || key
                .common
                .key_operations
                .as_ref()
                .is_some_and(|operations| !operations.contains(&KeyOperations::Verify))
        {
            continue;
        }
        let Some(key_id) = key
            .common
            .key_id
            .as_deref()
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if DecodingKey::from_jwk(key).is_err() {
            continue;
        }
        if !key_ids.insert(key_id) {
            return Err(TokenValidationError::EmptyJwks);
        }
    }
    if key_ids.is_empty() {
        return Err(TokenValidationError::EmptyJwks);
    }
    Ok(())
}

fn signing_algorithm(key: &Jwk) -> Option<Algorithm> {
    match (&key.algorithm, key.common.key_algorithm?) {
        (AlgorithmParameters::EllipticCurve(_), KeyAlgorithm::ES256) => Some(Algorithm::ES256),
        (AlgorithmParameters::RSA(_), KeyAlgorithm::RS256) => Some(Algorithm::RS256),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::ExternalTokenValidator;
    use crate::config::ExternalOAuthConfig;
    use axum::{routing::get, Json, Router};
    use jsonwebtoken::{encode, jwk::JwkSet, Algorithm, EncodingKey, Header};
    use serde::Serialize;
    use serde_json::json;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use time::OffsetDateTime;
    use url::Url;

    #[derive(Serialize)]
    struct Claims<'a> {
        iss: &'a str,
        aud: &'a str,
        exp: usize,
        scope: &'a str,
        jti: &'a str,
    }

    const PRIVATE_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgWTFfCGljY6aw3Hrt
kHmPRiazukxPLb6ilpRAewjW8nihRANCAATDskChT+Altkm9X7MI69T3IUmrQU0L
950IxEzvw/x5BMEINRMrXLBJhqzO9Bm+d6JbqA21YQmd1Kt4RzLJR1W+
-----END PRIVATE KEY-----"#;

    fn jwks(kid: &str) -> JwkSet {
        serde_json::from_value(json!({
            "keys": [{
                "kty": "EC",
                "crv": "P-256",
                "x": "w7JAoU_gJbZJvV-zCOvU9yFJq0FNC_edCMRM78P8eQQ",
                "y": "wQg1EytcsEmGrM70Gb53oluoDbVhCZ3Uq3hHMslHVb4",
                "alg": "ES256",
                "kid": kid,
                "use": "sig"
            }]
        }))
        .expect("test JWKS")
    }

    fn config(jwks_url: Url) -> ExternalOAuthConfig {
        ExternalOAuthConfig {
            issuer: Url::parse("https://identity.example.com").expect("issuer"),
            audience: "https://memory.example.com/mcp".to_string(),
            jwks_url,
            scopes: vec!["memory.read".to_string()],
        }
    }

    fn validator() -> ExternalTokenValidator {
        ExternalTokenValidator::from_jwks(
            config(Url::parse("https://identity.example.com/jwks").expect("JWKS URL")),
            jwks("test-key"),
        )
        .expect("validator")
    }

    fn token(kid: &str, issuer: &str, audience: &str, scope: &str, expires_in: i64) -> String {
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(kid.to_string());
        encode(
            &header,
            &Claims {
                iss: issuer,
                aud: audience,
                exp: (OffsetDateTime::now_utc().unix_timestamp() + expires_in) as usize,
                scope,
                jti: "test-jti",
            },
            &EncodingKey::from_ec_pem(PRIVATE_KEY.as_bytes()).expect("private key"),
        )
        .expect("sign token")
    }

    #[test]
    fn validates_issuer_audience_expiry_and_scope() {
        let validator = validator();
        let valid = token(
            "test-key",
            "https://identity.example.com/",
            "https://memory.example.com/mcp",
            "memory.read",
            300,
        );
        assert_eq!(
            validator
                .validate(&valid)
                .expect("valid token")
                .jti
                .as_deref(),
            Some("test-jti")
        );

        let wrong_audience = token(
            "test-key",
            "https://identity.example.com/",
            "https://other.example.com",
            "memory.read",
            300,
        );
        assert!(validator.validate(&wrong_audience).is_err());

        let wrong_issuer = token(
            "test-key",
            "https://evil.example.com/",
            "https://memory.example.com/mcp",
            "memory.read",
            300,
        );
        assert!(validator.validate(&wrong_issuer).is_err());

        let wrong_scope = token(
            "test-key",
            "https://identity.example.com/",
            "https://memory.example.com/mcp",
            "profile",
            300,
        );
        assert!(validator.validate(&wrong_scope).is_err());

        let expired = token(
            "test-key",
            "https://identity.example.com/",
            "https://memory.example.com/mcp",
            "memory.read",
            -300,
        );
        assert!(validator.validate(&expired).is_err());
    }

    #[test]
    fn rejects_symmetric_missing_algorithm_and_duplicate_jwks_keys() {
        let config = config(Url::parse("https://identity.example.com/jwks").expect("JWKS URL"));
        for value in [
            json!({"keys":[{"kty":"oct","k":"c2VjcmV0","alg":"HS256","kid":"one"}]}),
            json!({"keys":[{"kty":"EC","crv":"P-256","x":"x","y":"y","kid":"one"}]}),
            json!({"keys":[
                {"kty":"EC","crv":"P-256","x":"x","y":"y","alg":"ES256","kid":"same"},
                {"kty":"EC","crv":"P-256","x":"x","y":"y","alg":"ES256","kid":"same"}
            ]}),
        ] {
            let keys = serde_json::from_value(value).expect("syntactically valid JWKS");
            assert!(ExternalTokenValidator::from_jwks(config.clone(), keys).is_err());
        }
    }

    #[test]
    fn accepts_jwks_with_a_signing_key_and_unrelated_encryption_keys() {
        let mut value = serde_json::to_value(jwks("test-key")).expect("JWKS JSON");
        value["keys"]
            .as_array_mut()
            .expect("JWKS keys")
            .push(json!({
                "kty": "oct",
                "k": "c2VjcmV0",
                "alg": "HS256",
                "kid": "encryption-key",
                "use": "enc"
            }));
        let keys = serde_json::from_value(value).expect("syntactically valid mixed JWKS");

        assert!(ExternalTokenValidator::from_jwks(
            config(Url::parse("https://identity.example.com/jwks").expect("JWKS URL")),
            keys,
        )
        .is_ok());
    }

    #[tokio::test]
    async fn refreshes_jwks_once_when_a_new_signing_key_id_appears() {
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let app = Router::new().route(
            "/jwks",
            get(move || {
                let observed = observed.clone();
                async move {
                    let kid = if observed.fetch_add(1, Ordering::SeqCst) == 0 {
                        "old-key"
                    } else {
                        "new-key"
                    };
                    Json(serde_json::to_value(jwks(kid)).expect("JWKS JSON"))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let base = format!("http://{}", listener.local_addr().expect("address"));
        tokio::spawn(async move { axum::serve(listener, app).await.expect("serve") });

        let validator = ExternalTokenValidator::load(config(
            Url::parse(&format!("{base}/jwks")).expect("JWKS URL"),
        ))
        .await
        .expect("initial JWKS");
        let rotated = token(
            "new-key",
            "https://identity.example.com/",
            "https://memory.example.com/mcp",
            "memory.read",
            300,
        );

        assert!(validator.validate(&rotated).is_err());
        assert!(validator.validate_with_refresh(&rotated).await.is_ok());
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn publishes_only_resource_metadata_and_delegated_challenge() {
        let validator = validator();
        let metadata = validator
            .protected_resource_metadata(&Url::parse("https://memory.example.com").expect("URL"));
        assert_eq!(
            metadata["authorization_servers"],
            json!(["https://identity.example.com/"])
        );
        assert!(metadata.get("authorization_endpoint").is_none());
        assert!(validator
            .bearer_challenge(&Url::parse("https://memory.example.com").expect("URL"))
            .contains("/.well-known/oauth-protected-resource"));
    }
}
