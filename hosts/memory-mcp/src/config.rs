use std::{collections::BTreeMap, env, net::Ipv4Addr, path::PathBuf};

use thiserror::Error;
use url::Url;

const DEFAULT_PORT: u16 = 8765;

#[derive(Clone, Debug)]
pub enum AuthMode {
    LocalNone,
    ExternalOAuth(Box<ExternalOAuthConfig>),
}

#[derive(Clone, Debug)]
pub struct ExternalOAuthConfig {
    pub issuer: Url,
    pub audience: String,
    pub jwks_url: Url,
    pub scopes: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub db_path: PathBuf,
    pub public_url: Url,
    pub port: u16,
    pub bind_ip: Ipv4Addr,
    pub log_content: bool,
    pub auth: AuthMode,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_map(&env::vars().collect())
    }

    pub(crate) fn from_map(env: &BTreeMap<String, String>) -> Result<Self, ConfigError> {
        let missing = ["ELEGY_MCP_AUTH_MODE", "ELEGY_MCP_DB_PATH"]
            .into_iter()
            .filter(|name| env.get(*name).is_none_or(|value| value.trim().is_empty()))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(ConfigError::MissingRequiredEnv {
                names: missing.join(", "),
            });
        }

        let port = env
            .get("ELEGY_MCP_PORT")
            .map(|value| parse_port("ELEGY_MCP_PORT", value))
            .transpose()?
            .unwrap_or(DEFAULT_PORT);
        let bind_ip = env
            .get("ELEGY_MCP_BIND")
            .map_or(Ok(Ipv4Addr::LOCALHOST), |value| {
                value.trim().parse().map_err(|_| ConfigError::InvalidEnv {
                    name: "ELEGY_MCP_BIND",
                    reason: "must be an IPv4 address",
                    value: value.clone(),
                })
            })?;
        let log_content = env
            .get("ELEGY_MCP_LOG_CONTENT")
            .map(|value| parse_bool("ELEGY_MCP_LOG_CONTENT", value))
            .transpose()?
            .unwrap_or(false);
        let db_path = PathBuf::from(
            env.get("ELEGY_MCP_DB_PATH")
                .expect("required variable was checked"),
        );

        let mode = env
            .get("ELEGY_MCP_AUTH_MODE")
            .expect("required variable was checked")
            .trim();
        let (public_url, auth) = match mode {
            "local-none" => {
                if !bind_ip.is_loopback() {
                    return Err(ConfigError::UnsafeUnauthenticatedBind { bind_ip });
                }
                let public_url = Url::parse(&format!("http://{bind_ip}:{port}/"))
                    .expect("loopback URL is valid");
                (public_url, AuthMode::LocalNone)
            }
            "external-oauth" => {
                let required = [
                    "ELEGY_MCP_PUBLIC_URL",
                    "ELEGY_MCP_OAUTH_ISSUER",
                    "ELEGY_MCP_OAUTH_AUDIENCE",
                    "ELEGY_MCP_OAUTH_JWKS_URL",
                    "ELEGY_MCP_OAUTH_SCOPES",
                ];
                let missing = required
                    .into_iter()
                    .filter(|name| env.get(*name).is_none_or(|value| value.trim().is_empty()))
                    .collect::<Vec<_>>();
                if !missing.is_empty() {
                    return Err(ConfigError::MissingRequiredEnv {
                        names: missing.join(", "),
                    });
                }
                let mut public_url = parse_secure_url(env, "ELEGY_MCP_PUBLIC_URL", true)?;
                let issuer = parse_secure_url(env, "ELEGY_MCP_OAUTH_ISSUER", false)?;
                let jwks_url = parse_secure_url(env, "ELEGY_MCP_OAUTH_JWKS_URL", false)?;
                if !public_url.path().ends_with('/') {
                    let normalized = format!("{}/", public_url.path());
                    public_url.set_path(&normalized);
                }
                let audience = env["ELEGY_MCP_OAUTH_AUDIENCE"].trim().to_string();
                let scopes = env["ELEGY_MCP_OAUTH_SCOPES"]
                    .split(',')
                    .map(str::trim)
                    .filter(|scope| !scope.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                if scopes.is_empty() {
                    return Err(ConfigError::InvalidEnv {
                        name: "ELEGY_MCP_OAUTH_SCOPES",
                        reason: "must declare at least one comma-separated scope",
                        value: env["ELEGY_MCP_OAUTH_SCOPES"].clone(),
                    });
                }
                (
                    public_url,
                    AuthMode::ExternalOAuth(Box::new(ExternalOAuthConfig {
                        issuer,
                        audience,
                        jwks_url,
                        scopes,
                    })),
                )
            }
            other => {
                return Err(ConfigError::InvalidEnv {
                    name: "ELEGY_MCP_AUTH_MODE",
                    reason: "must be local-none or external-oauth",
                    value: other.to_string(),
                });
            }
        };

        Ok(Self {
            db_path,
            public_url,
            port,
            bind_ip,
            log_content,
            auth,
        })
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing required environment variables: {names}")]
    MissingRequiredEnv { names: String },
    #[error("{name} {reason}: {value:?}")]
    InvalidEnv {
        name: &'static str,
        reason: &'static str,
        value: String,
    },
    #[error("{name} must be a valid absolute URL: {value:?}: {source}")]
    InvalidUrl {
        name: &'static str,
        value: String,
        #[source]
        source: url::ParseError,
    },
    #[error("unauthenticated HTTP MCP may bind only to loopback, not {bind_ip}")]
    UnsafeUnauthenticatedBind { bind_ip: Ipv4Addr },
}

fn parse_url(env: &BTreeMap<String, String>, name: &'static str) -> Result<Url, ConfigError> {
    let value = env
        .get(name)
        .expect("required variable was checked")
        .trim()
        .to_string();
    Url::parse(&value).map_err(|source| ConfigError::InvalidUrl {
        name,
        value,
        source,
    })
}

fn parse_secure_url(
    env: &BTreeMap<String, String>,
    name: &'static str,
    allow_base_path: bool,
) -> Result<Url, ConfigError> {
    let url = parse_url(env, name)?;
    let invalid = url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || (!allow_base_path && name == "ELEGY_MCP_OAUTH_ISSUER" && url.path() != "/");
    if invalid {
        return Err(ConfigError::InvalidEnv {
            name,
            reason: "must be an HTTPS URL without credentials, query, or fragment",
            value: url.to_string(),
        });
    }
    Ok(url)
}

fn parse_port(name: &'static str, value: &str) -> Result<u16, ConfigError> {
    value
        .trim()
        .parse::<u16>()
        .ok()
        .filter(|port| *port > 0)
        .ok_or_else(|| ConfigError::InvalidEnv {
            name,
            reason: "must be an integer between 1 and 65535",
            value: value.to_string(),
        })
}

fn parse_bool(name: &'static str, value: &str) -> Result<bool, ConfigError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(ConfigError::InvalidEnv {
            name,
            reason: "must be one of 0, 1, true, false, yes, no, on, off",
            value: value.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthMode, Config, ConfigError};
    use std::{collections::BTreeMap, net::Ipv4Addr, path::PathBuf};

    #[test]
    fn local_none_is_explicit_and_loopback_only() {
        let env = env_map([
            ("ELEGY_MCP_AUTH_MODE", "local-none"),
            ("ELEGY_MCP_DB_PATH", "C:\\memory\\elegy.db"),
        ]);
        let config = Config::from_map(&env).expect("local config");

        assert_eq!(config.bind_ip, Ipv4Addr::LOCALHOST);
        assert!(matches!(config.auth, AuthMode::LocalNone));
        assert_eq!(config.db_path, PathBuf::from("C:\\memory\\elegy.db"));
    }

    #[test]
    fn local_none_rejects_non_loopback_binding() {
        let env = env_map([
            ("ELEGY_MCP_AUTH_MODE", "local-none"),
            ("ELEGY_MCP_DB_PATH", "C:\\memory\\elegy.db"),
            ("ELEGY_MCP_BIND", "0.0.0.0"),
        ]);
        let error = Config::from_map(&env).expect_err("public no-auth must fail");
        assert!(matches!(
            error,
            ConfigError::UnsafeUnauthenticatedBind { .. }
        ));
    }

    #[test]
    fn external_oauth_requires_resource_server_configuration() {
        let env = env_map([
            ("ELEGY_MCP_AUTH_MODE", "external-oauth"),
            ("ELEGY_MCP_DB_PATH", "C:\\memory\\elegy.db"),
            ("ELEGY_MCP_BIND", "0.0.0.0"),
            ("ELEGY_MCP_PUBLIC_URL", "https://memory.example.com"),
            ("ELEGY_MCP_OAUTH_ISSUER", "https://identity.example.com"),
            ("ELEGY_MCP_OAUTH_AUDIENCE", "https://memory.example.com/mcp"),
            (
                "ELEGY_MCP_OAUTH_JWKS_URL",
                "https://identity.example.com/.well-known/jwks.json",
            ),
            ("ELEGY_MCP_OAUTH_SCOPES", "memory.read,memory.write"),
        ]);
        let config = Config::from_map(&env).expect("external OAuth config");
        let AuthMode::ExternalOAuth(auth) = config.auth else {
            panic!("external auth expected");
        };
        assert_eq!(auth.issuer.as_str(), "https://identity.example.com/");
        assert_eq!(auth.audience, "https://memory.example.com/mcp");
        assert_eq!(auth.scopes, vec!["memory.read", "memory.write"]);
    }

    #[test]
    fn external_oauth_rejects_insecure_or_credential_bearing_urls() {
        for (name, value) in [
            ("ELEGY_MCP_PUBLIC_URL", "http://memory.example.com"),
            ("ELEGY_MCP_OAUTH_ISSUER", "http://identity.example.com"),
            (
                "ELEGY_MCP_OAUTH_JWKS_URL",
                "https://user:secret@identity.example.com/jwks",
            ),
        ] {
            let mut env = external_env();
            env.insert(name.to_string(), value.to_string());
            let error = Config::from_map(&env).expect_err("unsafe external URL must fail");
            assert!(matches!(error, ConfigError::InvalidEnv { .. }), "{name}");
        }
    }

    #[test]
    fn external_public_url_is_normalized_as_a_base_url() {
        let mut env = external_env();
        env.insert(
            "ELEGY_MCP_PUBLIC_URL".into(),
            "https://memory.example.com/agent".into(),
        );
        let config = Config::from_map(&env).expect("external OAuth config");
        assert_eq!(
            config.public_url.as_str(),
            "https://memory.example.com/agent/"
        );
    }

    #[test]
    fn missing_auth_mode_fails_closed() {
        let env = env_map([("ELEGY_MCP_DB_PATH", "C:\\memory\\elegy.db")]);
        let error = Config::from_map(&env).expect_err("auth mode is required");
        assert!(matches!(error, ConfigError::MissingRequiredEnv { .. }));
    }

    fn env_map<'a>(
        entries: impl IntoIterator<Item = (&'static str, &'a str)>,
    ) -> BTreeMap<String, String> {
        entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect()
    }

    fn external_env() -> BTreeMap<String, String> {
        env_map([
            ("ELEGY_MCP_AUTH_MODE", "external-oauth"),
            ("ELEGY_MCP_DB_PATH", "C:\\memory\\elegy.db"),
            ("ELEGY_MCP_BIND", "0.0.0.0"),
            ("ELEGY_MCP_PUBLIC_URL", "https://memory.example.com"),
            ("ELEGY_MCP_OAUTH_ISSUER", "https://identity.example.com"),
            ("ELEGY_MCP_OAUTH_AUDIENCE", "https://memory.example.com/mcp"),
            (
                "ELEGY_MCP_OAUTH_JWKS_URL",
                "https://identity.example.com/.well-known/jwks.json",
            ),
            ("ELEGY_MCP_OAUTH_SCOPES", "memory.read"),
        ])
    }
}
