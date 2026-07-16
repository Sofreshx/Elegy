use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use elegy_accountd::{
    AuthorizationSession, BrokerStore, DpapiProtector, NewAccessRequest, OAuthAdapterConfig,
    OAuthTransaction, ProviderCatalog, Vault, VerifiedCredential, exchange_and_verify,
    verify_cloudflare_token,
};
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::{ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

#[derive(Clone)]
struct AccountsMcp {
    broker: Arc<BrokerStore>,
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ListParams {
    provider: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct DiscoverParams {
    provider: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RequireParams {
    provider: String,
    purpose: String,
    operations: Vec<String>,
    account_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AccessParams {
    account_id: String,
    purpose: String,
    operations: Vec<String>,
    duration_minutes: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CreationParams {
    provider: String,
    purpose: String,
    constraints: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct StatusParams {
    request_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RevokeParams {
    grant_id: String,
    reason: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AuditParams {
    account_id: Option<String>,
    limit: Option<u32>,
}

impl AccountsMcp {
    fn new() -> Result<Self> {
        let database = local_data_dir()?.join("accounts.sqlite");
        if let Some(parent) = database.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let broker = BrokerStore::new(Vault::open(database, Arc::new(DpapiProtector))?);
        Ok(Self {
            broker: Arc::new(broker),
            tool_router: Self::tool_router(),
        })
    }
}

#[tool_router]
impl AccountsMcp {
    #[tool(
        description = "List locally connected account metadata. This never returns credentials or tokens."
    )]
    fn account_list(&self, Parameters(params): Parameters<ListParams>) -> String {
        let result = self.broker.vault().list_accounts().map(|accounts| {
            accounts
                .into_iter()
                .filter(|account| {
                    params
                        .provider
                        .as_ref()
                        .is_none_or(|provider| provider == &account.provider)
                })
                .collect::<Vec<_>>()
        });
        match result {
            Ok(accounts) => json!({ "accounts": accounts }).to_string(),
            Err(error) => {
                json!({ "error": "vault_unavailable", "message": error.to_string() }).to_string()
            }
        }
    }

    #[tool(
        description = "Discover safe connection methods and supported browser origins for a provider. Discovery hints are unverified until credential validation."
    )]
    fn account_discover(&self, Parameters(params): Parameters<DiscoverParams>) -> String {
        let catalog = ProviderCatalog::mvp();
        let providers: Vec<_> = catalog
            .list()
            .into_iter()
            .filter(|provider| params.provider.as_ref().is_none_or(|id| id == &provider.id))
            .collect();
        json!({ "providers": providers, "browser_boundary": "signed-in flow hint only; no password or cookie extraction" }).to_string()
    }

    #[tool(
        description = "Resolve a connected account for a purpose and named operations, or return a structured human interaction requirement."
    )]
    fn account_require(&self, Parameters(params): Parameters<RequireParams>) -> String {
        let accounts = self.broker.vault().list_accounts().unwrap_or_default();
        let match_account = accounts.into_iter().find(|account| {
            account.provider == params.provider
                && params
                    .account_id
                    .as_ref()
                    .is_none_or(|id| id == &account.id)
        });
        match match_account {
            Some(account) => json!({ "status": "account_available", "account": account, "purpose": params.purpose, "operations": params.operations }).to_string(),
            None => json!({ "status": "interaction_required", "kind": "connect_or_create", "provider": params.provider, "purpose": params.purpose, "operations": params.operations, "open_center": "http://127.0.0.1:43119/" }).to_string(),
        }
    }

    #[tool(
        description = "Request a revocable grant. The result is pending until the user approves it in Account Center; this tool cannot self-approve."
    )]
    fn account_request_access(&self, Parameters(params): Parameters<AccessParams>) -> String {
        match self.broker.request_access(NewAccessRequest { account_id: params.account_id, client_id: "codex-local".into(), purpose: params.purpose, operations: params.operations, duration_minutes: params.duration_minutes.unwrap_or(60).min(1440) }) {
            Ok(request) => json!({"request_id":request.id,"status":request.status,"kind":request.kind,"open_center":"http://127.0.0.1:43119/"}).to_string(),
            Err(error) => json!({"error":"request_failed","message":error.to_string()}).to_string(),
        }
    }

    #[tool(
        description = "Request creation of a provider account through an idempotent local saga. CAPTCHA, MFA, terms, payment, identity, and ambiguous choices always pause for the user."
    )]
    fn account_request_creation(&self, Parameters(params): Parameters<CreationParams>) -> String {
        let key = format!("{}:{}", params.provider, params.purpose);
        match self.broker.request_creation(&key, &params.provider, &params.purpose, params.constraints.unwrap_or_default()) {
            Ok(request) => json!({"request_id":request.id,"status":request.status,"kind":request.kind,"human_boundaries":["captcha","mfa","terms","payment","identity_verification","ambiguous_plan","unexpected_page"],"open_center":"http://127.0.0.1:43119/"}).to_string(),
            Err(error) => json!({"error":"request_failed","message":error.to_string()}).to_string(),
        }
    }

    #[tool(description = "Read sanitized status for an access or account-creation request.")]
    fn account_request_status(&self, Parameters(params): Parameters<StatusParams>) -> String {
        match self.broker.get_request(&params.request_id) {
            Ok(request) if request.status == "approved" && request.kind == "access" => {
                let lease = self
                    .broker
                    .grant_for_request(&request.id)
                    .and_then(|grant| {
                        self.broker
                            .issue_lease(&grant.id, 15)
                            .map(|lease| (grant, lease))
                    });
                match lease {
                    Ok((grant, lease)) => json!({"request_id":request.id,"status":"approved","grant_id":grant.id,"lease":lease,"scope":{"client_id":grant.client_id,"purpose":grant.purpose,"audience":grant.provider,"operations":grant.operations}}).to_string(),
                    Err(error) => json!({"request_id":request.id,"status":"approved","error":"lease_unavailable","message":error.to_string()}).to_string(),
                }
            }
            Ok(request) => json!(request).to_string(),
            Err(_) => json!({ "request_id": params.request_id, "status": "not_found" }).to_string(),
        }
    }

    #[tool(
        description = "Return the local Account Center URL for user review. No credential is included in the URL."
    )]
    fn account_open_center(&self) -> String {
        json!({ "url": "http://127.0.0.1:43119/", "local_only": true }).to_string()
    }

    #[tool(
        description = "Request immediate revocation of a grant and all derived leases. Remote provider revocation is attempted by the broker when supported."
    )]
    fn account_revoke_grant(&self, Parameters(params): Parameters<RevokeParams>) -> String {
        match self.broker.revoke_grant(&params.grant_id, params.reason.as_deref().unwrap_or("agent requested revocation")) {
            Ok(()) => json!({ "grant_id": params.grant_id, "status": "revoked", "leases_invalidated": true }).to_string(),
            Err(error) => json!({"error":"revocation_failed","message":error.to_string()}).to_string(),
        }
    }

    #[tool(
        description = "List sanitized local account-security audit events. Audit records never include credential values."
    )]
    fn account_audit_list(&self, Parameters(params): Parameters<AuditParams>) -> String {
        let events = self
            .broker
            .list_audit(params.limit.unwrap_or(50).min(200))
            .unwrap_or_default()
            .into_iter()
            .filter(|event| {
                params
                    .account_id
                    .as_ref()
                    .is_none_or(|id| event.account_id.as_ref() == Some(id))
            })
            .collect::<Vec<_>>();
        json!({ "events": events }).to_string()
    }
}

#[tool_handler]
impl ServerHandler for AccountsMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "Local account broker. Agents receive scoped capabilities, never credentials.",
        )
    }
}

fn local_data_dir() -> Result<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA is not set")?;
    Ok(PathBuf::from(base).join("Elegy").join("Accounts"))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let invoked_as_native_host = std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_stem()
                .map(|name| name.to_string_lossy().contains("native-host"))
        })
        .unwrap_or(false);
    if invoked_as_native_host || args.first().map(String::as_str) == Some("native-host") {
        return run_native_host();
    }
    match args.first().map(String::as_str) {
        None | Some("mcp") => {
            let service = AccountsMcp::new()?.serve(stdio()).await?;
            service.waiting().await?;
            Ok(())
        }
        Some("serve" | "broker") => run_account_center().await,
        Some("open") => run_open(args.iter().any(|arg| arg == "--print-url")).await,
        Some("status") => run_status(),
        Some("backup") => {
            let destination = args
                .get(1)
                .context("usage: elegy-accounts backup <destination.sqlite>")?;
            let source = local_data_dir()?.join("accounts.sqlite");
            Vault::open(source, Arc::new(DpapiProtector))?.export_backup(destination)?;
            Ok(())
        }
        Some("restore") => {
            let source = PathBuf::from(
                args.get(1)
                    .context("usage: elegy-accounts restore <backup.sqlite>")?,
            );
            let candidate = Vault::open(&source, Arc::new(DpapiProtector))?;
            for account in candidate.list_accounts()? {
                let _ = candidate.load_secret(&account.id)?;
            }
            drop(candidate);
            let destination = local_data_dir()?.join("accounts.sqlite");
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(source, destination)?;
            Ok(())
        }
        Some("proof-github") => {
            let destination = PathBuf::from(
                args.get(1)
                    .context("usage: elegy-accounts proof-github <evidence.json>")?,
            );
            run_live_github_proof(destination).await
        }
        Some(command) => anyhow::bail!("unknown command `{command}`"),
    }
}

const ACCOUNT_CENTER_URL: &str = "http://127.0.0.1:43119/";

async fn run_open(print_only: bool) -> Result<()> {
    if print_only {
        println!("{ACCOUNT_CENTER_URL}");
        return Ok(());
    }

    let health_url = format!("{ACCOUNT_CENTER_URL}api/state");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()?;
    let already_running = client
        .get(&health_url)
        .send()
        .await
        .is_ok_and(|response| response.status().is_success());

    if !already_running {
        let executable = std::env::current_exe()?;
        let mut command = Command::new(executable);
        command
            .arg("broker")
            .env("ELEGY_ACCOUNT_CENTER_DIST", account_center_ui_dir())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        command
            .spawn()
            .context("failed to start the local account broker")?;

        let mut ready = false;
        for _ in 0..30 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if client
                .get(&health_url)
                .send()
                .await
                .is_ok_and(|response| response.status().is_success())
            {
                ready = true;
                break;
            }
        }
        if !ready {
            anyhow::bail!("account broker did not become ready on the local loopback endpoint");
        }
    }

    #[cfg(windows)]
    Command::new("explorer.exe")
        .arg(ACCOUNT_CENTER_URL)
        .spawn()
        .context("failed to open Account Center")?;
    #[cfg(not(windows))]
    println!("{ACCOUNT_CENTER_URL}");
    Ok(())
}

fn account_center_ui_dir() -> PathBuf {
    if let Some(configured) = std::env::var_os("ELEGY_ACCOUNT_CENTER_DIST") {
        return PathBuf::from(configured);
    }
    if let Ok(executable) = std::env::current_exe()
        && let Some(plugin_root) = executable.parent().and_then(|bin| bin.parent())
    {
        let packaged = plugin_root.join("ui").join("account-center");
        if packaged.is_dir() {
            return packaged;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("apps")
        .join("account-center")
        .join("dist")
}

fn run_status() -> Result<()> {
    let data_dir = local_data_dir()?;
    std::fs::create_dir_all(&data_dir)?;
    let vault = Vault::open(data_dir.join("accounts.sqlite"), Arc::new(DpapiProtector))?;
    let accounts = vault.list_accounts()?;
    let sessions = vault.list_authorization_sessions()?;
    let attention = sessions
        .iter()
        .filter(|session| {
            matches!(
                session.status.as_str(),
                "interaction_required" | "expired" | "denied" | "failed"
            )
        })
        .count();
    println!(
        "{}",
        json!({
            "schemaVersion": "elegy-accounts-status/v1",
            "localOnly": true,
            "platformProtection": "DPAPI",
            "connectedAccounts": accounts.len(),
            "authorizationSessions": sessions.len(),
            "attentionRequired": attention,
            "dataDirectory": data_dir,
            "providers": ["github", "cloudflare"],
        })
    );
    Ok(())
}

async fn run_live_github_proof(destination: PathBuf) -> Result<()> {
    let output = Command::new("gh")
        .args(["auth", "token"])
        .env("GH_PROMPT_DISABLED", "1")
        .output()
        .context("GitHub CLI is unavailable")?;
    if !output.status.success() {
        anyhow::bail!("GitHub CLI has no usable authenticated account");
    }
    let mut secret = Zeroizing::new(output.stdout);
    while secret
        .last()
        .is_some_and(|byte| matches!(byte, b'\r' | b'\n' | b' ' | b'\t'))
    {
        secret.pop();
    }
    let secret_text = std::str::from_utf8(secret.as_slice())
        .context("GitHub CLI credential encoding is invalid")?;
    if secret_text.is_empty() {
        anyhow::bail!("GitHub CLI returned an empty credential");
    }

    let http = reqwest::Client::builder()
        .user_agent("Elegy-Accounts-Live-Proof/0.1")
        .build()?;
    let profile_response = http
        .get("https://api.github.com/user")
        .bearer_auth(secret_text)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .context("GitHub identity verification failed")?;
    if !profile_response.status().is_success() {
        anyhow::bail!("GitHub rejected identity verification");
    }
    let profile: serde_json::Value = profile_response.json().await?;
    let login = profile
        .get("login")
        .and_then(|value| value.as_str())
        .context("GitHub returned no verified login")?;

    let proof_dir = ProofDirectory::create(local_data_dir()?)?;
    let database = proof_dir.path.join("proof.sqlite");
    let backup = proof_dir.path.join("proof-backup.sqlite");
    let broker = BrokerStore::new(Vault::open(&database, Arc::new(DpapiProtector))?);
    let account = broker.vault().store_account(
        "github",
        login,
        "existing_cli_session_ephemeral",
        secret.as_slice(),
    )?;
    let request = broker.request_access(NewAccessRequest {
        account_id: account.id.clone(),
        client_id: "codex-local".into(),
        purpose: "supervised live GitHub read-only proof".into(),
        operations: vec!["profile.read".into()],
        duration_minutes: 5,
    })?;
    let grant = broker.approve_access(&request.id)?;
    let lease = broker.issue_lease(&grant.id, 5)?;
    broker.authorize(
        &lease.token,
        "codex-local",
        "supervised live GitHub read-only proof",
        "github",
        "profile.read",
    )?;

    let decrypted = broker.vault().load_secret(&account.id)?;
    let decrypted_text = std::str::from_utf8(decrypted.as_slice())
        .context("vault credential encoding is invalid")?;
    let action_response = http
        .get("https://api.github.com/user")
        .bearer_auth(decrypted_text)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await?;
    let read_only_api_call = action_response.status().is_success();
    let action_profile: serde_json::Value =
        action_response.json().await.unwrap_or_else(|_| json!({}));
    let identity_matches = action_profile.get("login") == profile.get("login")
        && action_profile.get("id") == profile.get("id");

    broker.vault().export_backup(&backup)?;
    let plaintext_absent = !contains_bytes(&std::fs::read(&database)?, secret.as_slice())
        && !contains_bytes(&std::fs::read(&backup)?, secret.as_slice());
    drop(broker);

    let restarted = BrokerStore::new(Vault::open(&database, Arc::new(DpapiProtector))?);
    let persisted = restarted.get_request(&request.id)?.status == "approved"
        && restarted.vault().load_secret(&account.id)?.as_slice() == secret.as_slice();
    restarted.revoke_grant(&grant.id, "live proof completed")?;
    let revocation_invalidated_lease = restarted
        .authorize(
            &lease.token,
            "codex-local",
            "supervised live GitHub read-only proof",
            "github",
            "profile.read",
        )
        .is_err();
    restarted.disconnect_account(&account.id)?;
    drop(restarted);

    let evidence = sanitized_github_evidence(
        &profile,
        read_only_api_call && identity_matches,
        persisted,
        revocation_invalidated_lease,
        plaintext_absent,
    );
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(destination, serde_json::to_vec_pretty(&evidence)?)?;
    Ok(())
}

fn sanitized_github_evidence(
    profile: &serde_json::Value,
    read_only_api_call: bool,
    restart_persistence: bool,
    revocation_invalidated_lease: bool,
    plaintext_absent: bool,
) -> serde_json::Value {
    json!({
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "provider": "github",
        "verified_identity": profile.get("login").and_then(|value| value.as_str()),
        "provider_user_id": profile.get("id").and_then(|value| value.as_u64()),
        "read_only_api_call": read_only_api_call,
        "restart_persistence": restart_persistence,
        "revocation_invalidated_lease": revocation_invalidated_lease,
        "plaintext_absent": plaintext_absent,
        "source_scope_risk": "existing_cli_credential_is_broader_than_the_mvp_grant",
        "persistent_account_created": false,
        "remote_mutations": 0,
    })
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

struct ProofDirectory {
    path: PathBuf,
}

impl ProofDirectory {
    fn create(base: PathBuf) -> Result<Self> {
        let path = base.join(format!("live-proof-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }
}

impl Drop for ProofDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[derive(Clone)]
struct WebState {
    broker: Arc<BrokerStore>,
    oauth: Arc<Mutex<HashMap<String, PendingOAuth>>>,
    devices: Arc<Mutex<HashMap<String, PendingDevice>>>,
    http: reqwest::Client,
}

struct PendingOAuth {
    transaction: OAuthTransaction,
    config: OAuthConfig,
}

struct PendingDevice {
    config: DeviceFlowConfig,
    device_code: Zeroizing<String>,
    expires_at: Instant,
    interval: u64,
    next_poll_at: Instant,
}

#[derive(Clone)]
struct OAuthConfig {
    provider: String,
    client_id: String,
    authorize_url: String,
    token_url: String,
    identity_url: String,
    scopes: String,
}

#[derive(Clone)]
struct DeviceFlowConfig {
    provider: String,
    client_id: String,
    scope: String,
    device_url: String,
    token_url: String,
    identity_url: String,
}

struct DeviceAuthorization {
    device_code: Zeroizing<String>,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Serialize)]
struct PublicDeviceAuthorization<'a> {
    mode: &'static str,
    user_code: &'a str,
    verification_uri: &'a str,
    expires_in: u64,
    interval: u64,
}

impl DeviceAuthorization {
    fn public_view(&self) -> PublicDeviceAuthorization<'_> {
        PublicDeviceAuthorization {
            mode: "device",
            user_code: &self.user_code,
            verification_uri: &self.verification_uri,
            expires_in: self.expires_in,
            interval: self.interval,
        }
    }
}

enum DevicePoll {
    Pending { interval: u64 },
    SlowDown { interval: u64 },
    Complete(VerifiedCredential),
    Denied(&'static str),
}

async fn start_device_flow(
    client: &reqwest::Client,
    config: &DeviceFlowConfig,
) -> Result<DeviceAuthorization> {
    let response = client
        .post(&config.device_url)
        .header(reqwest::header::ACCEPT, "application/json")
        .form(&[
            ("client_id", config.client_id.as_str()),
            ("scope", config.scope.as_str()),
        ])
        .send()
        .await
        .context("device authorization request failed")?;
    if !response.status().is_success() {
        anyhow::bail!("provider rejected device authorization")
    }
    let value: serde_json::Value = response
        .json()
        .await
        .context("invalid device authorization response")?;
    Ok(DeviceAuthorization {
        device_code: Zeroizing::new(
            value
                .get("device_code")
                .and_then(|v| v.as_str())
                .context("missing device code")?
                .to_owned(),
        ),
        user_code: value
            .get("user_code")
            .and_then(|v| v.as_str())
            .context("missing user code")?
            .to_owned(),
        verification_uri: value
            .get("verification_uri")
            .and_then(|v| v.as_str())
            .context("missing verification URI")?
            .to_owned(),
        expires_in: value
            .get("expires_in")
            .and_then(|v| v.as_u64())
            .unwrap_or(900),
        interval: value
            .get("interval")
            .and_then(|v| v.as_u64())
            .unwrap_or(5)
            .max(1),
    })
}

async fn poll_device_flow(
    client: &reqwest::Client,
    config: &DeviceFlowConfig,
    device_code: &str,
) -> Result<DevicePoll> {
    let response = client
        .post(&config.token_url)
        .header(reqwest::header::ACCEPT, "application/json")
        .form(&[
            ("client_id", config.client_id.as_str()),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await
        .context("device token request failed")?;
    let success = response.status().is_success();
    let value: serde_json::Value = response
        .json()
        .await
        .context("invalid device token response")?;
    if let Some(error) = value.get("error").and_then(|v| v.as_str()) {
        return Ok(match error {
            "authorization_pending" => DevicePoll::Pending { interval: 0 },
            "slow_down" => DevicePoll::SlowDown { interval: 5 },
            "access_denied" => DevicePoll::Denied("access_denied"),
            "expired_token" => DevicePoll::Denied("expired_token"),
            _ => DevicePoll::Denied("provider_rejected_authorization"),
        });
    }
    if !success {
        return Ok(DevicePoll::Denied("provider_rejected_authorization"));
    }
    let secret = Zeroizing::new(
        value
            .get("access_token")
            .and_then(|v| v.as_str())
            .context("missing access token")?
            .to_owned(),
    );
    let identity_response = client
        .get(&config.identity_url)
        .bearer_auth(secret.as_str())
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .context("identity verification failed")?;
    if !identity_response.status().is_success() {
        anyhow::bail!("provider rejected identity verification")
    }
    let identity_json: serde_json::Value = identity_response
        .json()
        .await
        .context("invalid identity response")?;
    let identity = ["login", "email", "username", "id"]
        .iter()
        .find_map(|key| {
            let value = identity_json.get(key)?;
            value
                .as_str()
                .map(str::to_owned)
                .or_else(|| value.as_u64().map(|id| id.to_string()))
        })
        .context("provider did not return a verifiable identity")?;
    Ok(DevicePoll::Complete(VerifiedCredential {
        provider: config.provider.clone(),
        identity,
        secret,
    }))
}

#[derive(Deserialize)]
struct OAuthCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct CloudflareTokenRequest {
    token: String,
}

async fn run_account_center() -> Result<()> {
    let database = local_data_dir()?.join("accounts.sqlite");
    if let Some(parent) = database.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let state = WebState {
        broker: Arc::new(BrokerStore::new(Vault::open(
            database,
            Arc::new(DpapiProtector),
        )?)),
        oauth: Arc::new(Mutex::new(HashMap::new())),
        devices: Arc::new(Mutex::new(HashMap::new())),
        http: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("Elegy-Accounts/0.1 (+local account broker)")
            .build()?,
    };
    for session in state
        .broker
        .vault()
        .list_authorization_sessions()?
        .into_iter()
        .filter(|session| session.status == "waiting_for_user")
    {
        spawn_device_worker(state.clone(), session.id);
    }
    let ui_dir = account_center_ui_dir();
    let app = Router::new()
        .route("/api/state", get(web_state))
        .route("/api/requests/{id}/approve", post(web_approve))
        .route("/api/requests/{id}/cancel", post(web_cancel))
        .route("/api/grants/{id}/revoke", post(web_revoke))
        .route("/api/accounts/{id}/disconnect", post(web_disconnect))
        .route("/api/connections/{provider}/start", post(web_start_connection))
        .route("/api/connections/cloudflare/token", post(web_connect_cloudflare_token))
        .route("/api/connections/device/{id}/poll", post(web_poll_device))
        .route("/oauth/callback", get(web_oauth_callback))
        .fallback_service(tower_http::services::ServeDir::new(ui_dir).append_index_html_on_directories(true))
        .layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(header::CACHE_CONTROL, HeaderValue::from_static("no-store")))
        .layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'self' http://127.0.0.1:*")))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:43119").await?;
    eprintln!("Elegy Account Center: http://127.0.0.1:43119/");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn web_state(State(state): State<WebState>) -> impl IntoResponse {
    let result = (|| -> Result<serde_json::Value> {
        Ok(json!({
            "accounts": state.broker.vault().list_accounts()?,
            "requests": state.broker.list_requests().map_err(anyhow::Error::from)?,
            "grants": state.broker.list_grants().map_err(anyhow::Error::from)?,
            "audit": state.broker.list_audit(100).map_err(anyhow::Error::from)?,
            "authorizations": state.broker.vault().list_authorization_sessions()?
                .into_iter().filter(|session| !matches!(session.status.as_str(), "connected" | "superseded" | "cancelled")).collect::<Vec<_>>(),
        }))
    })();
    match result {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"broker_unavailable","message":error.to_string()})),
        )
            .into_response(),
    }
}

async fn web_approve(
    State(state): State<WebState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !valid_user_intent(&headers) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"user_intent_required"})),
        )
            .into_response();
    }
    match state.broker.approve_access(&id) {
        Ok(grant) => (StatusCode::OK, Json(json!({"grant":grant}))).into_response(),
        Err(error) => (
            StatusCode::CONFLICT,
            Json(json!({"error":"approval_failed","message":error.to_string()})),
        )
            .into_response(),
    }
}

async fn web_cancel(
    State(state): State<WebState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !valid_user_intent(&headers) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"user_intent_required"})),
        )
            .into_response();
    }
    match state.broker.cancel_request(&id) {
        Ok(()) => (StatusCode::OK, Json(json!({"status":"cancelled"}))).into_response(),
        Err(error) => (
            StatusCode::CONFLICT,
            Json(json!({"error":"cancellation_failed","message":error.to_string()})),
        )
            .into_response(),
    }
}

async fn web_revoke(
    State(state): State<WebState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !valid_user_intent(&headers) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"user_intent_required"})),
        )
            .into_response();
    }
    match state.broker.revoke_grant(&id, "revoked in Account Center") {
        Ok(()) => (StatusCode::OK, Json(json!({"status":"revoked"}))).into_response(),
        Err(error) => (
            StatusCode::CONFLICT,
            Json(json!({"error":"revocation_failed","message":error.to_string()})),
        )
            .into_response(),
    }
}

async fn web_disconnect(
    State(state): State<WebState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !valid_user_intent(&headers) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"user_intent_required"})),
        )
            .into_response();
    }
    match state.broker.disconnect_account(&id) {
        Ok(()) => (StatusCode::OK, Json(json!({"status":"disconnected"}))).into_response(),
        Err(error) => (
            StatusCode::CONFLICT,
            Json(json!({"error":"disconnect_failed","message":error.to_string()})),
        )
            .into_response(),
    }
}

fn valid_user_intent(headers: &HeaderMap) -> bool {
    headers
        .get("x-elegy-intent")
        .is_some_and(|value| value == "user-action")
}

async fn web_start_connection(
    State(state): State<WebState>,
    Path(provider): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !valid_user_intent(&headers) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"user_intent_required"})),
        )
            .into_response();
    }
    if provider == "github" {
        return web_start_github_device(state).await;
    }
    if provider == "cloudflare" {
        return (
            StatusCode::OK,
            Json(json!({
                "mode":"manual_token",
                "creation_url":"https://dash.cloudflare.com/profile/api-tokens"
            })),
        )
            .into_response();
    }
    let Some(config) = oauth_config(&provider) else {
        return (StatusCode::PRECONDITION_REQUIRED, Json(json!({"error":"provider_configuration_required","message":format!("Set the local {} OAuth client ID before connecting.", provider)}))).into_response();
    };
    let redirect_uri = "http://127.0.0.1:43119/oauth/callback";
    let transaction = OAuthTransaction::new(
        &provider,
        &config.authorize_url,
        &config.identity_url,
        redirect_uri,
    );
    let mut authorization = match url::Url::parse(&config.authorize_url) {
        Ok(url) => url,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":"invalid_provider_configuration"})),
            )
                .into_response();
        }
    };
    authorization
        .query_pairs_mut()
        .append_pair("client_id", &config.client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", &config.scopes)
        .append_pair("state", &transaction.state)
        .append_pair("nonce", &transaction.nonce)
        .append_pair("code_challenge", &transaction.pkce_challenge)
        .append_pair("code_challenge_method", "S256");
    let state_key = transaction.state.clone();
    if let Ok(mut pending) = state.oauth.lock() {
        pending.insert(
            state_key,
            PendingOAuth {
                transaction,
                config,
            },
        );
    }
    (
        StatusCode::OK,
        Json(json!({"authorization_url": authorization.as_str()})),
    )
        .into_response()
}

async fn web_connect_cloudflare_token(
    State(state): State<WebState>,
    headers: HeaderMap,
    Json(request): Json<CloudflareTokenRequest>,
) -> impl IntoResponse {
    if !valid_user_intent(&headers) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"user_intent_required"})),
        )
            .into_response();
    }
    let token = Zeroizing::new(request.token);
    let verified = verify_cloudflare_token(
        &state.http,
        "https://api.cloudflare.com/client/v4/user/tokens/verify",
        token.as_str(),
    )
    .await;
    let Ok(verified) = verified else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error":"token_verification_failed",
                "message":"Cloudflare did not verify this token as active. Check its scope and try again."
            })),
        )
            .into_response();
    };
    match state.broker.vault().store_account(
        &verified.provider,
        &verified.identity,
        "guided_api_token",
        verified.secret.as_bytes(),
    ) {
        Ok(account) => (StatusCode::OK, Json(json!({"status":"connected","account":account}))).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"vault_store_failed","message":"The verified token could not be stored locally."})),
        )
            .into_response(),
    }
}

async fn web_start_github_device(state: WebState) -> axum::response::Response {
    let Some(config) = github_device_config() else {
        return (
            StatusCode::PRECONDITION_REQUIRED,
            Json(json!({
                "error":"provider_configuration_required",
                "message":"Set the local GitHub OAuth client ID before connecting."
            })),
        )
            .into_response();
    };
    let authorization = match start_device_flow(&state.http, &config).await {
        Ok(value) => value,
        Err(error) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error":"device_authorization_failed", "message":error.to_string()
                })),
            )
                .into_response();
        }
    };
    let request_id = format!("auth_{}", uuid::Uuid::new_v4().simple());
    let interval = authorization.interval;
    let expires_in = authorization.expires_in;
    let mut response = serde_json::to_value(authorization.public_view())
        .unwrap_or_else(|_| json!({"mode":"device"}));
    response["request_id"] = json!(request_id.clone());
    let now = chrono::Utc::now();
    if let Ok(existing) = state.broker.vault().list_authorization_sessions() {
        for mut session in existing.into_iter().filter(|item| {
            item.provider == "github"
                && matches!(
                    item.status.as_str(),
                    "waiting_for_user" | "interaction_required"
                )
        }) {
            session.status = "superseded".into();
            session.updated_at = now.to_rfc3339();
            let _ = state.broker.vault().update_authorization_session(&session);
            let _ = state
                .broker
                .vault()
                .delete_authorization_secret(&session.id);
        }
    }
    let session = AuthorizationSession {
        id: request_id.clone(),
        provider: config.provider.clone(),
        status: "waiting_for_user".into(),
        user_code: authorization.user_code.clone(),
        verification_uri: authorization.verification_uri.clone(),
        expires_at: (now + chrono::Duration::seconds(expires_in as i64)).to_rfc3339(),
        interval_seconds: interval,
        next_poll_at: (now + chrono::Duration::seconds(interval as i64)).to_rfc3339(),
        last_error: None,
        created_at: now.to_rfc3339(),
        updated_at: now.to_rfc3339(),
    };
    if state
        .broker
        .vault()
        .store_authorization_session(&session, authorization.device_code.as_bytes())
        .is_err()
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"authorization_store_failed"})),
        )
            .into_response();
    }
    spawn_device_worker(state, request_id.clone());
    (StatusCode::OK, Json(response)).into_response()
}

fn spawn_device_worker(state: WebState, session_id: String) {
    tokio::spawn(async move {
        loop {
            let Some(mut session) = state
                .broker
                .vault()
                .list_authorization_sessions()
                .ok()
                .and_then(|sessions| sessions.into_iter().find(|item| item.id == session_id))
            else {
                return;
            };
            if session.status != "waiting_for_user" {
                return;
            }
            let now = chrono::Utc::now();
            let expires_at = chrono::DateTime::parse_from_rfc3339(&session.expires_at)
                .ok()
                .map(|value| value.with_timezone(&chrono::Utc));
            if expires_at.is_none_or(|expiry| now >= expiry) {
                session.status = "interaction_required".into();
                session.user_code.clear();
                session.last_error = Some("expired_token".into());
                session.updated_at = now.to_rfc3339();
                let _ = state.broker.vault().update_authorization_session(&session);
                let _ = state
                    .broker
                    .vault()
                    .delete_authorization_secret(&session.id);
                return;
            }
            let next_poll = chrono::DateTime::parse_from_rfc3339(&session.next_poll_at)
                .ok()
                .map(|value| value.with_timezone(&chrono::Utc))
                .unwrap_or(now);
            if next_poll > now {
                tokio::time::sleep((next_poll - now).to_std().unwrap_or(Duration::from_secs(1)))
                    .await;
                continue;
            }
            let Some(config) = (session.provider == "github")
                .then(github_device_config)
                .flatten()
            else {
                session.status = "interaction_required".into();
                session.last_error = Some("provider_configuration_required".into());
                session.updated_at = now.to_rfc3339();
                let _ = state.broker.vault().update_authorization_session(&session);
                return;
            };
            let Ok(secret) = state.broker.vault().load_authorization_secret(&session.id) else {
                return;
            };
            let secret_text = String::from_utf8_lossy(secret.as_slice());
            match poll_device_flow(&state.http, &config, &secret_text).await {
                Ok(DevicePoll::Complete(verified)) => {
                    if state
                        .broker
                        .vault()
                        .store_account(
                            &verified.provider,
                            &verified.identity,
                            "oauth_device",
                            verified.secret.as_bytes(),
                        )
                        .is_ok()
                    {
                        session.status = "connected".into();
                        session.user_code.clear();
                        session.last_error = None;
                        session.updated_at = chrono::Utc::now().to_rfc3339();
                        let _ = state.broker.vault().update_authorization_session(&session);
                        let _ = state
                            .broker
                            .vault()
                            .delete_authorization_secret(&session.id);
                    }
                    return;
                }
                Ok(DevicePoll::Denied("expired_token")) => {
                    session.status = "interaction_required".into();
                    session.user_code.clear();
                    session.last_error = Some("expired_token".into());
                    let _ = state
                        .broker
                        .vault()
                        .delete_authorization_secret(&session.id);
                }
                Ok(DevicePoll::Denied(reason)) => {
                    session.status = "interaction_required".into();
                    session.last_error = Some(reason.into());
                }
                Ok(DevicePoll::SlowDown { interval }) => {
                    session.interval_seconds =
                        session.interval_seconds.saturating_add(interval).max(5);
                }
                Ok(DevicePoll::Pending { interval }) => {
                    session.interval_seconds = session.interval_seconds.max(interval).max(1);
                }
                Err(_) => {
                    session.last_error = Some("temporary_provider_error".into());
                }
            }
            let updated = chrono::Utc::now();
            session.updated_at = updated.to_rfc3339();
            session.next_poll_at =
                (updated + chrono::Duration::seconds(session.interval_seconds as i64)).to_rfc3339();
            let terminal = session.status != "waiting_for_user";
            let _ = state.broker.vault().update_authorization_session(&session);
            if terminal {
                return;
            }
        }
    });
}

async fn web_poll_device(
    State(state): State<WebState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !valid_user_intent(&headers) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"user_intent_required"})),
        )
            .into_response();
    }
    let pending = state
        .devices
        .lock()
        .ok()
        .and_then(|mut devices| devices.remove(&id));
    let Some(mut pending) = pending else {
        return (StatusCode::NOT_FOUND, Json(json!({"status":"expired","message":"This authorization request is no longer active."}))).into_response();
    };
    let now = Instant::now();
    if now >= pending.expires_at {
        return (StatusCode::GONE, Json(json!({"status":"expired"}))).into_response();
    }
    if now < pending.next_poll_at {
        let retry_after = pending.next_poll_at.duration_since(now).as_secs().max(1);
        if let Ok(mut devices) = state.devices.lock() {
            devices.insert(id, pending);
        }
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"status":"pending","retry_after":retry_after})),
        )
            .into_response();
    }
    match poll_device_flow(&state.http, &pending.config, pending.device_code.as_str()).await {
        Ok(DevicePoll::Complete(verified)) => {
            if state
                .broker
                .vault()
                .store_account(
                    &verified.provider,
                    &verified.identity,
                    "oauth_device",
                    verified.secret.as_bytes(),
                )
                .is_err()
            {
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"status":"failed","message":"The verified credential could not be stored."}))).into_response();
            }
            (StatusCode::OK, Json(json!({"status":"connected","provider":verified.provider,"identity":verified.identity}))).into_response()
        }
        Ok(DevicePoll::Pending { interval }) => {
            pending.interval = pending.interval.max(interval);
            pending.next_poll_at = Instant::now() + Duration::from_secs(pending.interval);
            let retry_after = pending.interval;
            if let Ok(mut devices) = state.devices.lock() {
                devices.insert(id, pending);
            }
            (
                StatusCode::OK,
                Json(json!({"status":"pending","retry_after":retry_after})),
            )
                .into_response()
        }
        Ok(DevicePoll::SlowDown { interval }) => {
            pending.interval = pending.interval.saturating_add(interval).max(5);
            pending.next_poll_at = Instant::now() + Duration::from_secs(pending.interval);
            let retry_after = pending.interval;
            if let Ok(mut devices) = state.devices.lock() {
                devices.insert(id, pending);
            }
            (
                StatusCode::OK,
                Json(json!({"status":"pending","retry_after":retry_after})),
            )
                .into_response()
        }
        Ok(DevicePoll::Denied(reason)) => (
            StatusCode::CONFLICT,
            Json(json!({"status":"denied","message":reason})),
        )
            .into_response(),
        Err(error) => {
            pending.next_poll_at = Instant::now() + Duration::from_secs(pending.interval);
            if let Ok(mut devices) = state.devices.lock() {
                devices.insert(id, pending);
            }
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"status":"pending","message":error.to_string()})),
            )
                .into_response()
        }
    }
}

async fn web_oauth_callback(
    State(state): State<WebState>,
    axum::extract::Query(query): axum::extract::Query<OAuthCallbackQuery>,
) -> impl IntoResponse {
    if query.error.is_some() {
        return oauth_redirect("authorization_denied");
    }
    let (Some(code), Some(callback_state)) = (query.code, query.state) else {
        return oauth_redirect("invalid_callback");
    };
    let pending = state
        .oauth
        .lock()
        .ok()
        .and_then(|mut pending| pending.remove(&callback_state));
    let Some(pending) = pending else {
        return oauth_redirect("invalid_or_expired_state");
    };
    if pending.transaction.state != callback_state {
        return oauth_redirect("state_mismatch");
    }
    let redirect_uri = "http://127.0.0.1:43119/oauth/callback";
    let adapter = OAuthAdapterConfig {
        provider: pending.config.provider.clone(),
        client_id: pending.config.client_id.clone(),
        token_url: pending.config.token_url.clone(),
        identity_url: pending.config.identity_url.clone(),
    };
    let verified = exchange_and_verify(
        &state.http,
        &adapter,
        &code,
        pending
            .transaction
            .pkce_verifier
            .expose_for_token_exchange(),
        redirect_uri,
    )
    .await;
    let Ok(verified) = verified else {
        return oauth_redirect("provider_verification_failed");
    };
    if state
        .broker
        .vault()
        .store_account(
            &verified.provider,
            &verified.identity,
            "oauth_pkce",
            verified.secret.as_bytes(),
        )
        .is_err()
    {
        return oauth_redirect("vault_store_failed");
    }
    oauth_redirect(&format!("connected_{}", pending.config.provider))
}

fn oauth_redirect(status: &str) -> axum::response::Response {
    let safe = status
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '_')
        .collect::<String>();
    (
        StatusCode::SEE_OTHER,
        [(header::LOCATION, format!("/?status={safe}"))],
    )
        .into_response()
}

fn oauth_config(_provider: &str) -> Option<OAuthConfig> {
    None
}

fn github_device_config() -> Option<DeviceFlowConfig> {
    let client_id = std::env::var("ELEGY_GITHUB_CLIENT_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())?;
    Some(DeviceFlowConfig {
        provider: "github".into(),
        client_id,
        scope: "read:user".into(),
        device_url: "https://github.com/login/device/code".into(),
        token_url: "https://github.com/login/oauth/access_token".into(),
        identity_url: "https://api.github.com/user".into(),
    })
}

fn run_native_host() -> Result<()> {
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    loop {
        let mut length_bytes = [0_u8; 4];
        match input.read_exact(&mut length_bytes) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(error) => return Err(error.into()),
        }
        let length = u32::from_le_bytes(length_bytes) as usize;
        if length == 0 || length > 1_048_576 {
            anyhow::bail!("native message size is invalid");
        }
        let mut payload = vec![0_u8; length];
        input.read_exact(&mut payload)?;
        let message: serde_json::Value = serde_json::from_slice(&payload)?;
        let response = handle_native_message(message);
        let encoded = serde_json::to_vec(&response)?;
        output.write_all(&(encoded.len() as u32).to_le_bytes())?;
        output.write_all(&encoded)?;
        output.flush()?;
    }
}

fn handle_native_message(message: serde_json::Value) -> serde_json::Value {
    let safe_discovery = message.get("type").and_then(|value| value.as_str())
        == Some("account.discovery")
        && message
            .pointer("/hint/providerId")
            .and_then(|value| value.as_str())
            .is_some()
        && message
            .pointer("/hint/origin")
            .and_then(|value| value.as_str())
            .is_some()
        && !contains_secret_key(&message);
    if safe_discovery {
        json!({ "ok": true, "status": "interaction_required", "openCenter": "http://127.0.0.1:43119/" })
    } else {
        json!({ "ok": false, "error": "Unsupported or unsafe native message" })
    }
}

fn contains_secret_key(value: &serde_json::Value) -> bool {
    const SECRET_KEYS: &[&str] = &[
        "authorization",
        "password",
        "cookie",
        "set-cookie",
        "access_token",
        "refresh_token",
        "api_key",
        "client_secret",
        "secret",
        "token",
    ];
    match value {
        serde_json::Value::Object(map) => map.iter().any(|(key, child)| {
            SECRET_KEYS
                .iter()
                .any(|secret| key.eq_ignore_ascii_case(secret))
                || contains_secret_key(child)
        }),
        serde_json::Value::Array(items) => items.iter().any(contains_secret_key),
        _ => false,
    }
}

#[cfg(test)]
mod native_host_tests {
    use super::{
        DeviceFlowConfig, DevicePoll, handle_native_message, poll_device_flow, start_device_flow,
        valid_user_intent,
    };
    use axum::{
        Json, Router,
        extract::State,
        http::{HeaderMap, HeaderValue, StatusCode},
        routing::{get, post},
    };
    use serde_json::json;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[test]
    fn accepts_discovery_hint_but_rejects_any_secret_bearing_message() {
        let safe = handle_native_message(json!({
            "type": "account.discovery", "hint": { "providerId": "cloudflare", "origin": "https://dash.cloudflare.com", "verified": false }
        }));
        assert_eq!(safe["ok"], true);
        assert!(safe.to_string().contains("interaction_required"));

        let unsafe_message = handle_native_message(json!({
            "type": "account.discovery", "hint": { "providerId": "cloudflare", "origin": "https://dash.cloudflare.com" }, "cookie": "canary"
        }));
        assert_eq!(unsafe_message["ok"], false);
        assert!(!unsafe_message.to_string().contains("canary"));
    }

    #[test]
    fn local_client_and_ui_mutation_boundaries_fail_closed() {
        let mut headers = HeaderMap::new();
        assert!(!valid_user_intent(&headers));
        headers.insert("x-elegy-intent", HeaderValue::from_static("user-action"));
        assert!(valid_user_intent(&headers));
    }

    #[test]
    fn live_proof_evidence_keeps_identity_but_never_credentials() {
        let evidence = super::sanitized_github_evidence(
            &serde_json::json!({"login":"Sofreshx","id":46634397,"name":"Private Name","email":"private@example.test"}),
            true,
            true,
            true,
            true,
        );
        assert_eq!(evidence["provider"], "github");
        assert_eq!(evidence["verified_identity"], "Sofreshx");
        assert_eq!(evidence["provider_user_id"], 46634397);
        assert_eq!(evidence["read_only_api_call"], true);
        assert_eq!(evidence["revocation_invalidated_lease"], true);
        assert_eq!(evidence["plaintext_absent"], true);
        let serialized = evidence.to_string();
        assert!(!serialized.contains("Private Name"));
        assert!(!serialized.contains("private@example.test"));
        assert!(!serialized.to_ascii_lowercase().contains("token"));
    }

    #[tokio::test]
    async fn github_device_flow_matches_provider_contract_without_exposing_secrets() {
        #[derive(Clone)]
        struct FakeState(Arc<AtomicUsize>);
        async fn device(headers: HeaderMap, body: String) -> (StatusCode, Json<serde_json::Value>) {
            assert_eq!(headers.get("accept").unwrap(), "application/json");
            assert!(body.contains("client_id=test-client"));
            assert!(body.contains("scope=read%3Auser"));
            (
                StatusCode::OK,
                Json(json!({
                    "device_code":"device-secret-canary", "user_code":"ABCD-EFGH",
                    "verification_uri":"https://github.com/login/device", "expires_in":900, "interval":1
                })),
            )
        }
        async fn token(
            State(state): State<FakeState>,
            headers: HeaderMap,
            body: String,
        ) -> (StatusCode, Json<serde_json::Value>) {
            assert_eq!(headers.get("accept").unwrap(), "application/json");
            let form = url::form_urlencoded::parse(body.as_bytes())
                .into_owned()
                .collect::<std::collections::HashMap<_, _>>();
            assert_eq!(
                form.get("client_id").map(String::as_str),
                Some("test-client")
            );
            assert_eq!(
                form.get("device_code").map(String::as_str),
                Some("device-secret-canary")
            );
            assert_eq!(
                form.get("grant_type").map(String::as_str),
                Some("urn:ietf:params:oauth:grant-type:device_code")
            );
            if state.0.fetch_add(1, Ordering::SeqCst) == 0 {
                (
                    StatusCode::OK,
                    Json(json!({"error":"authorization_pending"})),
                )
            } else {
                (
                    StatusCode::OK,
                    Json(
                        json!({"access_token":"access-secret-canary","token_type":"bearer","scope":"read:user"}),
                    ),
                )
            }
        }
        async fn identity(headers: HeaderMap) -> (StatusCode, Json<serde_json::Value>) {
            assert_eq!(
                headers.get("authorization").unwrap(),
                "Bearer access-secret-canary"
            );
            (StatusCode::OK, Json(json!({"login":"octocat","id":1})))
        }
        let app = Router::new()
            .route("/device/code", post(device))
            .route("/oauth/access_token", post(token))
            .route("/user", get(identity))
            .with_state(FakeState(Arc::new(AtomicUsize::new(0))));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let config = DeviceFlowConfig {
            provider: "github".into(),
            client_id: "test-client".into(),
            scope: "read:user".into(),
            device_url: format!("{base}/device/code"),
            token_url: format!("{base}/oauth/access_token"),
            identity_url: format!("{base}/user"),
        };
        let client = reqwest::Client::new();
        let start = start_device_flow(&client, &config).await.unwrap();
        let public = serde_json::to_string(&start.public_view()).unwrap();
        assert!(public.contains("ABCD-EFGH"));
        assert!(!public.contains("device-secret-canary"));
        assert!(matches!(
            poll_device_flow(&client, &config, &start.device_code)
                .await
                .unwrap(),
            DevicePoll::Pending { .. }
        ));
        let complete = poll_device_flow(&client, &config, &start.device_code)
            .await
            .unwrap();
        let DevicePoll::Complete(verified) = complete else {
            panic!("expected completed device authorization")
        };
        assert_eq!(verified.identity, "octocat");
        assert_eq!(verified.secret.as_bytes(), b"access-secret-canary");
        assert!(!format!("{verified:?}").contains("access-secret-canary"));
    }
}
