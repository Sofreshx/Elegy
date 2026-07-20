use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    AuthenticatedRequest, BrokerError, BrokerStore, NewAccessRequest, OperationExecutor,
    OperationRisk, ProviderCatalog, ProxyError,
};

const DEFAULT_READ_GRANT_MINUTES: u32 = 30 * 24 * 60;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TypedExecutionRequest {
    pub client_id: String,
    pub purpose_class: String,
    pub provider: String,
    pub operation: String,
    #[serde(default)]
    pub account_id: Option<String>,
    #[serde(default)]
    pub arguments: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum TypedExecutionOutcome {
    Completed {
        status: u16,
        result: Value,
        audit_id: String,
    },
    InteractionRequired {
        request_id: String,
        kind: String,
        duration_minutes: u32,
    },
    AccountSelectionRequired {
        accounts: Vec<TypedAccountChoice>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TypedAccountChoice {
    pub id: String,
    pub verified_identity: String,
}

#[derive(Debug, Error)]
pub enum TypedExecutionError {
    #[error("provider operation is unavailable")]
    InvalidOperation,
    #[error("provider operation arguments are invalid")]
    InvalidArguments,
    #[error("the selected account is unavailable")]
    AccountUnavailable,
    #[error(transparent)]
    Broker(#[from] BrokerError),
    #[error(transparent)]
    Proxy(#[from] ProxyError),
}

impl TypedExecutionError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidOperation => "invalid_operation",
            Self::InvalidArguments => "invalid_operation_arguments",
            Self::AccountUnavailable => "account_unavailable",
            Self::Broker(_) => "broker_unavailable",
            Self::Proxy(_) => "sanitized_provider_error",
        }
    }
}

impl BrokerStore {
    pub async fn execute_typed_operation(
        &self,
        client: &Client,
        catalog: &ProviderCatalog,
        request: TypedExecutionRequest,
    ) -> Result<TypedExecutionOutcome, TypedExecutionError> {
        let provider = catalog
            .get(&request.provider)
            .ok_or(TypedExecutionError::InvalidOperation)?;
        let operation = catalog
            .executable_operation(&request.provider, &request.operation)
            .ok_or(TypedExecutionError::InvalidOperation)?;
        if operation.risk == OperationRisk::Write {
            return Err(TypedExecutionError::InvalidOperation);
        }
        let (profile_id, method, path) = match &operation.executor {
            OperationExecutor::Http {
                profile,
                method,
                path,
            } => (profile, method, path),
            OperationExecutor::Adapter { .. } => {
                return Err(TypedExecutionError::InvalidOperation);
            }
        };
        let resolved_path = resolve_path(&operation.input_schema, path, &request.arguments)?;
        let profile = provider
            .auth_profiles
            .iter()
            .find(|candidate| &candidate.id == profile_id)
            .ok_or(TypedExecutionError::InvalidOperation)?;
        let url = format!(
            "{}{}",
            profile.audience.trim_end_matches('/'),
            resolved_path
        );

        let matching_accounts = self
            .vault()
            .list_accounts()
            .map_err(BrokerError::from)?
            .into_iter()
            .filter(|account| account.provider == request.provider)
            .collect::<Vec<_>>();
        let account = match request.account_id.as_deref() {
            Some(account_id) => matching_accounts
                .into_iter()
                .find(|account| account.id == account_id)
                .ok_or(TypedExecutionError::AccountUnavailable)?,
            None if matching_accounts.len() == 1 => matching_accounts
                .into_iter()
                .next()
                .ok_or(TypedExecutionError::AccountUnavailable)?,
            None if matching_accounts.len() > 1 => {
                return Ok(TypedExecutionOutcome::AccountSelectionRequired {
                    accounts: matching_accounts
                        .into_iter()
                        .map(|account| TypedAccountChoice {
                            id: account.id,
                            verified_identity: account.verified_identity,
                        })
                        .collect(),
                });
            }
            None => return Err(TypedExecutionError::AccountUnavailable),
        };

        let now = Utc::now();
        let active_grant = self.list_grants()?.into_iter().find(|grant| {
            !grant.revoked
                && grant.account_id == account.id
                && grant.provider == request.provider
                && grant.client_id == request.client_id
                && grant.purpose == request.purpose_class
                && grant
                    .operations
                    .iter()
                    .any(|item| item == &request.operation)
                && chrono::DateTime::parse_from_rfc3339(&grant.expires_at)
                    .is_ok_and(|expiry| expiry.with_timezone(&Utc) > now)
        });
        let Some(grant) = active_grant else {
            let existing = self.list_requests()?.into_iter().find(|pending| {
                pending.status == "awaiting_user"
                    && pending.account_id.as_deref() == Some(account.id.as_str())
                    && pending.client_id.as_deref() == Some(request.client_id.as_str())
                    && pending.purpose == request.purpose_class
                    && pending
                        .operations
                        .iter()
                        .any(|item| item == &request.operation)
            });
            let pending = match existing {
                Some(pending) => pending,
                None => self.request_access(NewAccessRequest {
                    account_id: account.id,
                    client_id: request.client_id,
                    purpose: request.purpose_class,
                    operations: vec![request.operation],
                    duration_minutes: DEFAULT_READ_GRANT_MINUTES,
                })?,
            };
            return Ok(TypedExecutionOutcome::InteractionRequired {
                request_id: pending.id,
                kind: "approve_read_access".into(),
                duration_minutes: pending.duration_minutes,
            });
        };

        let lease = self.issue_single_use_lease(&grant.id, 15)?;
        let response = self
            .execute_authenticated(
                client,
                catalog,
                AuthenticatedRequest {
                    lease: &lease.token,
                    client_id: &request.client_id,
                    purpose: &request.purpose_class,
                    provider: &request.provider,
                    operation: &request.operation,
                    method,
                    url: &url,
                    body: None,
                },
            )
            .await?;
        validate_result(&operation.result_schema, &response.body)?;
        let audit_id = format!("audit_{}", Uuid::new_v4().simple());
        self.audit(
            "operation.completed",
            Some(&grant.account_id),
            json!({
                "audit_id": audit_id,
                "client_id": request.client_id,
                "provider": request.provider,
                "operation": request.operation,
                "provider_status": response.status
            }),
        )?;
        Ok(TypedExecutionOutcome::Completed {
            status: response.status,
            result: response.body,
            audit_id,
        })
    }
}

fn resolve_path(
    schema: &Value,
    template: &str,
    arguments: &Value,
) -> Result<String, TypedExecutionError> {
    let arguments = arguments
        .as_object()
        .ok_or(TypedExecutionError::InvalidArguments)?;
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if schema.get("additionalProperties").and_then(Value::as_bool) == Some(false)
        && arguments.keys().any(|key| !properties.contains_key(key))
    {
        return Err(TypedExecutionError::InvalidArguments);
    }
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for name in required.iter().filter_map(Value::as_str) {
            if !arguments.contains_key(name) {
                return Err(TypedExecutionError::InvalidArguments);
            }
        }
    }
    let mut path = template.to_owned();
    for (name, value) in arguments {
        let value = value
            .as_str()
            .filter(|value| is_safe_path_segment(value))
            .ok_or(TypedExecutionError::InvalidArguments)?;
        let placeholder = format!("{{{name}}}");
        if !path.contains(&placeholder) {
            return Err(TypedExecutionError::InvalidArguments);
        }
        path = path.replace(&placeholder, value);
    }
    if path.contains('{') || path.contains('}') {
        return Err(TypedExecutionError::InvalidArguments);
    }
    Ok(path)
}

fn is_safe_path_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn validate_result(schema: &Value, result: &Value) -> Result<(), TypedExecutionError> {
    match schema.get("type").and_then(Value::as_str) {
        Some("object") if !result.is_object() => Err(TypedExecutionError::InvalidOperation),
        Some("array") if !result.is_array() => Err(TypedExecutionError::InvalidOperation),
        _ => Ok(()),
    }
}
