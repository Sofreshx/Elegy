use std::{
    collections::BTreeMap,
    env, fmt,
    path::{Path, PathBuf},
};

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, SaltString},
    Argon2, PasswordHasher,
};
use directories::ProjectDirs;
use thiserror::Error;
use url::Url;

const ELEGY_MCP_ADMIN_PASSWORD: &str = "ELEGY_MCP_ADMIN_PASSWORD";
const ELEGY_MCP_DB_PATH: &str = "ELEGY_MCP_DB_PATH";
const ELEGY_MCP_PUBLIC_URL: &str = "ELEGY_MCP_PUBLIC_URL";
const ELEGY_MCP_PORT: &str = "ELEGY_MCP_PORT";
const ELEGY_MCP_LOG_CONTENT: &str = "ELEGY_MCP_LOG_CONTENT";
const ELEGY_MCP_DATA_DIR: &str = "ELEGY_MCP_DATA_DIR";

const DEFAULT_PORT: u16 = 8765;

#[derive(Clone)]
pub struct Config {
    pub admin_password_verifier: String,
    pub db_path: PathBuf,
    pub public_url: Url,
    pub port: u16,
    pub log_content: bool,
    pub data_dir: PathBuf,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let env = env::vars().collect::<BTreeMap<_, _>>();
        let default_data_dir = default_data_dir()?;

        Self::from_map_with_default_data_dir(&env, default_data_dir)
    }

    fn from_map_with_default_data_dir(
        env: &BTreeMap<String, String>,
        default_data_dir: PathBuf,
    ) -> Result<Self, ConfigError> {
        let missing = [
            ELEGY_MCP_ADMIN_PASSWORD,
            ELEGY_MCP_DB_PATH,
            ELEGY_MCP_PUBLIC_URL,
        ]
        .into_iter()
        .filter(|name| env.get(*name).is_none_or(|value| value.trim().is_empty()))
        .collect::<Vec<_>>();

        if !missing.is_empty() {
            return Err(ConfigError::MissingRequiredEnv {
                names: missing.join(", "),
            });
        }

        let admin_password = env
            .get(ELEGY_MCP_ADMIN_PASSWORD)
            .expect("required env is checked above")
            .trim()
            .to_owned();
        validate_admin_password(ELEGY_MCP_ADMIN_PASSWORD, &admin_password)?;
        let admin_password_verifier =
            derive_admin_password_verifier(&admin_password).map_err(|source| {
                ConfigError::AdminPasswordHashFailed {
                    name: ELEGY_MCP_ADMIN_PASSWORD,
                    message: source.to_string(),
                }
            })?;
        let db_path = path_from_required(env, ELEGY_MCP_DB_PATH);
        let public_url = parse_url(env, ELEGY_MCP_PUBLIC_URL)?;

        let port = match env.get(ELEGY_MCP_PORT) {
            Some(value) => parse_port(ELEGY_MCP_PORT, value)?,
            None => DEFAULT_PORT,
        };

        let log_content = match env.get(ELEGY_MCP_LOG_CONTENT) {
            Some(value) => parse_bool(ELEGY_MCP_LOG_CONTENT, value)?,
            None => false,
        };

        let data_dir = match env.get(ELEGY_MCP_DATA_DIR) {
            Some(value) => path_from_optional(ELEGY_MCP_DATA_DIR, value)?,
            None => default_data_dir,
        };

        Ok(Self {
            admin_password_verifier,
            db_path,
            public_url,
            port,
            log_content,
            data_dir,
        })
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Config")
            .field("admin_password_verifier", &"<redacted>")
            .field("db_path", &self.db_path)
            .field("public_url", &self.public_url)
            .field("port", &self.port)
            .field("log_content", &self.log_content)
            .field("data_dir", &self.data_dir)
            .finish()
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
    #[error("{name} {reason}")]
    InvalidSecretEnv {
        name: &'static str,
        reason: &'static str,
    },
    #[error("{name} must be a valid absolute URL: {value:?}: {source}")]
    InvalidUrl {
        name: &'static str,
        value: String,
        #[source]
        source: url::ParseError,
    },
    #[error("failed to resolve default data directory via directories::ProjectDirs")]
    DefaultDataDirUnavailable,
    #[error("failed to derive startup argon2 verifier for {name}")]
    AdminPasswordHashFailed { name: &'static str, message: String },
}

fn default_data_dir() -> Result<PathBuf, ConfigError> {
    ProjectDirs::from("com", "holon", "elegy-mcp")
        .map(|dirs| dirs.data_local_dir().to_path_buf())
        .ok_or(ConfigError::DefaultDataDirUnavailable)
}

fn path_from_required(env: &BTreeMap<String, String>, name: &'static str) -> PathBuf {
    PathBuf::from(
        env.get(name)
            .expect("required env is checked above")
            .as_str(),
    )
}

fn path_from_optional(name: &'static str, value: &str) -> Result<PathBuf, ConfigError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ConfigError::InvalidEnv {
            name,
            reason: "must not be empty",
            value: value.to_owned(),
        });
    }

    Ok(Path::new(trimmed).to_path_buf())
}

fn parse_url(env: &BTreeMap<String, String>, name: &'static str) -> Result<Url, ConfigError> {
    let value = env
        .get(name)
        .expect("required env is checked above")
        .trim()
        .to_owned();

    Url::parse(&value).map_err(|source| ConfigError::InvalidUrl {
        name,
        value,
        source,
    })
}

fn parse_port(name: &'static str, value: &str) -> Result<u16, ConfigError> {
    let port = value
        .trim()
        .parse::<u16>()
        .map_err(|_| ConfigError::InvalidEnv {
            name,
            reason: "must be an integer between 1 and 65535",
            value: value.to_owned(),
        })?;

    if port == 0 {
        return Err(ConfigError::InvalidEnv {
            name,
            reason: "must be an integer between 1 and 65535",
            value: value.to_owned(),
        });
    }

    Ok(port)
}

fn parse_bool(name: &'static str, value: &str) -> Result<bool, ConfigError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(ConfigError::InvalidEnv {
            name,
            reason: "must be one of 0, 1, true, false, yes, no, on, off",
            value: value.to_owned(),
        }),
    }
}

fn validate_admin_password(name: &'static str, value: &str) -> Result<(), ConfigError> {
    if PasswordHash::new(value).is_ok() {
        return Err(ConfigError::InvalidSecretEnv {
            name,
            reason: "must be the admin password, not an argon2 hash string",
        });
    }

    Ok(())
}

pub(crate) fn derive_admin_password_verifier(
    value: &str,
) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(value.as_bytes(), &salt)
        .map(|hash| hash.to_string())
}

#[cfg(test)]
mod tests {
    use super::{derive_admin_password_verifier, Config, ConfigError, ELEGY_MCP_ADMIN_PASSWORD};
    use std::{collections::BTreeMap, path::PathBuf};

    #[test]
    fn loads_required_values_and_defaults() {
        let env = env_map([
            ("ELEGY_MCP_ADMIN_PASSWORD", "test-password"),
            ("ELEGY_MCP_DB_PATH", "C:\\memory\\elegy.db"),
            ("ELEGY_MCP_PUBLIC_URL", "https://elegy-memory.holon.it.com"),
        ]);

        let config = Config::from_map_with_default_data_dir(&env, PathBuf::from("C:\\appdata"))
            .expect("config should load");

        assert!(config.admin_password_verifier.starts_with("$argon2id$"));
        assert_eq!(config.db_path, PathBuf::from("C:\\memory\\elegy.db"));
        assert_eq!(
            config.public_url.as_str(),
            "https://elegy-memory.holon.it.com/"
        );
        assert_eq!(config.port, 8765);
        assert!(!config.log_content);
        assert_eq!(config.data_dir, PathBuf::from("C:\\appdata"));
    }

    #[test]
    fn loads_optional_values_from_env() {
        let env = env_map([
            ("ELEGY_MCP_ADMIN_PASSWORD", "test-password"),
            ("ELEGY_MCP_DB_PATH", "C:\\memory\\elegy.db"),
            ("ELEGY_MCP_PUBLIC_URL", "https://elegy-memory.holon.it.com"),
            ("ELEGY_MCP_PORT", "9001"),
            ("ELEGY_MCP_LOG_CONTENT", "yes"),
            ("ELEGY_MCP_DATA_DIR", "D:\\elegy-mcp"),
        ]);

        let config = Config::from_map_with_default_data_dir(&env, PathBuf::from("C:\\ignored"))
            .expect("config should load");

        assert_eq!(config.port, 9001);
        assert!(config.log_content);
        assert_eq!(config.data_dir, PathBuf::from("D:\\elegy-mcp"));
    }

    #[test]
    fn rejects_missing_required_values() {
        let error =
            Config::from_map_with_default_data_dir(&BTreeMap::new(), PathBuf::from("C:\\appdata"))
                .expect_err("config should fail");

        assert!(matches!(
            error,
            ConfigError::MissingRequiredEnv { names }
            if names == "ELEGY_MCP_ADMIN_PASSWORD, ELEGY_MCP_DB_PATH, ELEGY_MCP_PUBLIC_URL"
        ));
    }

    #[test]
    fn rejects_invalid_port() {
        let env = env_map([
            ("ELEGY_MCP_ADMIN_PASSWORD", "test-password"),
            ("ELEGY_MCP_DB_PATH", "C:\\memory\\elegy.db"),
            ("ELEGY_MCP_PUBLIC_URL", "https://elegy-memory.holon.it.com"),
            ("ELEGY_MCP_PORT", "0"),
        ]);

        let error = Config::from_map_with_default_data_dir(&env, PathBuf::from("C:\\appdata"))
            .expect_err("config should fail");

        assert!(matches!(
            error,
            ConfigError::InvalidEnv { name, .. } if name == "ELEGY_MCP_PORT"
        ));
    }

    #[test]
    fn rejects_invalid_log_content_value() {
        let env = env_map([
            ("ELEGY_MCP_ADMIN_PASSWORD", "test-password"),
            ("ELEGY_MCP_DB_PATH", "C:\\memory\\elegy.db"),
            ("ELEGY_MCP_PUBLIC_URL", "https://elegy-memory.holon.it.com"),
            ("ELEGY_MCP_LOG_CONTENT", "sometimes"),
        ]);

        let error = Config::from_map_with_default_data_dir(&env, PathBuf::from("C:\\appdata"))
            .expect_err("config should fail");

        assert!(matches!(
            error,
            ConfigError::InvalidEnv { name, .. } if name == "ELEGY_MCP_LOG_CONTENT"
        ));
    }

    #[test]
    fn rejects_invalid_public_url() {
        let env = env_map([
            ("ELEGY_MCP_ADMIN_PASSWORD", "test-password"),
            ("ELEGY_MCP_DB_PATH", "C:\\memory\\elegy.db"),
            ("ELEGY_MCP_PUBLIC_URL", "not-a-url"),
        ]);

        let error = Config::from_map_with_default_data_dir(&env, PathBuf::from("C:\\appdata"))
            .expect_err("config should fail");

        assert!(matches!(
            error,
            ConfigError::InvalidUrl { name, .. } if name == "ELEGY_MCP_PUBLIC_URL"
        ));
    }

    #[test]
    fn rejects_argon_password_hash_input() {
        let admin_password_hash = derive_admin_password_verifier("test-password")
            .unwrap_or_else(|_| panic!("password verifier should generate"));
        let env = env_map([
            ("ELEGY_MCP_ADMIN_PASSWORD", admin_password_hash.as_str()),
            ("ELEGY_MCP_DB_PATH", "C:\\memory\\elegy.db"),
            ("ELEGY_MCP_PUBLIC_URL", "https://elegy-memory.holon.it.com"),
        ]);

        let error = Config::from_map_with_default_data_dir(&env, PathBuf::from("C:\\appdata"))
            .expect_err("config should fail");

        assert!(matches!(
            error,
            ConfigError::InvalidSecretEnv { name, .. } if name == ELEGY_MCP_ADMIN_PASSWORD
        ));
        assert!(!error.to_string().contains(&admin_password_hash));
    }

    fn env_map<'a>(
        entries: impl IntoIterator<Item = (&'static str, &'a str)>,
    ) -> BTreeMap<String, String> {
        entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect()
    }
}
