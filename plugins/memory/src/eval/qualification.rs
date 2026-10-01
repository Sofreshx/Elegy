//! Product-local claim inventory and append-only qualification receipts.
//! Memory may recall pointers; exact inventory and evidence never use retrieval.
use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::Command,
    time::Instant,
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    fingerprint,
    scenarios::{self, ScenarioCheck, ScenarioReport},
};

type Result<T> = std::result::Result<T, String>;
const SCHEMA: &str = "memory-qualification/v1";
const INVENTORY_SCHEMA: &str = "memory-claim-inventory/v1";
const MAX_JSON: u64 = 1_048_576;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Claim {
    pub id: &'static str,
    pub promise: &'static str,
    pub falsifier: &'static str,
    pub protocol_version: u32,
    pub executor: &'static str,
    pub protocol: &'static [&'static str],
    pub required_checks: &'static [&'static str],
    pub prerequisite_checks: &'static [&'static str],
    pub limits: &'static str,
}

pub(crate) const CLAIMS: &[Claim] = &[
    Claim { id: "recall.contract", promise: "Bounded contextual recall preserves source state and scope/owner boundaries in the synthetic protocol.",
        falsifier: "A forbidden row is returned, source state changes, or a recall/feedback invariant fails.", protocol_version: 1, executor: "local-scenario",
        protocol: &["Create a disposable source and separate journal with synthetic memories.", "Exercise recall, exact filters, retries and conflicting/dismiss feedback.", "Inspect source state and journal identity; preserve every check result."],
        required_checks: &["positive-control", "context-budget", "agent-owner-isolation", "sensitivity-isolation", "tenant-owner-isolation", "user-owner-isolation", "scope-isolation", "turn-retry", "feedback-retry", "conflicting-feedback", "injected-dismiss-suppression", "suppression-session-boundary", "journal-header", "journal-version-rejection", "journal-identity-rejection", "source-unchanged"],
        prerequisite_checks: &["positive-control"],
        limits: "Source-library synthetic proof only; does not establish installed MCP behavior, answer quality or a latency guarantee." },
    Claim { id: "forgetting.retention", promise: "All five policies enforce the active-count budget; the default retains the labelled important facts in this fixed scenario.",
        falsifier: "A policy misses the budget or the default drops a labelled high-value fact.", protocol_version: 1, executor: "local-scenario",
        protocol: &["Seed identical synthetic memories with fixed IDs, timestamps and utility labels for each policy.", "Run the actual store budget enforcement for every policy.", "Report retained IDs and retention quality per policy; do not select a universal winner."],
        required_checks: &["importance-reliability.count-budget", "default-policy-identity", "default-labelled-retention", "fifo.count-budget", "lru.count-budget", "priority-decay.count-budget", "random-drop.count-budget"],
        prerequisite_checks: &[],
        limits: "Small synthetic comparative scenario, not FiFA/MaRS benchmark parity or a universal policy ranking." },
    Claim { id: "host.cross-session", promise: "A synthetic memory written through the installed MCP is recalled in a fresh client session using the same namespace.",
        falsifier: "A successful write cannot be recalled by ID and search after the client session is closed and reopened.", protocol_version: 1, executor: "external-observation",
        protocol: &["Identify installed artifact SHA-256, host version and configuration digest; use a dedicated synthetic namespace.", "Store a unique synthetic fact through the actual MCP; record the returned ID and write outcome.", "Close the client session; open a fresh session without pasted prior context; recall by ID and search.", "Record session identities and observations in reviewed evidence; delete the synthetic memory."],
        required_checks: &["installed-client", "stored-via-mcp", "fresh-session-recall", "same-namespace", "cleanup"],
        prerequisite_checks: &["installed-client", "stored-via-mcp", "same-namespace", "cleanup"],
        limits: "Agent/operator-reported observation of one installation. Independently review evidence before readiness promotion." },
    Claim { id: "host.agent-isolation", promise: "An installed MCP agent namespace prevents another agent from recalling its private synthetic memory.",
        falsifier: "A second configured agent returns the first agent's memory by ID, list or search.", protocol_version: 1, executor: "external-observation",
        protocol: &["Use two real client configurations on the same test database, with distinct agent namespaces.", "Store a synthetic marker under A and confirm A recalls it.", "Attempt ID recall, search and list under B; establish that an empty result is not a disconnected backend.", "Record reviewed observations and clean up the marker."],
        required_checks: &["same-database", "owner-can-recall", "other-agent-cannot-recall", "cleanup"],
        prerequisite_checks: &["same-database", "owner-can-recall", "cleanup"],
        limits: "Reported namespace isolation for the tested installation; not multi-tenant certification." },
    Claim { id: "recall.answer-quality", promise: "Recall increases mean predeclared rubric score on this paired sample while recording latency and cost.",
        falsifier: "Mean paired improvement is zero or negative while the experimental controls hold.", protocol_version: 1, executor: "external-observation",
        protocol: &["Freeze at least ten cases, including irrelevant/contradictory-memory controls, expected answers and rubric before running.", "Use the same model, configuration and inputs in fresh isolated with/without-recall sessions; counterbalance order.", "Have an independent blinded assessor or deterministic rubric score normalized 0..1 outcomes.", "Record paired scores, latency milliseconds and cost in the same declared unit; attach protocol and assessment evidence."],
        required_checks: &["paired-inputs", "fixed-model-config", "predeclared-rubric", "independent-scoring", "negative-controls"],
        prerequisite_checks: &["paired-inputs", "fixed-model-config", "predeclared-rubric", "independent-scoring", "negative-controls"],
        limits: "Reported improvement on the declared finite sample; no population-wide or causal guarantee beyond the recorded controls." },
];

fn claim(id: &str) -> Result<&'static Claim> {
    CLAIMS
        .iter()
        .find(|c| c.id == id)
        .ok_or_else(|| format!("unknown qualification claim: {id}"))
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn encode(value: &impl Serialize) -> Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(|e| e.to_string())
}
fn protocol_digest(c: &Claim) -> Result<String> {
    Ok(digest(&encode(
        &serde_json::json!({"schemaVersion":INVENTORY_SCHEMA,"claim":c}),
    )?))
}
fn valid_digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn is_link(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}
fn reject_links(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(meta) if is_link(&meta) => {
                return Err("evidence paths must not contain links or reparse points".into())
            }
            Ok(_) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}
fn bounded_read(path: &Path, max: u64) -> Result<Vec<u8>> {
    reject_links(path)?;
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || is_link(&meta) || meta.len() > max {
        return Err("evidence must be a bounded regular file".into());
    }
    let mut data = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(max + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() as u64 > max {
        return Err("evidence exceeds size bound".into());
    }
    Ok(data)
}
fn file_digest(path: &Path) -> Result<String> {
    let mut input = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut bytes = [0u8; 65536];
    loop {
        let n = input.read(&mut bytes).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&bytes[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Context {
    source_sha256: String,
    build_source_sha256: String,
    executable_sha256: String,
    git_revision: String,
    dirty: bool,
    os: String,
    arch: String,
}
fn git(project: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("unable to establish qualification Git context".into());
    }
    String::from_utf8(output.stdout)
        .map(|s| s.trim().to_owned())
        .map_err(|e| e.to_string())
}
fn context(project: &Path) -> Result<Context> {
    Ok(Context {
        source_sha256: fingerprint::source_digest(project).map_err(|e| e.to_string())?,
        build_source_sha256: env!("ELEGY_MEMORY_BUILD_SOURCE_SHA256").into(),
        executable_sha256: file_digest(&std::env::current_exe().map_err(|e| e.to_string())?)?,
        git_revision: git(project, &["rev-parse", "HEAD"])?,
        dirty: !git(
            project,
            &["status", "--porcelain", "--untracked-files=normal"],
        )?
        .is_empty(),
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
    })
}
fn same_context(a: &Context, b: &Context) -> bool {
    a.source_sha256 == b.source_sha256
        && a.build_source_sha256 == b.build_source_sha256
        && a.executable_sha256 == b.executable_sha256
        && a.os == b.os
        && a.arch == b.arch
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Outcome {
    Satisfied,
    Refuted,
    Inconclusive,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct EvidenceRef {
    pub path: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Subject {
    pub artifact_sha256: String,
    pub host: String,
    pub host_version: String,
    pub configuration_sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PairedCase {
    pub id: String,
    pub score_with: f64,
    pub score_without: f64,
    pub latency_with_ms: f64,
    pub latency_without_ms: f64,
    pub cost_with: f64,
    pub cost_without: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Observation {
    pub schema_version: String,
    pub claim_id: String,
    pub protocol_version: u32,
    pub subject: Subject,
    /// Non-personal session/client pseudonyms, not host transcript paths.
    pub sessions: Vec<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub checks: Vec<ScenarioCheck>,
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub paired_cases: Vec<PairedCase>,
    #[serde(default)]
    pub cost_unit: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Receipt {
    schema_version: String,
    pub id: String,
    sequence: u64,
    pub claim_id: String,
    protocol_sha256: String,
    created_at: DateTime<Utc>,
    context: Context,
    pub provenance: String,
    pub outcome: Outcome,
    pub checks: Vec<ScenarioCheck>,
    pub measurements: serde_json::Value,
    duration_ms: u64,
    observation: Option<Observation>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Stored {
    sha256: String,
    receipt: Receipt,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LedgerEntry {
    id: String,
    sha256: String,
}
struct WriterLock(PathBuf);
impl Drop for WriterLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub(crate) struct Repository {
    project: PathBuf,
    evidence: PathBuf,
}
impl Repository {
    pub fn new(project: &Path, evidence: Option<&Path>) -> Result<Self> {
        let project = project.canonicalize().map_err(|e| e.to_string())?;
        let evidence = evidence
            .map(|p| {
                if p.is_absolute() {
                    p.to_owned()
                } else {
                    project.join(p)
                }
            })
            .unwrap_or_else(|| project.join("plugins/memory/evidence/qualification"));
        reject_links(&evidence)?;
        Ok(Self { project, evidence })
    }
    fn verify_evidence(&self, item: &EvidenceRef) -> Result<()> {
        let path = Path::new(&item.path);
        if !valid_digest(&item.sha256)
            || path.is_absolute()
            || item.path.contains('\\')
            || !path.components().all(|c| matches!(c, Component::Normal(_)))
            || path.components().next().is_some_and(|c| {
                matches!(
                    c.as_os_str().to_str(),
                    Some("receipts" | "ledger.json" | ".writer.lock")
                )
            })
        {
            return Err("external evidence requires a relative path beneath the evidence directory, outside receipts/ and bookkeeping files, and a SHA-256".into());
        }
        let root = self.evidence.canonicalize().map_err(|e| e.to_string())?;
        let mut next = root.clone();
        for part in path.components() {
            next.push(part.as_os_str());
            if fs::symlink_metadata(&next)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink()
            {
                return Err("symlink evidence is not allowed".into());
            }
        }
        if !next
            .canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(&root)
        {
            return Err("external evidence escapes its directory".into());
        }
        if digest(&bounded_read(&next, 8 * MAX_JSON)?) != item.sha256 {
            return Err("external evidence digest mismatch".into());
        }
        Ok(())
    }
    fn read_receipts(&self) -> Result<Vec<Receipt>> {
        reject_links(&self.evidence)?;
        let dir = self.evidence.join("receipts");
        let ledger_path = self.evidence.join("ledger.json");
        let ledger: Vec<LedgerEntry> = if ledger_path.exists() {
            serde_json::from_slice(&bounded_read(&ledger_path, 4 * MAX_JSON)?)
                .map_err(|_| "malformed qualification ledger")?
        } else {
            Vec::new()
        };
        if ledger.len() > 10_000
            || ledger
                .iter()
                .map(|entry| &entry.id)
                .collect::<BTreeSet<_>>()
                .len()
                != ledger.len()
        {
            return Err("invalid qualification ledger inventory".into());
        }
        if !dir.exists() {
            if !ledger.is_empty() {
                return Err("receipt directory is missing from recorded ledger".into());
            }
            return Ok(Vec::new());
        }
        if fs::symlink_metadata(&dir)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("receipt directory must not be a symlink".into());
        }
        let mut receipts = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().and_then(|v| v.to_str()) != Some("json") {
                continue;
            }
            if receipts.len() >= 10_000 {
                return Err(
                    "receipt inventory exceeds 10000 entries; explicitly archive reviewed evidence"
                        .into(),
                );
            }
            let stored: Stored = serde_json::from_slice(&bounded_read(&path, MAX_JSON)?)
                .map_err(|_| "malformed qualification receipt")?;
            let r = stored.receipt;
            if stored.sha256 != digest(&encode(&r)?)
                || r.schema_version != SCHEMA
                || Uuid::parse_str(&r.id).is_err()
                || path.file_stem().and_then(|s| s.to_str()) != Some(r.id.as_str())
                || !valid_digest(&r.protocol_sha256)
                || !valid_digest(&r.context.source_sha256)
                || !valid_digest(&r.context.build_source_sha256)
                || !valid_digest(&r.context.executable_sha256)
            {
                return Err("qualification receipt integrity or schema mismatch".into());
            }
            let c = claim(&r.claim_id)?;
            let entry = ledger
                .get(
                    r.sequence
                        .checked_sub(1)
                        .ok_or("invalid receipt sequence")? as usize,
                )
                .ok_or("receipt missing from qualification ledger")?;
            if entry.id != r.id || entry.sha256 != stored.sha256 {
                return Err("qualification ledger mismatch".into());
            }
            if r.created_at > Utc::now() {
                return Err("receipt timestamp is in the future".into());
            }
            if r.provenance != c.executor
                || (r.provenance == "external-observation") != r.observation.is_some()
            {
                return Err("invalid receipt provenance".into());
            }
            if r.outcome != Outcome::Inconclusive && check_outcome(&r.checks)? != r.outcome {
                return Err("receipt outcome contradicts its checks".into());
            }
            if let Some(observation) = &r.observation {
                if r.protocol_sha256 == protocol_digest(c)? {
                    let (checks, measurements, outcome) = validate_observation(c, observation)?;
                    if encode(&checks)? != encode(&r.checks)?
                        || measurements != r.measurements
                        || outcome != r.outcome
                    {
                        return Err("receipt contradicts its external observation".into());
                    }
                }
                for item in &observation.evidence {
                    self.verify_evidence(item)?;
                }
            }
            receipts.push(r);
        }
        if receipts.len() != ledger.len() {
            return Err("qualification receipt missing from ledger history".into());
        }
        receipts.sort_by_key(|r| r.sequence);
        Ok(receipts)
    }
    fn save(&self, mut receipt: Receipt) -> Result<Receipt> {
        reject_links(&self.evidence)?;
        fs::create_dir_all(&self.evidence).map_err(|e| e.to_string())?;
        let lock_path = self.evidence.join(".writer.lock");
        let lock_file = OpenOptions::new().write(true).create_new(true).open(&lock_path)
            .map_err(|_| "qualification writer locked; retry after the current writer finishes, or inspect an interrupted writer before removing its lock")?;
        let _lock = WriterLock(lock_path);
        drop(lock_file);
        // Check all existing evidence before adding another result; never hide a corrupt history.
        let history = self.read_receipts()?;
        if history.len() >= 10_000 {
            return Err("qualification receipt limit reached".into());
        }
        receipt.sequence = history.len() as u64 + 1;
        let mut ledger: Vec<LedgerEntry> = history
            .iter()
            .map(|r| {
                Ok(LedgerEntry {
                    id: r.id.clone(),
                    sha256: digest(&encode(r)?),
                })
            })
            .collect::<Result<_>>()?;
        let dir = self.evidence.join("receipts");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let stored = Stored {
            sha256: digest(&encode(&receipt)?),
            receipt: receipt.clone(),
        };
        let data = serde_json::to_vec_pretty(&stored).map_err(|e| e.to_string())?;
        if data.len() as u64 > MAX_JSON {
            return Err("receipt exceeds size bound".into());
        }
        let pending = dir.join(format!(".{}.pending", receipt.id));
        let target = dir.join(format!("{}.json", receipt.id));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pending)
            .map_err(|e| e.to_string())?;
        file.write_all(&data)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        // Same-filesystem hard link is atomic and fails if target already exists.
        fs::hard_link(&pending, &target).map_err(|e| e.to_string())?;
        fs::remove_file(pending).map_err(|e| e.to_string())?;
        ledger.push(LedgerEntry {
            id: receipt.id.clone(),
            sha256: stored.sha256,
        });
        let pending_ledger = self
            .evidence
            .join(format!(".{}.ledger.pending", receipt.id));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pending_ledger)
            .map_err(|e| e.to_string())?;
        file.write_all(&encode(&ledger)?)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        // A crash between receipt creation and ledger replacement is a visible integrity error.
        fs::rename(pending_ledger, self.evidence.join("ledger.json")).map_err(|e| e.to_string())?;
        Ok(receipt)
    }
    pub fn history(&self, id: Option<&str>) -> Result<Vec<Receipt>> {
        if let Some(id) = id {
            claim(id)?;
        }
        Ok(self
            .read_receipts()?
            .into_iter()
            .filter(|r| id.is_none_or(|id| r.claim_id == id))
            .collect())
    }
    pub fn status(&self) -> Result<serde_json::Value> {
        let receipts = self.read_receipts()?;
        let ctx = context(&self.project)?;
        let mut entries = Vec::new();
        for c in CLAIMS {
            let latest = receipts.iter().rev().find(|r| r.claim_id == c.id);
            let status = match latest {
                None => "unverified",
                Some(r)
                    if !same_context(&r.context, &ctx)
                        || r.protocol_sha256 != protocol_digest(c)? =>
                {
                    "stale"
                }
                Some(r) => match r.outcome {
                    Outcome::Satisfied => "satisfied",
                    Outcome::Refuted => "refuted",
                    Outcome::Inconclusive => "inconclusive",
                },
            };
            let review_required = latest.is_some_and(|r| r.provenance == "external-observation");
            let status = if review_required && status != "stale" {
                format!("reported-{status}")
            } else {
                status.to_string()
            };
            let stale_reasons: Vec<&str> = latest
                .map(|r| {
                    let mut reasons = Vec::new();
                    if r.context.source_sha256 != ctx.source_sha256 {
                        reasons.push("source-changed");
                    }
                    if r.context.build_source_sha256 != ctx.build_source_sha256 {
                        reasons.push("build-source-changed");
                    }
                    if r.context.executable_sha256 != ctx.executable_sha256 {
                        reasons.push("executable-changed");
                    }
                    if r.context.os != ctx.os || r.context.arch != ctx.arch {
                        reasons.push("environment-changed");
                    }
                    if protocol_digest(c).is_ok_and(|d| d != r.protocol_sha256) {
                        reasons.push("protocol-changed");
                    }
                    reasons
                })
                .unwrap_or_default();
            entries.push(serde_json::json!({"claim":c,"status":status,"latestReceipt":latest.map(|r| &r.id),
                "receiptPath":latest.map(|r| format!("receipts/{}.json", r.id)),"staleReasons":stale_reasons,
                "protocolSha256":protocol_digest(c)?,"subject":latest.and_then(|r| r.observation.as_ref().map(|o| &o.subject)),
                "externalSubjectRequiresRecheck":review_required,
                "observationSchema":if c.executor == "external-observation" { Some("plugins/memory/schemas/qualification-observation.schema.json") } else { None },
                "provenance": latest.map(|r| &r.provenance), "reviewRequired":review_required,
                "receiptCount":receipts.iter().filter(|r| r.claim_id == c.id).count(),
                "nextInvocation":{"program":"elegy-memory","args":if c.executor == "local-scenario" {
                    vec!["eval".to_string(),"run".into(),"--claim".into(),c.id.into(),"--project".into(),self.project.to_string_lossy().into_owned(),"--evidence-dir".into(),self.evidence.to_string_lossy().into_owned(),"--json".into()]
                } else {
                    vec!["eval".to_string(),"record".into(),"--claim".into(),c.id.into(),"--observation".into(),"FILE".into(),"--project".into(),self.project.to_string_lossy().into_owned(),"--evidence-dir".into(),self.evidence.to_string_lossy().into_owned(),"--json".into()]
                }},
                "nextAction":if status == "reported-satisfied" { "Review the external evidence and recheck the tested subject before any readiness decision.".to_string() }
                    else if c.executor == "local-scenario" { format!("elegy-memory eval run --claim {} --json",c.id) }
                    else { format!("Follow the claim protocol, then elegy-memory eval record --claim {} --observation FILE --json",c.id) }}));
        }
        Ok(
            serde_json::json!({"schemaVersion":SCHEMA,"inventoryVersion":INVENTORY_SCHEMA,"buildMatchesSource":ctx.source_sha256==ctx.build_source_sha256,
            "rebuildRequired":ctx.source_sha256!=ctx.build_source_sha256,
            "claims":entries,"readinessPromotion":false,"storage":"Append-only evidence; commit or back up the evidence directory for durable cross-machine recovery."}),
        )
    }
    pub fn next(&self) -> Result<serde_json::Value> {
        let status = self.status()?;
        if status["buildMatchesSource"] == false {
            return Ok(rebuild_action(&self.project));
        }
        let claims = status["claims"]
            .as_array()
            .ok_or("invalid status inventory")?;
        let rank = |v: &serde_json::Value| match v["status"].as_str() {
            Some("refuted" | "reported-refuted") => 0,
            Some("stale") => 1,
            Some("inconclusive" | "reported-inconclusive") => 2,
            Some("unverified") => 3,
            _ if v["reviewRequired"] == true => 4,
            _ => 5,
        };
        let next = claims.iter().filter(|v| rank(v) < 5).min_by_key(|v| {
            (
                rank(v),
                v["claim"]["executor"] != "local-scenario",
                v["claim"]["id"].as_str().unwrap_or_default(),
            )
        });
        Ok(
            serde_json::json!({"schemaVersion":SCHEMA,"buildMatchesSource":true,"rebuildRequired":false,"next":next}),
        )
    }
    pub fn run(&self, id: &str) -> Result<Receipt> {
        let c = claim(id)?;
        if c.executor != "local-scenario" {
            return Err("this claim requires an actual external experiment; use eval status for its protocol and eval record for observations".into());
        }
        let ctx = context(&self.project)?;
        if ctx.source_sha256 != ctx.build_source_sha256 {
            return Err("source differs from this executable's build fingerprint; rebuild before qualification".into());
        }
        let started = Instant::now();
        let (report, outcome) = match scenarios::run(id) {
            Ok(report) => {
                let observed: BTreeSet<_> = report
                    .checks
                    .iter()
                    .map(|check| check.name.as_str())
                    .collect();
                let expected: BTreeSet<_> = c.required_checks.iter().copied().collect();
                let outcome = if observed != expected || prerequisites_failed(c, &report.checks) {
                    Outcome::Inconclusive
                } else {
                    check_outcome(&report.checks)?
                };
                (report, outcome)
            }
            Err(error) => {
                eprintln!("Qualification scenario could not complete: {error}");
                (
                    ScenarioReport {
                        checks: Vec::new(),
                        measurements: serde_json::json!({"error":"scenario-runtime-failure", "detail":sanitized_error(&error, &self.project)}),
                    },
                    Outcome::Inconclusive,
                )
            }
        };
        let mut outcome = outcome;
        if fingerprint::source_digest(&self.project).map_err(|e| e.to_string())?
            != ctx.source_sha256
        {
            outcome = Outcome::Inconclusive;
        }
        self.save(Receipt {
            schema_version: SCHEMA.into(),
            id: Uuid::new_v4().to_string(),
            sequence: 0,
            claim_id: id.into(),
            protocol_sha256: protocol_digest(c)?,
            created_at: Utc::now(),
            context: ctx,
            provenance: "local-scenario".into(),
            outcome,
            checks: report.checks,
            measurements: report.measurements,
            duration_ms: started.elapsed().as_millis() as u64,
            observation: None,
        })
    }
    pub fn record(&self, id: &str, input: &Path) -> Result<Receipt> {
        let c = claim(id)?;
        if c.executor != "external-observation" {
            return Err("local claims must be executed with eval run --claim".into());
        }
        self.read_receipts()?;
        let observation: Observation =
            serde_json::from_slice(&bounded_read(input, MAX_JSON)?).map_err(|e| e.to_string())?;
        let (checks, measurements, outcome) = validate_observation(c, &observation)?;
        for evidence in &observation.evidence {
            self.verify_evidence(evidence)?;
        }
        self.save(Receipt {
            schema_version: SCHEMA.into(),
            id: Uuid::new_v4().to_string(),
            sequence: 0,
            claim_id: id.into(),
            protocol_sha256: protocol_digest(c)?,
            created_at: Utc::now(),
            context: context(&self.project)?,
            provenance: "external-observation".into(),
            outcome,
            checks,
            measurements,
            duration_ms: (observation.completed_at - observation.started_at).num_milliseconds()
                as u64,
            observation: Some(observation),
        })
    }
}

fn rebuild_action(project: &Path) -> serde_json::Value {
    serde_json::json!({"schemaVersion":SCHEMA,"buildMatchesSource":false,"rebuildRequired":true,"next":null,
        "nextAction":"Rebuild Memory from this checkout, then rerun eval next with the newly built executable.",
        "nextInvocation":{"program":"cargo","args":["build","--manifest-path",project.join("Cargo.toml"),"-p","elegy-memory"]}})
}

fn sanitized_error(error: &str, project: &Path) -> String {
    let mut detail = error.to_owned();
    for (path, placeholder) in [
        (std::env::temp_dir(), "<temp>"),
        (project.to_owned(), "<project>"),
    ] {
        let path = path.to_string_lossy();
        detail = detail
            .replace(path.as_ref(), placeholder)
            .replace(&path.replace('\\', "/"), placeholder);
    }
    detail.chars().take(2048).collect()
}

fn check_outcome(checks: &[ScenarioCheck]) -> Result<Outcome> {
    let mut names = BTreeSet::new();
    if checks.is_empty() {
        return Ok(Outcome::Inconclusive);
    }
    if checks.len() > 32 {
        return Err("too many qualification checks".into());
    }
    for check in checks {
        if check.name.is_empty()
            || check.name.len() > 128
            || check.detail.len() > 2048
            || !names.insert(&check.name)
        {
            return Err("invalid or duplicate qualification check".into());
        }
    }
    Ok(if checks.iter().all(|c| c.passed) {
        Outcome::Satisfied
    } else {
        Outcome::Refuted
    })
}
fn prerequisites_failed(c: &Claim, checks: &[ScenarioCheck]) -> bool {
    c.prerequisite_checks.iter().any(|name| {
        !checks
            .iter()
            .any(|check| check.name == *name && check.passed)
    })
}
fn validate_observation(
    c: &Claim,
    o: &Observation,
) -> Result<(Vec<ScenarioCheck>, serde_json::Value, Outcome)> {
    if o.schema_version != "memory-observation/v1"
        || o.claim_id != c.id
        || o.protocol_version != c.protocol_version
        || !valid_digest(&o.subject.artifact_sha256)
        || !valid_digest(&o.subject.configuration_sha256)
        || o.subject.host.trim().is_empty()
        || o.subject.host.len() > 128
        || o.subject.host_version.trim().is_empty()
        || o.subject.host_version.len() > 128
        || o.completed_at < o.started_at
        || o.completed_at > Utc::now()
        || o.evidence.is_empty()
        || o.evidence.len() > 16
        || o.sessions.len() < 2
        || o.sessions.len() > 2000
        || o.sessions
            .iter()
            .any(|s| s.trim().is_empty() || s.len() > 128)
        || o.sessions.iter().collect::<BTreeSet<_>>().len() != o.sessions.len()
        || o.error
            .as_ref()
            .is_some_and(|e| e.trim().is_empty() || e.len() > 2048)
    {
        return Err("invalid external observation identity, environment, time or evidence".into());
    }
    let mut checks = o.checks.clone();
    check_outcome(&checks)?;
    let names: BTreeSet<_> = checks.iter().map(|v| v.name.as_str()).collect();
    let required: BTreeSet<_> = c.required_checks.iter().copied().collect();
    if names != required {
        return Err("observation checks must exactly match the claim protocol".into());
    }
    let mut measurements = serde_json::json!({});
    if c.id == "recall.answer-quality" {
        if o.paired_cases.len() < 10
            || o.paired_cases.len() > 1000
            || o.cost_unit
                .as_ref()
                .is_none_or(|s| s.trim().is_empty() || s.len() > 64)
        {
            return Err(
                "answer quality requires 10..1000 paired cases and a declared cost unit".into(),
            );
        }
        let mut ids = BTreeSet::new();
        let mut delta = 0.0;
        for case in &o.paired_cases {
            if case.id.is_empty()
                || case.id.len() > 128
                || !ids.insert(&case.id)
                || ![case.score_with, case.score_without]
                    .iter()
                    .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                || ![
                    case.latency_with_ms,
                    case.latency_without_ms,
                    case.cost_with,
                    case.cost_without,
                ]
                .iter()
                .all(|v| v.is_finite() && *v >= 0.0)
            {
                return Err("invalid paired-case measurements".into());
            }
            delta += case.score_with - case.score_without;
        }
        let improvement = delta / o.paired_cases.len() as f64;
        checks.push(ScenarioCheck {
            name: "mean-quality-improves".into(),
            passed: improvement > 0.0,
            detail: "Mean paired normalized score must strictly improve; limited to this sample."
                .into(),
        });
        measurements = serde_json::json!({"pairedCaseCount":o.paired_cases.len(),"meanScoreImprovement":improvement,"costUnit":o.cost_unit});
    } else if !o.paired_cases.is_empty() || o.cost_unit.is_some() {
        return Err("paired cases are only valid for answer-quality observations".into());
    }
    let outcome = if o.error.is_some() || prerequisites_failed(c, &checks) {
        Outcome::Inconclusive
    } else {
        check_outcome(&checks)?
    };
    Ok((checks, measurements, outcome))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_context() -> Context {
        Context {
            source_sha256: "a".repeat(64),
            build_source_sha256: "a".repeat(64),
            executable_sha256: "b".repeat(64),
            git_revision: "c".repeat(40),
            dirty: false,
            os: "test-os".into(),
            arch: "test-arch".into(),
        }
    }
    #[test]
    fn freshness_binds_source_build_binary_and_environment_not_git_bookkeeping() {
        let original = sample_context();
        let mut other = original.clone();
        other.git_revision = "d".repeat(40);
        other.dirty = true;
        assert!(same_context(&original, &other));
        for field in ["source", "build", "binary", "os", "arch"] {
            let mut changed = original.clone();
            match field {
                "source" => changed.source_sha256 = "e".repeat(64),
                "build" => changed.build_source_sha256 = "e".repeat(64),
                "binary" => changed.executable_sha256 = "e".repeat(64),
                "os" => changed.os = "other".into(),
                _ => changed.arch = "other".into(),
            }
            assert!(!same_context(&original, &changed), "{field}");
        }
    }
    #[test]
    fn rebuild_guidance_precedes_claim_execution() {
        let action = rebuild_action(Path::new("synthetic-project"));
        assert_eq!(action["rebuildRequired"], true);
        assert!(action["next"].is_null());
        assert_eq!(action["nextInvocation"]["program"], "cargo");
        assert_eq!(action["nextInvocation"]["args"][4], "elegy-memory");
    }
    #[test]
    fn failure_missing_measurement_and_duplicate_checks_cannot_pass() {
        assert_eq!(check_outcome(&[]).expect("outcome"), Outcome::Inconclusive);
        let failed = ScenarioCheck {
            name: "boundary".into(),
            passed: false,
            detail: "counterexample".into(),
        };
        assert_eq!(
            check_outcome(std::slice::from_ref(&failed)).expect("outcome"),
            Outcome::Refuted
        );
        assert!(check_outcome(&[failed.clone(), failed]).is_err());
    }
    fn answer_observation() -> Observation {
        let c = claim("recall.answer-quality").expect("claim");
        Observation {
            schema_version: "memory-observation/v1".into(),
            claim_id: c.id.into(),
            protocol_version: 1,
            subject: Subject {
                artifact_sha256: "a".repeat(64),
                configuration_sha256: "b".repeat(64),
                host: "synthetic-assessment".into(),
                host_version: "1".into(),
            },
            sessions: vec!["with".into(), "without".into()],
            started_at: Utc::now() - chrono::Duration::seconds(2),
            completed_at: Utc::now() - chrono::Duration::seconds(1),
            checks: c
                .required_checks
                .iter()
                .map(|name| ScenarioCheck {
                    name: (*name).into(),
                    passed: true,
                    detail: "synthetic unit fixture".into(),
                })
                .collect(),
            evidence: vec![EvidenceRef {
                path: "reviewed.json".into(),
                sha256: "c".repeat(64),
            }],
            paired_cases: (0..10)
                .map(|i| PairedCase {
                    id: format!("case-{i}"),
                    score_with: 0.8,
                    score_without: 0.5,
                    latency_with_ms: 100.0,
                    latency_without_ms: 80.0,
                    cost_with: 2.0,
                    cost_without: 1.0,
                })
                .collect(),
            cost_unit: Some("synthetic-units".into()),
            error: None,
        }
    }
    #[test]
    fn paired_outcome_is_computed_and_broken_controls_are_inconclusive() {
        let c = claim("recall.answer-quality").expect("claim");
        let mut o = answer_observation();
        assert_eq!(
            validate_observation(c, &o).expect("observation").2,
            Outcome::Satisfied
        );
        for case in &mut o.paired_cases {
            case.score_with = case.score_without;
        }
        assert_eq!(
            validate_observation(c, &o).expect("observation").2,
            Outcome::Refuted
        );
        o.checks[0].passed = false;
        assert_eq!(
            validate_observation(c, &o).expect("observation").2,
            Outcome::Inconclusive
        );
        o.paired_cases[0].score_with = f64::NAN;
        assert!(validate_observation(c, &o).is_err());
    }
    #[test]
    fn old_protocol_is_visible_as_stale_and_failed_history_is_preserved() {
        let evidence = tempfile::tempdir().expect("evidence");
        let evidence_path = evidence.path().canonicalize().expect("evidence path");
        let project = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("project");
        let repo = Repository::new(&project, Some(&evidence_path)).expect("repository");
        let c = claim("recall.contract").expect("claim");
        let receipt = Receipt {
            schema_version: SCHEMA.into(),
            id: Uuid::new_v4().to_string(),
            sequence: 0,
            claim_id: c.id.into(),
            protocol_sha256: "0".repeat(64),
            created_at: Utc::now(),
            context: context(&project).expect("context"),
            provenance: "local-scenario".into(),
            outcome: Outcome::Refuted,
            checks: vec![ScenarioCheck {
                name: "historical-boundary".into(),
                passed: false,
                detail: "historical counterexample".into(),
            }],
            measurements: serde_json::json!({}),
            duration_ms: 1,
            observation: None,
        };
        repo.save(receipt).expect("receipt");
        let status = repo.status().expect("status");
        let entry = status["claims"]
            .as_array()
            .expect("claims")
            .iter()
            .find(|v| v["claim"]["id"] == c.id)
            .expect("claim");
        assert_eq!(entry["status"], "stale");
        assert!(entry["staleReasons"]
            .as_array()
            .expect("reasons")
            .contains(&serde_json::json!("protocol-changed")));
        assert_eq!(
            repo.history(Some(c.id)).expect("history")[0].outcome,
            Outcome::Refuted
        );
    }
}
